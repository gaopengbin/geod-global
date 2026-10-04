"""Verify a real WFS acquisition through an explicitly selected native runtime.

No public endpoint, collection or bbox is built in. The script talks only to an
already-running loopback runtime; the runtime performs the bounded acquisition.
Use --asset-id to inspect a saved acquisition without contacting its provider.
Requires Shapely for independent GEOS validation. Never retries automatically.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import io
import ipaddress
import json
import math
from pathlib import Path
import sys
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

from shapely.geometry import shape
from shapely.validation import explain_validity

MAX_BYTES = 20 * 1024 * 1024
WFS = 'http://www.opengis.net/wfs/2.0'
GML = 'http://www.opengis.net/gml/3.2'
XSD = 'http://www.w3.org/2001/XMLSchema'
XSI = 'http://www.w3.org/2001/XMLSchema-instance'
EPSG4326 = 'urn:ogc:def:crs:EPSG::4326'
CRS84 = 'urn:ogc:def:crs:OGC:1.3:CRS84'
NS = {'w': WFS, 'g': GML, 'x': XSD}
SAFE_INTEGER = 9007199254740991


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def raw_object_field(data, field):
    text = data.decode('utf-8')
    decoder, index = json.JSONDecoder(), 0
    while text[index].isspace():
        index += 1
    require(text[index] == '{', 'Inspection is not a JSON object')
    index += 1
    while True:
        while text[index].isspace() or text[index] == ',':
            index += 1
        require(text[index] != '}', f'Missing inspection field {field}')
        key, index = decoder.raw_decode(text, index)
        while text[index].isspace():
            index += 1
        require(text[index] == ':', 'Malformed inspection JSON')
        index += 1
        while text[index].isspace():
            index += 1
        start = index
        _, index = decoder.raw_decode(text, index)
        if key == field:
            return text[start:index].encode('utf-8')


def xml_document(text):
    raw = text.encode('utf-8')
    require(len(raw) <= MAX_BYTES and '<!DOCTYPE' not in text and '<!ENTITY' not in text, 'Unsupported XML declaration or excessive XML')
    namespaces, parents, stack, pending = {}, {}, [], []
    for event, node in ET.iterparse(io.BytesIO(raw), events=('start', 'end', 'start-ns')):
        if event == 'start-ns':
            pending.append(node)
        elif event == 'start':
            mapping = dict(namespaces[stack[-1]]) if stack else {}
            mapping.update(pending)
            pending = []
            namespaces[node] = mapping
            if stack:
                parents[node] = stack[-1]
            stack.append(node)
        else:
            root = stack.pop()
    require(not any(e.tag.split('}')[-1] in ('ExceptionReport', 'ServiceExceptionReport') for e in root.iter()), 'Provider returned an XML exception')
    return root, namespaces, parents


def qname(value, node, namespaces):
    prefix, local = value.split(':', 1) if ':' in value else ('', value)
    require(prefix in namespaces[node], f'Unresolved QName {value}')
    return namespaces[node][prefix], local


def reference_schema(text, expected):
    root, namespaces, _ = xml_document(text)
    require(root.tag == '{' + XSD + '}schema' and root.get('targetNamespace') == expected['namespace'], 'XSD target namespace differs')
    declarations = [n for n in root.findall('x:element', NS) if n.get('name') == expected['elementName']]
    require(len(declarations) == 1, 'Selected feature schema is ambiguous')
    namespace, type_name = qname(declarations[0].get('type'), declarations[0], namespaces)
    require(namespace == expected['namespace'], 'Unsupported external feature type')
    complex_types = [n for n in root.findall('x:complexType', NS) if n.get('name') == type_name]
    require(len(complex_types) == 1, 'Missing selected complex type')
    fields = complex_types[0].findall('x:complexContent/x:extension/x:sequence/x:element', NS)
    geometry_types = {'PointPropertyType': 'Point', 'MultiPointPropertyType': 'MultiPoint',
        'CurvePropertyType': 'LineString', 'LineStringPropertyType': 'LineString',
        'MultiCurvePropertyType': 'MultiLineString', 'MultiLineStringPropertyType': 'MultiLineString',
        'SurfacePropertyType': 'Polygon', 'PolygonPropertyType': 'Polygon',
        'MultiSurfacePropertyType': 'MultiPolygon', 'MultiPolygonPropertyType': 'MultiPolygon',
        'GeometryPropertyType': 'Geometry', 'MultiGeometryPropertyType': 'GeometryCollection'}
    attributes, geometry = [], None
    for field in fields:
        require(field.get('type') is not None and field.get('maxOccurs', '1') == '1', 'QA reference supports flat single-valued typed fields')
        namespace, kind = qname(field.get('type'), field, namespaces)
        descriptor = {'name': field.get('name'), 'nullable': field.get('nillable', 'false') in ('true', '1'), 'optional': field.get('minOccurs', '1') == '0'}
        if namespace == GML:
            require(geometry is None and kind in geometry_types, 'Unsupported QA reference geometry schema')
            geometry = descriptor | {'geometryType': geometry_types[kind]}
        else:
            require(namespace == XSD, 'QA reference does not resolve custom schema imports')
            attributes.append(descriptor | {'fieldType': kind})
    require(attributes == expected['fields'], 'Stored scalar schema differs from original XSD')
    require(geometry is not None, 'Original XSD has no supported geometry property')
    require((geometry['name'], geometry['geometryType'], geometry['nullable'], geometry['optional']) ==
        (expected['geometryField'], expected['geometryType'], expected['geometryNullable'], expected['geometryOptional']), 'Stored geometry schema differs from original XSD')
    return attributes


def scalar(text, field):
    kind, trimmed = field['fieldType'], text.strip()
    integers = {'integer', 'long', 'int', 'short', 'byte', 'unsignedLong', 'unsignedInt', 'unsignedShort', 'unsignedByte', 'positiveInteger', 'nonNegativeInteger', 'negativeInteger', 'nonPositiveInteger'}
    if kind in integers:
        n = int(trimmed)
        return n if -SAFE_INTEGER <= n <= SAFE_INTEGER else trimmed
    if kind == 'boolean':
        require(trimmed in ('true', 'false', '1', '0'), 'Invalid XML boolean')
        return trimmed in ('true', '1')
    if kind == 'decimal':
        return trimmed
    if kind in ('float', 'double'):
        number = float(trimmed)
        require(math.isfinite(number), 'Non-finite source number')
        return number
    return text


def inherited(node, attribute, parents):
    while node is not None:
        if attribute in node.attrib:
            return node.get(attribute)
        node = parents.get(node)
    return None


def gml_positions(node, parents):
    srs = inherited(node, 'srsName', parents)
    latitude_first = srs in (EPSG4326, 'http://www.opengis.net/def/crs/EPSG/0/4326')
    require(latitude_first or srs in (CRS84, 'urn:ogc:def:crs:OGC::CRS84', 'http://www.opengis.net/def/crs/OGC/1.3/CRS84'), 'QA reference requires explicit supported GML axes')
    dimension = inherited(node, 'srsDimension', parents) or '2'
    require(dimension == '2', 'QA reference does not drop extra coordinate dimensions')
    values = [float(n) for n in (node.text or '').split()]
    require(values and len(values) % 2 == 0 and all(math.isfinite(n) for n in values), 'Malformed source coordinate sequence')
    coordinates = [values[i:i + 2] for i in range(0, len(values), 2)]
    return [[p[1], p[0]] for p in coordinates] if latitude_first else coordinates


def gml_geometry(node, parents):
    require(node.tag.startswith('{' + GML + '}'), 'Foreign geometry namespace')
    kind = node.tag.split('}')[-1]
    if kind in ('Point', 'LineString', 'LinearRing'):
        coords = []
        for child in node:
            require(child.tag in ('{' + GML + '}pos', '{' + GML + '}posList'), 'Unsupported coordinate encoding')
            coords.extend(gml_positions(child, parents))
        if kind == 'Point':
            require(len(coords) == 1, 'Point is not one position')
            coords = coords[0]
        return {'type': kind, 'coordinates': coords}
    if kind == 'Polygon':
        exterior, holes = [], []
        for boundary in node:
            require(boundary.tag in ('{' + GML + '}exterior', '{' + GML + '}interior') and len(boundary) == 1, 'Unsupported polygon boundary')
            ring = gml_geometry(boundary[0], parents)
            require(ring['type'] == 'LinearRing', 'Polygon boundary is not linear')
            (exterior if boundary.tag.endswith('}exterior') else holes).append(ring['coordinates'])
        require(len(exterior) == 1, 'Polygon needs one exterior ring')
        return {'type': 'Polygon', 'coordinates': exterior + holes}
    aggregate = {'MultiPoint': ('MultiPoint', 'Point'), 'MultiCurve': ('MultiLineString', 'LineString'),
        'MultiLineString': ('MultiLineString', 'LineString'), 'MultiSurface': ('MultiPolygon', 'Polygon'),
        'MultiPolygon': ('MultiPolygon', 'Polygon'), 'MultiGeometry': ('GeometryCollection', None)}
    require(kind in aggregate, f'QA reference does not silently linearize {kind}')
    output_type, member_type = aggregate[kind]
    members = [gml_geometry(child, parents) for wrapper in node for child in wrapper]
    require(members and all(member_type is None or m['type'] == member_type for m in members), 'Unsupported aggregate member')
    return {'type': output_type, 'geometries': members} if member_type is None else {'type': output_type, 'coordinates': [m['coordinates'] for m in members]}


def parse_gml(text, schema):
    root, _, parents = xml_document(text)
    require(root.tag == '{' + WFS + '}FeatureCollection', 'Unexpected WFS response root')
    returned = int(root.attrib['numberReturned'])
    matched_text = root.attrib['numberMatched']
    matched = None if matched_text == 'unknown' else int(matched_text)
    require(root.get('timeStamp'), 'Missing WFS response timestamp')
    datetime.fromisoformat(root.get('timeStamp').replace('Z', '+00:00'))
    fields = {f['name']: f for f in schema['fields']}
    features = []
    for member in root.findall('w:member', NS):
        require(len(member) == 1, 'Unsupported feature member')
        source = member[0]
        require(source.tag == '{' + schema['namespace'] + '}' + schema['elementName'], 'Unexpected source feature type')
        ident = source.get('{' + GML + '}id')
        require(ident, 'Missing original source ID')
        properties, geometry = {}, None
        for prop in source:
            if prop.tag == '{' + GML + '}boundedBy':
                continue
            require(prop.tag.startswith('{' + schema['namespace'] + '}'), 'Foreign feature property')
            name = prop.tag.split('}')[-1]
            nil = prop.get('{' + XSI + '}nil') in ('true', '1')
            if name == schema['geometryField']:
                if not nil:
                    require(len(prop) == 1, 'Unsupported geometry field')
                    geometry = gml_geometry(prop[0], parents)
            else:
                require(name in fields and name not in properties and len(prop) == 0, 'Unknown, repeated or complex source property')
                properties[name] = None if nil else scalar(prop.text or '', fields[name])
        features.append({'type': 'Feature', 'id': ident, 'properties': properties, 'geometry': geometry})
    require(returned == len(features), 'GML numberReturned differs from actual members')
    return features, matched, {'numberReturned': returned, 'numberMatched': matched_text, 'timeStamp': root.get('timeStamp'), 'next': root.get('next')}


def parse_page(text, schema, format_id):
    if format_id == 'gml32':
        return parse_gml(text, schema)
    data = json.loads(text)
    require(data.get('type') == 'FeatureCollection', 'Source JSON is not a FeatureCollection')
    features = data['features']
    require(all('id' in f for f in features), 'Source JSON omits original feature IDs')
    require(data.get('numberReturned', len(features)) == len(features), 'JSON numberReturned differs')
    matched = data.get('numberMatched', data.get('totalFeatures'))
    require(data.get('numberMatched', matched) == data.get('totalFeatures', matched), 'JSON matching totals disagree')
    return features, None if matched == 'unknown' else matched, {k: v for k, v in data.items() if k != 'features'}


def positions(geometry):
    if geometry is None:
        return []
    if geometry['type'] == 'GeometryCollection':
        return [p for member in geometry['geometries'] for p in positions(member)]
    def walk(values):
        if values and isinstance(values[0], (int, float)):
            return [values]
        return [p for child in values for p in walk(child)]
    return walk(geometry['coordinates'])


def geometry_counts(geometry):
    if geometry is None:
        return Counter()
    result = Counter({geometry['type']: 1})
    if geometry['type'] == 'GeometryCollection':
        for member in geometry['geometries']:
            result.update(geometry_counts(member))
    return result


def holes(geometry):
    if geometry.geom_type == 'Polygon':
        return len(geometry.interiors)
    return sum(holes(g) for g in geometry.geoms) if geometry.geom_type in ('MultiPolygon', 'GeometryCollection') else 0


def verify_request(receipt, source, operation, start=None, returned=0):
    url = urllib.parse.urlsplit(receipt['url'])
    endpoint = urllib.parse.urlsplit(source['serviceUrl'])
    require((url.scheme, url.netloc, url.path) == (endpoint.scheme, endpoint.netloc, endpoint.path), 'Receipt leaves the configured HTTPS endpoint')
    pairs = urllib.parse.parse_qsl(url.query, keep_blank_values=True)
    actual = dict(pairs)
    require(len(pairs) == len(actual), 'Duplicate request parameter')
    expected = {'service': 'WFS', 'version': '2.0.0', 'request': operation, 'typeNames': source['collectionId']}
    if operation == 'GetFeature':
        w, s, e, n = source['requestedBounds']
        bbox = actual.pop('bbox', '').split(',')
        require(len(bbox) == 5 and [float(v) for v in bbox[:4]] == [s, w, n, e] and bbox[4] == EPSG4326, 'Request did not use explicit EPSG latitude-first bbox')
        if start is None:
            expected['resultType'] = 'hits'
        else:
            metadata = source['wfs']
            expected.update(outputFormat=metadata['format']['mime'], srsName=EPSG4326, startIndex=str(start), count=str(metadata['pageSize']))
            if metadata.get('sortField'):
                expected['sortBy'] = metadata['sortField'] + ' A'
    require(actual == expected, 'Receipt request differs from canonical complete-feature query')
    require(receipt['returned'] == returned and not receipt.get('parameters'), 'Receipt count or transport differs')


def verify_snapshot(raw_bytes, inspection_bytes, record, service, output):
    inspection = json.loads(inspection_bytes)
    asset, exported = inspection['asset'], inspection['geojson']
    require(record['asset'] == asset, 'Persisted asset differs from native inspection')
    require(asset['format'] == 'wfs-snapshot' and asset['storageMode'] == 'managed', 'Not a managed WFS source archive')
    require(asset['sourceSha256'] == sha(raw_bytes) and asset['bytes'] == len(raw_bytes) <= MAX_BYTES, 'Original archive bytes/hash differ')
    source = asset['remoteSource']
    metadata = source['wfs']
    schema = metadata['schema']
    require(exported['geodSource'] == source and not source.get('arcgis') and not asset.get('osmSource'), 'Trusted WFS provenance differs')
    require(source['selection'] == 'wfs-bbox-full-features' and source['serviceUrl'] == service['url'], 'Wrong source selection semantics or endpoint')
    require(metadata['capabilitiesSha256'] == service['wfs']['capabilitiesSha256'], 'Connection capabilities hash differs')
    require((metadata['fees'], metadata['accessConstraints']) == (service['wfs']['fees'], service['wfs']['accessConstraints']), 'Rights declarations changed')
    require(not asset.get('licenseUrl') and not source['licenseLinks'], 'An undeclared dataset license was inferred')
    timestamp = datetime.fromisoformat(source['requestedAt'].replace('Z', '+00:00'))
    require(timestamp.tzinfo is not None, 'Acquisition timestamp lacks timezone')
    archive = json.loads(raw_bytes)
    require(archive['version'] == metadata['rawArchiveVersion'] == 1, 'Unsupported source archive version')
    source_dir = output / 'original-documents'
    source_dir.mkdir()
    receipt_report = []
    def document(text, receipt, filename):
        raw = text.encode('utf-8')
        require(len(raw) == receipt['bytes'] and sha(raw) == receipt['sha256'], f'Original document changed: {filename}')
        (source_dir / filename).write_bytes(raw)
        receipt_report.append(dict(receipt, originalDocument='original-documents/' + filename))
    for archive_key, receipt_key, filename, operation in [
        ('schemaXml', 'schemaReceipt', 'schema-before.xsd', 'DescribeFeatureType'),
        ('schemaAfterXml', 'schemaAfterReceipt', 'schema-after.xsd', 'DescribeFeatureType'),
        ('hitsBeforeXml', 'hitsBefore', 'hits-before.xml', 'GetFeature'),
        ('hitsAfterXml', 'hitsAfter', 'hits-after.xml', 'GetFeature')]:
        document(archive[archive_key], metadata[receipt_key], filename)
        verify_request(metadata[receipt_key], source, operation)
    require(sha(archive['schemaXml'].encode()) == schema['sha256'], 'Parsed schema hash differs from original')
    reference_schema(archive['schemaXml'], schema)
    reference_schema(archive['schemaAfterXml'], schema)
    count = metadata['matchedCount']
    for text in (archive['hitsBeforeXml'], archive['hitsAfterXml']):
        features, matched, _ = parse_gml(text, schema)
        require(not features and matched == count, 'Hits totals changed or were not numeric')
    passes, page_metadata = [], []
    source_key = metadata.get('sortField')
    for pass_name, texts, receipts in [('first', archive['pages'], source['pages']), ('verification', archive['verificationPages'], metadata['verificationPages'])]:
        require(len(texts) == len(receipts), 'Archive page count differs from receipts')
        assembled, ids, keys = [], set(), []
        for index, (text, receipt) in enumerate(zip(texts, receipts)):
            extension = 'xml' if metadata['format']['id'] == 'gml32' else 'json'
            document(text, receipt, f'{pass_name}-{index + 1:03}.{extension}')
            features, matched, response_metadata = parse_page(text, schema, metadata['format']['id'])
            require(features and len(features) <= metadata['pageSize'] and matched in (None, count), 'Page is incomplete, excessive, or reports changed totals')
            verify_request(receipt, source, 'GetFeature', start=len(assembled), returned=len(features))
            for feature in features:
                identity = (type(feature['id']).__name__, str(feature['id']))
                require(identity not in ids, 'Repeated source feature identity')
                ids.add(identity)
                if source_key:
                    key = int(feature['properties'][source_key])
                    require(not keys or key > keys[-1], 'Source keys are not unique and ascending')
                    keys.append(key)
            assembled.extend(features)
            page_metadata.append({'pass': pass_name, 'page': index + 1, 'responseMetadata': response_metadata})
        require(len(assembled) == count, 'Pages do not account for all numeric hits')
        passes.append(assembled)
    first, second = passes
    if source_key:
        require([(f['properties'], f['geometry']) for f in first] == [(f['properties'], f['geometry']) for f in second], 'Second read changed source keys, properties or geometry')
    else:
        require(first == second, 'Second read changed identity or content')
    require(exported['features'] == first, 'Export differs from original first-pass IDs, attributes or full geometry')
    require(asset['featureCount'] == source['featureCount'] == source['numberMatched'] == len(first), 'Export count differs from source completeness evidence')
    geometries, coords, total_holes, detail = Counter(), [], 0, []
    w, s, e, n = source['requestedBounds']
    for feature in first:
        geom = feature['geometry']
        points = positions(geom)
        require(all(len(p) == 2 and all(math.isfinite(v) for v in p) and -180 <= p[0] <= 180 and -90 <= p[1] <= 90 for p in points), 'Invalid WGS84 output coordinate')
        count_holes, valid = 0, True
        if geom is not None:
            independent = shape(geom)
            valid = independent.is_valid and not independent.is_empty
            require(valid, 'GEOS rejected original geometry: ' + explain_validity(independent))
            count_holes = holes(independent)
        total_holes += count_holes
        geometries.update(geometry_counts(geom))
        coords.extend(points)
        detail.append({'id': feature['id'], 'sourceKey': feature['properties'].get(source_key) if source_key else None,
            'geometryType': geom['type'] if geom else None, 'coordinates': len(points), 'holes': count_holes, 'geosValid': valid,
            'propertyNames': sorted(feature['properties']), 'coordinatesOutsideSelection': sum(not (w <= p[0] <= e and s <= p[1] <= n) for p in points)})
    bounds = [min(p[0] for p in coords), min(p[1] for p in coords), max(p[0] for p in coords), max(p[1] for p in coords)] if coords else None
    require(asset['coordinateCount'] == len(coords) and asset['geometryCounts'] == dict(geometries), 'Native geometry statistics differ')
    require(asset['bounds'] == bounds and asset['crs'] == 'EPSG:4326', 'Native full geometry bounds differ')
    exported_bytes = raw_object_field(inspection_bytes, 'geojson')
    require(sha(exported_bytes) == asset['geojsonSha256'], 'Exact export byte hash differs')
    (output / 'export.geojson').write_bytes(exported_bytes)
    save_json(output / 'expected-ids.json', [f['id'] for f in first])
    return {'assetId': asset['id'], 'sourceBytes': len(raw_bytes), 'sourceSha256': sha(raw_bytes), 'exportSha256': sha(exported_bytes),
        'sourceFeatureCount': count, 'coordinateCount': len(coords), 'geometryCounts': dict(geometries), 'holes': total_holes,
        'bounds': bounds, 'schemaProperties': schema['fields'], 'format': metadata['format'], 'sourceKey': source_key,
        'expectedIds': [f['id'] for f in first], 'sourceRawDocuments': receipt_report, 'pageMetadata': page_metadata, 'features': detail,
        'sourceDocumentsBytes': sum(r['bytes'] for r in receipt_report), 'twoReadsVerified': True, 'transactionalSnapshotClaimed': False,
        'originalFirstPassIdsPreserved': True, 'allPropertiesAndFullCoordinatesVerified': True,
        'independentGeometryMethod': 'Separate Python XML/JSON decoding, exact coordinates/properties comparison and Shapely/GEOS validity/hole counts',
        'rawResponseFile': 'source-archive.json', 'exportFile': 'export.geojson', 'nonEmpty': count > 0}


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
            with self.opener.open(request, timeout=180) as response:
                data = response.read(100 * 1024 * 1024 + 1)
            require(len(data) <= 100 * 1024 * 1024, 'Runtime response too large')
            return json.loads(data), data
        except urllib.error.HTTPError as error:
            raise RuntimeError(f'Runtime HTTP {error.code}: {error.read(16384).decode("utf-8", errors="replace")}') from error


def parse_bounds(text):
    try:
        result = [float(n) for n in text.split(',')]
        require(len(result) == 4 and all(math.isfinite(n) for n in result), 'Expected W,S,E,N finite coordinates')
        w, s, e, n = result
        require(-180 <= w < e <= 180 and -90 <= s < n <= 90, 'Invalid WGS84 bounds')
        require(e - w <= 1 and n - s <= 1, 'QA probes require at most one degree per bbox side')
        return result
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True, help='Explicit local runtime, e.g. http://127.0.0.1:4364')
    parser.add_argument('--data-dir', required=True, type=Path, help='Actual isolated runtime backing directory')
    parser.add_argument('--endpoint', required=True, help='Explicit authorized public HTTPS WFS endpoint; no default')
    parser.add_argument('--type-name', required=True, help='Exact discovered qualified feature type name')
    parser.add_argument('--bounds', required=True, type=parse_bounds, metavar='W,S,E,N')
    parser.add_argument('--format', choices=('gml32', 'geojson'), default='gml32')
    parser.add_argument('--page-size', type=int, default=2, help='1-200; use a small value for actual paging evidence')
    parser.add_argument('--service-id', help='Reuse an exact connection without another capability request')
    parser.add_argument('--asset-id', help='Inspect this already-acquired asset; no provider request is made')
    parser.add_argument('--require-nonempty', action='store_true')
    parser.add_argument('--output', required=True, type=Path, help='New evidence directory')
    args = parser.parse_args()
    require(args.data_dir.is_dir(), 'Runtime data directory does not exist')
    require(1 <= args.page_size <= 200, 'Page size must be in 1-200')
    endpoint = urllib.parse.urlsplit(args.endpoint)
    require(endpoint.scheme == 'https' and endpoint.hostname and not endpoint.username and not endpoint.password and not endpoint.query and not endpoint.fragment, 'Use a credential-free HTTPS endpoint with no query/fragment')
    output = args.output.resolve()
    require(not output.exists(), 'Output directory must be new; evidence will not be overwritten')
    output.mkdir(parents=True)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(), 'mode': 'Actual native WFS snapshot; no synthetic data',
        'server': args.server, 'endpoint': args.endpoint, 'dataDirectory': str(args.data_dir.resolve()), 'typeName': args.type_name,
        'bounds': args.bounds, 'format': args.format, 'pageSize': args.page_size, 'automaticRetries': 0, 'desktopWebViewAccepted': False, 'receipts': []}
    save_json(output / 'report.json', report)
    try:
        runtime = Runtime(args.server)
        services, _ = runtime.call('/feature-services')
        matches = [s for s in services if s['url'] == args.endpoint and s.get('wfs') and (not args.service_id or s['id'] == args.service_id)]
        require(not args.service_id or len(matches) == 1, 'Specified service does not match the WFS endpoint')
        if matches:
            require(len(matches) == 1, 'Ambiguous endpoint registrations; pass --service-id')
            service = matches[0]
            report['connection'] = 'Reused exact persisted WFS connection'
        else:
            require(not args.asset_id, '--asset-id requires an existing matching service to avoid a public request')
            service, _ = runtime.call('/feature-services', {'name': 'WFS independent verification', 'url': args.endpoint, 'protocol': 'WFS2'})
            report['connection'] = 'Discovered through native GetCapabilities'
        require(service.get('wfs') and service['url'] == args.endpoint, 'Connected service differs')
        require(any(c['id'] == args.type_name for c in service['collections']), 'Requested type was not discovered as usable')
        registry = json.loads((args.data_dir / 'feature-services.json').read_text(encoding='utf-8'))
        require(registry[service['id']] == service, 'Data directory does not match runtime service registry')
        save_json(output / 'service.json', service)
        request = {'serviceId': service['id'], 'collectionId': args.type_name, 'bounds': args.bounds, 'responseFormat': args.format, 'pageSize': args.page_size}
        save_json(output / 'request.json', request)
        if args.asset_id:
            inspection, inspection_bytes = runtime.call('/vectors/' + args.asset_id)
            asset = inspection['asset']
        else:
            print('Acquiring one bounded snapshot through the native WFS client', flush=True)
            asset, _ = runtime.call('/feature-services/query', request)
            inspection, inspection_bytes = runtime.call('/vectors/' + asset['id'])
            require(inspection['asset'] == asset, 'Query asset differs from fresh inspection')
        source = asset['remoteSource']
        require(source['collectionId'] == args.type_name and source['requestedBounds'] == args.bounds and source['wfs']['format']['id'] == args.format and source['wfs']['pageSize'] == args.page_size, 'Asset differs from requested query')
        vectors = json.loads((args.data_dir / 'vectors.json').read_text(encoding='utf-8'))
        record = vectors[asset['id']]
        backing = Path(record['path'])
        expected = args.data_dir / 'vectors' / (asset['id'] + '.json')
        require(backing.is_file() and backing.samefile(expected) and backing.stat().st_size <= MAX_BYTES, 'Unexpected source backing file')
        raw = backing.read_bytes()
        (output / 'source-archive.json').write_bytes(raw)
        (output / 'inspection.json').write_bytes(inspection_bytes)
        save_json(output / 'registry-record.json', record)
        receipt = verify_snapshot(raw, inspection_bytes, record, service, output)
        receipt['backingPath'] = str(backing)
        report['receipts'].append(receipt)
        require(receipt['nonEmpty'] or not args.require_nonempty, 'Complete empty snapshot; positive acceptance was requested')
        report['status'] = 'passed'
    except Exception as error:
        report['status'], report['error'] = 'failed', str(error)
        raise
    finally:
        report['completedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'report': str(output / 'report.json'), 'status': report['status'], 'features': receipt['sourceFeatureCount']}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'Verification failed: {error}', file=sys.stderr)
        sys.exit(1)
