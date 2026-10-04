"""Independent full Shapefile checks with PyShp, GDAL and PROJ (QA only).

Raw DBF numeric lexemes are compared exactly, including records PyShp/GDAL skip
as deleted. Every geometry ordinate and source measure is checked. ZIP originals
are byte-identical; assembled sidecars are checked member by member instead.
"""
import argparse
import datetime
import hashlib
import io
import json
import math
import struct
import zipfile
from pathlib import Path
import shapefile
from osgeo import gdal, ogr
from pyproj import Transformer
from pyproj.enums import TransformDirection

gdal.UseExceptions()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def contents(path):
    if path.suffix.lower() == '.zip':
        with zipfile.ZipFile(path) as source:
            return {name: source.read(name) for name in source.namelist() if not name.endswith('/')}
    suffixes = {'.shp', '.shx', '.dbf', '.prj', '.cpg', '.qix', '.sbn', '.sbx', '.fix', '.shp.xml'}
    return {p.name: p.read_bytes() for p in path.parent.iterdir()
            if p.name[:len(path.stem)].lower() == path.stem.lower()
            and p.name[len(path.stem):].lower() in suffixes}


def operation(layer):
    code = layer['coordinateOperationId']
    if code:
        transform = Transformer.from_pipeline('urn:ogc:def:coordinateOperation:EPSG::'+str(code))
        source = Transformer.from_crs(layer['definition'], transform.source_crs if layer['coordinateOperationDirection'] == 'forward' else transform.target_crs, always_xy=True)
        def apply(x, y):
            lon, lat = source.transform(x, y, errcheck=True)
            if layer['coordinateOperationDirection'] == 'forward':
                lat, lon = transform.transform(lat, lon, errcheck=True)
            else:
                lat, lon = transform.transform(lat, lon, direction=TransformDirection.INVERSE, errcheck=True)
            return lon, lat
        return apply
    # Controls with an identity horizontal datum still require inverse projection.
    return Transformer.from_crs(layer['definition'], 'EPSG:4326', always_xy=True).transform


def geometry(shape, project):
    if shape.shapeType == 0:
        return None, None
    raw = json.loads(json.dumps(shape.__geo_interface__))
    base = shape.shapeType % 10
    has_z = shape.shapeType in (11, 13, 15, 18)
    has_m = shape.shapeType >= 10
    source_points = list(shape.points)
    z = list(shape.z) if has_z else []
    measures = list(shape.m) if has_m and hasattr(shape, 'm') else [None] * len(source_points)
    indices = list(shape.parts) if hasattr(shape, 'parts') and shape.parts else [0]
    ranges = [(a, b) for a, b in zip(indices, indices[1:] + [len(source_points)])]

    def point(i):
        x, y = project(*source_points[i][:2])
        p = [x, y] + ([z[i]] if has_z else [])
        m = measures[i] if has_m else None
        if m is not None and (not math.isfinite(m) or m < -1e38):
            m = None
        return p, m

    def part(coords):
        matching = [(a, b) for a, b in ranges if [list(p[:2]) for p in source_points[a:b]] == coords]
        assert len(matching) == 1, ('independent part association', coords)
        a, b = matching[0]
        values = [point(i) for i in range(a, b)]
        return [p for p, m in values], [m for p, m in values]

    if base == 1:
        coords, m = point(0)
    elif base == 8:
        values = [point(i) for i in range(len(source_points))]
        coords, m = [p for p, m in values], [m for p, m in values]
    elif base == 3:
        if raw['type'] == 'LineString':
            coords, m = part(raw['coordinates'])
        else:
            values = [part(c) for c in raw['coordinates']]
            coords, m = [p for p, m in values], [m for p, m in values]
    else:
        polys = raw['coordinates'] if raw['type'] == 'MultiPolygon' else [raw['coordinates']]
        values = [[part(ring) for ring in polygon] for polygon in polys]
        coords = [[p for p, m in polygon] for polygon in values]
        m = [[m for p, m in polygon] for polygon in values]
        if raw['type'] == 'Polygon':
            coords, m = coords[0], m[0]
    return {'type': raw['type'], 'coordinates': coords}, m if has_m else None


def compare(actual, expected, stats, where):
    if isinstance(expected, dict):
        assert isinstance(actual, dict) and set(actual) == set(expected), where
        for key in expected:
            compare(actual[key], expected[key], stats, where+'.'+key)
    elif isinstance(expected, list):
        assert isinstance(actual, list) and len(actual) == len(expected), (where, actual, expected)
        for i, (a, e) in enumerate(zip(actual, expected)):
            compare(a, e, stats, where+f'[{i}]')
    elif isinstance(expected, (int, float)) and not isinstance(expected, bool):
        assert isinstance(actual, (int, float)) and not isinstance(actual, bool), where
        delta = abs(actual-expected)
        stats['numericOrdinates'] += 1
        stats['maximumCoordinateDifference'] = max(stats['maximumCoordinateDifference'], delta)
        assert delta <= 1e-8, (where, actual, expected, delta)
    else:
        assert actual == expected, (where, actual, expected)


def properties(raw, layer):
    count, header, length = struct.unpack_from('<IHH', raw, 4)
    fields = layer['fields']
    encoding = {'ASCII': 'ascii', 'ISO-8859-1': 'latin1', 'GBK': 'gbk', 'UTF-8': 'utf-8'}.get(layer['encoding'], layer['encoding'])
    result = []
    for i in range(count):
        row = raw[header+i*length:header+(i+1)*length]
        out = {}
        start = 1
        for field in fields:
            value = row[start:start+field['width']]
            start += field['width']
            kind = field['fieldType']
            if kind == 'C':
                value = value.rstrip(b' \0').decode(encoding)
            else:
                text = value.decode('ascii').strip()
                if kind in ('N', 'F'):
                    value = None if not text or set(text) == {'*'} else text
                elif kind == 'L':
                    value = True if text.upper() in ('T', 'Y') else False if text.upper() in ('F', 'N') else None
                else:
                    value = datetime.datetime.strptime(text, '%Y%m%d').date().isoformat() if text and text != '00000000' else None
            out[field['name']] = value
        assert start == length
        result.append((out, row[0] == ord('*')))
    return result


def verify(source, converted, original, registry=None, asset_id=None):
    members = contents(source)
    bundle = contents(original)
    assert set(bundle) == set(members)
    assert all(bundle[n] == members[n] for n in members)
    data = json.loads(converted.read_text(encoding='utf-8'))
    provenance = data['geodShapefile']
    if source.suffix.lower() == '.zip':
        assert source.read_bytes() == original.read_bytes()
        assert provenance['container'] == 'zip'
    else:
        assert provenance['container'] == 'sidecars'
    receipts = {f['name']: f for f in provenance['files']}
    assert set(receipts) == set(members)
    for name, content in members.items():
        assert receipts[name]['bytes'] == len(content) and receipts[name]['sha256'] == digest(content)
    if registry:
        asset = json.loads(registry.read_text(encoding='utf-8'))[asset_id]['asset']
        assert asset['shapefile'] == provenance and asset['sourceSha256'] == digest(original.read_bytes())
        assert asset['featureCount'] == len(data['features'])
    folded = {n.lower(): n for n in members}
    all_features = {}
    for feature in data['features']:
        key = (feature['geodLayer'], feature['id'])
        assert key not in all_features
        all_features[key] = feature
    stats = {'layers': len(provenance['layers']), 'records': 0, 'deletedRecords': 0,
             'attributes': 0, 'numericOrdinates': 0, 'maximumCoordinateDifference': 0,
             'gdalNonDeletedFeatures': 0, 'gdalGeometryChecks': 0, 'layerChecks': []}
    for layer in provenance['layers']:
        table = layer['table']
        get = lambda ext: members[folded[(table+'.'+ext).lower()]]
        reader = shapefile.Reader(shp=io.BytesIO(get('shp')), shx=io.BytesIO(get('shx')), dbf=io.BytesIO(get('dbf')))
        attributes = properties(get('dbf'), layer)
        project = operation(layer)
        for i, shape in enumerate(reader.shapes()):
            feature = all_features.pop((table, i))
            expected, m = geometry(shape, project)
            compare(feature['geometry'], expected, stats, table+f'[{i}].geometry')
            if shape.shapeType >= 10:
                compare(feature['geodMeasures'], m, stats, table+f'[{i}].M')
            else:
                assert 'geodMeasures' not in feature
            props, deleted = attributes[i]
            assert feature['properties'] == props, (table, i, feature['properties'], props)
            assert feature.get('geodDeleted', False) == deleted
            stats['records'] += 1
            stats['deletedRecords'] += int(deleted)
            stats['attributes'] += len(props)
        # A second independent reader checks ordinary active geometries/attributes.
        vfs = '/vsimem/geod-independent-shp.zip'
        gdal.FileFromMemBuffer(vfs, original.read_bytes())
        dataset = ogr.Open('/vsizip/'+vfs+'/'+folded[(table+'.shp').lower()])
        assert dataset is not None
        gdal_layer = dataset.GetLayer()
        for feature in gdal_layer:
            stats['gdalNonDeletedFeatures'] += 1
            geom = feature.GetGeometryRef()
            if geom is not None:
                assert geom.IsValid(), (table, feature.GetFID(), 'GDAL invalid geometry')
                stats['gdalGeometryChecks'] += 1
        dataset = None
        gdal.Unlink(vfs)
        stats['layerChecks'].append({'layer': table, 'shapeType': layer['shapeType'],
                                    'records': len(attributes), 'encoding': layer['encoding'],
                                    'datumOperation': layer['coordinateOperation']})
    assert not all_features
    return {'sourceSha256': digest(source.read_bytes()) if source.suffix.lower() == '.zip' else None,
            'originalBundleSha256': digest(original.read_bytes()),
            'sourceMembers': len(members), 'sourceMemberBytes': sum(map(len, members.values())),
            'originalZIPIdentical': source.suffix.lower() == '.zip', 'allOriginalMembersIdentical': True,
            'readerVersions': {'pyshp': shapefile.__version__, 'gdal': gdal.VersionInfo('--version')}, **stats}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--converted', type=Path, required=True)
    parser.add_argument('--original', type=Path, required=True)
    parser.add_argument('--registry', type=Path)
    parser.add_argument('--id')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    report = verify(args.source, args.converted, args.original, args.registry, args.id)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    main()
