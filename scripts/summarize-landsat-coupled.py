"""Publish only mutually bound real Landsat coherent-selection receipts."""
import argparse, hashlib, json, shutil
from datetime import datetime, timezone
from pathlib import Path

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()

def validate_receipts(native,mcp,ui,cache,native_hash):
    receipts=[native,mcp,ui,cache]
    assert all(r['status']=='passed' and r['nativeBinarySha256']==native['nativeBinarySha256'] for r in receipts)
    assert all(r['nativeReceiptSha256']==native_hash for r in [mcp,ui,cache])
    matrix={(label,policy,snow) for label in ['fallback','fallback-polygon','mosaic','polygon'] for policy,snow in [('cloud_free',False),('cloud_free_conservative',False),('cloud_free_conservative',True)]}
    matrix|={('landsat8-original',policy,snow) for policy,snow in [('none',False),('cloud_free',False),('cloud_free_conservative',False),('cloud_free_conservative',True)]}
    assert len(native['cases'])==16 and {(c['case'],c['policy'],c['excludeSnow']) for c in native['cases']}==matrix
    assert native['sourceFilesUnchanged']==15 and native['restartInspections']==16
    assert len(native['controls'])==4 and all(not c['queued'] for c in native['controls'])
    for c in native['cases']:
        job=c['job'];assert job['status']=='succeeded' and job['rgbOutput'].get('qualityMask')==c['counts']
        assert c['allDnCompared']==3*job['rgbSpec']['grid']['width']*job['rgbSpec']['grid']['height']
        if c['case']=='landsat8-original':
            assert job['rgbSpec']['sources'][0]['itemId'].startswith('LC08')
            assert (not job['rgbSpec'].get('qualityMask')) if c['policy']=='none' else job['rgbSpec']['qualityMask']['schemaVersion']=='geod-landsat-rgb-mask/v1'
        else:
            assert job['rgbSpec']['qualityMask']['schemaVersion']=='geod-landsat-rgb-mask/v2'
            assert sum(c['counts']['coupled']['sceneValidPixels'])==job['rgbOutput']['commonValidPixels']
            if c['case'].startswith('fallback'):assert c['counts']['coupled']['fallbackPixels']>0
    assert len(mcp['cases'])==2 and {c['mode'] for c in mcp['cases']}=={'direct','loopback'}
    assert mcp['writesDeniedInReadOnlyMode'] and mcp['invalidPolicyAndDuplicateFlagsRejected'] and mcp['cleanProtocolEof']
    assert next(c for c in mcp['cases'] if c['mode']=='loopback')['reconnected']
    assert len(ui['cases'])==3 and {c['kind'] for c in ui['cases']}=={'fallback','fallback-polygon','polygon'}
    assert not ui.get('diagnostic') and not ui['errors'] and not ui['remoteRequests'] and ui['nativeWindowTested'] is False
    assert ui['createdByUi']['job']['rgbSpec']['qualityMask']['schemaVersion']=='geod-landsat-rgb-mask/v2'
    assert ui['preflightReadRecovery']['latestSelectedRulesRecovered'] and ui['preflightReadRecovery']['editedNamePreserved']
    for c in ui['cases']:
        assert c['actualDraw']['allSourceRgbaIdentical'] and not c['horizontalOverflow']
        details=c['sourceDetails'];assert details['originalLinks']==details['pinnedOriginals']==5*len(c['mask']['coupled']['scenes'])
        assert details['displayedQualityCounts']=={k:c['counts'][k] for k in ['rejectedPixels','removedValidPixels']}
        reference=next(v for v in native['cases'] if v['case']==c['kind'] and v['policy']=='cloud_free_conservative' and v['excludeSnow'])
        assert c['counts']==reference['counts'] and c['mask']==reference['job']['rgbSpec']['qualityMask']
        thumb=next(t['thumbnail'] for t in cache['cases'] if t['id']==reference['job']['id'])
        assert c['libraryThumbnail']['complete'] and c['libraryThumbnail']['width']==thumb['width'] and c['libraryThumbnail']['height']==thumb['height']
        assert c['libraryThumbnail']['pngSha256']==thumb['pngSha256']
    assert len(cache['cases'])==16 and cache['allParentsAbsent'] and cache['restarted'] and len(cache['controls'])==3
    assert all(c['cacheBytesFileIdentityCreationUnchanged'] and c['readWithoutParents'] for c in cache['cases'])
    assert {c['id'] for c in cache['cases']}=={c['job']['id'] for c in native['cases']}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--qa',required=True);p.add_argument('--offline',required=True);p.add_argument('--snapshot',required=True)
    p.add_argument('--source',default='.verification/landsat-coupled-sources-20261004');p.add_argument('--output',default='prototype/qa/landsat-coupled-verification.json');a=p.parse_args()
    workspace=Path.cwd().resolve();qa,offline,snapshot,source=(Path(v).resolve() for v in [a.qa,a.offline,a.snapshot,a.source])
    for folder in [qa,offline,source]:assert folder.parent==workspace/'.verification' and folder.name.startswith('landsat-coupled-')
    assert snapshot.parent==workspace/'.verification' and snapshot.name.startswith('renderer-landsat-coupled-')
    paths={'source':source/'source-verification.json','native':qa/'native-verification.json','mcp':qa/'mcp-verification.json','ui':qa/'ui-verification.json','cache':offline/'cache-verification.json','gate':qa/'summary-controls-verification.json'}
    r={k:json.loads(p.read_text(encoding='utf-8')) for k,p in paths.items()};native,mcp,ui,cache=(r[k] for k in ['native','mcp','ui','cache'])
    validate_receipts(native,mcp,ui,cache,sha(paths['native']))
    gate=r['gate'];assert gate['status']=='passed' and len(gate['controls'])==11 and all(c['rejected'] for c in gate['controls'])
    assert gate['receipts']=={paths[k].name:sha(paths[k]) for k in ['native','mcp','ui','cache']}
    assert r['source']['status']=='passed' and len(r['source']['originals'])==15 and len(r['source']['reusedOriginals'])==9
    assert native['sourceReceiptSha256']==sha(paths['source'])
    assert sha(native['nativeBinary'])==native['nativeBinarySha256']
    for c in native['cases']:
        assert sha(c['job']['outputPath'])==c['job']['sha256'] and sha(c['package']['path'])==c['package']['sha256']
    for c in mcp['cases']:assert sha(c['job']['outputPath'])==c['job']['sha256'] and sha(c['package']['path'])==c['package']['sha256']
    assert sha(ui['createdByUi']['job']['outputPath'])==ui['createdByUi']['job']['sha256']
    for j in r['source']['originals']:assert sha(j['outputPath'])==j['sha256'] and Path(j['outputPath']).stat().st_size==j['bytesDownloaded']
    frozen=json.loads((snapshot/'receipt.json').read_text(encoding='utf-8'));assert len(frozen['distFiles'])>=650
    assert len({f['path'] for f in frozen['distFiles']})==len(frozen['distFiles'])
    assert frozen['uiReceiptSha256']==sha(paths['ui'])
    for f in frozen['distFiles']:
        assert sha(snapshot/'dist'/f['path'])==f['sha256'] and sha(workspace/'prototype/dist'/f['path'])==f['sha256']
        assert (snapshot/'dist'/f['path']).stat().st_size==f['bytes']
    for file,digest in ui['resources'].items():assert sha(workspace/file)==digest
    assert sha(frozen['desktop']['path'])==frozen['desktop']['sha256'] and Path(frozen['desktop']['path']).stat().st_size==frozen['desktop']['bytes']
    prior=workspace/'.verification/renderer-landsat-rgb-mask-accepted-20261004';previous=json.loads((prior/'receipt.json').read_text(encoding='utf-8'))
    for f in previous['distFiles']:assert sha(prior/'dist'/f['path'])==f['sha256']
    assert sha(previous['desktop']['path'])==previous['desktop']['sha256']
    assert sha(workspace/'target/debug/geod-runtime.exe')=='3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b'
    evidence=qa/'evidence';evidence.mkdir(exist_ok=True);proof={}
    for name,path in paths.items():
        digest=sha(path);target=evidence/f'{name}-{digest}.json'
        if not target.exists():shutil.copy2(path,target)
        assert sha(target)==digest;proof[name]={'snapshot':str(target),'sha256':digest}
    rows=[]
    for c in native['cases']:
        j=c['job'];rows.append({k:c[k] for k in ['case','policy','excludeSnow','counts','preview','outsideOrHolePixels']}|{'id':j['id'],'sha256':j['sha256'],'grid':j['rgbSpec']['grid'],'samplesSha256':j['rgbOutput']['samplesSha256'],'qualitySpec':j['rgbSpec'].get('qualityMask'),'packageSha256':c['package']['sha256']})
    result={'schema':'geod-landsat-coupled-verification/v1','status':'passed','checkedAt':datetime.now(timezone.utc).isoformat(),'nativeFinishedAt':native['finishedAt'],
        'scope':'Real Landsat 8 same-scene RGB plus Landsat 8/9 coherent multi-scene quality selection, with whole RGB triplets retained together',
        'definition':native['definition'],'evidence':proof,'nativeBinary':{'path':native['nativeBinary'],'sha256':native['nativeBinarySha256']},
        'sources':{'newDownloads':6,'reusedOriginals':9,'originals':[{k:j[k] for k in ['id','itemId','assetKey','href','bytesDownloaded','sha256']} for j in r['source']['originals']], 'catalog':r['source']['catalog']},
        'native':{'outputs':16,'allDnCompared':sum(c['allDnCompared'] for c in native['cases']),'previewRgbaPixelsCompared':sum(c['preview']['rgbaPixelsCompared'] for c in native['cases']),
            'rawPixelsCompared':sum(len(c['pixels']) for c in native['cases']),'cases':rows,'controls':native['controls'],'restartInspections':16,'sourceFilesUnchanged':15},
        'mcp':{'outputs':2,'modes':['direct','loopback'],'allDnCompared':sum(c['samplesCompared'] for c in mcp['cases']),'rawPixelsCompared':sum(len(c['pixels']) for c in mcp['cases']),
            'writesDeniedInReadOnlyMode':True,'loopbackReconnectPassed':True},
        'ui':{'cases':ui['cases'],'createdByUi':{'id':ui['createdByUi']['job']['id'],'sha256':ui['createdByUi']['job']['sha256']},'resources':ui['resources'],'preflightReadRecovery':ui['preflightReadRecovery'],
            'drawnSourceRgbaPixelsCompared':sum(c['actualDraw']['width']*c['actualDraw']['height'] for c in ui['cases']),'renderer':ui['renderer'],'nativeWindowTested':False},
        'cache':{'filesRestored':16,'missingParentJobs':len(cache['missingParentJobs']),'thumbnailRgbaPixelsCompared':sum(c['thumbnail']['rgbaPixelsCompared'] for c in cache['cases']),
            'cacheEntriesReused':16,'controls':cache['controls'],'cases':cache['cases'],'allParentsAbsent':True,'restarted':True},
        'rendererSnapshot':{'root':str(snapshot),'receiptSha256':sha(snapshot/'receipt.json'),'files':len(frozen['distFiles']),'desktop':frozen['desktop']},
        'summaryControls':gate,
        'boundaries':['The six new public RGB downloads are actual native application downloads. Existing nine originals and prior receipts remain unchanged.',
            'Same UTM CRS and integer-aligned 30 metre grids only; no reprojection or resampling.',
            'Quality flag selection does not establish atmospheric accuracy; aerosol/temperature/other quality layers remain separate work.',
            'Synthetic flag, saturation, unsigned extreme, incomplete channel and Point/Area controls are separately labelled Rust tests.',
            'Built production renderer under exact desktop CSP and native bridge is not a native WebView/window test.',
            'NASA/Copernicus in-app account entries exist; positive production authorization/protected-file acceptance remain pending.']}
    output=Path(a.output).resolve();assert output.is_relative_to(workspace/'prototype/qa');dump=lambda p,d:p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    dump(output,result);print(json.dumps({'status':'passed','outputs':16,'dnSamples':result['native']['allDnCompared'],'rendererFiles':len(frozen['distFiles']),'desktopSha256':frozen['desktop']['sha256']}))

if __name__=='__main__':main()
