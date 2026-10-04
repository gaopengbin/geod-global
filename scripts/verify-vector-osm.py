"""Independent acceptance of a manually obtained OSM extract and native export.

GEOS polygonization verifies relation topology independently of GeoD's endpoint
joining. This script does not contact an Overpass server or import test fixtures
into product storage. Requires Shapely in the QA environment only.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
from shapely.geometry import Point, LineString, Polygon, shape
from shapely.ops import polygonize_full, unary_union


def coords(geometry):
    return [(p['lon'], p['lat']) for p in geometry]


def polygons(lines):
    result, cuts, dangles, invalid = polygonize_full(lines)
    assert cuts.is_empty and dangles.is_empty and invalid.is_empty, 'Incomplete relation geometry'
    return unary_union(list(result.geoms))


def reference(element):
    kind = element['type']
    if kind == 'node':
        return Point(element['lon'], element['lat'])
    if kind == 'way':
        xy = coords(element['geometry'])
        tags = element.get('tags', {})
        area = tags.get('area') == 'yes' or (tags.get('area') != 'no' and 'highway' not in tags
            and any(key in tags for key in ['building', 'landuse', 'leisure', 'amenity']))
        area = area or (tags.get('natural') == 'water' and tags.get('area') != 'no')
        return Polygon(xy) if area else LineString(xy)
    assert element['tags']['type'] in ['multipolygon', 'boundary']
    outer = [LineString(coords(m['geometry'])) for m in element['members'] if m['type'] == 'way' and m['role'] in ['', 'outer']]
    inner = [LineString(coords(m['geometry'])) for m in element['members'] if m['type'] == 'way' and m['role'] == 'inner']
    return polygons(outer).difference(polygons(inner)) if inner else polygons(outer)


def positions(geometry):
    if geometry['type'] == 'GeometryCollection':
        return [p for g in geometry['geometries'] for p in positions(g)]
    def walk(value):
        if value and isinstance(value[0], (float, int)):
            return [value]
        return [p for child in value for p in walk(child)]
    return walk(geometry['coordinates'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', required=True)
    parser.add_argument('--export', required=True, dest='exported')
    parser.add_argument('--registry', required=True)
    parser.add_argument('--report', required=True)
    args = parser.parse_args()
    raw = Path(args.source).read_bytes()
    original = json.loads(raw)
    data = json.loads(Path(args.exported).read_text(encoding='utf-8'))
    registry = json.loads(Path(args.registry).read_text(encoding='utf-8'))
    assert not original.get('remark')
    elements = {f"{e['type']}/{e['id']}": e for e in original['elements']}
    features = {f['id']: f for f in data['features']}
    assert len(features) == len(data['features']) == len(elements)
    assert set(elements) == set(features), 'Lost or invented OSM identity'
    holes = 0
    for identity, e in elements.items():
        f = features[identity]
        assert f['properties'] == {'osm_type': e['type'], 'osm_id': e['id'], 'tags': e.get('tags', {})}
        actual = shape(f['geometry'])
        assert actual.is_valid, f'Invalid topology {identity}'
        expected = reference(e)
        assert actual.equals(expected), f'GEOS reference mismatch {identity}'
        if e['type'] == 'way':
            assert f['geometry']['coordinates'] == ([list(p) for p in coords(e['geometry'])] if actual.geom_type == 'LineString'
                else [[list(p) for p in coords(e['geometry'])]])
        if actual.geom_type in ['Polygon', 'MultiPolygon']:
            holes += sum(len(p.interiors) for p in (actual.geoms if actual.geom_type == 'MultiPolygon' else [actual]))
    counts = dict(sorted(collections.Counter(f['geometry']['type'] for f in data['features']).items()))
    points = [p for f in data['features'] for p in positions(f['geometry'])]
    bounds = [min(p[0] for p in points), min(p[1] for p in points), max(p[0] for p in points), max(p[1] for p in points)]
    assets = [r['asset'] for r in registry.values() if r['asset']['sourceSha256'] == hashlib.sha256(raw).hexdigest()]
    assert {a['storageMode'] for a in assets} == {'reference', 'managed'}
    for asset in assets:
        assert asset['featureCount'] == len(features) and asset['coordinateCount'] == len(points)
        assert asset['bounds'] == bounds and asset['geometryCounts'] == counts
        assert asset['dataTimestamp'] == original['osm3s']['timestamp_osm_base']
        assert asset['licenseUrl'] == 'https://www.openstreetmap.org/copyright'
        assert asset['attribution'] == '© OpenStreetMap contributors'
        record = registry[asset['id']]
        assert hashlib.sha256(Path(record['path']).read_bytes()).hexdigest() == asset['sourceSha256']
    report = {
        'source': {'provider': 'OpenStreetMap / Overpass API', 'acquisition': 'One manual bounded QA request; no public Overpass backend in the application',
            'query': '(way[highway](52.516,13.375,52.518,13.38);way[building](52.516,13.375,52.518,13.38);relation[building][type=multipolygon](52.516,13.375,52.518,13.38);node[amenity](52.516,13.375,52.518,13.38););out body geom;',
            'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'timestamp':original['osm3s']['timestamp_osm_base'],
            'license':'ODbL','attribution':'© OpenStreetMap contributors','licenseUrl':'https://www.openstreetmap.org/copyright'},
        'acceptance': {'features':len(features),'coordinates':len(points),'geometryCounts':counts,'bounds':bounds,'holes':holes,
            'allIdentitiesAndTagsEqual':True,'allGeometriesEqualIndependentGEOSReference':True,'allExportedGeometriesValid':True,
            'wayCoordinateOrderAndPrecisionPreserved':True,'referenceAndExplicitManagedCopyByteHashesEqual':True},
        'limits': {'formats':['RFC 7946 GeoJSON','Overpass JSON with complete embedded geometry'], 'maxBytes':20971520,'maxFeatures':50000,'maxCoordinates':500000,
            'excluded':['Online OSM service integration','PBF/XML','General OSM relation types','Automatic CRS inference','General topology repair']},
    }
    Path(args.report).write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report['acceptance'],ensure_ascii=False))


if __name__ == '__main__':
    main()
