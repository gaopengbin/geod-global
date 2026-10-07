"""Recheck a settled real public MCP acquisition in direct and loopback modes.

Requires a passed acquisition receipt, an unowned verification store and the same
native binary. This is offline reuse evidence, never a fresh public transfer.
"""
import argparse
from datetime import datetime, timezone
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


mcp = module('stac_offline_transport', 'verify-mcp.py')
qa = module('stac_offline_reference', 'verify-stac-public.py')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    root, binary, output = args.root.resolve(strict=True), args.executable.resolve(strict=True), args.output.resolve()
    store, evidence = root / 'store', root / 'evidence'
    assert '.verification' in store.parts and not output.exists()
    acquired = qa.read_json(evidence / 'acceptance.json')
    assert acquired['status'] == 'passed' and acquired['binarySha256'] == qa.sha(binary.read_bytes())
    assert all(j['status'] not in ('queued', 'running') for j in qa.read_json(store / 'jobs.json').values())
    # Establish exclusive ownership before changing only this QA store's proxy.
    client = mcp.Client(binary, data_dir=store)
    client.call('geod_health')
    client.close()
    before = {p.name: qa.sha(p.read_bytes()) for p in store.glob('*.json')}
    proxy_file = store / 'proxy-settings.json'
    proxy_before = proxy_file.read_bytes()
    attempts = []

    class DenyProxy(BaseHTTPRequestHandler):
        def deny(self):
            attempts.append({'method': self.command, 'target': self.path})
            self.send_error(502, 'Offline STAC acceptance')
        do_CONNECT = deny
        do_GET = deny
        do_POST = deny
        def log_message(self, *_):
            pass

    trap = ThreadingHTTPServer(('127.0.0.1', 0), DenyProxy)
    threading.Thread(target=trap.serve_forever, daemon=True).start()
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        origin = f'http://127.0.0.1:{probe.getsockname()[1]}'
    opener = build_opener(ProxyHandler({}))
    report = {'schema': 'geod-stac-mcp-offline/v1', 'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(),
              'binarySha256': qa.sha(binary.read_bytes()), 'acquisitionReceiptSha256': qa.sha((evidence / 'acceptance.json').read_bytes()),
              'successfulPublicTransfers': 0, 'actualModelCall': False, 'usedUserDesktop': False, 'sessions': []}
    server, client = None, None
    stdout, stderr = output.with_suffix('.stdout.log').open('wb'), output.with_suffix('.stderr.log').open('wb')
    try:
        qa.save_json(proxy_file, {'mode': 'custom', 'url': f'http://127.0.0.1:{trap.server_port}'})
        for mode in ('direct', 'loopback'):
            if mode == 'loopback':
                server = subprocess.Popen([str(binary), 'serve', '--data-dir', str(store), '--port', origin.rsplit(':', 1)[1]],
                                          stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
                for i in range(100):
                    assert server.poll() is None
                    try:
                        with opener.open(origin + '/health', timeout=3) as response:
                            health = json.load(response)
                        location = health['storageRoot'].removeprefix('\\\\?\\')
                        assert Path(location).samefile(store)
                        break
                    except OSError:
                        if i == 99:
                            raise
                        time.sleep(.1)
            client = mcp.Client(binary, server=origin if mode == 'loopback' else None, data_dir=store if mode == 'direct' else None)
            reads = {'geod_stac_connections', 'geod_stac_catalog', 'geod_stac_snapshot', 'geod_stac_assets', 'geod_stac_inspect', 'geod_stac_pixel'}
            writes = {'geod_stac_connect', 'geod_stac_search', 'geod_stac_project_save', 'geod_stac_download', 'geod_stac_forget'}
            names = {t['name'] for t in client.request('tools/list')['result']['tools']}
            assert reads <= names and writes.isdisjoint(names)
            for name in writes:
                assert client.request('tools/call', {'name': name, 'arguments': {}})['error']['code'] == -32602
            pixels = 0
            for index, case in enumerate(acquired['cases']):
                out = evidence / f'case-{index+1}'
                snapshot = client.call('geod_stac_snapshot', {'id': case['snapshotId']})
                assert snapshot == next(s for s in qa.read_json(out / 'snapshots.json') if s['id'] == case['snapshotId'])
                assert client.call('geod_project_get', {'id': case['projectId']}) == qa.read_json(out / 'project.json')
                if case.get('jobId'):
                    job = client.call('geod_job_status', {'id': case['jobId']})
                    assert job['status'] == 'succeeded' and job['settled']
                    assert qa.sha(Path(job['outputPath']).read_bytes()) == case['sha256']
                    reference = qa.read_json(out / 'reference.json')
                    qa.verify_inspection(client.call('geod_stac_inspect', {'id': job['id']}), reference, job)
                    for sample in reference['samples']:
                        qa.verify_pixel(client.call('geod_stac_pixel', {'id': job['id'], 'column': sample['column'], 'row': sample['row']}), sample, job)
                        pixels += 1
            client.close()
            client = None
            report['sessions'].append({'mode': mode, 'snapshots': len(acquired['cases']), 'pixelChecks': pixels})
            if server:
                server.terminate()
                server.wait(timeout=10)
                server = None
        assert attempts == []
        proxy_file.write_bytes(proxy_before)
        assert before == {p.name: qa.sha(p.read_bytes()) for p in store.glob('*.json')}
        report.update(status='passed', blockedProxyAttempts=attempts, registryBytesUnchanged=True)
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        if client:
            client.close()
        if server and server.poll() is None:
            server.terminate()
            server.wait(timeout=10)
        proxy_file.write_bytes(proxy_before)
        trap.shutdown()
        trap.server_close()
        stdout.close()
        stderr.close()
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        qa.save_json(output, report)
    print(json.dumps({'status': report['status'], 'sessions': report['sessions']}), flush=True)


if __name__ == '__main__':
    main()
