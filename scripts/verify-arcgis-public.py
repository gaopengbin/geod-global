"""Actual public ArcGIS query acceptance, independent HTTP and GeoJSON comparison."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import urllib.parse
from datetime import datetime, timezone


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--direct', action='store_true', help='Bypass proxy for independent public verification only')
    args = parser.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)
    local = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    remote = urllib.request.build_opener(urllib.request.ProxyHandler({})) if args.direct else urllib.request.build_opener()

    def request(path, data=None):
        body = json.dumps(data).encode() if data is not None else None
        req = urllib.request.Request(args.server + path, data=body, headers={
            'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global'})
        try:
            with local.open(req, timeout=160) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError(error.read().decode()) from error

    root = 'https://sampleserver6.arcgisonline.com/arcgis/rest/services/Earthquakes_Since1970/FeatureServer'
    service = request('/feature-services', {'name': 'Esri · Historical earthquakes', 'url': root, 'protocol': 'ArcGIS'})
    assert service['arcgis'] and any(c['id'] == '0' for c in service['collections'])
    asset = request('/feature-services/query', {'serviceId': service['id'], 'collectionId': '0',
                    'bounds': [-123, 37, -121, 39], 'areaGeometry': None, 'pageSize': 2})
    read = request('/vectors/' + asset['id'])
    assert read['asset'] == asset
    geo = read['geojson']
    source = asset['remoteSource']
    assert source == geo['geodSource']
    assert source['featureCount'] == asset['featureCount'] == 6 and len(source['pages']) == 3
    assert source['arcgis']['objectIds'] == sorted(f['id'] for f in geo['features'])
    features = {f['id']: f for f in geo['features']}
    independent = []
    receipts = []
    for index, page in enumerate(source['pages']):
        body = urllib.parse.urlencode(page['parameters']).encode()
        req = urllib.request.Request(page['url'], data=body)
        with remote.open(req, timeout=45) as response:
            raw = response.read(20 * 1024 * 1024 + 1)
        (out / f'independent-batch-{index}.geojson').write_bytes(raw)
        value = json.loads(raw)
        assert 'error' not in value
        assert len(value['features']) == page['returned']
        for f in value['features']:
            assert f == features[f['id']]
        independent.extend(value['features'])
        receipts.append({'batch': index, 'features': len(value['features']),
                         'sameParsedGeometryAndProperties': True,
                         'responseSha256': hashlib.sha256(raw).hexdigest(),
                         'exactResponseBytesHash': hashlib.sha256(raw).hexdigest() == page['sha256']})
    assert sorted(f['id'] for f in independent) == source['arcgis']['objectIds']
    # An independent Esri JSON response additionally checks the field values
    # and coordinates before server GeoJSON formatting, not just a second parse.
    params = {'f': 'json', 'objectIds': ','.join(map(str, source['arcgis']['objectIds'])),
              'outFields': '*', 'outSR': '4326', 'returnGeometry': 'true'}
    req = urllib.request.Request(root + '/0/query', data=urllib.parse.urlencode(params).encode())
    with remote.open(req, timeout=45) as response:
        raw = response.read(20 * 1024 * 1024 + 1)
    (out / 'independent-esri.json').write_bytes(raw)
    esri = json.loads(raw)
    fields = {f['name'] for f in source['arcgis']['layer']['fields']}
    values = coordinates = nulls = 0
    for original in esri['features']:
        attributes = original['attributes']
        f = features[attributes[source['arcgis']['layer']['objectIdField']]]
        assert set(attributes) == set(f['properties']) == fields
        assert attributes == f['properties']
        assert f['geometry'] == {'type': 'Point', 'coordinates': [original['geometry']['x'], original['geometry']['y']]}
        values += len(attributes)
        nulls += sum(v is None for v in attributes.values())
        coordinates += 2
    assert values == 168 and coordinates == 12
    empty = request('/feature-services/query', {'serviceId': service['id'], 'collectionId': '0',
                    'bounds': [-10, 0, -9.999, 0.001], 'areaGeometry': None})
    assert empty['featureCount'] == 0 and empty['remoteSource']['pages'] == []
    report = {'checkedAt': datetime.now(timezone.utc).isoformat(),
              'mode': 'Real public Esri sample through native runtime plus independent HTTP',
              'independentNetworkRoute': 'direct' if args.direct else 'system',
              'serviceUrl': root, 'serviceId': service['id'], 'asset': asset,
              'checks': {'actualBatches': 3, 'featureCount': 6, 'propertyValues': values,
                         'nullValues': nulls, 'coordinateValues': coordinates,
                         'allParsedPropertiesAndCoordinatesEqual': True,
                         'schemaFieldsRetained': len(fields), 'realEmptySelectionAccepted': True},
              'batchChecks': receipts, 'emptyAssetId': empty['id'],
              'fixturesUsed': False, 'nativeDesktopUiAccepted': False,
              'limits': ['Not a transactional snapshot of live service values.',
                         'JSON number spellings can differ while the decoded Double values are equal.',
                         'This is service-provided WGS84 GeoJSON, not the original geodatabase.']}
    (out / 'saved.geojson').write_text(json.dumps(geo, ensure_ascii=False), encoding='utf-8')
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report['checks']))


if __name__ == '__main__':
    main()
