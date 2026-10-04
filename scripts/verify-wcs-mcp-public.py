"""Acquire a fresh bounded public WCS subset through real write-enabled MCP.

Uses a new private store and actual native runtime. Metadata/grid interpretation
and all returned samples are checked independently; no existing file is adopted.
Healthy native work is retained if its observer fails. No user desktop is used.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from datetime import datetime, timezone
from urllib.request import build_opener, ProxyHandler, Request


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


mcp = module('wcs_mcp_public_transport', 'verify-mcp.py')
wcs = module('wcs_mcp_public_reference', 'verify-wcs-public.py')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--coverage', required=True)
    parser.add_argument('--bounds', required=True, type=lambda value: [float(x) for x in value.split(',')])
    parser.add_argument('--port', type=int, default=4612)
    parser.add_argument('--offline-port', type=int, default=4613)
    args = parser.parse_args()
    root, binary = args.root.resolve(), args.executable.resolve(strict=True)
    assert root.parent == Path('.verification').resolve() and root.name.startswith('wcs-mcp-public-')
    assert not root.exists(), 'Use a new store; do not relabel an existing acquisition'
    assert len(args.bounds) == 4 and args.bounds[0] < args.bounds[2] and args.bounds[1] < args.bounds[3]
    for port in (args.port, args.offline_port):
        assert 1 <= port <= 65535
        with socket.socket() as probe:
            assert probe.connect_ex(('127.0.0.1', port)) != 0, 'A requested verification port is already owned'
    store, evidence = root / 'store', root / 'evidence'
    store.mkdir(parents=True)
    evidence.mkdir()
    (store / 'proxy-settings.json').write_text(json.dumps({'mode': 'system'}), encoding='utf-8')
    report = {'schema': 'geod-wcs-mcp-public-verification/v1', 'status': 'running',
              'startedAt': datetime.now(timezone.utc).isoformat(), 'nativeBinarySha256': digest(binary.read_bytes()),
              'endpoint': args.endpoint, 'coverageId': args.coverage, 'requestedBounds': args.bounds,
              'freshStoreBeforeConnect': True, 'usedUserDesktop': False, 'syntheticData': False,
              'originalProviderArchiveClaimed': False, 'nativeWindowTested': False, 'automaticRetries': 0}
    report_file = evidence / 'report.json'

    def save():
        report_file.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    opener, base = build_opener(ProxyHandler({})), f'http://127.0.0.1:{args.port}'

    def api(route):
        with opener.open(base + route, timeout=6) as response:
            return json.load(response)

    def owned():
        health = api('/health')
        location = health['storageRoot']
        if os.name == 'nt' and location.startswith('\\\\?\\'):
            location = location[4:]
        assert Path(location).samefile(store), 'The loopback service belongs to a different store'

    server, client = None, None
    stdout, stderr = (root / 'runtime.stdout.log').open('wb'), (root / 'runtime.stderr.log').open('wb')
    try:
        server = subprocess.Popen([str(binary), 'serve', '--data-dir', str(store), '--port', str(args.port)],
                                  stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        report['ownerPid'] = server.pid
        save()
        for i in range(100):
            assert server.poll() is None, 'The private runtime exited before readiness'
            try:
                owned()
                break
            except OSError:
                if i == 99:
                    raise
                time.sleep(.1)
        assert api('/jobs') == [] and list((store / 'assets').glob('*.tif')) == []
        client = mcp.Client(binary, server=base, allow_write=True)
        tools = {tool['name']: tool for tool in client.request('tools/list')['result']['tools']}
        for name in ['geod_wcs_connect', 'geod_wcs_describe', 'geod_wcs_prepare', 'geod_wcs_project_save', 'geod_wcs_download']:
            assert tools[name]['annotations']['readOnlyHint'] is False
        summary = client.call('geod_wcs_connect', {'request': {'name': 'Actual public WCS via MCP', 'url': args.endpoint}})
        assert summary['coveragesTool']['tool'] == 'geod_wcs_coverages'
        assert summary['coveragesTool']['arguments']['id'] == summary['id']
        connection = {key: value for key, value in summary.items() if key not in ('coverageCount', 'coveragesTool')}
        catalog, pages, offset = [], [], 0
        while True:
            page = client.call('geod_wcs_coverages', {'id': summary['id'], 'offset': offset, 'limit': 20})
            assert page['connection'] == connection and page['total'] == summary['coverageCount']
            assert len(page['coverages']) <= 20
            catalog.extend(page['coverages'])
            pages.append(page)
            if page['nextOffset'] is None:
                break
            assert page['nextOffset'] == offset + len(page['coverages']) and page['nextOffset'] > offset
            offset = page['nextOffset']
        assert len(catalog) == summary['coverageCount'] and len({entry['id'] for entry in catalog}) == len(catalog)
        assert any(entry['id'] == args.coverage for entry in catalog)
        connection['coverages'] = catalog
        (evidence / 'mcp-connection-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
        (evidence / 'mcp-catalog-pages.json').write_text(json.dumps(pages, ensure_ascii=False, indent=2), encoding='utf-8')
        description = client.call('geod_wcs_describe', {'request': {'connectionId': connection['id'], 'coverageId': args.coverage}})
        plan = client.call('geod_wcs_prepare', {'request': {'descriptionId': description['id'], 'bounds': args.bounds}})
        assert description['coverageId'] == args.coverage and plan['description'] == description
        metadata, grid = wcs.retain_metadata(store, connection, description, plan, evidence)
        expected_grid = wcs.verify_plan(plan, grid, args.bounds, args.endpoint)
        project = client.call('geod_wcs_project_save', {'request': {'name': 'Actual public WCS via MCP',
                              'bounds': args.bounds, 'selections': [{'planId': plan['id']}]}})
        assert len(project['wcsItems']) == 1 and project['wcsItems'][0]['planId'] == plan['id']
        assert api('/jobs') == [], 'Preparing metadata must not silently download a coverage'
        queued = client.call('geod_wcs_download', {'request': {'projectId': project['id'], 'selections': [{'planId': plan['id']}]}})
        assert len(queued['jobs']) == 1
        ticket = queued['jobs'][0]
        assert ticket['poll']['tool'] == 'geod_job_status'
        job_id, last_print = ticket['jobId'], 0
        while True:
            job = client.call('geod_job_status', {'id': job_id})
            assert job['status'] not in ('failed', 'cancelled', 'interrupted'), job.get('error')
            if job['status'] == 'succeeded' and job['settled']:
                break
            if time.monotonic() - last_print >= 30:
                print(json.dumps({'stage': 'mcp-public-download', 'jobId': job_id, 'status': job['status'],
                                  'bytes': job['bytesDownloaded']}), flush=True)
                last_print = time.monotonic()
            time.sleep(1)
        assert job['wcsSource'] == {'planId': plan['id']} and job['href'] == plan['requestUrl']
        path = Path(job['outputPath'])
        assert path.samefile(store / 'assets' / (job_id + '.tif'))
        raw = path.read_bytes()
        assert len(raw) <= 1024 * 1024 and len(raw) == job['bytesDownloaded'] == job['totalBytes']
        assert digest(raw) == job['sha256']
        (evidence / 'native-subset.tif').write_bytes(raw)
        # A separate public request and independent decoder prove full content,
        # rather than trusting the native job's success or five sampled points.
        request = Request(plan['requestUrl'], headers={'Accept-Encoding': 'identity', 'User-Agent': 'GeoD-Global-WCS-Verification/1'})
        public_opener = build_opener()
        with public_opener.open(request, timeout=60) as response:
            independent_raw = response.read(1024 * 1024 + 1)
            assert response.status == 200 and response.geturl() == plan['requestUrl']
            assert response.headers.get('Content-Encoding', 'identity') == 'identity'
        assert len(independent_raw) <= 1024 * 1024 and independent_raw == raw
        independent_path = evidence / 'independent-subset.tif'
        independent_path.write_bytes(independent_raw)
        reference = wcs.qa.raster_reference(path)
        independent_reference = wcs.qa.raster_reference(independent_path)
        assert reference == independent_reference
        assert (reference['width'], reference['height']) == (expected_grid['width'], expected_grid['height'])
        assert reference['crs'] == grid['crs'] and reference['bandCount'] == len(grid['fields'])
        wcs.close_sequence(reference['transform'], expected_grid['transform'], 'Response grid differs from original declarations')
        wcs.close_sequence(reference['bounds'], expected_grid['nativeBounds'], 'Response bounds differ from the native grid')
        inspection = client.call('geod_wcs_inspect', {'id': job_id})
        assert inspection['previewOmitted'] and 'previewDataUrl' not in inspection
        wcs.qa.verify_inspection(inspection, reference, job)
        points = []
        for sample in reference['samples']:
            value = client.call('geod_wcs_pixel', {'id': job_id, 'column': sample['column'], 'row': sample['row']})
            wcs.qa.verify_pixel(value, sample, job)
            points.append(value)
        for name, value in [('connection', connection), ('description', description), ('plan', plan), ('project', project),
                            ('queue', queued), ('job', job), ('rasterio-reference', reference), ('mcp-pixels', points)]:
            (evidence / (name + '.json')).write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
        client.close()
        client = None
        assert not any(j['status'] in ('running', 'queued') for j in api('/jobs'))
        server.terminate()
        server.wait(timeout=10)
        server = None
        offline_file = evidence / 'offline-mcp-verification.json'
        offline = subprocess.run([str(Path(os.sys.executable)), '-X', 'utf8', str(Path(__file__).with_name('verify-wcs-mcp.py')),
                                  '--executable', str(binary), '--data-dir', str(store), '--project', project['id'],
                                  '--job', job_id, '--plan', plan['id'], '--report', str(offline_file), '--port', str(args.offline_port)],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8',
                                 creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        assert offline.returncode == 0, offline.stderr
        offline_report = json.loads(offline_file.read_text(encoding='utf-8'))
        assert offline_report['status'] == 'passed' and offline_report['registryBytesUnchanged']
        report.update(status='passed', jobId=job_id, projectId=project['id'], planId=plan['id'], metadata=metadata,
                      bytes=len(raw), sha256=job['sha256'], width=reference['width'], height=reference['height'],
                      crs=reference['crs'], bands=reference['bands'],
                      independentSamplesCompared=sum(band['samples'] for band in reference['bands']),
                      independentHttpPayloadIdentical=True, mcpPixelChecks=len(points), protocolVersion='2025-11-25',
                      offlineSessions=len(offline_report['sessions']), offlinePixelChecks=len(offline_report['independentPixelChecks']),
                      offlineBlockedProxyRequests=offline_report['blockedProxyAttempts'], declaredFields=grid['fields'],
                      catalogPages=len(pages), coveragesCompared=len(catalog),
                      linkedRightsMetadataFetched=False)
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
            if jobs is not None and not any(j['status'] in ('running', 'queued') for j in jobs):
                server.terminate()
                server.wait(timeout=10)
            else:
                report['ownerRetainedForActiveOrUnknownWork'] = server.pid
        stdout.close()
        stderr.close()
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save()
    print(json.dumps({key: report[key] for key in ['status', 'jobId', 'bytes', 'independentSamplesCompared', 'mcpPixelChecks', 'offlineSessions']}))


if __name__ == '__main__':
    main()
