"""Real public static STAC discovery/search acceptance, in a fresh owned store.

Metadata and project acceptance only: no original-file download is claimed.
Native HTTP requests perform discovery; independent checks use exact archived
bytes, directory links, raw identities and inclusive bbox/time intersection.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import socket
import subprocess
import time
import urllib.parse
spec = importlib.util.spec_from_file_location('static_stac_qa_helpers', Path(__file__).with_name('verify-stac-public.py'))
helpers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helpers)
Runtime, require, retain_metadata, save_json = helpers.Runtime, helpers.require, helpers.retain_metadata, helpers.save_json


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def document(root, receipt):
    raw = (root / 'stac' / f"document-{receipt['sha256']}.json").read_bytes()
    require(sha(raw) == receipt['sha256'], 'Archived source document hash changed')
    return json.loads(raw)


def matches(item, bounds, interval):
    b = item.get('bbox')
    if b is None or b[0] > bounds[2] or b[2] < bounds[0] or b[1] > bounds[3] or b[3] < bounds[1]:
        return False
    if interval is None:
        return True
    prop = item['properties']
    start = prop.get('datetime') or prop.get('start_datetime')
    end = prop.get('datetime') or prop.get('end_datetime')
    if not start or not end:
        return False
    parse = lambda value: datetime.fromisoformat(value.replace('Z', '+00:00'))
    parts = interval.split('/')
    first, last = parts[0], parts[-1]
    return (first == '..' or parse(end) >= parse(first)) and (last == '..' or parse(start) <= parse(last))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--directory-id', required=True)
    parser.add_argument('--bounds', required=True, nargs=4, type=float)
    parser.add_argument('--datetime')
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    root = output / 'store'
    binary = args.binary.resolve()
    with socket.socket() as reserved:
        reserved.bind(('127.0.0.1', 0)); port = reserved.getsockname()[1]
    stdout = (output / 'server.stdout.log').open('wb')
    stderr = (output / 'server.stderr.log').open('wb')
    process = subprocess.Popen([str(binary), 'serve', '--data-dir', str(root), '--port', str(port)],
        stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
    runtime = Runtime(f'http://127.0.0.1:{port}', output)
    report = {'schema': 'geod-static-stac-acceptance/v1', 'status': 'pending',
        'nativeBinarySha256': sha(binary.read_bytes()), 'endpoint': args.endpoint, 'directoryId': args.directory_id,
        'startedAt': datetime.now(timezone.utc).isoformat(), 'nativeWindowTested': False,
        'originalDownloads': 0, 'scope': 'Real public metadata, bounded local search, pinned project and offline restart', 'cases': []}
    try:
        deadline = time.monotonic() + 30
        while True:
            try:
                runtime.call('/health'); break
            except Exception:
                require(process.poll() is None and time.monotonic() < deadline, 'Owned native service did not start')
                time.sleep(.2)
        connection = runtime.call('/stac/connections', {'name': 'Public static STAC acceptance', 'url': args.endpoint, 'kind': 'catalog'})
        save_json(output / 'connection.json', connection)
        require(not connection['collections'] and not any(connection['capabilities'].values()), 'Static directory invented API capabilities')
        nodes = connection['catalogNodes']; receipts = connection['metadataDocuments']
        require(len(nodes) == len(receipts) and len(nodes) > 1, 'Real recursive discovery was not exercised')
        original_nodes = {}
        for node, receipt in zip(nodes, receipts):
            raw = document(root, receipt)
            require(node['url'] == receipt['url'] and node['key'] == sha(node['url'].encode()), 'Directory key was not derived from its URL')
            require(node['id'] == raw['id'] and node['kind'] == raw['type'] and node['description'] == raw['description'], 'Directory declarations changed')
            require(node['license'] == (raw['license'] if raw['type'] == 'Collection' else None), 'License declaration changed')
            original_nodes[node['key']] = raw
            if node['parentKey']:
                parent = next(n for n in nodes if n['key'] == node['parentKey'])
                urls = [urllib.parse.urljoin(parent['url'], link['href']) for link in original_nodes[parent['key']]['links'] if link['rel'] == 'child']
                require(node['url'] in urls, 'Child directory is not linked by its archived parent')
        chosen = [node for node in nodes if node['id'] == args.directory_id]
        require(len(chosen) == 1, 'Requested upstream directory ID is absent or ambiguous')
        chosen = chosen[0]
        query = {'connectionId': connection['id'], 'collectionId': chosen['key'], 'bounds': args.bounds,
            'datetime': args.datetime, 'limit': 2, 'cursor': None}
        all_items, pages = [], []
        while True:
            page = runtime.call('/stac/search', query); pages.append(page); all_items.extend(page['items'])
            require(page['scannedItems'] <= 1000 and not page['limitReached'], 'Positive query reached its bounded scan limit')
            if len(pages) == 1 and page['nextCursor']:
                rejected = False
                try:
                    runtime.call('/stac/search', {**query, 'cursor': page['nextCursor'], 'bounds': [0, 0, 1, 1]})
                except RuntimeError:
                    rejected = True
                require(rejected, 'Cursor accepted foreign filters'); report['cases'].append('cursor filters remain bound')
            previous = query['cursor']
            if previous:
                rejected = False
                try:
                    runtime.call('/stac/search', {**query, 'cursor': previous})
                except RuntimeError:
                    rejected = True
                require(rejected, 'Consumed cursor was reused'); report['cases'].append('consumed cursor rejected')
            if page['complete']:
                require(page['nextCursor'] is None, 'Complete scan still has continuation'); break
            require(page['nextCursor'] and len(pages) < 20, 'Static scan did not continue within acceptance bound')
            query['cursor'] = page['nextCursor']
        require(all_items, 'Positive public query returned no items')
        save_json(output / 'pages.json', pages)
        save_json(output / 'snapshots.json', all_items)
        expected_urls = {urllib.parse.urljoin(chosen['url'], link['href']) for link in original_nodes[chosen['key']]['links'] if link['rel'] == 'item'}
        # Chosen positive leaf intentionally has no children, so all six/eighteen
        # linked documents can be independently counted without inventing totals.
        require(not any(link['rel'] == 'child' for link in original_nodes[chosen['key']]['links']), 'Acceptance requires an explicit leaf directory')
        require(pages[-1]['scannedItems'] == len(expected_urls), 'Complete scan omitted or repeated linked item documents')
        sources = {json.loads(path.read_bytes()).get('id'): json.loads(path.read_bytes()) for path in (root / 'stac').glob('document-*.json')
            if json.loads(path.read_bytes()).get('type') == 'Feature'}
        expected = {identity for identity, source in sources.items() if matches(source, args.bounds, args.datetime)}
        require({item['itemId'] for item in all_items} == expected, 'Native results disagree with independent bbox/time filter')
        for item in all_items:
            retain_metadata(root, item, output)
            require(item['provenance']['searchMode'] == 'catalog' and item['provenance']['search']['collectionId'] == chosen['key'], 'Local branch provenance changed')
            require(item['provenance']['documentUrl'] in expected_urls, 'Returned item is outside the selected leaf')
            if item['collectionId']:
                require(item['provenance']['collection']['id'] == item['collectionId'], 'Actual Collection identity was aliased')
        selected = next((item, asset) for item in all_items for asset in item['assets'] if asset['eligible'])
        item, asset = selected
        pin = {'snapshotId': item['id'], 'assetKey': asset['key']}
        project = runtime.call('/stac/project', {'name': 'Public static catalog project', 'bounds': args.bounds, 'selections': [pin]})
        require(project['stacItems'][0]['snapshotId'] == item['id'] and project['stacItems'][0]['collectionId'] == item['collectionId'], 'Saved project changed its original source identity')
        save_json(output / 'project.json', project)
        require(runtime.call('/jobs') == [], 'Metadata acceptance unexpectedly created a transfer')
        report.update(directoryCount=len(nodes), itemCount=len(all_items), scannedItems=pages[-1]['scannedItems'], pageCount=len(pages), projectId=project['id'])
        runtime.call('/stac/connections/' + connection['id'] + '/forget', {})
        require(runtime.call('/stac/snapshots/' + item['id']) == item, 'Forget removed project-pinned metadata')
        process.terminate(); process.wait(timeout=15)
        request = [str(binary), 'stac', 'snapshot', '--id', item['id'], '--data-dir', str(root)]
        reopened = subprocess.run(request, capture_output=True, timeout=60, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        (output / 'restart.stdout.json').write_bytes(reopened.stdout); (output / 'restart.stderr.log').write_bytes(reopened.stderr)
        require(reopened.returncode == 0 and json.loads(reopened.stdout) == item, 'Offline snapshot/project recovery changed its pinned declarations')
        report['cases'].extend(['recursive source documents and real identities verified', 'complete local search independently matched',
            'project saved without transfer', 'forget preserves pinned source', 'offline native restart restores snapshot'])
        report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed', error=str(error)); raise
    finally:
        if process.poll() is None:
            # This script never queues transfers, so its owned service can stop.
            process.terminate(); process.wait(timeout=15)
        stdout.close(); stderr.close()
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'acceptance.json', report)
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    main()
