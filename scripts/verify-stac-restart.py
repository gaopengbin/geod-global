"""Offline final-binary reopen of real STAC projects, source documents and rasters.

Stop the runtime first. Runs only list/snapshot/inspect/pixel CLI operations.
No connection, search, transfer, retry or network operation is requested.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

REPOSITORY = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('stac_public_qa', Path(__file__).with_name('verify-stac-public.py'))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)
require, sha, read_json, save_json = qa.require, qa.sha, qa.read_json, qa.save_json


class Native:
    def __init__(self, binary, root, output):
        self.binary, self.root, self.output, self.commands = binary, root, output, []

    def call(self, group, action, ident=None, sample=None):
        require((group, action) in {('jobs', 'list'), ('stac', 'list'), ('stac', 'snapshot'), ('stac', 'inspect'), ('stac', 'pixel')}, 'Offline QA command not allowed')
        command = [str(self.binary), group, action, '--data-dir', str(self.root)]
        if ident:
            command.extend(['--id', ident])
        if sample:
            command.extend(['--column', str(sample['column']), '--row', str(sample['row'])])
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
        label = f'{len(self.commands)+1:03d}-{group}-{action}'
        (self.output / (label + '.stdout.json')).write_bytes(result.stdout)
        (self.output / (label + '.stderr.txt')).write_bytes(result.stderr)
        self.commands.append({'arguments': command[1:], 'exitCode': result.returncode, 'stdout': label + '.stdout.json', 'stderr': label + '.stderr.txt'})
        save_json(self.output / 'commands.json', self.commands)
        require(result.returncode == 0, 'Native read failed; stop server without bypassing lock: ' + result.stderr.decode('utf-8', errors='replace'))
        return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--reports', required=True, type=Path, nargs='+')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--binary', type=Path, default=REPOSITORY / 'target/debug/geod-runtime.exe')
    args = parser.parse_args()
    root, output, binary = args.data_dir.resolve(), args.output.resolve(), args.binary.resolve()
    require(root.is_dir() and binary.is_file() and not output.exists() and not output.is_relative_to(root), 'Use existing storage/binary and new external output directory')
    before = {name: (root / name).read_bytes() for name in ('stac-connections.json', 'projects.json', 'jobs.json')}
    expected = []
    for path in args.reports:
        path = path / 'report.json' if path.is_dir() else path
        previous = read_json(path)
        require(previous['status'] == 'passed' and Path(previous['dataDirectory']).resolve() == root, 'Supply passed reports from this storage')
        expected.append((path.parent, previous))
    require(expected, 'No accepted reports')
    output.mkdir(parents=True)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(), 'dataDirectory': str(root),
        'binary': str(binary), 'binarySha256': sha(binary.read_bytes()), 'networkRequests': 0,
        'mode': 'Actual CLI reopen; complete original-file and metadata comparison; no COG conformance claim',
        'snapshots': [], 'jobs': [], 'projects': []}
    native = Native(binary, root, output)
    save_json(output / 'report.json', report)
    try:
        connections = {c['id']: c for c in native.call('stac', 'list')}
        jobs = {j['id']: j for j in native.call('jobs', 'list')}
        projects = json.loads(before['projects.json'])
        all_snapshot_ids, all_jobs = set(), set()
        for directory, previous in expected:
            connection = read_json(directory / 'connection.json')
            connection.setdefault('searchMethod', 'GET')
            restored = connections.get(connection['id'])
            require(restored is not None and set(connection['snapshotIds']).issubset(restored['snapshotIds']), 'Saved connection snapshots did not restore')
            # Later searches may add receipts. Legacy registries default to GET without rewriting their bytes.
            require({**restored, 'snapshotIds': connection['snapshotIds']} == connection, 'Connection declarations did not restore unchanged')
            project = read_json(directory / 'project.json')
            require(projects.get(project['id']) == project, 'Saved project changed')
            report['projects'].append(project['id'])
            for discovery in directory.glob('discovery-*.json'):
                raw = discovery.read_bytes()
                digest = discovery.stem.removeprefix('discovery-')
                require(sha(raw) == digest and (root / 'stac' / f'document-{digest}.json').read_bytes() == raw, 'Discovery raw document changed')
            for page in previous.get('searchPages', []):
                raw = (directory / page['file']).read_bytes()
                require(len(raw) == page['bytes'] and sha(raw) == page['sha256'] and (root / 'stac' / f"document-{page['sha256']}.json").read_bytes() == raw, 'Original search page or final empty page changed')
            snapshots = {s['id']: s for s in read_json(directory / 'snapshots.json')}
            for receipt in previous['snapshots']:
                ident = receipt['snapshotId']
                for field, prefix in [('recordFile', 'snapshot'), ('documentFile', 'document')]:
                    evidence = directory / receipt[field]
                    digest = ident if prefix == 'snapshot' else receipt['documentSha256']
                    raw = evidence.read_bytes()
                    require(sha(raw) == digest and (root / 'stac' / f'{prefix}-{digest}.json').read_bytes() == raw, 'Original metadata bytes changed')
                if ident not in all_snapshot_ids:
                    current = native.call('stac', 'snapshot', ident)
                    require(current == snapshots[ident], 'Reopened snapshot differs')
                    qa.retain_metadata(root, current, output)
                    all_snapshot_ids.add(ident)
                    report['snapshots'].append(ident)
            for receipt in previous['jobs']:
                ident = receipt['jobId']
                require(ident not in all_jobs, 'Duplicate source job reports')
                all_jobs.add(ident)
                job = read_json(directory / 'job.json')
                require(jobs.get(ident) == job, 'Source job did not restore unchanged')
                raw = (directory / receipt['originalFile']).read_bytes()
                require(len(raw) == receipt['bytes'] and sha(raw) == receipt['sha256'], 'Original evidence changed')
                require(Path(job['outputPath']).samefile(root / 'assets' / f'{ident}.tif'), 'Restored original path differs')
                require(Path(job['outputPath']).read_bytes() == raw, 'Managed original bytes changed')
                reference = qa.raster_reference(Path(job['outputPath']))
                require(reference == read_json(directory / 'rasterio-reference.json'), 'Independent raster pixels/geotags changed')
                inspection = native.call('stac', 'inspect', ident)
                qa.verify_inspection(inspection, reference, job)
                require(inspection == read_json(directory / 'native-inspection.json'), 'Final native inspection differs')
                selected_sample = reference['samples'][len(reference['samples']) // 2]
                pixel = native.call('stac', 'pixel', ident, selected_sample)
                qa.verify_pixel(pixel, selected_sample, job)
                save_json(output / f'rasterio-{ident}.json', reference)
                report['jobs'].append({'jobId': ident, 'bytes': len(raw), 'sha256': sha(raw), 'fullIndependentDecode': True,
                                       'sampleCount': sum(b['samples'] for b in reference['bands'])})
        for name, raw in before.items():
            require((root / name).read_bytes() == raw, f'Offline reads changed {name}')
        report.update(status='passed', registryBytesUnchanged=True, restoredConnections=len(connections), nativeCliRuns=len(native.commands))
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'report.json'), 'snapshots': len(report['snapshots']),
                      'projects': len(report['projects']), 'jobs': len(report['jobs']), 'networkRequests': 0}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
