"""Offline restart verification of actual Overpass snapshots via the native CLI.

Stop the runtime server first. Every command opens --data-dir exclusively and
exits again. This script only calls list, inspect and export; it never connects
to an endpoint, submits a query, retries a job, or changes proxy settings.
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


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8'))


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


class Native:
    def __init__(self, binary, data_dir, evidence):
        self.binary, self.data_dir, self.evidence = binary, data_dir, evidence
        self.commands = []

    def call(self, group, operation, *, ident=None, out=None):
        require((group, operation) in {
            ('feature-services', 'list'), ('vectors', 'list'), ('vectors', 'inspect'), ('vectors', 'export')
        }, 'Offline verification may only list, inspect or export')
        command = [str(self.binary), group, operation, '--data-dir', str(self.data_dir)]
        if ident:
            command.extend(['--id', ident])
        if out:
            command.extend(['--out', str(out)])
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        label = f'{len(self.commands) + 1:02d}-{group}-{operation}' + (f'-{ident}' if ident else '')
        (self.evidence / f'{label}.stdout.json').write_bytes(result.stdout)
        if result.stderr:
            (self.evidence / f'{label}.stderr.txt').write_bytes(result.stderr)
        self.commands.append({'arguments': command[1:], 'exitCode': result.returncode,
                              'stdoutFile': f'{label}.stdout.json', 'stderrFile': f'{label}.stderr.txt' if result.stderr else None})
        require(result.returncode == 0,
                'Native CLI failed; stop the server before this check and do not bypass the storage lock: '
                + result.stderr.decode('utf-8', errors='replace').strip())
        return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir', required=True, type=Path, help='Existing runtime storage; its server must be stopped')
    parser.add_argument('--reports', required=True, type=Path, nargs='+', help='Passed verify-overpass-public.py report.json files or their directories')
    parser.add_argument('--output', required=True, type=Path, help='New evidence directory outside runtime storage')
    parser.add_argument('--binary', type=Path, default=REPOSITORY / 'target/debug/geod-runtime.exe', help='Built native CLI, default target/debug/geod-runtime.exe')
    args = parser.parse_args()
    root, binary, output = args.data_dir.resolve(), args.binary.resolve(), args.output.resolve()
    require(root.is_dir(), 'Runtime data directory does not exist')
    require(binary.is_file(), 'Native runtime binary does not exist; build it before verification')
    require(not output.exists(), 'Output must be a new directory; earlier acceptance evidence is retained')
    require(not output.is_relative_to(root), 'Native exports must be outside managed runtime storage')
    vector_registry_path, service_registry_path = root / 'vectors.json', root / 'feature-services.json'
    registry_before, services_before = read_json(vector_registry_path), read_json(service_registry_path)
    require(isinstance(registry_before, dict) and isinstance(services_before, dict), 'Invalid persisted registries')
    expected, identities, service_evidence = [], set(), {}
    for requested in args.reports:
        report_path = requested / 'report.json' if requested.is_dir() else requested
        prior = read_json(report_path)
        require(prior.get('status') == 'passed' and bool(prior.get('receipts')), f'Not a passed acquisition report: {report_path}')
        require(Path(prior['dataDirectory']).resolve() == root, f'Acquisition report refers to other storage: {report_path}')
        saved_service = read_json(report_path.parent / 'service.json')
        require(saved_service.get('overpass') and saved_service['url'] == prior['endpoint'], 'Acquisition service evidence differs from endpoint')
        require(services_before.get(saved_service['id']) == saved_service, 'Original connection is missing or changed before restart')
        service_evidence[saved_service['id']] = saved_service
        for receipt in prior['receipts']:
            ident, preset = receipt['assetId'], receipt['preset']
            require(ident not in identities, f'Asset is listed in more than one report: {ident}')
            identities.add(ident)
            prior_record = read_json(report_path.parent / f'{preset}.registry-record.json')
            require(registry_before.get(ident) == prior_record, f'Asset registry changed before restart: {ident}')
            raw_bytes = (report_path.parent / receipt['rawResponseFile']).read_bytes()
            export_bytes = (report_path.parent / receipt['exportFile']).read_bytes()
            require(len(raw_bytes) == receipt['sourceBytes'] and sha(raw_bytes) == receipt['sourceSha256'], 'Saved public raw evidence hash mismatch')
            require(sha(export_bytes) == receipt['exportSha256'], 'Saved public GeoJSON evidence hash mismatch')
            expected_ids = read_json(report_path.parent / f'{preset}.expected-ids.json')
            require(expected_ids == receipt['expectedIds'], 'Expected ID evidence differs from report')
            expected.append({'reportPath': str(report_path.resolve()), 'receipt': receipt, 'record': prior_record,
                             'rawBytes': raw_bytes, 'exportBytes': export_bytes, 'expectedIds': expected_ids})
    require(bool(expected), 'No successful acquired snapshots were supplied')
    output.mkdir(parents=True)
    report = {'startedAt': datetime.now(timezone.utc).isoformat(), 'status': 'running',
              'mode': 'Actual native CLI reopening persisted Overpass snapshots with exclusive storage access',
              'dataDirectory': str(root), 'binary': str(binary), 'networkRequests': 0,
              'fixturesUsed': False, 'desktopWebViewAccepted': False,
              'sourceReports': [item['reportPath'] for item in expected], 'receipts': []}
    native = Native(binary, root, output)
    save_json(output / 'report.json', report)
    try:
        restored_services = native.call('feature-services', 'list')
        require({s['id']: s for s in restored_services} == services_before, 'Native CLI did not restore the exact service registry')
        report['restoredServiceIds'] = sorted(s['id'] for s in restored_services)
        report['overpassServiceIds'] = sorted(service_evidence)
        report['serviceRegistryRestoredExactly'] = True
        restored_vectors = native.call('vectors', 'list')
        require({v['id']: v for v in restored_vectors} == {ident: value['asset'] for ident, value in registry_before.items()}, 'Native CLI did not restore the exact vector registry')
        report['allRegisteredVectorIds'] = sorted(v['id'] for v in restored_vectors)
        for item in expected:
            receipt, record = item['receipt'], item['record']
            ident, preset = receipt['assetId'], receipt['preset']
            print(f'Checking offline reopen and export: {preset} / {ident}', flush=True)
            raw_path = Path(record['path'])
            require(raw_path.samefile(root / 'vectors' / f'{ident}.json'), 'Registered original is outside its expected managed file')
            require(raw_path.read_bytes() == item['rawBytes'], 'Original provider bytes changed before native reopen')
            inspection = native.call('vectors', 'inspect', ident=ident)
            require(inspection['asset'] == record['asset'], 'Restored asset metadata/provenance differs')
            previous_geojson = json.loads(item['exportBytes'])
            require(inspection['geojson'] == previous_geojson, 'Restored parsed features or provenance changed')
            require([f['id'] for f in inspection['geojson']['features']] == item['expectedIds'], 'Restored selected feature IDs/order changed')
            require(inspection['asset']['featureCount'] == receipt['selectedCounts']['total'], 'Dependencies were promoted into exported features')
            destination = output / f'{preset}-{ident}.geojson'
            exported = native.call('vectors', 'export', ident=ident, out=destination)
            require(exported.get('saved') is True and destination.is_file(), 'CLI did not confirm an actual saved export')
            current_export = destination.read_bytes()
            require(current_export == item['exportBytes'], 'Native export bytes differ from original verified GeoJSON')
            require(sha(current_export) == receipt['exportSha256'] == inspection['asset']['geojsonSha256'], 'Native export SHA-256 changed')
            require(raw_path.read_bytes() == item['rawBytes'], 'Export modified original provider bytes')
            report['receipts'].append({
                'assetId': ident, 'preset': preset, 'featureCount': receipt['selectedCounts']['total'],
                'dependencyCount': receipt['dependencyCounts']['total'], 'sourceBytes': receipt['sourceBytes'],
                'sourceSha256': receipt['sourceSha256'], 'geojsonSha256': receipt['exportSha256'],
                'rawBackingPath': str(raw_path), 'exportFile': destination.name,
                'originalProviderBytesUnchanged': True, 'parsedGeojsonAndAllProvenanceUnchanged': True,
                'exactExportBytesAndHashUnchanged': True, 'selectedIdsAndOrderUnchanged': True,
                'geometryUnchangedFromIndependentPublicAcceptance': True,
            })
            save_json(output / 'report.json', report)
        require(read_json(vector_registry_path) == registry_before, 'Read/export operations changed the vector registry')
        require(read_json(service_registry_path) == services_before, 'Read/export operations changed the service registry')
        report['registeredFilesUnchanged'] = True
        report['independentNativeProcessRuns'] = len(native.commands)
        report['status'] = 'passed'
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        report['commands'] = native.commands
        report['completedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'verifiedSnapshots': len(report['receipts']), 'report': str(output / 'report.json')}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'Offline restart verification failed: {error}', file=sys.stderr)
        sys.exit(1)
