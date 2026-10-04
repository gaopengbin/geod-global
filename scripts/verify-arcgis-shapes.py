"""Verify actual public line/polygon extraction against independent Esri responses.

This opt-in acceptance probe writes only to the chosen local runtime. Remote
requests are read-only queries of Esri's mutable sample service, not fixtures.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import urllib.parse
import urllib.request

ROOT = 'https://sampleserver6.arcgisonline.com/arcgis/rest/services/Wildfire/FeatureServer'
CASES = [('line', '1', [-121.5, 38.655, -121.498, 38.657]),
         ('polygon', '2', [28.9, -25.9, 28.91, -25.89])]


def positions(value):
    if value and isinstance(value[0], (int, float)):
        assert len(value) == 2
        return [tuple(value)]
    return [p for child in value for p in positions(child)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--direct', action='store_true')
    args = parser.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)
    local = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    remote = urllib.request.build_opener(urllib.request.ProxyHandler({})) if args.direct else urllib.request.build_opener()

    def request(path, data=None):
        req = urllib.request.Request(args.server + path,
            data=json.dumps(data).encode() if data is not None else None,
            headers={'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global'})
        try:
            with local.open(req, timeout=160) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError(error.read().decode()) from error

    def independent(label, layer_id, parameters):
        req = urllib.request.Request(ROOT + '/' + layer_id + '/query',
            data=urllib.parse.urlencode(parameters).encode())
        with remote.open(req, timeout=45) as response:
            raw = response.read(20 * 1024 * 1024 + 1)
        assert len(raw) <= 20 * 1024 * 1024
        (out / (label + '.json')).write_bytes(raw)
        value = json.loads(raw)
        assert 'error' not in value
        assert value.get('exceededTransferLimit', False) is False
        assert value.get('properties', {}).get('exceededTransferLimit', False) is False
        return value

    service = request('/feature-services', {'name': 'Esri · Sample line and polygon data', 'url': ROOT, 'protocol': 'ArcGIS'})
    results = []
    for label, layer_id, bounds in CASES:
        asset = request('/feature-services/query', {'serviceId': service['id'], 'collectionId': layer_id,
            'bounds': bounds, 'areaGeometry': None, 'pageSize': 4})
        read = request('/vectors/' + asset['id'])
        assert read['asset'] == asset
        source, geo = asset['remoteSource'], read['geojson']
        assert geo['geodSource'] == source
        assert 0 < asset['featureCount'] <= 200, 'Sample contents changed; review a bounded test region'
        layer = source['arcgis']['layer']
        assert layer['spatialReference']['latestWkid'] == 3857
        features = {f['id']: f for f in geo['features']}
        assert sorted(features) == source['arcgis']['objectIds']
        independent_ids = []
        for i, page in enumerate(source['pages']):
            batch = independent(label + '-batch-' + str(i), layer_id, page['parameters'])
            assert len(batch['features']) == page['returned']
            for feature in batch['features']:
                assert feature == features[feature['id']]
                independent_ids.append(feature['id'])
        assert sorted(independent_ids) == source['arcgis']['objectIds']
        original = independent(label + '-esri', layer_id, {'f': 'json',
            'objectIds': ','.join(map(str, source['arcgis']['objectIds'])),
            'outFields': '*', 'outSR': '4326', 'returnGeometry': 'true'})
        assert len(original['features']) == len(features)
        names = {f['name'] for f in layer['fields']}
        values = coords = nulls = 0
        outside = False
        for item in original['features']:
            properties = item['attributes']
            feature = features[properties[layer['objectIdField']]]
            assert set(properties) == names and properties == feature['properties']
            native = positions(item['geometry']['paths' if label == 'line' else 'rings'])
            saved = positions(feature['geometry']['coordinates'])
            # GeoJSON can reverse polygon ring traversal; check all positions
            # and multiplicity against Esri JSON, exact topology against GeoJSON.
            assert Counter(native) == Counter(saved)
            coords += len(saved) * 2
            values += len(properties)
            nulls += sum(v is None for v in properties.values())
            outside |= any(not (bounds[0] <= x <= bounds[2] and bounds[1] <= y <= bounds[3]) for x, y in saved)
        assert outside, 'This acceptance region must demonstrate unclipped geometry'
        raw = json.dumps(geo, ensure_ascii=False).encode()
        (out / (label + '-saved.geojson')).write_bytes(raw)
        results.append({'label': label, 'assetId': asset['id'], 'serviceId': service['id'],
            'layerId': layer_id, 'bounds': bounds, 'featureCount': len(features), 'dataBatches': len(source['pages']),
            'fields': len(names), 'propertyValues': values, 'coordinateValues': coords, 'nullValues': nulls,
            'geometryTypes': sorted({f['geometry']['type'] for f in features.values()}),
            'sourceSpatialReference': layer['spatialReference'], 'outputCrs': 'EPSG:4326',
            'allParsedGeoJSONAndPropertiesEqual': True, 'independentEsriPositionsEqual': True,
            'fullGeometryOutsideSelectionRetained': True, 'savedEvidenceSha256': hashlib.sha256(raw).hexdigest()})
    report = {'checkedAt': datetime.now(timezone.utc).isoformat(), 'serviceUrl': ROOT,
        'mode': 'Actual native extraction and independent public HTTP comparison',
        'independentNetworkRoute': 'direct' if args.direct else 'system', 'fixturesUsed': False,
        'authenticationUsed': False, 'editingRequests': False, 'cases': results,
        'notice': 'Mutable Esri sample service; validates service GeoJSON, not geodatabase or transactional consistency.'}
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(results))


if __name__ == '__main__':
    main()
