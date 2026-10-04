"""Read real WMTS REST evidence after process restart with a rejecting proxy."""
import argparse, base64, hashlib, json, pathlib, urllib.request
from datetime import datetime, timezone


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--server', required=True)
    p.add_argument('--public-output', required=True)
    p.add_argument('--ui-output', required=True)
    p.add_argument('--data-dir', required=True)
    args = p.parse_args()
    public, ui, store = map(pathlib.Path, [args.public_output, args.ui_output, args.data_dir])
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def request(path, data=None, binary=False):
        body = json.dumps(data).encode() if data is not None else None
        headers = {'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global'}
        with opener.open(urllib.request.Request(args.server + path, data=body, headers=headers), timeout=30) as r:
            raw = r.read()
        return raw if binary else json.loads(raw)

    report = json.loads((public / 'report.json').read_text('utf-8'))
    cases = [(r['asset'], public / (r['asset']['crs'].split(':')[1] + '-' + r['asset']['source']['layerName']) / 'map.zip') for r in report['receipts']]
    for width in [1440, 1024]:
        cases.append((json.loads((ui / f'{width}-asset.json').read_text('utf-8')), ui / f'{width}-export.zip'))
    assets = request('/map-images')
    assert {a['id'] for a in assets} == {a['id'] for a, _ in cases}
    before_services = json.loads((store / 'map-services.json').read_text('utf-8'))
    services = request('/map-services')
    assert {s['id']: s for s in services} == before_services
    original = request('/proxy')
    request('/proxy', {'mode': 'custom', 'url': 'http://127.0.0.1:9'})
    checks = []
    try:
        assert request('/proxy')['mode'] == 'custom'
        for a, package in cases:
            read = request('/map-images/' + a['id'])
            assert read['asset'] == a
            image = base64.b64decode(read['imageUrl'].split(',', 1)[1])
            assert len(image) == a['bytes'] and hashlib.sha256(image).hexdigest() == a['sha256']
            assert request('/map-images/' + a['id'] + '/export', binary=True) == package.read_bytes()
            checks.append({'assetId': a['id'], 'sameMetadata': True, 'exactImageBytes': True, 'exactExportBytes': True})
    finally:
        request('/proxy', original)
    result = {'passed': True, 'checkedAt': datetime.now(timezone.utc).isoformat(),
              'mode': 'Actual native process restart; rejecting outbound proxy; local inspect/export only',
              'servicesRestored': len(services), 'images': checks, 'remoteOperationsInvoked': [],
              'networkNeededForRead': False, 'usedUserDesktop': False}
    (public / 'restart.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
