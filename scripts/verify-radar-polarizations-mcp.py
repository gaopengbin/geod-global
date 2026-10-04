"""Read independently verified VH/HH/HV originals/results through offline MCP.

Full mode requires the private native service to be closed first. Partial mode
only reads independently accepted records through its active loopback API.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import URLError

spec = importlib.util.spec_from_file_location('geod_verify_mcp', Path(__file__).with_name('verify-mcp.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--port', type=int, default=4605)
    parser.add_argument('--partial', action='store_true')
    parser.add_argument('--completed-projects', action='store_true')
    parser.add_argument('--acquisition-originals', action='store_true')
    args = parser.parse_args()
    root = args.root.resolve()
    assert root.parent == Path('.verification').resolve() and root.name.startswith('radar-polarizations-')
    assert not (args.completed_projects and args.acquisition_originals)
    if args.completed_projects or args.acquisition_originals:
        assert args.partial, 'Ready project checks must preserve the active acquisition owner'
    native_file = 'early-projects-native-verification.json' if args.completed_projects else 'native-polarizations-verification.json'
    proof_file = ('acquisition-originals-independent-verification.json' if args.acquisition_originals else
                  'early-projects-independent-verification.json' if args.completed_projects else 'independent-polarizations-verification.json')
    proof_raw = (root / proof_file).read_bytes()
    independent = json.loads(proof_raw)
    if args.acquisition_originals:
        native_file = 'evidence-snapshots/' + independent['nativeReceiptSha256'] + '.json'
        assert independent['nativeReceiptSnapshot'] == native_file and independent['outputs'] == []
    native_raw = (root / native_file).read_bytes()
    native = json.loads(native_raw)
    if args.acquisition_originals or args.completed_projects or not args.partial:
        assert hashlib.sha256(native_raw).hexdigest() == independent['nativeReceiptSha256']
    assert args.partial or native['status'] == independent['status'] == 'passed'
    assert native['nativeBinarySha256'] == independent['nativeBinarySha256']
    binary = Path('target/debug/geod-runtime.exe').resolve()
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == native['nativeBinarySha256']
    cases = [{'case': 'original', 'key': entry['job']['assetKey'], **entry} for entry in native['originals']]
    cases.extend(native['outputs'])
    accepted = {entry['jobId']: entry['sha256'] for entry in independent['originals'] + independent['outputs']}
    cases = [entry for entry in cases if accepted.get(entry['job']['id']) == entry['job']['sha256']]
    if args.completed_projects:
        cases = [entry for entry in cases if entry['job']['kind'] == 'raster_mosaic']
    assert cases, 'No independently accepted real files are available'
    if not args.partial:
        assert len(cases) == 15
    report = {'schema': 'geod-radar-polarizations-mcp/v1', 'nativeBinarySha256': native['nativeBinarySha256'],
              'nativeReceiptSha256': hashlib.sha256(native_raw).hexdigest(),
              'independentReceiptSha256': hashlib.sha256(proof_raw).hexdigest(),
              'checkedAt': datetime.now(timezone.utc).isoformat(), 'readOnly': True, 'usedUserDesktop': False,
              'disconnectedAdapterRejected': False, 'cases': []}
    base = f'http://127.0.0.1:{args.port}'
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def owned_health():
        with opener.open(base + '/health', timeout=2) as response:
            health = json.load(response)
        value = health['storageRoot']
        if os.name == 'nt' and value.startswith('\\\\?\\'):
            value = value[4:]
        assert Path(value).resolve() == root

    try:
        owned_health()
        assert args.partial, 'The completed acquisition service must close before direct MCP access'
    except URLError:
        assert not args.partial

    def inspect(client, mode):
        def accepted_call(name, arguments):
            for attempt in range(120):
                try:
                    return client.call(name, arguments)
                except AssertionError as error:
                    if 'The raster worker is busy' not in str(error) or attempt == 119:
                        raise
                    time.sleep(.5)

        tools = client.request('tools/list')['result']['tools']
        for name in ['geod_raster_inspect', 'geod_raster_pixel']:
            tool = next(tool for tool in tools if tool['name'] == name)
            assert tool['annotations']['readOnlyHint']
        for entry in native['cases']:
            assert accepted_call('geod_project_get', {'id': entry['project']['id']}) == entry['project']
        for entry in cases:
            job, expected = entry['job'], entry['metadata']
            metadata = accepted_call('geod_raster_inspect', {'id': job['id']})
            assert metadata['previewOmitted'] and 'previewDataUrl' not in metadata
            assert metadata['sha256'] == job['sha256']
            for field in ['width', 'height', 'dataType', 'crs', 'bounds', 'pixelSize', 'nodata', 'radar', 'classes']:
                assert metadata[field] == expected[field]
            assert metadata['radar']['polarization'] == entry['key'].upper()
            for pixel in entry['pixels']:
                x, y = pixel['coordinate']
                assert accepted_call('geod_raster_pixel', {'id': job['id'], 'x': x, 'y': y}) == pixel
            report['cases'].append({'mode': mode, 'case': entry['case'], 'key': entry['key'],
                                    'jobId': job['id'], 'itemId': job['itemId'], 'sha256': job['sha256'], 'pixelsCompared': len(entry['pixels']),
                                    'previewOmitted': True, 'radarProfileAndRawFloat32Retained': True})
            print(json.dumps({'mode': mode, 'case': entry['case'], 'key': entry['key']}), flush=True)

    if not args.partial:
        client = module.Client(str(binary), data_dir=root)
        try:
            inspect(client, 'direct read-only; rejected upstream proxy')
        finally:
            client.close()
    runtime = None if args.partial else subprocess.Popen([str(binary), 'serve', '--data-dir', str(root), '--port', str(args.port)],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
    client = None
    try:
        for i in range(100):
            try:
                owned_health()
                break
            except Exception:
                assert runtime is not None and runtime.poll() is None
                if i == 99:
                    raise
                time.sleep(.1)
        client = module.Client(str(binary), server=base)
        inspect(client, 'loopback read-only; active acquisition store' if args.partial else 'loopback read-only; rejected upstream proxy')
        if not args.partial:
            runtime.terminate()
            runtime.wait(timeout=10)
            failed = client.request('tools/call', {'name': 'geod_raster_inspect',
                                             'arguments': {'id': native['outputs'][0]['job']['id']}})
            assert failed['result']['isError']
            report['disconnectedAdapterRejected'] = True
    finally:
        if client:
            client.close()
        if runtime is not None and runtime.poll() is None:
            runtime.terminate()
            runtime.wait(timeout=10)
    report.update(status='partial' if args.partial else 'passed', pixelsCompared=sum(entry['pixelsCompared'] for entry in report['cases']))
    if args.partial:
        report['scope'] = 'Read-only loopback access to completed, independently accepted actual originals/results; remaining downloads and full offline/disconnection checks are pending.'
    if args.completed_projects:
        report['scope'] = 'Read-only loopback checks of independently accepted ready project outputs; original-file MCP proof is kept separately and the acquisition owner is untouched.'
    if args.acquisition_originals:
        report['scope'] = 'Read-only loopback checks of independently accepted acquisition originals bound to an immutable receipt; no direct, disconnection or full-matrix offline acceptance.'
    output_file = ('acquisition-originals-mcp-verification.json' if args.acquisition_originals else
                   'early-projects-mcp-verification.json' if args.completed_projects else 'mcp-polarizations-verification.json')
    output = root / output_file
    temporary = output.with_suffix(output.suffix + '.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
    temporary.replace(output)
    print(json.dumps({key: report[key] for key in ['status', 'pixelsCompared', 'disconnectedAdapterRejected']}))


if __name__ == '__main__':
    main()
