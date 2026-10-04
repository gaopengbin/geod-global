"""Produce a bounded, credential-free receipt from actual NAIP verifier output.

Partial input stays partial. This does not download files or certify missing
cases, and keeps only hashes and checked results rather than imagery or PNGs.
"""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('naip-resolutions-')


def read(relative):
    file = root / relative
    raw = file.read_bytes()
    return json.loads(raw), {'file': relative, 'sha256': hashlib.sha256(raw).hexdigest()}


native, native_file = read('native-resolution-verification.json')
independent, independent_file = read('independent-resolution-verification.json')
ui, ui_file = read('ui/resolution-verification.json')
mcp, mcp_file = read('mcp-resolution-verification.json')
online, online_file = read('online/resolution-online-verification.json')
assert all(value['nativeBinarySha256'] == native['nativeBinarySha256'] for value in [independent, ui, mcp, online])
assert all(value['status'] in ['partial', 'passed'] for value in [independent, ui, mcp])
assert ui['errors'] == ui['remoteRequests'] == []
assert ui['usedUserDesktop'] is False and mcp['usedUserDesktop'] is False
assert online['status'] == 'passed' and online['wholeOriginalAcceptance'] is False and online['usedUserDesktop'] is False
assert {entry['group'] for entry in online['cases']} == {'1m', '0.3m'}
for entry in online['cases']:
    assert entry['status'] == 'passed' and entry['errors'] == entry['rejectedRemote'] == []
    assert any(probe['opaqueSamples'] > 100 and probe['distinctRgb'] > 40 and probe['glError'] == 0 for probe in entry['rasterPixels'])
originals, outputs = [], []
for entry in independent['originals']:
    assert any(source['job']['id'] == entry['jobId'] and source['job']['sha256'] == entry['sha256'] for source in native['originals'])
    assert '?' not in entry['href'] and entry['sourceBytesUnchanged']
    originals.append({key: entry[key] for key in ['jobId', 'itemId', 'href', 'bytes', 'sha256', 'grid', 'physicalSourceTags',
                    'gdalSourceColorInterpretation', 'bandRoles', 'sourceBytesUnchanged', 'points', 'images']})
for entry in independent['outputs']:
    assert any(source['job']['id'] == entry['jobId'] and source['job']['sha256'] == entry['sha256'] for source in native['outputs'])
    assert entry['exactFourChannelsEqual'] and entry['exactCoverageMaskEqual']
    outputs.append(entry)
assert originals and outputs
accepted_ids = {entry['jobId'] for entry in originals + outputs}
assert {entry['jobId'] for entry in mcp['cases']} == accepted_ids
assert all(entry['rawFourChannelsRetained'] and entry['paintedRaster'] for entry in ui['cases'])
evidence_reports = [native_file, independent_file, ui_file, mcp_file, online_file]
offline_subset = None
if (root / 'subset-cache-verification.json').exists():
    cache, cache_file = read('subset-cache-verification.json')
    snapshot_mcp, snapshot_mcp_file = read('mcp-resolution-snapshot-verification.json')
    subset_pins = {entry['job']['id']: entry['job']['sha256'] for entry in native['originals'] + native['outputs']
                   if entry['group'] == '1m' and entry['job']['id'] in accepted_ids}
    assert len(subset_pins) == 5
    for value in [cache, snapshot_mcp]:
        assert value['status'] == 'passed' and value['group'] == '1m'
        assert value['nativeBinarySha256'] == native['nativeBinarySha256'] and value['usedUserDesktop'] is False
    assert {entry['jobId']: entry['sha256'] for entry in cache['entries']} == subset_pins
    assert all(entry['thumbnailIdentical'] and entry['cacheBytesFileIdentityCreationUnchanged'] for entry in cache['entries'])
    assert cache['projectsRestored'] == 3 and cache['filesRestored'] == 5
    assert cache['changedDerivedRejected'] and cache['invalidCacheRebuiltFromUnchangedSource']
    assert {entry['jobId']: entry['sha256'] for entry in snapshot_mcp['cases']} == subset_pins
    assert len(snapshot_mcp['cases']) == 10 and snapshot_mcp['pixelsCompared'] == 50
    assert snapshot_mcp['readOnly'] and snapshot_mcp['disconnectedAdapterRejected']
    assert all(entry['fourOriginalChannelsRetained'] and entry['aerialProfileAndCoverageRetained'] for entry in snapshot_mcp['cases'])
    evidence_reports += [cache_file, snapshot_mcp_file]
    offline_subset = {'status': 'passed', 'group': '1m', 'copiedPreviouslyAcceptedFiles': True,
                      'scope': 'Offline restart, persistent cache and read-only MCP for five actual 1 m files in an explicit isolated local snapshot.',
                      'cache': cache, 'mcp': snapshot_mcp, 'desktopFrontend': None}
    if (root / 'ui-snapshot/resolution-verification.json').exists():
        snapshot_ui, snapshot_ui_file = read('ui-snapshot/resolution-verification.json')
        assert snapshot_ui['status'] == 'passed' and snapshot_ui['group'] == '1m'
        assert snapshot_ui['nativeBinarySha256'] == native['nativeBinarySha256'] and snapshot_ui['usedUserDesktop'] is False
        assert snapshot_ui['errors'] == snapshot_ui['remoteRequests'] == [] and snapshot_ui['readOnly'] is False
        assert len(snapshot_ui['cases']) == 5 and len(snapshot_ui['createdByUi']) == 3
        assert len(snapshot_ui['compactSourceAccess']) == 2 and {entry['locale'] for entry in snapshot_ui['compactSourceAccess']} == {'en', 'zh-CN'}
        assert all(entry['collapsedByDefault'] and entry['bothDialogsAccessible'] and entry['focusRestored'] and entry['noSaveOrDownloadRequested'] for entry in snapshot_ui['compactSourceAccess'])
        assert snapshot_ui['frontendFiles'] and all('..' not in entry['file'] and '?' not in entry['file'] and len(entry['sha256']) == 64 for entry in snapshot_ui['frontendFiles'])
        assert all(entry['rawFourChannelsRetained'] and entry['paintedRaster'] and entry['pixel']['sha256'] in subset_pins.values() for entry in snapshot_ui['cases'])
        assert all(entry['sha256'] in subset_pins.values() and entry['pinnedSourcesIdentical'] and entry['outputBytesIdentical'] and entry['planIdentical'] for entry in snapshot_ui['createdByUi'])
        offline_subset['desktopFrontend'] = {key: snapshot_ui[key] for key in ['status', 'scope', 'renderer', 'frontendFiles', 'readOnly', 'nativeWindowTested', 'usedUserDesktop', 'createdByUi', 'compactSourceAccess']}
        evidence_reports.append(snapshot_ui_file)
complete = (all(value['status'] == 'passed' for value in [native, independent, ui, mcp])
            and len(originals) == 3 and len(outputs) == 5)
if complete:
    assert len(accepted_ids) == 8 and len(ui['cases']) == 8 and len(ui['createdByUi']) == 5 and not ui['readOnly']
    expected_processing = {(entry['group'], entry['case'], entry['sha256']) for entry in outputs}
    assert {(entry['group'], entry['case'], entry['sha256']) for entry in ui['createdByUi']} == expected_processing
    assert all(entry['pinnedSourcesIdentical'] and entry['outputBytesIdentical'] and entry['planIdentical'] for entry in ui['createdByUi'])
    assert len(mcp['cases']) == 16 and mcp['pixelsCompared'] == 80 and mcp['disconnectedAdapterRejected']
    expected_modes = {'direct read-only; rejected upstream proxy', 'loopback read-only; rejected upstream proxy'}
    assert {entry['mode'] for entry in mcp['cases']} == expected_modes
    for mode in expected_modes:
        assert {entry['jobId'] for entry in mcp['cases'] if entry['mode'] == mode} == accepted_ids
    restart = native['restart']
    assert restart['projectsRestored'] == 5 and restart['completedFilesRestored'] == 8 and restart['thumbnailEntriesUnchanged']
    assert {entry['jobId'] for entry in restart['hits']} == accepted_ids
receipt = {'schema': 'geod-naip-resolution-verification/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
           'status': 'passed' if complete else 'partial', 'nativeBinarySha256': native['nativeBinarySha256'],
           'scope': 'Actual official public NAIP original COGs and native same-grid four-channel area results; no installer or release.',
           'evidenceReports': evidence_reports,
           'catalogueReplayIsNotOriginalFileAcceptance': True, 'signaturesPersisted': False,
           'originals': originals, 'outputs': outputs,
           'independent': {'tools': independent['tools'], 'fourChannelSamplesCompared': sum(entry['fourChannelSamplesCompared'] for entry in outputs),
                           'coverageMaskPixelsCompared': sum(entry['maskPixelsCompared'] for entry in outputs),
                           'previewPixelsCompared': sum(entry['images']['preview']['pixelsCompared'] for entry in originals + outputs),
                           'thumbnailPixelsCompared': sum(entry['images']['thumbnail']['pixelsCompared'] for entry in originals + outputs)},
           'desktopFrontend': {'renderer': ui['renderer'], 'nativeWindowTested': False, 'usedUserDesktop': False,
                               'readOnly': ui['readOnly'], 'cases': [{key: entry[key] for key in ['group', 'case', 'jobId', 'width', 'locale', 'theme',
                                                                  'paintedRaster', 'rawFourChannelsRetained', 'sourceExtraSample', 'projectCards', 'projectCardHeight', 'pixel']}
                                                                   for entry in ui['cases']], 'createdByUi': ui['createdByUi'], 'errors': [], 'remoteRequests': []},
           'mcp': {'status': mcp['status'], 'readOnly': True, 'cases': mcp['cases'], 'pixelsCompared': mcp['pixelsCompared'],
                   'disconnectedAdapterRejected': mcp.get('disconnectedAdapterRejected', False)},
           'onlineDisplay': {'status': 'passed', 'wholeOriginalAcceptance': False, 'renderer': online['renderer'],
                             'readbackInstrumentation': online['readbackInstrumentation'], 'usedUserDesktop': False,
                             'cases': [{key: entry[key] for key in ['group', 'actualItemId', 'gridSpacing', 'width', 'locale', 'theme',
                                        'catalog', 'cog', 'rasterPixels', 'restoredGridWaitGuard']} for entry in online['cases']]},
           'restart': native.get('restart'), 'offline1mSubset': offline_subset, 'pending': [] if complete else [
               'Complete 0.3 m original and its single/polygon outputs, independent pixels, UI and MCP.',
               'Complete all-file offline restart and unchanged persistent thumbnail cache validation.',
               'Full-matrix direct MCP and disconnection test; processing buttons for both actual resolution variants in the production renderer.'
           ],
           'limitations': ['No installed WebView manual acceptance.', 'No NIR/false-colour display, reflectance calibration or general reprojection.',
                           'Two-date identifiers retain their official identity; the second date is not labelled as another acquisition date.']}
destination = Path('prototype/qa/naip-resolution-verification.json')
destination.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
print(json.dumps({'status': receipt['status'], 'originals': len(originals), 'outputs': len(outputs), 'samples': receipt['independent']['fourChannelSamplesCompared']}))
