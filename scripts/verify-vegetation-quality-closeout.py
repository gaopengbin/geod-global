"""Bind actual MOD13 VI-selection files, independent checks and development UI.

This receipt gate is not a download, scientific oracle or native-window test.
It preserves earlier acceptances and never builds an installer or publishes.
"""
import argparse, copy, hashlib, json, shutil
from pathlib import Path
from datetime import datetime, timezone

p = argparse.ArgumentParser(description=__doc__)
for name in ['native', 'cache', 'ui', 'compatibility', 'root']:
    p.add_argument(name, type=Path)
a = p.parse_args()
workspace = Path.cwd().resolve()
root = a.root.resolve()
assert root.parent == workspace / '.verification' and root.name.startswith('modis-vi-quality-')

def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def require(value, message):
    if not value:
        raise ValueError(message)

def local(path):
    return Path(path.removeprefix('\\\\?\\')).resolve()

def files(directory):
    return sorted([{'path':p.relative_to(directory).as_posix(), 'sha256':sha(p)}
                   for p in directory.rglob('*') if p.is_file()], key=lambda e:e['path'])

paths = {'native':a.native / 'verification.json', 'mcp':a.native / 'mcp-verification.json',
         'cache':a.cache / 'cache-verification.json', 'ui':a.ui / 'ui/verification.json',
         'compatibility':a.compatibility / 'verification.json'}
bundle = {key:json.loads(path.read_text(encoding='utf-8')) for key,path in paths.items()}

def validate(b):
    require(all(r['status'] == 'passed' for r in b.values()), 'Every stage must pass')
    n,m,c,u,v = [b[key] for key in ['native','mcp','cache','ui','compatibility']]
    require(all(r['nativeBinarySha256'] == n['nativeBinarySha256'] for r in b.values()), 'Runtime identity differs')
    require(len(n['originals']) == 12 and len(n['outputs']) == 20, 'Twelve originals and twenty outputs required')
    require({r['name'] for r in n['cases']} == {'single','temporal','polygon','adjacent','three_scene_polygon'}, 'Case coverage differs')
    require(sum(r['dnPixelsRead'] for r in n['originals']) == 276480000, 'Original DN count differs')
    require(n['outputDnPixelsCompared'] == 3038664 and n['previewAndThumbnailPixelsCompared'] == 1698248, 'Independent comparison count differs')
    require(n['pairedSelectionsIdentical'] and len(n['negativeControls']) == 8, 'Coupling or native refusals missing')
    require(n['restart'] == {'rastersChecked':32,'cacheEntriesUnchanged':32}, 'Ordinary restart missing')
    require(all(b[k]['nativeWindowTested'] is False for k in ['native','cache','ui']), 'Native-window evidence boundary differs')
    originals = {r['job']['id']:r['job'] for r in n['originals']}
    for case in n['cases']:
        for policy in ['good','usable']:
            pair = [next(r for r in n['outputs'] if r['case'] == case['name'] and r['policy'] == policy and r['job']['assetKey'] == key) for key in ['ndvi','evi']]
            require(pair[0]['job']['mosaic']['viSelection'] == pair[1]['job']['mosaic']['viSelection'], 'Index source pins differ')
            require(pair[0]['job']['mosaicOutput']['viQuality'] == pair[1]['job']['mosaicOutput']['viQuality'], 'Index winner map or paired DN digests differ')
            for r in pair:
                j=r['job'];info=r['metadata'];result=j['mosaicOutput']['viQuality'];spec=j['mosaic']['viSelection']
                require(j['status'] == 'succeeded' and j['settled'], 'Output did not settle')
                require(info['dataType'] == 'Int16' and info['nodata'] == -3000 and info['vegetation']['scale'] == .0001, 'Scientific type or scale differs')
                require(result['countsFullResolution'] and info['vegetation']['qualitySelection'] == result, 'Full-resolution selection metadata differs')
                require('science' not in info and 'reflectance' not in info, 'Metadata families mixed')
                require(result['policy'] == policy and spec['selection'] == 'newest-qualified-complete-ndvi-evi-observation', 'Selection rule differs')
                require(r['dnPixelsCompared'] == info['width'] * info['height'], 'Incomplete output comparison')
                require(r['manifest']['viSelection'] == spec and r['manifest']['plan'] == j['mosaicOutput'], 'Provenance differs')
                for scene in spec['scenes']:
                    for key, pin in zip(['ndvi','evi','vi_quality','vi_reliability'],scene['sources']):
                        original=originals[pin['jobId']]
                        require(original['assetKey'] == key and original['itemId'] == scene['itemId'] and original['sha256'] == pin['sha256'] and original['href'] == pin['href'] and original['bytesDownloaded'] == pin['bytes'], 'Original pin differs')
            if case['name'] in ['temporal','polygon']:
                require(pair[0]['fallbackPixels'] > 0, 'Real quality-rejection fallback missing')
    require(c['originalRecordsAndFilesAbsent'] == 12 and c['derivedFilesRestored'] == c['persistentEntriesNotRegenerated'] == 20 and c['projectsAbsent'], 'Parent-free persistent restoration missing')
    require(len(c['negativeControls']) == 5, 'Cache/header refusal controls missing')
    require(len(m['cases']) == 40 and m['pixelsCompared'] == 216 and m['readOnlyWriteDeniedInBothModes'] and m['disconnectedAdapterRejected'] and m['protocolOnlyStdoutAndCleanExit'], 'MCP coverage or refusal differs')
    require(len(u['cases']) == 3 and len(u['createdByUi']) == 6 and not u['errors'] and not u['remoteRequests'], 'Actual UI processing missing')
    require({r['width'] for r in u['cases']} == {900,1024,1440} and {r['locale'] for r in u['cases']} == {'en','zh-CN'} and {r['theme'] for r in u['cases']} == {'light','dark'}, 'UI layout coverage missing')
    require(all(r['pairedSelectionsIdentical'] and r['fullResolutionStatisticsVisible'] and r['fullHeightThumbnails'] and r['paintedRaster'] for r in u['cases']), 'UI interaction or layout missing')
    for r in u['createdByUi']:
        reference=next(e for e in n['outputs'] if e['case'] == r['case'] and e['policy'] == r['policy'] and e['job']['assetKey'] == r['key'])
        require(reference['job']['sha256'] == r['sha256'] and all(r[k] for k in ['sourcePinsIdentical','outputBytesIdentical','planIdentical','displayIdentical']), 'UI result differs from scientific reference')
    require(len(v['originals']) == 6 and len(v['outputs']) == 6 and v['outputDnPixelsCompared'] == 99812, 'Previous unmasked index regression missing')

validate(bundle)
hashes = {key:sha(path) for key,path in paths.items()}
n,m,c,u,v = [bundle[key] for key in ['native','mcp','cache','ui','compatibility']]
require(all(r['nativeReceiptSha256'] == hashes['native'] for r in [m,c,u]), 'Local receipt binding differs')
for record in n['sourceReceipts']:
    path=local(record['path']);require(sha(path) == record['sha256'], 'Original-download receipt changed')
    source=json.loads(path.read_text(encoding='utf-8'));require(source['status'] == 'passed', 'Original download did not pass')
    originals={e['job']['id']:e['job'] for e in source['cases']}
    for e in n['originals']:
        j=e['job']
        if j['id'] in originals:
            require(all(j[k] == originals[j['id']][k] for k in ['assetKey','itemId','href','sha256','bytesDownloaded']), 'Downloaded original identity differs')
for r in n['originals'] + n['outputs'] + v['originals'] + v['outputs']:
    require(sha(local(r['job']['outputPath'])) == r['job']['sha256'], 'Actual original or output file changed')
    if r['job'].get('manifestPath'):
        require(local(r['job']['manifestPath']).is_file(), 'Actual provenance file missing')
previous=json.loads((workspace/'prototype/qa/modis-vegetation-verification.json').read_text(encoding='utf-8'))
old={(r['case'],r['assetKey']):r['sha256'] for r in previous['native']['outputs']}
require(all(old[(r['case'],r['job']['assetKey'])] == r['job']['sha256'] for r in v['outputs']), 'Previous NDVI/EVI output bytes changed')
require(sha(workspace/'target/debug/geod-runtime.exe') == '3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b', 'User runtime changed')
native=a.native/f"runtime-{n['nativeBinarySha256'][:16]}.exe"
require(sha(native) == n['nativeBinarySha256'], 'Frozen native program changed')
renderer_files=sorted(u['rendererFiles'],key=lambda e:e['path'])
require(files(a.ui/'renderer') == files(workspace/'prototype/dist') == renderer_files, 'Accepted renderer differs')
require(u['csp'] == json.loads((workspace/'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['app']['security']['csp'], 'Desktop CSP differs')

controls=[]
for name,change in [
    ('failed-stage',lambda b:b['native'].update(status='failed')),
    ('wrong-index-scale',lambda b:b['native']['outputs'][0]['metadata']['vegetation'].update(scale=10000)),
    ('false-preview-statistics',lambda b:b['native']['outputs'][0]['job']['mosaicOutput']['viQuality'].update(countsFullResolution=False)),
    ('changed-qa-pin',lambda b:b['native']['outputs'][0]['job']['mosaic']['viSelection']['scenes'][0]['sources'][2].update(sha256='0'*64)),
    ('partial-output-comparison',lambda b:b['native']['outputs'][0].update(dnPixelsCompared=0)),
    ('no-real-fallback',lambda b:next(e for e in b['native']['outputs'] if e['case']=='temporal').update(fallbackPixels=0)),
    ('parents-present',lambda b:b['cache'].update(originalRecordsAndFilesAbsent=0)),
    ('false-native-window',lambda b:b['ui'].update(nativeWindowTested=True)),
    ('unmatched-ui-output',lambda b:b['ui']['createdByUi'][0].update(sha256='0'*64)),
    ('inflated-mcp',lambda b:b['mcp'].update(pixelsCompared=217)),
]:
    changed=copy.deepcopy(bundle);change(changed)
    try:validate(changed)
    except (ValueError,KeyError,TypeError):controls.append(name)
    else:raise AssertionError('Invalid receipt accepted: '+name)

root.mkdir(exist_ok=False);(root/'evidence').mkdir();evidence={}
for key,path in paths.items():
    saved=root/'evidence'/f'{key}-{hashes[key]}.json';shutil.copy2(path,saved)
    require(sha(saved) == hashes[key], 'Frozen receipt differs');evidence[key]={'path':str(saved),'sha256':hashes[key]}
snapshot=workspace/'.verification/renderer-modis-vi-quality-accepted-20261004'
snapshot.mkdir(exist_ok=False);shutil.copytree(a.ui/'renderer',snapshot/'renderer')
desktop=workspace/'.verification/naip-native-target/debug/geod-global-desktop.exe';desktop_sha=sha(desktop)
frozen=snapshot/f'desktop-{desktop_sha[:16]}.exe';shutil.copy2(desktop,frozen)
require(sha(frozen) == desktop_sha and files(snapshot/'renderer') == renderer_files, 'Frozen development program differs')
gate={'status':'passed','refusedControls':controls,'canonicalRuntimeUnchanged':True,'legacyNdviEviBytesUnchanged':True,'scope':'Receipt and hash binding; not a new download or native-window acceptance'}
(root/'gate.json').write_text(json.dumps(gate,indent=2)+'\n',encoding='utf-8')
summary={'schema':'geod-modis-vi-quality-verification/v1','status':'passed','checkedAt':datetime.now(timezone.utc).isoformat(),
    'scope':'MOD13Q1/MYD13Q1 v061 Planetary Computer same-scene NDVI/EVI/QA/reliability selection',
    'definition':'https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf','evidence':evidence,
    'nativeBinary':{'path':str(native.resolve()),'sha256':n['nativeBinarySha256']},
    'sources':{'previouslyDownloadedOriginalsReused':12,'newDownloads':0,'bytes':sum(e['job']['bytesDownloaded'] for e in n['originals']),'downloadReceipts':n['sourceReceipts'],
        'originals':[{k:e['job'][k] for k in ['id','itemId','assetKey','href','sha256','bytesDownloaded']} for e in n['originals']]},
    'native':{'originalDnRead':276480000,'outputDnCompared':n['outputDnPixelsCompared'],'previewAndThumbnailRgbaPixelsCompared':n['previewAndThumbnailPixelsCompared'],
        'independentRawPointsCompared':sum(len(e['pixels']) for e in n['outputs']),'pairedSelectionAndIndexDigestsIdentical':True,'restart':n['restart'],
        'outputs':[{'id':e['job']['id'],'case':e['case'],'policy':e['policy'],'assetKey':e['job']['assetKey'],'sha256':e['job']['sha256'],'allDnCompared':e['dnPixelsCompared'],'fullResolutionSelection':e['job']['mosaicOutput']['viQuality'],'maskedPixels':e['maskedPixels']} for e in n['outputs']],
        'negativeControls':[e['name'] for e in n['negativeControls']]},
    'mcp':{'modes':['direct','loopback adapter'],'rawPointsCompared':m['pixelsCompared'],'writeDenied':True,'disconnectedAdapterRejected':True},'cache':c,
    'ui':{'actualNativeProcessingButtons':6,'cases':[{k:r[k] for k in ['case','policy','width','locale','theme','pairedSelectionsIdentical','fullResolutionStatisticsVisible','allSceneOriginalLinks','uniformCardHeight','fullHeightThumbnails','paintedRaster','pixelInteraction']} for r in u['cases']],'nativeWindowTested':False,'usedUserDesktop':False},
    'rendererSnapshot':{'root':str(snapshot),'files':renderer_files,'desktop':{'path':str(frozen),'bytes':frozen.stat().st_size,'sha256':desktop_sha}},'summaryControls':gate,
    'boundaries':['GeoD good/usable thresholds are product choices based on NASA QA definitions, not NASA-recommended screening policies.',
        'Composite-start ordering does not expose the selected per-pixel observation date. Ancillary layers still process independently.',
        'One selected index is exported per action; identical pinned inputs, policy and area yield identical paired selections for NDVI/EVI.',
        'Original signed DN, scale, NoData and same sinusoidal grid are retained. No resampling, reprojection or averaging.',
        'Winner-map and both index digests are stored; no winner ordinal, coupled QA/reliability/day raster or complete original NASA HDF is exported.',
        'Preview statistics stay sampled; selection statistics cover all output pixels.',
        'Hidden production-renderer/API checks do not accept a native WebView or installed application. Protected originals still await accounts. No installer or publication.']}
destination=workspace/'prototype/qa/modis-vi-quality-verification.json'
require(not destination.exists(), 'Preserve any earlier public receipt')
destination.write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(root/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'status':'passed','refusedControls':len(controls),'desktop':summary['rendererSnapshot']['desktop'],'rendererFiles':len(renderer_files),'publicReceipt':str(destination)}))
