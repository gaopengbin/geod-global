"""Independently check an actual Agent WCS review/queue result and a fresh subset.

The input must be a passed native public workflow record, not renderer fixtures.
No endpoint is selected by default; the retained explicit source controls the check.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('wcs_reference', ROOT / 'scripts/verify-wcs-public.py')
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workflow', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    workflow, output, binary = args.workflow.resolve(), args.output.resolve(), args.binary.resolve()
    root = workflow.parent
    qa.require(binary.is_file() and workflow.is_file() and not output.exists() and not output.is_relative_to(root), 'Use a real record, native binary and new separate evidence directory')
    record = json.loads(workflow.read_text(encoding='utf-8'))
    qa.require(record['synthetic'] is False and isinstance(record['modelUsed'], bool) and record['usedUserDesktop'] is False, 'This verifier requires the native public review workflow')
    qa.require(record['job']['status'] == 'succeeded' and record['completed']['jobs'][0]['settled'] is True, 'A queued result is not a completed file')
    output.mkdir(parents=True)
    report = {'status': 'running', 'checkedAt': datetime.now(timezone.utc).isoformat(), 'synthetic': False,
              'modelUsed': record['modelUsed'], 'nativeWindowsTested': False, 'usedUserDesktop': False,
              'originalSceneClaimed': False, 'automaticRetries': 0, 'calls': [],
              'binarySha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'source': record['source']}
    save = lambda name, value: (output / name).write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding='utf-8')
    save('report.json', report)

    def native(group, action, ident=None, sample=None):
        command = [str(binary), group, action, '--data-dir', str(root)]
        if ident: command += ['--id', ident]
        if sample: command += ['--column', str(sample['column']), '--row', str(sample['row'])]
        result = subprocess.run(command, capture_output=True, timeout=90)
        report['calls'].append({'group': group, 'action': action, 'exitCode': result.returncode})
        qa.require(result.returncode == 0, result.stderr.decode('utf-8', errors='replace'))
        value = json.loads(result.stdout)
        save(f'{len(report["calls"]):03d}-{group}-{action}.json', value)
        return value

    before = {name: (root / name).read_bytes() for name in ('projects.json', 'jobs.json', 'wcs-connections.json')}
    try:
        connection = next(c for c in native('wcs', 'list') if c['id'] == record['connection']['id'])
        job = next(j for j in native('jobs', 'list') if j['id'] == record['job']['id'])
        qa.require(job == record['job'], 'Persisted task differs from the actual confirmed result')
        qa.require(job['agentApproval']['planId'] == record['downloadReview']['planId'] and job['agentApproval']['planHash'] == record['downloadReview']['planHash'] and job['agentApproval']['sessionId'] == record['sessionId'], 'Queue approval is not bound to the actual review')
        plan = native('wcs', 'snapshot', job['wcsSource']['planId'])
        description = native('wcs', 'description', plan['description']['id'])
        metadata, grid = qa.retain_metadata(root, connection, description, plan, output)
        expected = qa.verify_plan(plan, grid, record['source']['bounds'], record['source']['url'])
        path = Path(job['outputPath'])
        qa.require(path.samefile(root / 'assets' / f'{job["id"]}.tif'), 'Output left the managed store')
        raw = path.read_bytes()
        qa.require(len(raw) == job['bytesDownloaded'] == job['totalBytes'] <= 1048576 and qa.sha(raw) == job['sha256'], 'Complete subset bytes changed')
        reference = qa.qa.raster_reference(path)
        qa.require((reference['width'], reference['height'], reference['crs']) == (expected['width'], expected['height'], grid['crs']), 'Independent grid differs')
        qa.close_sequence(reference['transform'], expected['transform'], 'Independent pixel grid shifted')
        qa.close_sequence(reference['bounds'], expected['nativeBounds'], 'Independent raster bounds shifted')
        qa.require(reference['bandCount'] == len(grid['fields']), 'Range fields differ')
        inspection = native('wcs', 'inspect', job['id'])
        qa.qa.verify_inspection(inspection, reference, job)
        for sample in reference['samples']:
            qa.qa.verify_pixel(native('wcs', 'pixel', job['id'], sample), sample, job)
        save('rasterio-reference.json', reference)
        fresh = output / 'independent-public-subset.tif'
        result = subprocess.run(['curl.exe', '--fail', '--silent', '--show-error', '--max-time', '45', '--max-filesize', '1048576', '--output', str(fresh), plan['requestUrl']], capture_output=True, timeout=55)
        qa.require(result.returncode == 0, 'Independent public GetCoverage failed: ' + result.stderr.decode('utf-8', errors='replace'))
        independent = qa.qa.raster_reference(fresh)
        qa.compare_research(reference, independent)
        qa.require(fresh.read_bytes() == raw, 'Fresh public subset bytes differ')
        qa.require(all((root / name).read_bytes() == value for name, value in before.items()), 'Read-only verification changed native records')
        report.update(status='passed', metadata=metadata, jobId=job['id'], planId=plan['id'],
                      bytes=len(raw), sha256=job['sha256'], width=reference['width'], height=reference['height'],
                      channels=reference['bandCount'], sampleCount=reference['width'] * reference['height'] * reference['bandCount'],
                      fullRasterSamplesAndMasksEqual=True, independentResponseBytesEqual=True,
                      declaredFields=grid['fields'], nativeRecordsUnchanged=True)
    except Exception as error:
        report.update(status='failed', error=str(error))
        raise
    finally:
        save('report.json', report)
    print(json.dumps({k: report[k] for k in ('status', 'bytes', 'sha256', 'width', 'height', 'sampleCount', 'fullRasterSamplesAndMasksEqual', 'independentResponseBytesEqual')}, indent=2))


if __name__ == '__main__':
    main()
