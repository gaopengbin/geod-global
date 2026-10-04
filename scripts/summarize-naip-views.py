"""Bind actual NAIP channel/mask and production-renderer evidence to one summary."""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('naip-views-')
snapshots = root / 'evidence-snapshots'
snapshots.mkdir(exist_ok=True)


def read(relative):
    file = root / relative
    if not file.exists():
        return None, None
    raw = file.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    snapshot = snapshots / (digest + '.json')
    snapshot.write_bytes(raw)
    return json.loads(raw), {'file': relative, 'sha256': digest, 'snapshot': 'evidence-snapshots/' + snapshot.name}


science, science_pin = read('views-verification.json')
assert science and science['schema'] == 'geod-naip-views/v1'
ui, ui_pin = read('ui/views-verification.json')
bands = {'rgb': [1, 2, 3], 'cir': [4, 1, 2], 'nir': [4, 4, 4]}
for receipt in ['sourceReceipt', 'sourceIndependentReceipt']:
    pin = science[receipt]
    assert hashlib.sha256((root / pin['snapshot']).read_bytes()).hexdigest() == pin['sha256']
source = json.loads((root / science['sourceReceipt']['snapshot']).read_bytes())
legacy_060 = science.get('cohort') == '060'
if legacy_060:
    historical = json.loads((root / science['sourceIndependentReceipt']['snapshot']).read_bytes())
    assert source['nativeOriginal']['status'] == 'succeeded'
    accepted = [{'group': '0.6m', 'case': 'original', 'job': {'id': item['jobId'], 'sha256': item['sha256']}}
                for item in historical['sources']]
    accepted += [{'group': '0.6m', 'case': kind, 'job': {'id': item['jobId'], 'sha256': item['sha256']}}
                 for kind, item in [('single', historical['singleSceneClip']), ('polygon', historical)]]
    assert science['acquisitionNativeBinarySha256'] is None
    file_count = 4
else:
    accepted = source['originals'] + source['outputs']
    assert science['acquisitionNativeBinarySha256'] == source['nativeBinarySha256']
    file_count = 8
pins = {entry['job']['id']: entry for entry in accepted}
assert len(pins) == file_count
checked = {}
for entry in science['entries']:
    original = pins[entry['jobId']]
    assert entry['jobId'] not in checked and entry['sha256'] == original['job']['sha256']
    assert (entry['group'], entry['case']) == (original['group'], original.get('case', 'original'))
    assert {view['view'] for view in entry['views']} == set(bands) and len(entry['views']) == 3
    for view in entry['views']:
        assert view['displayBands'] == bands[view['view']]
        assert view['pixelsCompared'] > 0 and view['exactChannelsEqual'] and view['exactIndependentAlphaEqual']
        assert hashlib.sha256((root / view['pngFile']).read_bytes()).hexdigest() == view['pngSha256']
    checked[entry['jobId']] = entry
complete = science['status'] == 'passed' and len(checked) == file_count
if complete:
    assert science['sourceBytesUnchanged'] and science['unknownViewRejected']
    assert science['displaySelectionLeavesDefaultThumbnailCacheUnchanged']
    assert science['restart'] == {'offline': True, 'filesRestored': file_count, 'unchangedRgbThumbnails': file_count, 'unchangedCacheFileIdentityAndBytes': True}
seen_ui = set()
subset_pin = None
if ui:
    assert ui['nativeBinarySha256'] == science['nativeBinarySha256']
    assert ui['independentReceiptSha256'] == science_pin['sha256']
    assert ui['usedUserDesktop'] is False and ui['nativeWindowTested'] is False
    assert ui['errors'] == ui['remoteRequests'] == []
    assert ui.get('frontendFiles')
    renderer_root = Path('prototype/dist').resolve()
    for entry in ui['frontendFiles']:
        file = (renderer_root / entry['file']).resolve()
        assert file.is_relative_to(renderer_root)
        assert hashlib.sha256(file.read_bytes()).hexdigest() == entry['sha256']
    if ui.get('acceptedSubset'):
        subset, subset_pin = read(ui['acceptedSubset']['file'])
        assert subset_pin['sha256'] == ui['acceptedSubset']['sha256']
        assert subset['schema'] == 'geod-naip-views-ui-subset/v1' and subset['status'] == 'passed'
        assert subset['nativeBinarySha256'] == science['nativeBinarySha256']
        assert subset['independentReceiptSha256'] == science_pin['sha256']
        assert len(subset['entries']) == ui['resumedAcceptedCases']
        assert subset['entries'] == ui['entries'][:ui['resumedAcceptedCases']]
        assert hashlib.sha256((root / subset['sourceReport']['file']).read_bytes()).hexdigest() == subset['sourceReport']['sha256']
        assert len(subset['frames']) == ui['resumedAcceptedCases'] * 3
        for frame in subset['frames']:
            assert hashlib.sha256((root / frame['file']).read_bytes()).hexdigest() == frame['sha256']
    for entry in ui['entries']:
        id = entry['jobId']
        assert id in checked and id not in seen_ui
        assert entry['sha256'] == checked[id]['sha256']
        assert (entry['group'], entry['case']) == (checked[id]['group'], checked[id]['case'])
        assert entry['rawPixelUnchanged'] and entry['panAndZoomRetained'] and entry['cacheHitForThreeRevisits']
        assert entry['exactCirChannelsEqual'] and entry['exactNirGrayEqual'] and entry['exactAlphaEqual']
        assert entry['renderedCanvasPixelsCompared'] > 0
        assert entry['pixel']['sha256'] == entry['sha256'] and len(entry['pixel']['values']) == 3
        assert entry['pixel']['label'] == 'RGB + NIR' and isinstance(entry['pixel']['nearInfrared'], int)
        assert entry['previews'] == [{'view': v['view'], 'sha256': v['pngSha256']} for v in checked[id]['views']]
        assert all(call['method'] == 'GET' for call in entry['calls'])
        seen_ui.add(id)
    complete = complete and ui['status'] == 'passed' and seen_ui == set(pins)
else:
    complete = False
separate_cohorts = []
separate_060 = Path('prototype/qa/naip-views-060-verification.json')
if not legacy_060 and separate_060.exists():
    separate_raw = separate_060.read_bytes()
    separate = json.loads(separate_raw)
    if separate['status'] == 'passed':
        assert separate['schema'] == 'geod-naip-views-summary/v1' and separate['cohort'] == '060'
        assert separate['nativeBinarySha256'] == science['nativeBinarySha256']
        assert separate['acquisitionNativeBinarySha256'] is None and separate['displayBands'] == bands
        assert separate['filesIndependentlyChecked'] == separate['filesCheckedInProductionRenderer'] == 4
        assert len(separate['entries']) == 4 and all(entry['group'] == '0.6m' for entry in separate['entries'])
        assert not set(pins).intersection(entry['jobId'] for entry in separate['entries'])
        assert {entry['file']: entry['sha256'] for entry in separate['frontendFiles']} == {
            entry['file']: entry['sha256'] for entry in (ui or {}).get('frontendFiles', [])}
        assert separate['offlineThumbnailCache'] == {'offline': True, 'filesRestored': 4,
            'unchangedRgbThumbnails': 4, 'unchangedCacheFileIdentityAndBytes': True}
        separate_hash = hashlib.sha256(separate_raw).hexdigest()
        separate_snapshot = snapshots / (separate_hash + '.json')
        separate_snapshot.write_bytes(separate_raw)
        separate_cohorts.append({'file': separate_060.as_posix(), 'sha256': separate_hash,
            'snapshot': 'evidence-snapshots/' + separate_snapshot.name, 'cohort': '060',
            'filesIndependentlyChecked': 4, 'filesCheckedInProductionRenderer': 4})
summary = {'schema': 'geod-naip-views-summary/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
           'qaOnly': True, 'status': 'passed' if complete else 'partial', 'provider': 'planetary-naip',
           'cohort': '060' if legacy_060 else '030-100',
           'scope': 'Actual previously accepted 0.6 m originals and aligned outputs; new RGB/CIR/NIR display acceptance, no new download.' if legacy_060 else 'Actual 0.3 m and 1 m accepted originals and aligned outputs; RGB/CIR/NIR display, not spectral analysis or a release.',
           'nativeBinarySha256': science['nativeBinarySha256'], 'acquisitionNativeBinarySha256': science['acquisitionNativeBinarySha256'],
           'evidence': {'independentChannelsAndMask': science_pin, 'productionUi': ui_pin, 'acceptedUiSubset': subset_pin,
                        'acquisition': science['sourceReceipt'], 'sourceIndependent': science['sourceIndependentReceipt'],
                        'separateCohorts': separate_cohorts},
           'displayBands': bands, 'filesIndependentlyChecked': len(checked), 'filesCheckedInProductionRenderer': len(seen_ui),
           'rgbaPreviewPixelsCompared': sum(v['pixelsCompared'] for entry in checked.values() for v in entry['views']),
           'rawFullResolutionPointsCompared': sum(len(entry['rawPixels']) for entry in checked.values()),
           'renderedCanvasPixelsCompared': sum(entry['renderedCanvasPixelsCompared'] for entry in (ui or {}).get('entries', [])),
           'frontendFiles': (ui or {}).get('frontendFiles', []),
           'offlineThumbnailCache': science.get('restart'), 'usedUserDesktop': False, 'nativeWindowTested': False,
           'entries': [{'jobId': entry['jobId'], 'sha256': entry['sha256'], 'group': entry['group'], 'case': entry['case'],
                        'views': [{key: view[key] for key in ['view', 'displayBands', 'dimensions', 'pngSha256', 'pixelsCompared', 'opaqueZeroNirPreviewPixels']} for view in entry['views']]}
                       for entry in checked.values()],
           'remaining': ([] if legacy_060 or separate_cohorts else ['0.6 m display views require separate actual-file acceptance']) + ['Spectral analysis and reflectance calibration',
                         'Other encodings/grids and general reprojection', 'Installed native WebView acceptance']}
output = Path('prototype/qa/naip-views-060-verification.json' if legacy_060 else 'prototype/qa/naip-views-verification.json')
temporary = output.with_suffix('.tmp')
temporary.write_text(json.dumps(summary, indent=2) + '\n', encoding='utf8')
temporary.replace(output)
print(json.dumps({key: summary[key] for key in ['status', 'filesIndependentlyChecked', 'filesCheckedInProductionRenderer', 'rgbaPreviewPixelsCompared']}))
