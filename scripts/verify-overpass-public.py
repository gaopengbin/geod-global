"""Verify actual Overpass acquisitions through an explicitly selected native runtime.

No public endpoint is built in. Network calls go only to --server on loopback;
the native runtime connects to --endpoint and saves the actual provider bytes.
Requires Shapely in the QA environment. No synthetic data or automatic retries.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import ipaddress
import json
import math
from pathlib import Path
import sys
import urllib.error
import urllib.parse
import urllib.request

from shapely.geometry import GeometryCollection, LineString, Point, Polygon, shape
from shapely.ops import polygonize_full, unary_union
from shapely.validation import explain_validity


PRESETS = {
    'buildings': ('Buildings', ['[building][building!=no]', '["building:part"]["building:part"!=no]']),
    'roads': ('Roads', ['[highway][highway!=no]']),
    'water': ('Water', ['[natural=water]', '[waterway][waterway!=no]', '[landuse=reservoir]', '[landuse=basin]']),
    'landuse': ('Land use', ['[landuse][landuse!=no]']),
    'pois': ('Points of interest', ['[amenity][amenity!=no]', '[shop][shop!=no]', '[tourism][tourism!=no]', '[leisure][leisure!=no]']),
}
BERLIN_BOUNDS = [13.3773, 52.5167, 13.3783, 52.5174]
LICENSE = 'https://www.openstreetmap.org/copyright'
ATTRIBUTION = '© OpenStreetMap contributors'
META_KEYS = ('version', 'timestamp', 'changeset', 'uid', 'user', 'visible')
RULES_PATH = Path(__file__).resolve().parents[1] / 'crates/geod-runtime/src/vector/osm/polygon-features.json'
MAX_BYTES = 20 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def timestamp(value):
    result = datetime.fromisoformat(value.replace('Z', '+00:00'))
    require(result.tzinfo is not None, 'Timestamp is missing timezone')
    return result


def identity(element):
    require(element.get('type') in {'node', 'way', 'relation'}, 'Non-OSM element in object group')
    ident = element.get('id')
    require(type(ident) is int and 0 < ident <= 9007199254740991, 'Invalid OSM object ID')
    return f"{element['type']}/{ident}"


def counts(elements):
    kinds = Counter(e['type'] for e in elements)
    return dict(nodes=kinds['node'], ways=kinds['way'], relations=kinds['relation'], total=len(elements))


def split_groups(raw):
    require(raw.get('version') == 0.6, 'Unsupported Overpass JSON version')
    require(str(raw.get('generator', '')).startswith('Overpass API'), 'Not an Overpass response')
    require('error' not in raw and ('remark' not in raw or raw['remark'] == ''), 'Provider reported incomplete processing')
    timestamp(raw['osm3s']['timestamp_osm_base'])
    copyright_text = raw['osm3s']['copyright']
    require('openstreetmap.org' in copyright_text and 'ODbL' in copyright_text, 'Missing provider OSM attribution')
    elements = raw['elements']
    require(isinstance(elements, list) and len(elements) <= 50002, 'Invalid or excessive element list')
    groups, current, seen = [], [], set()
    for element in elements:
        if element.get('type') == 'count':
            require(element.get('id') == 0 and set(element.get('tags', {})) == {'nodes', 'ways', 'relations', 'total'}, 'Invalid completion sentinel')
            require(all(isinstance(n, str) and n.isascii() and n.isdecimal() for n in element['tags'].values()), 'Non-integer completion count')
            actual = counts(current)
            require({key: int(value) for key, value in element['tags'].items()} == actual, 'Completion count differs from the preceding group')
            groups.append(current)
            current = []
        else:
            key = identity(element)
            require(key not in seen, f'Duplicate OSM identity {key}')
            seen.add(key)
            require(all(isinstance(v, str) for v in element.get('tags', {}).values()), f'Non-string tags on {key}')
            current.append(element)
    require(len(groups) == 2 and not current and elements[-1]['type'] == 'count', 'Both selected/dependency completion markers are required')
    require(sum(map(len, groups)) <= 50000, 'Element limit exceeded')
    lookup = {identity(e): e for group in groups for e in group}
    for element in lookup.values():
        if element['type'] == 'way':
            refs = [('node', ref) for ref in element['nodes']]
        elif element['type'] == 'relation':
            refs = [(m['type'], m['ref']) for m in element['members']]
            require(all(isinstance(m['role'], str) for m in element['members']), 'Relation role is not a string')
        else:
            refs = []
        for kind, ref in refs:
            require(f'{kind}/{ref}' in lookup, f'Missing recursive dependency {kind}/{ref}')
    return groups[0], groups[1], lookup


def xy(point):
    coordinate = (point['lon'], point['lat'])
    require(all(type(n) in (int, float) and math.isfinite(n) for n in coordinate), 'Non-finite source coordinate')
    require(-180 <= coordinate[0] <= 180 and -90 <= coordinate[1] <= 90, 'Source coordinate outside WGS84')
    return coordinate


def positions(geometry):
    if geometry['type'] == 'GeometryCollection':
        return [p for child in geometry['geometries'] for p in positions(child)]
    def walk(values):
        if values and isinstance(values[0], (int, float)):
            return [tuple(values)]
        return [p for child in values for p in walk(child)]
    return walk(geometry['coordinates'])


def geometry_counts(geometry):
    result = Counter({geometry['type']: 1})
    if geometry['type'] == 'GeometryCollection':
        for child in geometry['geometries']:
            result.update(geometry_counts(child))
    return result


def hole_count(geometry):
    if geometry.geom_type == 'Polygon':
        return len(geometry.interiors)
    if geometry.geom_type in ('MultiPolygon', 'GeometryCollection'):
        return sum(hole_count(g) for g in geometry.geoms)
    return 0


def edges(paths):
    return Counter(tuple(sorted((tuple(a), tuple(b)))) for path in paths for a, b in zip(path, path[1:]))


class Reference:
    """GEOS polygonization, independent of runtime endpoint-joining code.

    Area-versus-line classification uses the pinned published polygon rules;
    report that shared declarative input separately from geometry validation.
    """
    def __init__(self, lookup):
        self.lookup = lookup
        self.rules_bytes = RULES_PATH.read_bytes()
        self.rules = json.loads(self.rules_bytes)

    def way(self, element):
        refs = element['nodes']
        points = [xy(self.lookup[f'node/{ref}']) for ref in refs]
        require(len(points) >= 2, 'Way has fewer than two nodes')
        if 'geometry' in element:
            require([xy(p) for p in element['geometry']] == points, 'Embedded way differs from node dependencies')
        return points

    def member_way(self, member):
        points = self.way(self.lookup[f"way/{member['ref']}"])
        if 'geometry' in member:
            require([xy(p) for p in member['geometry']] == points, 'Embedded relation member differs from full way')
        return points

    def is_area(self, element, points):
        tags = element.get('tags', {})
        closed = len(points) >= 4 and points[0] == points[-1] and element['nodes'][0] == element['nodes'][-1]
        require(tags.get('area') != 'yes' or closed, 'Explicit area is not a closed way')
        if not closed or tags.get('area') == 'no':
            return False
        if tags.get('area') == 'yes':
            return True
        for rule in self.rules:
            value = tags.get(rule['key'])
            if value is None or value == 'no':
                continue
            if rule['polygon'] == 'all' or (rule['polygon'] == 'whitelist' and value in rule['values']) or (rule['polygon'] == 'blacklist' and value not in rule['values']):
                return True
        return False

    @staticmethod
    def polygonize(paths):
        polygons, cuts, dangles, invalid = polygonize_full([LineString(p) for p in paths])
        require(cuts.is_empty and dangles.is_empty and invalid.is_empty, 'GEOS found incomplete or invalid relation boundaries')
        return unary_union(list(polygons.geoms))

    def geometry(self, element, active=()):
        key, kind = identity(element), element['type']
        require(key not in active and len(active) <= 8, 'Cyclic or excessive relation nesting')
        if kind == 'node':
            return Point(xy(element))
        if kind == 'way':
            points = self.way(element)
            return Polygon(points) if self.is_area(element, points) else LineString(points)
        members = element['members']
        require(bool(members), 'Empty relation has no geometry')
        if element.get('tags', {}).get('type') in {'multipolygon', 'boundary'}:
            outer, inner = [], []
            for member in members:
                if member['type'] == 'way' and member['role'] in ('outer', '', 'inner'):
                    (inner if member['role'] == 'inner' else outer).append(self.member_way(member))
                else:
                    require(member['type'] == 'node' and member['role'] in ('label', 'admin_centre'), 'Unsupported area relation member')
            require(bool(outer), 'Area relation has no outer boundary')
            result = self.polygonize(outer)
            return result.difference(self.polygonize(inner)) if inner else result
        geometries = []
        for member in members:
            child = self.lookup[f"{member['type']}/{member['ref']}"]
            if member['type'] == 'way':
                geometries.append(LineString(self.member_way(member)))
            elif member['type'] == 'node':
                if 'lon' in member or 'lat' in member:
                    require(xy(member) == xy(child), 'Embedded relation node differs from dependency')
                geometries.append(Point(xy(child)))
            else:
                geometries.append(self.geometry(child, active + (key,)))
        return GeometryCollection(geometries)

    def verify_geometry(self, element, exported, active=()):
        expected, actual = self.geometry(element, active), shape(exported)
        require(expected.is_valid, f'Invalid source geometry {identity(element)}: {explain_validity(expected)}')
        require(actual.is_valid, f'Invalid export {identity(element)}: {explain_validity(actual)}')
        require(actual.equals(expected), f'Independent GEOS geometry differs for {identity(element)}')
        if element['type'] == 'node':
            require(exported['type'] == 'Point' and tuple(exported['coordinates']) == xy(element), 'Point coordinate changed')
        elif element['type'] == 'way':
            coords = self.way(element)
            area = self.is_area(element, coords)
            require(exported['type'] == ('Polygon' if area else 'LineString'), 'Way geometry type changed')
            actual_points = exported['coordinates'][0] if area else exported['coordinates']
            require([tuple(p) for p in actual_points] == coords, 'Way coordinate precision or order changed')
        elif element.get('tags', {}).get('type') in {'multipolygon', 'boundary'}:
            require(exported['type'] == 'MultiPolygon', 'Area relation lost its MultiPolygon representation')
            source_paths = [self.member_way(m) for m in element['members'] if m['type'] == 'way']
            output_paths = [ring for polygon in exported['coordinates'] for ring in polygon]
            require(edges(source_paths) == edges(output_paths), 'Area relation edge/vertex precision changed')
        else:
            require(exported['type'] == 'GeometryCollection', 'General relation lost its member geometry collection')
            children = exported['geometries']
            require(len(children) == len(element['members']), 'General relation member omitted')
            for member, child_geometry in zip(element['members'], children):
                child = self.lookup[f"{member['type']}/{member['ref']}"]
                if member['type'] == 'way':
                    require(child_geometry['type'] == 'LineString' and [tuple(p) for p in child_geometry['coordinates']] == self.member_way(member), 'Relation way member order or coordinates changed')
                else:
                    self.verify_geometry(child, child_geometry, active + (identity(element),))
        return hole_count(actual)


def canonical_query(preset, bounds):
    def number(value):
        text = str(value)
        return text[:-2] if text.endswith('.0') else text
    west, south, east, north = bounds
    bbox = ','.join(number(v) for v in (south, west, north, east))
    clauses = ''.join(f'nwr{tag}({bbox});' for tag in PRESETS[preset][1])
    return ('[out:json][timeout:25][maxsize:16777216];(' + clauses + ')->.selected;'
            '.selected out meta geom;.selected out count;.selected >> ->.dependencies;'
            '(.dependencies; - .selected;)->.dependencies;.dependencies out meta geom;.dependencies out count;')


def raw_object_field(data, field):
    """Get exact nested JSON bytes without changing runtime float serialization."""
    text = data.decode('utf-8')
    decoder = json.JSONDecoder()
    index = 0
    while text[index].isspace():
        index += 1
    require(text[index] == '{', 'Runtime inspection is not an object')
    index += 1
    while True:
        while text[index].isspace() or text[index] == ',':
            index += 1
        require(text[index] != '}', f'Runtime inspection has no {field}')
        key, index = decoder.raw_decode(text, index)
        while text[index].isspace():
            index += 1
        require(text[index] == ':', 'Invalid runtime JSON object')
        index += 1
        while text[index].isspace():
            index += 1
        start = index
        _, index = decoder.raw_decode(text, index)
        if key == field:
            return text[start:index].encode('utf-8')


def verify_snapshot(raw_bytes, inspection_bytes, record, preset, requested_bounds, service):
    inspection = json.loads(inspection_bytes)
    asset, exported = inspection['asset'], inspection['geojson']
    require(record['asset'] == asset, 'Runtime asset differs from persisted registry')
    require(asset['format'] == 'overpass-json' and asset['storageMode'] == 'managed' and asset['osmConversion'] == 2, 'Unexpected snapshot storage/conversion')
    require(not asset.get('remoteSource'), 'OSM snapshot was promoted as another protocol')
    require(len(raw_bytes) == asset['bytes'] <= MAX_BYTES and sha(raw_bytes) == asset['sourceSha256'], 'Stored provider bytes differ from registry')
    raw = json.loads(raw_bytes)
    selected, dependencies, lookup = split_groups(raw)
    source = asset['osmSource']
    require(exported['geodOsmSource'] == source, 'Export lost trusted query provenance')
    require(source['serviceUrl'] == service['url'] and source['serviceName'] == service['name'], 'Wrong query service attribution')
    require(source['preset'] == preset and source['presetTitle'] == PRESETS[preset][0], 'Preset attribution mismatch')
    require(source['requestedBounds'] == requested_bounds and source['areaGeometry'] is None, 'Selection bounds changed')
    require(source['query'] == canonical_query(preset, requested_bounds), 'Query differs from declared complete preset template')
    require(source['selection'] == 'overpass-bbox-full-geometry', 'Snapshot incorrectly claims clipping')
    require(source['bytes'] == len(raw_bytes) and source['responseSha256'] == sha(raw_bytes), 'Raw-byte provenance mismatch')
    require(source['elementCounts'] == counts(selected) and source['dependencyCounts'] == counts(dependencies), 'Provenance group counts changed')
    require(source['generator'] == raw['generator'] and source['apiVersion'] == raw['version'], 'Provider identity changed')
    require(source['dataTimestamp'] == asset['dataTimestamp'] == exported['data_timestamp'] == raw['osm3s']['timestamp_osm_base'], 'Data timestamps differ')
    require(source['copyrightText'] == raw['osm3s']['copyright'], 'Provider copyright declaration changed')
    timestamp(source['requestedAt'])
    require(asset['attribution'] == exported['attribution'] == ATTRIBUTION and asset['licenseUrl'] == exported['license'] == LICENSE, 'Missing OSM credit/license')
    features = exported['features']
    expected_ids = [identity(e) for e in selected]
    require([f['id'] for f in features] == expected_ids, 'Selected IDs/order differ or dependency objects leaked as features')
    require(asset['featureCount'] == len(selected), 'Dependency count inflated exported feature count')
    reference, holes, detailed, all_positions, actual_counts = Reference(lookup), 0, [], [], Counter()
    for element, feature in zip(selected, features):
        properties = {'osm_type': element['type'], 'osm_id': element['id'], 'tags': element.get('tags', {})}
        if 'nodes' in element:
            properties['osm_nodes'] = element['nodes']
        if 'members' in element:
            properties['osm_members'] = [{k: m[k] for k in ('type', 'ref', 'role')} for m in element['members']]
        metadata = {k: element[k] for k in META_KEYS if k in element}
        if metadata:
            properties['osm_metadata'] = metadata
        require(feature['properties'] == properties, f"Source attributes/node IDs/member roles/metadata changed: {feature['id']}")
        count = reference.verify_geometry(element, feature['geometry'])
        holes += count
        detailed.append({'id': feature['id'], 'geometryType': feature['geometry']['type'], 'holes': count, 'independentGeosEqual': True})
        all_positions.extend(positions(feature['geometry']))
        actual_counts.update(geometry_counts(feature['geometry']))
    actual_bounds = [min(p[0] for p in all_positions), min(p[1] for p in all_positions), max(p[0] for p in all_positions), max(p[1] for p in all_positions)] if all_positions else None
    require(asset['coordinateCount'] == len(all_positions), 'Coordinate count mismatch')
    require(asset['geometryCounts'] == dict(actual_counts), 'Geometry type counts mismatch')
    require(asset['bounds'] == actual_bounds and asset['crs'] == 'EPSG:4326', 'Actual geometry bounds/CRS mismatch')
    export_bytes = raw_object_field(inspection_bytes, 'geojson')
    require(sha(export_bytes) == asset['geojsonSha256'], 'Runtime exact export hash differs from registered GeoJSON')
    west, south, east, north = requested_bounds
    outside = sum(not (west <= p[0] <= east and south <= p[1] <= north) for p in all_positions)
    return {
        'assetId': asset['id'], 'preset': preset, 'nonEmpty': bool(selected),
        'sourceBytes': len(raw_bytes), 'sourceSha256': sha(raw_bytes), 'exportSha256': sha(export_bytes),
        'selectedCounts': counts(selected), 'dependencyCounts': counts(dependencies),
        'dataTimestamp': source['dataTimestamp'], 'requestedAt': source['requestedAt'],
        'requestedBounds': requested_bounds, 'actualGeometryBounds': actual_bounds,
        'coordinateCount': len(all_positions), 'geometryCounts': dict(actual_counts), 'holes': holes,
        'coordinatesOutsideSelection': outside, 'expectedIds': expected_ids, 'features': detailed,
        'rawBytesAndProvenanceVerified': True, 'selectedObjectsOnly': True,
        'allTagsNodeIdsMemberReferencesRolesAndMetadataEqual': True,
        'allGeometriesValidAndEqualIndependentGeos': True,
        'wayCoordinatePrecisionAndOrderEqual': True, 'relationEdgesPreserved': True,
        'areaRuleSource': str(RULES_PATH), 'areaRulesSha256': sha(reference.rules_bytes),
        'geometryMethod': 'Independent Shapely/GEOS polygonization and topological equality, plus exact way coordinates and relation edge multisets; shared pinned declarative OSM area classification rules',
    }, export_bytes


class Runtime:
    def __init__(self, server):
        url = urllib.parse.urlsplit(server)
        require(url.scheme == 'http' and not url.username and not url.password and not url.query and not url.fragment and url.path in ('', '/'), 'Use an HTTP loopback runtime root URL')
        try:
            local = ipaddress.ip_address(url.hostname).is_loopback
        except ValueError:
            local = url.hostname == 'localhost'
        require(local, '--server must be loopback')
        self.server = server.rstrip('/')
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def call(self, path, body=None):
        encoded = None if body is None else json.dumps(body, ensure_ascii=False).encode('utf-8')
        request = urllib.request.Request(self.server + path, data=encoded,
            headers={'Content-Type': 'application/json', 'Accept': 'application/json', 'X-GeoD-Client': 'geod-global'})
        try:
            with self.opener.open(request, timeout=90) as response:
                data = response.read(100 * 1024 * 1024 + 1)
            require(len(data) <= 100 * 1024 * 1024, 'Runtime response too large')
            return json.loads(data), data
        except urllib.error.HTTPError as error:
            message = error.read(16384).decode('utf-8', errors='replace')
            raise RuntimeError(f'Runtime HTTP {error.code}: {message}') from error


def parse_bounds(value):
    try:
        result = [float(n) for n in value.split(',')]
        require(len(result) == 4 and all(math.isfinite(n) for n in result), 'Expected four finite numbers')
        w, s, e, n = result
        require(-180 <= w < e <= 180 and -90 <= s < n <= 90, 'Invalid WGS84 bounds')
        require(e - w <= 1 and n - s <= 1, 'Each side must be at most one degree')
        area = 6371.0088 ** 2 * math.radians(e - w) * (math.sin(math.radians(n)) - math.sin(math.radians(s)))
        require(area <= 100, 'Area must be at most 100 square kilometres')
        return result
    except (ValueError, TypeError) as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True, help='Already running isolated native HTTP loopback server')
    parser.add_argument('--data-dir', required=True, type=Path, help='Exact backing data directory of that server')
    parser.add_argument('--endpoint', required=True, help='Explicitly authorized HTTPS Overpass interpreter URL; no default')
    parser.add_argument('--service-id', help='Reuse this registered Overpass connection; require its URL to match --endpoint and never probe again')
    parser.add_argument('--output', required=True, type=Path, help='New output directory for evidence and report.json')
    parser.add_argument('--preset', choices=['all', *PRESETS], default='all', help='One category or all five, serially')
    parser.add_argument('--bounds', type=parse_bounds, default=BERLIN_BOUNDS, metavar='W,S,E,N', help='Default tiny Berlin sample; positive water/landuse acceptance requires a suitable supplied region')
    parser.add_argument('--require-nonempty', action='store_true', help='Fail if any requested preset has zero selected objects')
    args = parser.parse_args()
    require(args.data_dir.is_dir(), '--data-dir must exist and belong to the running runtime')
    endpoint = urllib.parse.urlsplit(args.endpoint)
    require(endpoint.scheme == 'https' and endpoint.hostname and not endpoint.username and not endpoint.password and not endpoint.query and not endpoint.fragment, '--endpoint must be credential-free HTTPS without query or fragment')
    output = args.output.resolve()
    require(not output.exists(), '--output must be a new directory so earlier evidence is never overwritten')
    output.mkdir(parents=True)
    report = {'startedAt': datetime.now(timezone.utc).isoformat(), 'status': 'running',
              'mode': 'Actual native Overpass acquisition; no fixture data', 'server': args.server,
              'dataDirectory': str(args.data_dir.resolve()), 'endpoint': args.endpoint,
              'requestedPresets': list(PRESETS) if args.preset == 'all' else [args.preset],
              'bounds': args.bounds, 'automaticRetries': 0, 'desktopWebViewAccepted': False, 'receipts': []}
    save_json(output / 'report.json', report)
    try:
        runtime = Runtime(args.server)
        services, _ = runtime.call('/feature-services')
        candidates = [s for s in services if s['url'] == args.endpoint and s.get('overpass') and (not args.service_id or s['id'] == args.service_id)]
        require(not args.service_id or len(candidates) == 1, '--service-id is not an Overpass connection for the supplied endpoint')
        if candidates:
            require(len(candidates) == 1, 'Ambiguous existing endpoint registration')
            service = candidates[0]
            report['connection'] = 'Reused exact existing Overpass endpoint; no additional public probe'
        else:
            service, _ = runtime.call('/feature-services', {'name': 'OSM independent verification', 'url': args.endpoint, 'protocol': 'Overpass'})
            report['connection'] = 'Connected through native zero-count probe'
        require(service['url'] == args.endpoint and service.get('overpass'), 'Native connection differs from requested Overpass endpoint')
        require([c['id'] for c in service['collections']] == list(PRESETS), 'Native preset catalog differs from expected scope')
        persisted_services = json.loads((args.data_dir / 'feature-services.json').read_text(encoding='utf-8'))
        require(persisted_services[service['id']] == service, '--data-dir does not match the connected runtime registry')
        save_json(output / 'service.json', service)
        for preset in report['requestedPresets']:
            print(f'Querying {preset} once through native runtime at {args.bounds}', flush=True)
            request = {'serviceId': service['id'], 'collectionId': preset, 'bounds': args.bounds}
            save_json(output / f'{preset}.request.json', request)
            asset, _ = runtime.call('/feature-services/query', request)
            inspection, inspection_bytes = runtime.call('/vectors/' + asset['id'])
            require(inspection['asset'] == asset, 'Query result differs from fresh native inspection')
            registry = json.loads((args.data_dir / 'vectors.json').read_text(encoding='utf-8'))
            record = registry[asset['id']]
            backing = Path(record['path'])
            expected_backing = args.data_dir / 'vectors' / (asset['id'] + '.json')
            require(backing.is_file() and backing.samefile(expected_backing), 'Unexpected raw source backing file')
            require(backing.stat().st_size <= MAX_BYTES, 'Raw backing file exceeds response limit')
            raw_bytes = backing.read_bytes()
            # Save real artifacts before validation so a failure is reviewable.
            (output / f'{preset}.response.json').write_bytes(raw_bytes)
            (output / f'{preset}.inspection.json').write_bytes(inspection_bytes)
            save_json(output / f'{preset}.registry-record.json', record)
            receipt, export_bytes = verify_snapshot(raw_bytes, inspection_bytes, record, preset, args.bounds, service)
            receipt['backingPath'] = str(backing)
            receipt['rawResponseFile'] = f'{preset}.response.json'
            receipt['exportFile'] = f'{preset}.geojson'
            (output / receipt['exportFile']).write_bytes(export_bytes)
            save_json(output / f'{preset}.expected-ids.json', receipt['expectedIds'])
            report['receipts'].append(receipt)
            save_json(output / 'report.json', report)
            require(receipt['nonEmpty'] or not args.require_nonempty, f'{preset} returned a valid empty result; positive acceptance was requested')
            print(f"Verified {preset}: {receipt['selectedCounts']['total']} selected, {receipt['dependencyCounts']['total']} dependencies, {receipt['holes']} holes", flush=True)
        report['status'] = 'passed'
        report['allRequestedPresetsNonEmpty'] = all(r['nonEmpty'] for r in report['receipts'])
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        report['completedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'report': str(output / 'report.json'), 'status': report['status'], 'presets': len(report['receipts'])}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'Verification failed: {error}', file=sys.stderr)
        sys.exit(1)
