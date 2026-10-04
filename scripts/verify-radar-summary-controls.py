"""Exercise receipt rejection with copies of actual completed radar evidence.

Only private JSON copies are changed. These synthetic receipt controls do not
constitute extra downloads, raster processing, UI actions or native restarts.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('radar-polarizations-')
reports = [
    'native-polarizations-verification.json', 'independent-polarizations-verification.json',
    'mcp-polarizations-verification.json', 'ui/polarizations-verification.json', 'resume-live-verification.json',
    'early-originals-native-verification.json', 'early-originals-independent-verification.json',
    'early-projects-native-verification.json', 'early-projects-independent-verification.json',
    'early-projects-mcp-verification.json', 'ui-completed/polarizations-verification.json',
]
control_root = root.with_name(root.name + '-receipt-controls-' + uuid.uuid4().hex[:8])
control_root.mkdir()
for name in reports:
    destination = control_root / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes((root / name).read_bytes())
acquisition_file = 'acquisition-originals-independent-verification.json'
if (root / acquisition_file).is_file():
    acquisition_raw = (root / acquisition_file).read_bytes()
    acquisition = json.loads(acquisition_raw)
    snapshot_file = 'evidence-snapshots/' + acquisition['nativeReceiptSha256'] + '.json'
    assert acquisition['nativeReceiptSnapshot'] == snapshot_file
    snapshot_raw = (root / snapshot_file).read_bytes()
    assert hashlib.sha256(snapshot_raw).hexdigest() == acquisition['nativeReceiptSha256']
    (control_root / acquisition_file).write_bytes(acquisition_raw)
    snapshot = control_root / snapshot_file
    snapshot.parent.mkdir(exist_ok=True)
    snapshot.write_bytes(snapshot_raw)
    acquisition_mcp_file = 'acquisition-originals-mcp-verification.json'
    if (root / acquisition_mcp_file).is_file():
        (control_root / acquisition_mcp_file).write_bytes((root / acquisition_mcp_file).read_bytes())
retry_file = 'hv-retry-boundary-verification.json'
if (root / retry_file).is_file():
    retry_raw = (root / retry_file).read_bytes()
    retry = json.loads(retry_raw)
    snapshot_file = 'evidence-snapshots/' + retry['checkpoint']['sha256'] + '.json'
    assert retry['checkpoint']['snapshot'] == snapshot_file
    snapshot_raw = (root / snapshot_file).read_bytes()
    assert hashlib.sha256(snapshot_raw).hexdigest() == retry['checkpoint']['sha256']
    for name in [retry_file, 'hv-retry-queued-verification.json', 'hv-retry-live-verification.json']:
        (control_root / name).write_bytes((root / name).read_bytes())
    snapshot = control_root / snapshot_file
    snapshot.parent.mkdir(exist_ok=True)
    snapshot.write_bytes(snapshot_raw)
ui_file = 'ui-completed/polarizations-verification.json'
original_ui = (root / ui_file).read_bytes()
baseline = json.loads(original_ui)
assert baseline['pendingCase'] is None and len(baseline['cases']) == len(baseline['createdByUi'])
summary = Path('scripts/summarize-radar-polarizations.py').resolve()


def run(name, accepted):
    destination = control_root / (name + '.json')
    result = subprocess.run([sys.executable, '-X', 'utf8', str(summary), str(control_root),
                             '--output', str(destination)], capture_output=True, text=True, encoding='utf8')
    if accepted:
        assert result.returncode == 0, result.stderr
        receipt = json.loads(destination.read_text(encoding='utf8'))
        assert receipt['readyProjectSubset']['status'] == 'passed'
    else:
        assert result.returncode != 0 and 'AssertionError' in result.stderr, result.stdout + result.stderr
        assert not destination.exists(), 'A rejected receipt must not be written'
    return {'control': name, 'accepted': accepted, 'status': 'passed'}


checks = [run('actual-cohort-baseline', True)]
changed = json.loads(original_ui)
single = next(entry for entry in changed['cases'] if entry['case'] == 'single' and entry['key'] == 'hh')
mosaic = next(entry for entry in changed['cases'] if entry['case'] == 'mosaic' and entry['key'] == 'hh')
assert single['sha256'] == mosaic['sha256'], 'This control requires an actual identical-byte overlap'
mosaic['case'] = 'single'
next(entry for entry in changed['createdByUi'] if entry['id'] == mosaic['jobId'])['case'] = 'single'
(control_root / ui_file).write_text(json.dumps(changed), encoding='utf8')
checks.append(run('same-bytes-wrong-processing-case', False))

changed = json.loads(original_ui)
changed['pendingCase'] = {'jobId': changed['cases'][-1]['jobId']}
(control_root / ui_file).write_text(json.dumps(changed), encoding='utf8')
checks.append(run('incomplete-ui-checkpoint', False))

changed = json.loads(original_ui)
changed['nativeReceiptSha256'] = '0' * 64
(control_root / ui_file).write_text(json.dumps(changed), encoding='utf8')
checks.append(run('different-native-cohort', False))
if (root / acquisition_file).is_file():
    (control_root / ui_file).write_bytes(original_ui)
    changed_acquisition = json.loads(acquisition_raw)
    changed_acquisition['nativeReceiptSha256'] = '0' * 64
    (control_root / acquisition_file).write_text(json.dumps(changed_acquisition), encoding='utf8')
    checks.append(run('different-acquisition-snapshot', False))
    assert (root / acquisition_file).read_bytes() == acquisition_raw
    (control_root / acquisition_file).write_bytes(acquisition_raw)
if (root / retry_file).is_file():
    (control_root / ui_file).write_bytes(original_ui)
    changed_retry = json.loads(retry_raw)
    changed_retry['workerReportedResumeBytes'] += 1
    (control_root / retry_file).write_text(json.dumps(changed_retry), encoding='utf8')
    checks.append(run('different-retry-boundary', False))
    assert (root / retry_file).read_bytes() == retry_raw
assert (root / ui_file).read_bytes() == original_ui
result = {'schema': 'geod-radar-receipt-controls/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
          'status': 'passed', 'scope': 'Synthetic receipt controls using private copies of actual evidence; no additional data or UI acceptance.',
          'actualUiReceiptSha256': hashlib.sha256(original_ui).hexdigest(), 'actualUiReceiptUnchanged': True,
          'nativeWritesPerformed': False, 'usedUserDesktop': False, 'checks': checks}
(root / 'summary-controls-verification.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf8')
print(json.dumps({'status': result['status'], 'controls': len(checks), 'actualUiReceiptUnchanged': True}))
