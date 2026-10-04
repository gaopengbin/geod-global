"""Read independently accepted NAIP originals/results through local MCP.

Partial mode uses only the already running loopback service. Full mode requires
the acquisition driver to have exited, and tests direct plus offline loopback
access without starting a user window or requesting account authorization.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import URLError
from urllib.request import ProxyHandler, build_opener

spec = importlib.util.spec_from_file_location('geod_verify_mcp', Path(__file__).with_name('verify-mcp.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--port', type=int, default=4608)
    parser.add_argument('--partial', action='store_true')
    parser.add_argument('--snapshot-root', type=Path, help='Completed local subset copied by verify-naip-resolution-cache.mjs')
    args = parser.parse_args()
    root = args.root.resolve()
    assert root.parent == Path('.verification').resolve() and root.name.startswith('naip-resolutions-')
    native = json.loads((root / 'native-resolution-verification.json').read_text(encoding='utf8'))
    independent = json.loads((root / 'independent-resolution-verification.json').read_text(encoding='utf8'))
    assert not (args.partial and args.snapshot_root)
    snapshot_proof = None
    data_root = root
    if args.snapshot_root:
        data_root = args.snapshot_root.resolve()
        assert data_root != root and data_root.parent == root.parent and data_root.name.startswith('naip-resolutions-')
        snapshot_proof = json.loads((root / 'subset-cache-verification.json').read_text(encoding='utf8'))
        assert snapshot_proof['status'] == 'passed' and snapshot_proof['group'] == '1m'
        assert snapshot_proof['nativeBinarySha256'] == native['nativeBinarySha256']
    assert args.partial or snapshot_proof or native['status'] == independent['status'] == 'passed'
    assert native['nativeBinarySha256'] == independent['nativeBinarySha256']
    binary = data_root / f'runtime-{native["nativeBinarySha256"][:16]}.exe'
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == native['nativeBinarySha256']
    accepted = {entry['jobId']: entry['sha256'] for entry in independent['originals'] + independent['outputs']}
    cases = [entry for entry in native['originals'] + native['outputs'] if accepted.get(entry['job']['id']) == entry['job']['sha256']]
    if snapshot_proof:
        subset = {entry['jobId']: entry['sha256'] for entry in snapshot_proof['entries']}
        cases = [entry for entry in cases if subset.get(entry['job']['id']) == entry['job']['sha256']]
        assert len(cases) == 5 and all(entry['group'] == '1m' for entry in cases)
    assert cases, 'No independently accepted NAIP files are available'
    if not args.partial and not snapshot_proof:
        assert len(cases) == 8
    report = {'schema': 'geod-naip-resolution-mcp/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
              'nativeBinarySha256': native['nativeBinarySha256'], 'readOnly': True, 'usedUserDesktop': False, 'cases': []}
    base = f'http://127.0.0.1:{args.port}'
    opener = build_opener(ProxyHandler({}))

    def get(route):
        with opener.open(base + route, timeout=120) as response:
            return json.load(response)

    def owned_health():
        health = get('/health')
        value = health['storageRoot']
        if os.name == 'nt' and value.startswith('\\\\?\\'):
            value = value[4:]
        assert Path(value).resolve() == data_root
        return health

    try:
        owned_health()
        assert args.partial, 'The completed acquisition service must be closed before direct MCP access'
    except URLError:
        assert not args.partial

    def inspect(client, mode):
        def accepted_call(name, arguments):
            # Pixel/preview work shares a bounded worker with the renderer.
            # Retry only its explicit busy response, never scientific errors.
            for attempt in range(120):
                try:
                    return client.call(name, arguments)
                except AssertionError as error:
                    if 'The raster worker is busy' not in str(error) or attempt == 119:
                        raise
                    time.sleep(.5)

        tools = client.request('tools/list')['result']['tools']
        for name in ['geod_raster_inspect', 'geod_raster_pixel']:
            assert next(tool for tool in tools if tool['name'] == name)['annotations']['readOnlyHint']
        for entry in native['cases']:
            if snapshot_proof and entry['group'] != snapshot_proof['group']:
                continue
            assert client.call('geod_project_get', {'id': entry['project']['id']}) == entry['project']
        for entry in cases:
            job, expected = entry['job'], entry['metadata']
            metadata = accepted_call('geod_raster_inspect', {'id': job['id']})
            assert metadata['previewOmitted'] and 'previewDataUrl' not in metadata
            assert metadata['sha256'] == job['sha256']
            for field in ['width', 'height', 'bandCount', 'dataType', 'crs', 'bounds', 'pixelSize', 'nodata', 'aerial', 'classes']:
                assert metadata[field] == expected[field]
            assert metadata['aerial']['bands'] == ['red', 'green', 'blue', 'nir']
            assert metadata['aerial']['displayBands'] == [1, 2, 3]
            for pixel in entry['pixels']:
                x, y = pixel['coordinate']
                assert accepted_call('geod_raster_pixel', {'id': job['id'], 'x': x, 'y': y}) == pixel
            report['cases'].append({'mode': mode, 'group': entry['group'], 'case': entry.get('case', 'original'), 'jobId': job['id'],
                                    'sha256': job['sha256'], 'pixelsCompared': len(entry['pixels']), 'previewOmitted': True,
                                    'fourOriginalChannelsRetained': True, 'aerialProfileAndCoverageRetained': True,
                                    'sourceExtraSample': metadata['aerial'].get('sourceExtraSample', 0)})
            print(json.dumps({'mode': mode, 'group': entry['group'], 'case': entry.get('case', 'original')}), flush=True)

    if not args.partial:
        client = module.Client(str(binary), data_dir=data_root)
        try:
            inspect(client, 'direct read-only; rejected upstream proxy')
        finally:
            client.close()
    runtime = None
    client = None
    try:
        if not args.partial:
            runtime = subprocess.Popen([str(binary), 'serve', '--data-dir', str(data_root), '--port', str(args.port)],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                       creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            for attempt in range(100):
                try:
                    owned_health()
                    break
                except URLError:
                    assert runtime.poll() is None
                    if attempt == 99:
                        raise
                    time.sleep(.1)
            assert get('/proxy') == {'mode': 'custom', 'url': 'http://127.0.0.1:9'}
        client = module.Client(str(binary), server=base)
        inspect(client, 'loopback read-only; existing acquisition service' if args.partial else 'loopback read-only; rejected upstream proxy')
        if runtime is not None:
            runtime.terminate()
            runtime.wait(timeout=10)
            failed = client.request('tools/call', {'name': 'geod_raster_inspect', 'arguments': {'id': cases[0]['job']['id']}})
            assert failed['result']['isError']
            report['disconnectedAdapterRejected'] = True
    finally:
        if client is not None:
            client.close()
        if runtime is not None and runtime.poll() is None:
            runtime.terminate()
            runtime.wait(timeout=10)
    report.update(status='partial' if args.partial else 'passed', pixelsCompared=sum(entry['pixelsCompared'] for entry in report['cases']))
    if snapshot_proof:
        report.update(scope='Actual 1 m subset in an explicit offline local snapshot; no new downloads or 0.3 m acceptance claimed.', group='1m', snapshotFiles=5)
    destination = 'mcp-resolution-snapshot-verification.json' if snapshot_proof else 'mcp-resolution-verification.json'
    (root / destination).write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
    print(json.dumps({'status': report['status'], 'files': len(cases), 'pixelsCompared': report['pixelsCompared']}))


if __name__ == '__main__':
    main()
