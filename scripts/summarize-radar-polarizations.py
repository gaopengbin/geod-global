"""Keep bounded, credential-free evidence of actual radar polarization checks.

Incomplete matrices remain partial. Originals and processing outputs must be
bound to independent checks; renderer/MCP results do not replace raw-file proof.
"""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('--output', type=Path, help='Optional private receipt inside the verification root')
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('radar-polarizations-')


def read(relative):
    raw = (root / relative).read_bytes()
    value = json.loads(raw)
    sha = hashlib.sha256(raw).hexdigest()
    snapshot_file = 'evidence-snapshots/' + sha + '.json'
    snapshot = root / snapshot_file
    snapshot.parent.mkdir(exist_ok=True)
    try:
        with snapshot.open('xb') as destination:
            destination.write(raw)
    except FileExistsError:
        assert snapshot.read_bytes() == raw, 'The immutable evidence snapshot changed'
    return value, {'file': relative, 'sha256': sha, 'snapshot': snapshot_file}


native, native_file = read('native-polarizations-verification.json')
independent, independent_file = read('independent-polarizations-verification.json')
mcp, mcp_file = read('mcp-polarizations-verification.json')
ui, ui_file = read('ui/polarizations-verification.json')
recovery, recovery_file = read('resume-live-verification.json')
assert all(value['nativeBinarySha256'] == native['nativeBinarySha256'] for value in [independent, mcp, ui])
assert all(value['status'] in ['partial', 'passed'] for value in [independent, mcp, ui])
assert all(value['usedUserDesktop'] is False for value in [independent, mcp, ui])
assert independent['terrainCorrectionAccuracyAssessed'] is False
assert independent['additionalCalibrationApplied'] is independent['speckleFilteringApplied'] is False
assert mcp['readOnly'] and ui['errors'] == ui['remoteRequests'] == []
assert ui['frontendFiles'] and all('..' not in entry['file'] and '?' not in entry['file'] and len(entry['sha256']) == 64 for entry in ui['frontendFiles'])

originals, outputs = [], []
for proof in independent['originals']:
    record = next(entry for entry in native['originals'] if entry['job']['id'] == proof['jobId'])
    job, metadata = record['job'], record['metadata']
    assert job['sha256'] == proof['sha256'] and job['itemId'] == proof['itemId'] and job['assetKey'] == proof['key']
    assert job['status'] == 'succeeded' and job['settled'] and job['bytesDownloaded'] == job['totalBytes'] == proof['bytes']
    assert '?' not in job['href'] and all(entry['exactFloat32Match'] for entry in proof['pixels'])
    assert proof['preview']['exactRgbaMatch'] and proof['thumbnail']['exactRgbaMatch']
    originals.append({**proof, 'href': job['href'], 'grid': {key: metadata[key] for key in ['width', 'height', 'crs', 'bounds', 'pixelSize', 'nodata', 'radar']},
                      'sourceBytesUnchanged': True})
for proof in independent['outputs']:
    record = next(entry for entry in native['outputs'] if entry['job']['id'] == proof['jobId'])
    assert record['job']['sha256'] == proof['sha256'] and record['job']['mosaicOutput'] == proof['plan']
    assert proof['allFloat32BitsMatch'] and proof['preview']['exactRgbaMatch'] and proof['thumbnail']['exactRgbaMatch']
    outputs.append(proof)
assert originals
pins = {entry['jobId']: entry['sha256'] for entry in originals + outputs}
assert {entry['jobId']: entry['sha256'] for entry in mcp['cases']} == pins
assert all(entry['radarProfileAndRawFloat32Retained'] and entry['previewOmitted'] for entry in mcp['cases'])
assert all(entry['sha256'] in pins.values() and entry['pixel']['sha256'] == entry['sha256'] and entry['linearGamma0AndDbRetained'] and entry['paintedRaster'] for entry in ui['cases'])
assert all(entry['sha256'] in pins.values() and entry['pinnedSourcesIdentical'] and entry['outputBytesIdentical'] and entry['planIdentical'] for entry in ui['createdByUi'])

recovered = next(entry for entry in originals if entry['jobId'] == recovery['jobId'])
assert recovery['sameJobId'] and recovery['crashCheckpointRestored'] and recovery['rangeAccepted']
assert recovery['originalBytes'] == recovered['bytes'] and recovery['independentBoundary']['exactSourceBytes']
assert recovery['independentBoundary']['bytes'] == 65536
assert recovery['independentBoundary']['from'] == recovery['verifiedPrefixBytes'] - 32768
assert recovery['independentBoundary']['to'] == recovery['verifiedPrefixBytes'] + 32767
expected_cases = {(kind, key) for kind in ['single', 'mosaic', 'polygon'] for key in ['vh', 'hh', 'hv']}
complete = (all(value.get('status') == 'passed' for value in [native, independent, mcp, ui])
            and len(originals) == 6 and all(sum(entry['key'] == key for entry in originals) == 2 for key in ['vh', 'hh', 'hv'])
            and len(outputs) == 9 and {(entry['case'], entry['key']) for entry in outputs} == expected_cases
            and independent.get('nativeReceiptSha256') == native_file['sha256']
            and mcp.get('nativeReceiptSha256') == native_file['sha256']
            and mcp.get('independentReceiptSha256') == independent_file['sha256']
            and len(mcp['cases']) == 30
            and {(entry['mode'], entry['jobId']) for entry in mcp['cases']} == {
                (mode, job_id) for mode in ['direct read-only; rejected upstream proxy', 'loopback read-only; rejected upstream proxy']
                for job_id in pins}
            and mcp['disconnectedAdapterRejected'] and ui['offlineRestart'] is not None
            and ui.get('pendingCase') is None and ui.get('nativeReceiptSha256') == native_file['sha256']
            and ui.get('independentReceiptSha256') == independent_file['sha256']
            and len(ui['cases']) == len(ui['createdByUi']) == 9
            and {(entry['case'], entry['key']) for entry in ui['cases']} == expected_cases
            and {(entry['case'], entry['key']) for entry in ui['createdByUi']} == expected_cases)
receipt = {'schema': 'geod-radar-polarizations-verification/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
           'status': 'passed' if complete else 'partial', 'nativeBinarySha256': native['nativeBinarySha256'],
           'scope': 'Actual official Sentinel-1 IW RTC originals and independently checked same-grid project outputs; no installer or release.',
           'evidenceReports': [native_file, independent_file, mcp_file, ui_file, recovery_file],
           'signaturesPersisted': False, 'originals': originals, 'outputs': outputs,
           'independent': {key: independent[key] for key in ['independentLibraries', 'float32ValuesCompared', 'pixelChecks', 'rgbaPixelsCompared']},
           'recovery': {'checkpointEvidence': recovery, 'wholeOriginalAcceptedAfterRecovery': True,
                        'jobId': recovered['jobId'], 'sha256': recovered['sha256'], 'bytes': recovered['bytes']},
           'mcp': mcp,
           'desktopFrontend': {key: ui[key] for key in ['status', 'renderer', 'frontendFiles', 'readOnly', 'nativeWindowTested', 'usedUserDesktop', 'cases', 'createdByUi', 'offlineRestart']},
           'pending': [] if complete else ['Remaining actual acquisition originals and the full nine-output native project matrix.',
                                          'Full-matrix independent Float32 checks, offline restart/cache, processing buttons and direct/disconnected MCP.'],
           'limitations': ['No installed WebView manual acceptance.', 'No cross-grid reprojection, additional calibration, speckle filtering or vendor terrain-correction accuracy assessment.']}
retry_file = 'hv-retry-boundary-verification.json'
if (root / retry_file).is_file():
    retry, retry_proof_file = read(retry_file)
    queued, queued_file = read('hv-retry-queued-verification.json')
    live, live_file = read('hv-retry-live-verification.json')
    assert retry['status'] == 'partial' and retry['nativeBinarySha256'] == queued['nativeBinarySha256'] == native['nativeBinarySha256']
    assert retry['jobId'] == queued['jobId'] == live['jobId'] and retry['key'] == queued['key'] == live['key'] == 'hv'
    assert retry['workerReportedResumeBytes'] == queued['previousReportedBytes'] == live['resumedBytes']
    assert retry['totalBytes'] == queued['totalBytes'] == live['totalBytes']
    assert retry['sameJobId'] and queued['sameJobId'] and live['sameJobId']
    assert all(value['originalAccepted'] is value['nativeRestarted'] is value['usedUserDesktop'] is False for value in [retry, queued, live])
    assert retry['workerRequestCaptured'] is retry['signaturesPersisted'] is False
    assert '?' not in retry['href'] and any('hv' in case['keys'] and scene['itemId'] == retry['itemId']
        and scene['assets']['hv']['href'] == retry['href'] for case in native['cases'] for scene in case['project']['scenes'])
    assert retry['evidenceReports'] == [{'file': entry['file'], 'sha256': entry['sha256']} for entry in [queued_file, live_file]]
    checkpoint = retry['checkpoint']
    assert checkpoint['snapshot'] == 'evidence-snapshots/' + checkpoint['sha256'] + '.json'
    saved, saved_file = read(checkpoint['snapshot'])
    assert saved_file['sha256'] == checkpoint['sha256'] and saved['schema'] == 'geod-partial-transfer/v1'
    assert saved['binding'] == checkpoint['binding'] and saved['bytes'] == checkpoint['prefixBytes']
    assert saved['sha256'] == checkpoint['prefixSha256'] and checkpoint['independentHashMatch']
    assert saved['etag'] == retry['etag'] and saved['total'] == retry['totalBytes']
    assert retry['workerReportedResumeBytes'] <= checkpoint['prefixBytes'] < retry['totalBytes']
    boundary = retry['independentBoundary']
    assert boundary['bytes'] == 65536 and boundary['from'] == retry['workerReportedResumeBytes'] - 32768
    assert boundary['to'] == retry['workerReportedResumeBytes'] + 32767
    assert boundary['conditionalRangeAccepted'] and boundary['exactSourceBytes'] and len(boundary['sha256']) == 64
    receipt['retryResumeSubset'] = {'status': 'partial', 'wholeOriginalAccepted': False,
        'scope': retry['scope'], 'evidenceReports': [queued_file, live_file, retry_proof_file, saved_file], 'verification': retry}

acquisition_file = 'acquisition-originals-independent-verification.json'
if (root / acquisition_file).is_file():
    acquisition, acquisition_proof_file = read(acquisition_file)
    expected_snapshot = 'evidence-snapshots/' + acquisition['nativeReceiptSha256'] + '.json'
    assert acquisition['nativeReceiptSnapshot'] == expected_snapshot
    acquisition_native, acquisition_native_file = read(expected_snapshot)
    assert acquisition_native_file['sha256'] == acquisition['nativeReceiptSha256']
    assert acquisition['nativeBinarySha256'] == acquisition_native['nativeBinarySha256'] == native['nativeBinarySha256']
    assert acquisition['status'] == 'partial' and acquisition['outputs'] == []
    assert acquisition['usedUserDesktop'] is False
    if 'usedUserDesktop' in acquisition_native:
        assert acquisition_native['usedUserDesktop'] is False
    assert acquisition['terrainCorrectionAccuracyAssessed'] is False
    assert acquisition['additionalCalibrationApplied'] is acquisition['speckleFilteringApplied'] is False
    acquisition_originals = []
    for proof in acquisition['originals']:
        record = next(entry for entry in acquisition_native['originals'] if entry['job']['id'] == proof['jobId'])
        job, metadata = record['job'], record['metadata']
        assert job['status'] == 'succeeded' and job['settled'] and job['sha256'] == proof['sha256']
        assert job['itemId'] == proof['itemId'] and job['assetKey'] == proof['key']
        assert job['bytesDownloaded'] == job['totalBytes'] == proof['bytes'] and '?' not in job['href']
        assert all(pixel['exactFloat32Match'] for pixel in proof['pixels'])
        assert proof['preview']['exactRgbaMatch'] and proof['thumbnail']['exactRgbaMatch']
        acquisition_originals.append({**proof, 'href': job['href'],
            'grid': {key: metadata[key] for key in ['width', 'height', 'crs', 'bounds', 'pixelSize', 'nodata', 'radar']},
            'sourceBytesUnchanged': True})
    assert acquisition_originals and len(acquisition_originals) == len(acquisition_native['originals'])
    assert len({entry['jobId'] for entry in acquisition_originals}) == len(acquisition_originals)
    receipt['acquisitionOriginalSubset'] = {'status': 'passed', 'fullMatrixAccepted': False,
        'scope': 'Independently checked settled acquisition originals bound to an immutable native receipt; no additional processing, UI, MCP or offline claims.',
        'evidenceReports': [acquisition_native_file, acquisition_proof_file], 'originals': acquisition_originals,
        'pixelChecks': acquisition['pixelChecks'], 'rgbaPixelsCompared': acquisition['rgbaPixelsCompared']}
    if (root / 'acquisition-originals-mcp-verification.json').is_file():
        acquisition_mcp, acquisition_mcp_file = read('acquisition-originals-mcp-verification.json')
        assert acquisition_mcp['nativeBinarySha256'] == native['nativeBinarySha256']
        assert acquisition_mcp['nativeReceiptSha256'] == acquisition_native_file['sha256']
        assert acquisition_mcp['independentReceiptSha256'] == acquisition_proof_file['sha256']
        assert acquisition_mcp['status'] == 'partial' and acquisition_mcp['readOnly']
        assert acquisition_mcp['usedUserDesktop'] is acquisition_mcp['disconnectedAdapterRejected'] is False
        expected_pins = {entry['jobId']: entry['sha256'] for entry in acquisition_originals}
        assert len(acquisition_mcp['cases']) == len(expected_pins)
        assert {entry['jobId']: entry['sha256'] for entry in acquisition_mcp['cases']} == expected_pins
        assert all(entry['case'] == 'original' and entry['radarProfileAndRawFloat32Retained'] and entry['previewOmitted']
                   and entry['mode'] == 'loopback read-only; active acquisition store' for entry in acquisition_mcp['cases'])
        receipt['acquisitionOriginalSubset']['evidenceReports'].append(acquisition_mcp_file)
        receipt['acquisitionOriginalSubset']['mcp'] = acquisition_mcp

ready_original_files = ['early-originals-native-verification.json', 'early-originals-independent-verification.json']
if all((root / file).is_file() for file in ready_original_files):
    (ready_native, ready_native_file), (ready_proof, ready_proof_file) = [read(file) for file in ready_original_files]
    assert ready_native['nativeBinarySha256'] == ready_proof['nativeBinarySha256'] == native['nativeBinarySha256']
    assert ready_native['status'] == ready_proof['status'] == 'partial'
    assert ready_proof['nativeReceiptSha256'] == ready_native_file['sha256']
    assert ready_proof['nativeReceiptFile'] == ready_native_file['file']
    assert ready_native['usedUserDesktop'] is ready_proof['usedUserDesktop'] is False
    assert ready_proof['terrainCorrectionAccuracyAssessed'] is False
    assert ready_proof['additionalCalibrationApplied'] is ready_proof['speckleFilteringApplied'] is False
    assert ready_native['outputs'] == ready_proof['outputs'] == []
    ready_originals = []
    for proof in ready_proof['originals']:
        record = next(entry for entry in ready_native['originals'] if entry['job']['id'] == proof['jobId'])
        job, metadata = record['job'], record['metadata']
        assert job['status'] == 'succeeded' and job['settled'] and job['sha256'] == proof['sha256']
        assert job['itemId'] == proof['itemId'] and job['assetKey'] == proof['key']
        assert job['bytesDownloaded'] == job['totalBytes'] == proof['bytes'] and '?' not in job['href']
        assert all(pixel['exactFloat32Match'] for pixel in proof['pixels'])
        assert proof['preview']['exactRgbaMatch'] and proof['thumbnail']['exactRgbaMatch']
        ready_originals.append({**proof, 'href': job['href'], 'grid': {key: metadata[key] for key in ['width', 'height', 'crs', 'bounds', 'pixelSize', 'nodata', 'radar']},
                               'sourceBytesUnchanged': True})
    assert ready_originals and len(ready_originals) == len(ready_native['originals'])
    receipt['readyOriginalSubset'] = {'status': 'passed', 'fullMatrixAccepted': False,
        'scope': 'Settled actual originals checked without stopping the acquisition owner; no new offline, UI or MCP claims.',
        'evidenceReports': [ready_native_file, ready_proof_file], 'originals': ready_originals,
        'pixelChecks': ready_proof['pixelChecks'], 'rgbaPixelsCompared': ready_proof['rgbaPixelsCompared']}
subset_files = ['early-projects-native-verification.json', 'early-projects-independent-verification.json',
                'early-projects-mcp-verification.json', 'ui-completed/polarizations-verification.json']
if all((root / file).is_file() for file in subset_files):
    (early_native, early_native_file), (early_proof, early_proof_file), (early_mcp, early_mcp_file), (early_ui, early_ui_file) = [read(file) for file in subset_files]
    assert all(value['nativeBinarySha256'] == native['nativeBinarySha256'] for value in [early_native, early_proof, early_mcp, early_ui])
    assert early_proof['nativeReceiptSha256'] == early_native_file['sha256']
    assert early_ui['nativeReceiptSha256'] == early_native_file['sha256']
    assert early_ui['independentReceiptSha256'] == early_proof_file['sha256']
    assert early_ui['pendingCase'] is None
    assert early_proof['status'] == early_mcp['status'] == early_ui['status'] == 'partial'
    original_pins = {entry['jobId']: entry['sha256'] for entry in originals}
    for entry in receipt.get('readyOriginalSubset', {}).get('originals', []):
        assert entry['jobId'] not in original_pins or original_pins[entry['jobId']] == entry['sha256']
        original_pins[entry['jobId']] = entry['sha256']
    assert all(original_pins.get(entry['jobId']) == entry['sha256'] for entry in early_proof['originals'])
    subset_outputs = early_proof['outputs']
    assert subset_outputs and len(subset_outputs) == len(early_native['outputs'])
    for proof in subset_outputs:
        record = next(entry for entry in early_native['outputs'] if entry['job']['id'] == proof['jobId'])
        assert record['job']['status'] == 'succeeded' and record['job']['settled']
        assert proof['sha256'] == record['job']['sha256'] and proof['plan'] == record['job']['mosaicOutput']
        assert proof['allFloat32BitsMatch'] and proof['preview']['exactRgbaMatch'] and proof['thumbnail']['exactRgbaMatch']
        assert all(original_pins.get(pin['jobId']) == pin['sha256'] for pin in record['job']['mosaic']['sources'])
    subset_pins = {entry['jobId']: entry['sha256'] for entry in subset_outputs}
    assert {entry['jobId']: entry['sha256'] for entry in early_mcp['cases']} == subset_pins
    assert early_mcp['readOnly'] and not early_mcp['disconnectedAdapterRejected']
    assert all(entry['radarProfileAndRawFloat32Retained'] and entry['previewOmitted'] for entry in early_mcp['cases'])
    assert early_ui['errors'] == early_ui['remoteRequests'] == [] and early_ui['readOnly'] is False
    assert early_ui['offlineRestart'] is None and early_ui['frontendFiles']
    assert all(value['usedUserDesktop'] is False for value in [early_proof, early_mcp, early_ui])
    assert len(early_ui['cases']) == len(early_ui['createdByUi']) == 2 * len(subset_outputs)
    subset_cases = {(entry['case'], entry['key']) for entry in subset_outputs}
    assert {(entry['case'], entry['key']) for entry in early_ui['cases']} == subset_cases
    assert {(entry['case'], entry['key']) for entry in early_ui['createdByUi']} == subset_cases
    for proof in subset_outputs:
        scenes = [case for case in early_ui['cases'] if (case['case'], case['key']) == (proof['case'], proof['key'])]
        assert len(scenes) == 2 and all(case['sha256'] == proof['sha256'] for case in scenes)
        assert {(case['width'], case['locale'], case['theme']) for case in scenes} == {(1440, 'en', 'light'), (1024, 'zh-CN', 'dark')}
        assert all(case['pixel']['sha256'] == proof['sha256'] and case['linearGamma0AndDbRetained'] and case['paintedRaster'] for case in scenes)
        buttons = [entry for entry in early_ui['createdByUi'] if (entry['case'], entry['key']) == (proof['case'], proof['key'])]
        assert len(buttons) == 2 and all(entry['sha256'] == proof['sha256'] for entry in buttons)
    assert all(entry['sha256'] in subset_pins.values() and entry['pinnedSourcesIdentical'] and entry['outputBytesIdentical'] and entry['planIdentical'] for entry in early_ui['createdByUi'])
    receipt['readyProjectSubset'] = {'status': 'passed', 'fullMatrixAccepted': False,
        'scope': 'Ready projects processed in the active acquisition store; no owner restart or full-matrix offline acceptance.',
        'evidenceReports': [early_native_file, early_proof_file, early_mcp_file, early_ui_file],
        'outputs': subset_outputs, 'float32ValuesCompared': early_proof['float32ValuesCompared'],
        'mcp': early_mcp, 'desktopFrontend': {key: early_ui[key] for key in ['renderer', 'frontendFiles', 'nativeWindowTested', 'usedUserDesktop', 'cases', 'createdByUi']}}
    controls = []
    for file in ['summary-controls-verification.json', 'ui-checkpoint-resume-control.json']:
        if not (root / file).is_file():
            continue
        control, control_file = read(file)
        if control['actualUiReceiptSha256'] != early_ui_file['sha256']:
            continue
        assert control['status'] == 'passed' and control['usedUserDesktop'] is False
        if file.startswith('summary-'):
            assert control['nativeWritesPerformed'] is False and control['actualUiReceiptUnchanged']
        else:
            assert control['nativeInterrupted'] is control['newProcessingJobCreated'] is False
            assert control['allUiProcessingJobIdsAndBytesUnchanged'] and control['nativeServiceStillReachable']
        controls.append({'evidenceReport': control_file, 'verification': control})
    if controls:
        receipt['readyProjectSubset']['verificationControls'] = controls
destination = args.output.resolve() if args.output else Path('prototype/qa/radar-polarizations-verification.json')
if args.output:
    assert destination.parent == root
destination.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
print(json.dumps({'status': receipt['status'], 'originals': len(originals), 'outputs': len(outputs), 'rawPixels': independent['pixelChecks']}))
