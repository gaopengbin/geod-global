"""Offline native CLI reopen/export verification of actual saved WFS snapshots.

Stop the server first. Only list, inspect and export commands are allowed; no
endpoint connection, query, network retry, proxy change, or lock bypass occurs.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

REPOSITORY = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8'))


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


class Native:
    def __init__(self, binary, root, output):
        self.binary, self.root, self.output = binary, root, output
        self.commands = []

    def call(self, group, operation, ident=None, destination=None):
        require((group, operation) in {('feature-services', 'list'), ('vectors', 'list'), ('vectors', 'inspect'), ('vectors', 'export')}, 'Offline checker only lists, inspects or exports')
        command = [str(self.binary), group, operation, '--data-dir', str(self.root)]
        if ident:
            command.extend(['--id', ident])
        if destination:
            command.extend(['--out', str(destination)])
        response = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        label = f'{len(self.commands) + 1:02d}-{group}-{operation}' + (f'-{ident}' if ident else '')
        stdout_file, stderr_file = label + '.stdout.json', label + '.stderr.txt'
        (self.output / stdout_file).write_bytes(response.stdout)
        if response.stderr:
            (self.output / stderr_file).write_bytes(response.stderr)
        self.commands.append({'arguments': command[1:], 'exitCode': response.returncode, 'stdoutFile': stdout_file, 'stderrFile': stderr_file if response.stderr else None})
        require(response.returncode == 0, 'Native CLI failed; stop the server and retain the exclusive storage lock: ' + response.stderr.decode('utf-8', errors='replace'))
        return json.loads(response.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--reports', required=True, type=Path, nargs='+', help='Passed verify-wfs-public.py reports or their directories')
    parser.add_argument('--output', required=True, type=Path, help='New evidence directory outside runtime storage')
    parser.add_argument('--binary', type=Path, default=REPOSITORY / 'target/debug/geod-runtime.exe')
    args = parser.parse_args()
    root, output, binary = args.data_dir.resolve(), args.output.resolve(), args.binary.resolve()
    require(root.is_dir() and binary.is_file(), 'Existing storage and a built runtime are required')
    require(not output.exists() and not output.is_relative_to(root), 'Use a new evidence directory outside managed storage')
    vector_path, service_path = root / 'vectors.json', root / 'feature-services.json'
    vector_bytes, service_bytes = vector_path.read_bytes(), service_path.read_bytes()
    vectors, services = json.loads(vector_bytes), json.loads(service_bytes)
    expected, ids, service_ids = [], set(), set()
    for requested in args.reports:
        path = requested / 'report.json' if requested.is_dir() else requested
        prior = read_json(path)
        require(prior.get('status') == 'passed' and prior.get('receipts'), f'Not a passed public acceptance report: {path}')
        require(Path(prior['dataDirectory']).resolve() == root, 'Report refers to another workspace')
        service = read_json(path.parent / 'service.json')
        require(service.get('wfs') and service['url'] == prior['endpoint'], 'Report service does not match WFS endpoint')
        require(services.get(service['id']) == service, 'WFS connection changed before offline reopen')
        service_ids.add(service['id'])
        for receipt in prior['receipts']:
            ident = receipt['assetId']
            require(ident not in ids, 'Repeated asset in supplied reports')
            ids.add(ident)
            record = read_json(path.parent / 'registry-record.json')
            require(vectors.get(ident) == record, 'WFS asset record changed before reopen')
            original = (path.parent / receipt['rawResponseFile']).read_bytes()
            exported = (path.parent / receipt['exportFile']).read_bytes()
            expected_ids = read_json(path.parent / 'expected-ids.json')
            require(len(original) == receipt['sourceBytes'] and sha(original) == receipt['sourceSha256'], 'Original archive evidence changed')
            require(sha(exported) == receipt['exportSha256'] and expected_ids == receipt['expectedIds'], 'Export/ID evidence changed')
            for document in receipt['sourceRawDocuments']:
                content = (path.parent / document['originalDocument']).read_bytes()
                require(len(content) == document['bytes'] and sha(content) == document['sha256'], 'Retained source document evidence changed')
            expected.append({'report': str(path.resolve()), 'receipt': receipt, 'record': record, 'original': original, 'exported': exported, 'ids': expected_ids})
    require(expected, 'No successfully acquired WFS assets were supplied')
    output.mkdir(parents=True)
    native = Native(binary, root, output)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(),
        'mode': 'Actual final native CLI reopening complete WFS source archives', 'dataDirectory': str(root),
        'binary': str(binary), 'binarySha256': sha(binary.read_bytes()), 'networkRequests': 0,
        'fixturesUsed': False, 'desktopWebViewAccepted': False, 'sourceReports': [e['report'] for e in expected], 'receipts': []}
    save_json(output / 'report.json', report)
    try:
        reopened_services = native.call('feature-services', 'list')
        require({s['id']: s for s in reopened_services} == services, 'Native CLI did not restore exact service metadata')
        report['restoredWfsServiceIds'] = sorted(service_ids)
        report['serviceRegistryRestoredExactly'] = True
        reopened_vectors = native.call('vectors', 'list')
        require({a['id']: a for a in reopened_vectors} == {ident: record['asset'] for ident, record in vectors.items()}, 'Native CLI did not restore exact vector registry')
        for item in expected:
            receipt, record = item['receipt'], item['record']
            ident = receipt['assetId']
            print('Checking actual offline WFS reopen and export: ' + ident, flush=True)
            backing = Path(record['path'])
            require(backing.samefile(root / 'vectors' / (ident + '.json')), 'Managed source path differs')
            require(backing.read_bytes() == item['original'], 'Original archive bytes changed')
            inspection = native.call('vectors', 'inspect', ident=ident)
            require(inspection['asset'] == record['asset'], 'Reopened metadata/provenance differs')
            require(inspection['geojson'] == json.loads(item['exported']), 'Reopened features or provenance differ')
            require([f['id'] for f in inspection['geojson']['features']] == item['ids'], 'Original feature identities/order changed')
            require(inspection['asset']['featureCount'] == receipt['sourceFeatureCount'] and inspection['asset']['coordinateCount'] == receipt['coordinateCount'], 'Reopened source feature/coordinate counts differ')
            destination = output / (ident + '.geojson')
            saved = native.call('vectors', 'export', ident=ident, destination=destination)
            require(saved.get('saved') is True and destination.is_file(), 'CLI did not save a real export')
            exported = destination.read_bytes()
            require(exported == item['exported'] and sha(exported) == receipt['exportSha256'] == inspection['asset']['geojsonSha256'], 'Reopened export bytes/hash changed')
            require(backing.read_bytes() == item['original'], 'Export modified the source archive')
            report['receipts'].append({'assetId': ident, 'sourceFeatureCount': receipt['sourceFeatureCount'],
                'coordinateCount': receipt['coordinateCount'], 'sourceBytes': receipt['sourceBytes'],
                'sourceSha256': receipt['sourceSha256'], 'exportSha256': receipt['exportSha256'],
                'sourceFormat': record['asset']['remoteSource']['wfs']['format'], 'rawBackingPath': str(backing), 'exportFile': destination.name,
                'originalArchiveAndAllSourceDocumentsUnchanged': True, 'allParsedFeaturesAndProvenanceUnchanged': True,
                'exactExportBytesAndHashUnchanged': True, 'originalFeatureIdsAndOrderUnchanged': True})
            save_json(output / 'report.json', report)
        require(vector_path.read_bytes() == vector_bytes and service_path.read_bytes() == service_bytes, 'Read-only commands changed registry bytes')
        report.update(status='passed', registriesByteForByteUnchanged=True, independentNativeProcessRuns=len(native.commands))
    except Exception as error:
        report['status'], report['error'] = 'failed', str(error)
        raise
    finally:
        report['commands'] = native.commands
        report['completedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'snapshots': len(report['receipts']), 'report': str(output / 'report.json')}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'Offline WFS verification failed: {error}', file=sys.stderr)
        sys.exit(1)
