"""Verify explicitly selected public STAC paths through real MCP and offline restart.

Cases are supplied in JSON; acquisition uses a fresh private store and the native
queue. Independent original HTTP bytes and GDAL decode are checked separately.
No mock upstream, previous-file adoption, user desktop or model call is involved.
"""
import argparse
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import threading
import time
from urllib.request import build_opener, ProxyHandler, Request


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


mcp = module('stac_mcp_transport', 'verify-mcp.py')
qa = module('stac_mcp_reference', 'verify-stac-public.py')
sha = qa.sha
save_json = qa.save_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--cases', required=True, type=Path)
    args = parser.parse_args()
    root, binary = args.root.resolve(), args.executable.resolve(strict=True)
    assert root.parent == Path('.verification').resolve() and root.name.startswith('stac-mcp-public-')
    assert not root.exists(), 'Use a new evidence/store directory'
    cases = json.loads(args.cases.read_text(encoding='utf-8'))
    assert 1 <= len(cases) <= 4 and len({c['name'] for c in cases}) == len(cases)
    for case in cases:
        assert case['kind'] in ('api', 'catalog', 'item', 'raster')
        assert case['endpoint'].startswith('https://')
        assert len(case['bounds']) == 4
        if case.get('download'):
            assert 0 < case['expectedBytes'] <= 32 * 1024 * 1024
    evidence, store = root / 'evidence', root / 'store'
    evidence.mkdir(parents=True)
    store.mkdir()
    save_json(store / 'proxy-settings.json', {'mode': 'system'})
    report = {'schema': 'geod-stac-mcp-public/v1', 'status': 'running',
              'startedAt': datetime.now(timezone.utc).isoformat(), 'binarySha256': sha(binary.read_bytes()),
              'freshAcquisition': True, 'syntheticSources': False, 'usedUserDesktop': False,
              'nativeWindowTested': False, 'actualModelCall': False, 'cases': [], 'offlineSessions': [],
              'automaticRetries': 0, 'cogConformanceClaimed': False, 'scientificCalibrationClaimed': False}
    report_file = evidence / 'acceptance.json'
    opener = build_opener(ProxyHandler({}), qa.NoRedirect())
    public_opener = build_opener(qa.NoRedirect())
    with socket.socket() as port:
        port.bind(('127.0.0.1', 0))
        origin = f'http://127.0.0.1:{port.getsockname()[1]}'
    server, client, trap = None, None, None
    stdout, stderr = (root / 'runtime.stdout.log').open('wb'), (root / 'runtime.stderr.log').open('wb')

    def api(route):
        with opener.open(Request(origin + route, headers={'X-GeoD-Client': 'geod-global'}), timeout=8) as response:
            return json.load(response)

    def owned():
        health = api('/health')
        path = health['storageRoot']
        if path.startswith('\\\\?\\'):
            path = path[4:]
        assert Path(path).samefile(store), 'Verification runtime owner mismatch'

    try:
        server = subprocess.Popen([str(binary), 'serve', '--data-dir', str(store), '--port', origin.rsplit(':', 1)[1]],
                                  stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        report['ownerPid'] = server.pid
        save_json(report_file, report)
        for i in range(100):
            assert server.poll() is None, 'Owned runtime exited before readiness'
            try:
                owned()
                break
            except OSError:
                if i == 99:
                    raise
                time.sleep(.1)
        assert api('/jobs') == [] and api('/stac/connections') == []
        client = mcp.Client(binary, server=origin, allow_write=True)
        tools = {t['name']: t for t in client.request('tools/list')['result']['tools']}
        reads = {'geod_stac_connections', 'geod_stac_catalog', 'geod_stac_snapshot', 'geod_stac_assets', 'geod_stac_inspect', 'geod_stac_pixel'}
        writes = {'geod_stac_connect', 'geod_stac_search', 'geod_stac_project_save', 'geod_stac_download', 'geod_stac_forget'}
        assert reads | writes <= tools.keys()
        assert tools['geod_stac_search']['annotations']['readOnlyHint'] is False
        for index, case in enumerate(cases):
            out = evidence / f'case-{index+1}'
            out.mkdir()
            print(json.dumps({'stage': 'connect', 'name': case['name']}), flush=True)
            connection = client.call('geod_stac_connect', {'request': {'name': case['name'], 'kind': case['kind'], 'url': case['endpoint']}})
            catalog, offset = [], 0
            while True:
                page = client.call('geod_stac_catalog', {'id': connection['id'], 'offset': offset, 'limit': 2})
                catalog += page['entries']
                if page['nextOffset'] is None:
                    break
                assert page['nextOffset'] > offset
                offset = page['nextOffset']
            actual_connection = next(c for c in api('/stac/connections') if c['id'] == connection['id'])
            expected_entries = {'api': actual_connection['collections'], 'catalog': actual_connection.get('catalogNodes', []),
                                'item': [{'snapshotId': s} for s in actual_connection['snapshotIds']],
                                'raster': [{'snapshotId': s} for s in actual_connection['snapshotIds']]}[case['kind']]
            assert catalog == expected_entries
            save_json(out / 'connection.json', actual_connection)
            save_json(out / 'mcp-catalog.json', catalog)
            snapshots, pages, cursor = [], [], None
            if case['kind'] in ('api', 'catalog'):
                collection = case.get('collection')
                if case['kind'] == 'catalog':
                    branches = [n for n in catalog if n['id'] == case['directory']]
                    assert len(branches) == 1
                    collection = branches[0]['key']
                request = {'connectionId': connection['id'], 'collectionId': collection,
                           'bounds': case['bounds'], 'datetime': case.get('datetime'), 'limit': case.get('pageSize', 4)}
                seen = set()
                for _ in range(10):
                    page = client.call('geod_stac_search', {'request': {**request, 'cursor': cursor}})
                    pages.append(page)
                    for summary in page['items']:
                        snapshot = client.call('geod_stac_snapshot', {'id': summary['id']})
                        assert summary['itemId'] == snapshot['itemId'] and summary['collectionId'] == snapshot['collectionId']
                        assert summary['documentSha256'] == snapshot['documentSha256'] and summary['assetCount'] == len(snapshot['assets'])
                        identity = (snapshot.get('collectionId'), snapshot['itemId'])
                        assert identity not in seen
                        seen.add(identity)
                        snapshots.append(snapshot)
                    if page['complete']:
                        assert page['nextCursor'] is None and not page['limitReached']
                        break
                    assert page['nextCursor'] and not page['limitReached'], 'Public case exceeded the intended bounded query'
                    cursor = page['nextCursor']
                else:
                    raise AssertionError('Public case exceeded ten query pages')
            else:
                snapshots = [client.call('geod_stac_snapshot', {'id': s['snapshotId']}) for s in catalog]
            assert snapshots
            for snapshot in snapshots:
                qa.retain_metadata(store, snapshot, out)
            save_json(out / 'snapshots.json', snapshots)
            save_json(out / 'search-pages.json', pages)
            selected = [s for s in snapshots if not case.get('itemId') or s['itemId'] == case['itemId']]
            assert selected
            snapshot = selected[0]
            assets, offset = [], 0
            while True:
                page = client.call('geod_stac_assets', {'id': snapshot['id'], 'offset': offset, 'limit': 3})
                assets += page['assets']
                if page['nextOffset'] is None:
                    break
                assert page['nextOffset'] > offset
                offset = page['nextOffset']
            assert assets == snapshot['assets'], 'MCP asset pages omitted or changed declarations'
            asset = next(a for a in assets if a['key'] == case['assetKey'])
            assert asset['eligible']
            pin = {'snapshotId': snapshot['id'], 'assetKey': asset['key']}
            project = client.call('geod_stac_project_save', {'request': {'name': case['name'], 'bounds': case['bounds'], 'selections': [pin]}})
            appended = client.call('geod_stac_project_save', {'request': {'projectId': project['id'], 'bounds': case['bounds'], 'selections': [pin]}})
            assert appended == project and project['stacItems'][0]['snapshotId'] == snapshot['id']
            save_json(out / 'project.json', project)
            receipt = {'name': case['name'], 'kind': case['kind'], 'connectionId': connection['id'], 'entries': len(catalog),
                       'pages': len(pages), 'items': len(snapshots), 'projectId': project['id'], 'snapshotId': snapshot['id'],
                       'pin': pin, 'metadataOnly': not case.get('download'), 'sourceLicense': snapshot['provenance'].get('collection', {}).get('license') if snapshot['provenance'].get('collection') else None}
            report['cases'].append(receipt)
            if case.get('download'):
                with public_opener.open(Request(asset['href'], method='HEAD'), timeout=30) as response:
                    assert response.status == 200 and response.geturl() == asset['href']
                    assert int(response.headers['Content-Length']) == case['expectedBytes']
                    save_json(out / 'independent-head.json', dict(response.headers))
                queued = client.call('geod_stac_download', {'request': {'projectId': project['id'], 'selections': [pin]}})
                job_id = queued['jobs'][0]['jobId']
                deadline, last = time.monotonic() + 300, 0
                while True:
                    job = client.call('geod_job_status', {'id': job_id})
                    assert job['status'] not in ('failed', 'interrupted', 'cancelled'), job.get('error')
                    if job['status'] == 'succeeded' and job['settled']:
                        break
                    assert time.monotonic() < deadline, 'Owned transfer still active; do not duplicate it'
                    if time.monotonic() - last > 20:
                        print(json.dumps({'stage': 'download', 'jobId': job_id, 'bytes': job['bytesDownloaded']}), flush=True)
                        last = time.monotonic()
                    time.sleep(.5)
                assert job['stacSource'] == pin and job['href'] == asset['href']
                path = Path(job['outputPath'])
                assert path.samefile(store / 'assets' / f'{job_id}.tif')
                raw = path.read_bytes()
                assert len(raw) == case['expectedBytes'] == job['totalBytes'] == job['bytesDownloaded'] and sha(raw) == job['sha256']
                with public_opener.open(Request(asset['href'], headers={'Accept-Encoding': 'identity'}), timeout=60) as response:
                    independent = response.read(case['expectedBytes'] + 1)
                    assert response.status == 200 and response.geturl() == asset['href']
                assert independent == raw
                (out / 'independent-original.tif').write_bytes(independent)
                reference = qa.raster_reference(path)
                assert reference == qa.raster_reference(out / 'independent-original.tif')
                inspection = client.call('geod_stac_inspect', {'id': job_id})
                assert inspection['previewOmitted'] and 'previewDataUrl' not in inspection
                qa.verify_inspection(inspection, reference, job)
                pixels = []
                for sample in reference['samples']:
                    value = client.call('geod_stac_pixel', {'id': job_id, 'column': sample['column'], 'row': sample['row']})
                    qa.verify_pixel(value, sample, job)
                    pixels.append(value)
                reuse = client.call('geod_stac_download', {'request': {'projectId': project['id']}})
                assert reuse['jobs'][0]['jobId'] == job_id and len(api('/jobs')) == sum(c.get('download', False) for c in cases[:index+1])
                receipt.update(jobId=job_id, bytes=len(raw), sha256=sha(raw), fullDecodedSamples=sum(b['samples'] for b in reference['bands']), pixelChecks=len(pixels), identicalIndependentHttp=True)
                for name, value in [('job', job), ('reference', reference), ('inspection', inspection), ('pixels', pixels)]:
                    save_json(out / (name + '.json'), value)
            save_json(report_file, report)
        assert all(j['status'] == 'succeeded' for j in api('/jobs'))
        # Forget only discovery, then prove pinned metadata and files remain.
        client.call('geod_stac_forget', {'id': report['cases'][0]['connectionId']})
        assert client.call('geod_stac_snapshot', {'id': report['cases'][0]['snapshotId']})['id'] == report['cases'][0]['snapshotId']
        client.close()
        client = None
        server.terminate()
        server.wait(timeout=10)
        server = None
        before = {p.name: sha(p.read_bytes()) for p in store.iterdir() if p.suffix == '.json'}
        attempts = []

        class DenyProxy(BaseHTTPRequestHandler):
            def deny(self):
                attempts.append({'method': self.command, 'target': self.path})
                self.send_error(502, 'Offline verification')
            do_CONNECT = deny
            do_GET = deny
            do_POST = deny
            def log_message(self, *_):
                pass

        trap = ThreadingHTTPServer(('127.0.0.1', 0), DenyProxy)
        threading.Thread(target=trap.serve_forever, daemon=True).start()
        proxy_path = store / 'proxy-settings.json'
        proxy_before = proxy_path.read_bytes()
        save_json(proxy_path, {'mode': 'custom', 'url': f'http://127.0.0.1:{trap.server_port}'})
        try:
            for iteration in range(2):
                client = mcp.Client(binary, data_dir=store)
                names = {t['name'] for t in client.request('tools/list')['result']['tools']}
                assert reads <= names and writes.isdisjoint(names)
                for name in writes:
                    response = client.request('tools/call', {'name': name, 'arguments': {}})
                    assert response['error']['code'] == -32602
                checked = 0
                for index, receipt in enumerate(report['cases']):
                    out = evidence / f'case-{index+1}'
                    restored = client.call('geod_stac_snapshot', {'id': receipt['snapshotId']})
                    assert restored == next(s for s in qa.read_json(out / 'snapshots.json') if s['id'] == receipt['snapshotId'])
                    assert client.call('geod_project_get', {'id': receipt['projectId']}) == qa.read_json(out / 'project.json')
                    if receipt.get('jobId'):
                        job = client.call('geod_job_status', {'id': receipt['jobId']})
                        native = client.call('geod_stac_inspect', {'id': receipt['jobId']})
                        reference = qa.read_json(out / 'reference.json')
                        qa.verify_inspection(native, reference, job)
                        for sample in reference['samples']:
                            pixel = client.call('geod_stac_pixel', {'id': receipt['jobId'], 'column': sample['column'], 'row': sample['row']})
                            qa.verify_pixel(pixel, sample, job)
                            checked += 1
                client.close()
                client = None
                report['offlineSessions'].append({'mode': 'direct', 'iteration': iteration+1, 'pixelChecks': checked})
            assert attempts == [], 'Offline reads made external requests'
        finally:
            proxy_path.write_bytes(proxy_before)
        assert before == {p.name: sha(p.read_bytes()) for p in store.iterdir() if p.suffix == '.json'}, 'Offline reads altered native records'
        report.update(status='passed', offlineProxyAttempts=attempts, offlineRecordsUnchanged=True,
                      originalTransfers=sum(c.get('download', False) for c in cases), completeStacAgentWorkflow=False)
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        if client is not None:
            client.close()
        if server is not None and server.poll() is None:
            try:
                owned()
                jobs = api('/jobs')
            except Exception:
                jobs = None
            if jobs is not None and not any(j['status'] in ('queued', 'running') for j in jobs):
                server.terminate()
                server.wait(timeout=10)
            else:
                report['ownerRetainedForActiveOrUnknownWork'] = server.pid
        if trap:
            trap.shutdown()
            trap.server_close()
        stdout.close()
        stderr.close()
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(report_file, report)
    print(json.dumps({'status': report['status'], 'cases': len(report['cases']), 'transfers': report['originalTransfers'], 'offlineSessions': len(report['offlineSessions'])}), flush=True)


if __name__ == '__main__':
    main()
