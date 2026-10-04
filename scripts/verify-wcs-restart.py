"""Reopen real WCS subset artifacts with the stopped native CLI; no network requests.

This validates exact retained server-subset bytes, immutable metadata and decoded
measurements. It does not claim acquisition of an original provider archive.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('wcs_qa', Path(__file__).with_name('verify-wcs-public.py'))
qa = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qa)
require, sha, read_json, save_json = qa.require, qa.sha, qa.read_json, qa.save_json


class Native:
    def __init__(self, binary, root, output):
        self.binary, self.root, self.output, self.calls = binary, root, output, []

    def call(self, group, action, ident=None, sample=None):
        require((group, action) in {('jobs', 'list'), ('wcs', 'list'), ('wcs', 'description'),
                                   ('wcs', 'snapshot'), ('wcs', 'inspect'), ('wcs', 'pixel')}, 'Only read-only CLI actions are allowed')
        command = [str(self.binary), group, action, '--data-dir', str(self.root)]
        if ident:
            command.extend(['--id', ident])
        if sample:
            command.extend(['--column', str(sample['column']), '--row', str(sample['row'])])
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
        name = f'{len(self.calls)+1:03d}-{group}-{action}'
        (self.output / (name + '.stdout.json')).write_bytes(result.stdout)
        (self.output / (name + '.stderr.txt')).write_bytes(result.stderr)
        self.calls.append({'arguments': command[1:], 'exitCode': result.returncode,
                           'stdout': name + '.stdout.json', 'stderr': name + '.stderr.txt'})
        save_json(self.output / 'commands.json', self.calls)
        require(result.returncode == 0, 'Native read failed; stop server without bypassing its lock: ' + result.stderr.decode('utf-8', errors='replace'))
        return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--reports', required=True, type=Path, nargs='+')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/geod-runtime.exe')
    args = parser.parse_args()
    root, output, binary = args.data_dir.resolve(), args.output.resolve(), args.binary.resolve()
    require(root.is_dir() and binary.is_file() and not output.exists() and not output.is_relative_to(root), 'Use existing storage/binary and a new evidence directory outside data storage')
    previous_reports = []
    for path in args.reports:
        path = path / 'report.json' if path.is_dir() else path
        report = read_json(path)
        require(report['status'] == 'passed' and Path(report['dataDirectory']).resolve() == root, 'Provide passed reports for this storage')
        previous_reports.append((path.parent, report))
    before = {name: (root / name).read_bytes() for name in ('wcs-connections.json', 'projects.json', 'jobs.json')}
    output.mkdir(parents=True)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(), 'dataDirectory': str(root),
              'binary': str(binary), 'binarySha256': sha(binary.read_bytes()), 'networkRequests': 0,
              'productKind': 'server-generated WCS coverage subsets', 'originalProviderArchiveClaimed': False,
              'projects': [], 'plans': [], 'jobs': []}
    native = Native(binary, root, output)
    save_json(output / 'report.json', report)
    try:
        connections = {c['id']: c for c in native.call('wcs', 'list')}
        jobs = {j['id']: j for j in native.call('jobs', 'list')}
        projects = json.loads(before['projects.json'])
        seen_plans, seen_jobs = set(), set()
        for directory, prior in previous_reports:
            connection, description, plan, project, job = [read_json(directory / (n + '.json')) for n in ('connection', 'description', 'plan', 'project', 'job')]
            require(connections.get(connection['id']) == connection, 'Connection did not restore exactly')
            require(projects.get(project['id']) == project, 'Saved project changed')
            report['projects'].append(project['id'])
            for receipt in prior['metadata']:
                evidence = directory / receipt['file']
                raw = evidence.read_bytes()
                require(len(raw) == receipt['bytes'] and sha(raw) == receipt['sha256'], 'Original metadata evidence changed')
                require((root / 'wcs' / evidence.name).read_bytes() == raw, 'Managed source metadata changed')
            if plan['id'] not in seen_plans:
                current_description = native.call('wcs', 'description', description['id'])
                current_plan = native.call('wcs', 'snapshot', plan['id'])
                require(current_description == description and current_plan == plan, 'Reopened source or subset plan differs')
                _, grid = qa.retain_metadata(root, connection, current_description, current_plan, output)
                qa.verify_plan(current_plan, grid, prior['requestedBounds'], prior['endpoint'])
                seen_plans.add(plan['id'])
                report['plans'].append(plan['id'])
            require(job['id'] not in seen_jobs, 'Duplicate job report supplied')
            seen_jobs.add(job['id'])
            require(jobs.get(job['id']) == job, 'Job did not restore exactly')
            path = Path(job['outputPath'])
            require(path.samefile(root / 'assets' / f"{job['id']}.tif"), 'Restored subset path differs')
            raw = (directory / 'subset.tif').read_bytes()
            require(len(raw) == prior['bytes'] and sha(raw) == prior['sha256'], 'Original response evidence changed')
            require(path.read_bytes() == raw, 'Managed subset bytes changed')
            reference = qa.qa.raster_reference(path)
            require(reference == read_json(directory / 'rasterio-reference.json'), 'Complete decoded sample hashes, masks or geotags changed')
            inspection = native.call('wcs', 'inspect', job['id'])
            qa.qa.verify_inspection(inspection, reference, job)
            require(inspection == read_json(directory / 'native-inspection.json'), 'Reopened native inspection differs')
            sample = reference['samples'][len(reference['samples']) // 2]
            pixel = native.call('wcs', 'pixel', job['id'], sample)
            qa.qa.verify_pixel(pixel, sample, job)
            save_json(output / f"rasterio-{job['id']}.json", reference)
            report['jobs'].append({'jobId': job['id'], 'bytes': len(raw), 'sha256': sha(raw),
                                   'independentDecodedSamples': sum(b['samples'] for b in reference['bands'])})
        for name, raw in before.items():
            require((root / name).read_bytes() == raw, 'Read-only reopen changed ' + name)
        report.update(status='passed', registryBytesUnchanged=True, nativeCliRuns=len(native.calls), restoredConnections=len(connections))
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'report.json'), 'projects': len(report['projects']),
                      'plans': len(report['plans']), 'jobs': len(report['jobs']), 'networkRequests': 0}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
