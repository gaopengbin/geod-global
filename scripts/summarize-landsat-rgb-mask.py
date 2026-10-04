"""Save current Landsat RGB acceptance only after mutually bound native,
independent GDAL, stdio/loopback MCP, built UI and parent-free cache evidence.
"""
import argparse, hashlib, importlib.util, json
from datetime import datetime, timezone
from pathlib import Path
spec=importlib.util.spec_from_file_location('evidence',Path(__file__).with_name('summarize-landsat-quality-processing.py'))
evidence=importlib.util.module_from_spec(spec);spec.loader.exec_module(evidence)
sha,load,preserve,managed=evidence.sha,evidence.load,evidence.preserve,evidence.managed
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--qa',required=True);p.add_argument('--cache',required=True);p.add_argument('--snapshot',required=True);p.add_argument('--output',default='prototype/qa/landsat-rgb-mask-verification.json');a=p.parse_args()
    workspace=Path.cwd().resolve();qa,cache,snapshot=(Path(v).resolve() for v in [a.qa,a.cache,a.snapshot])
    assert all(v.parent==workspace/'.verification' for v in [qa,cache,snapshot]);assert qa.name.startswith('landsat-rgb-mask-') and cache.name.startswith('landsat-rgb-mask-') and snapshot.name.startswith('renderer-landsat-rgb-mask-')
    files={name:qa/f'{name}-verification.json' for name in ['native','mcp','ui']};files['cache']=cache/'cache-verification.json'
    records={name:load(path) for name,path in files.items()};native,mcp,ui,cached=(records[name] for name in ['native','mcp','ui','cache'])
    assert all(r['status']=='passed' for r in records.values())
    native_sha=sha(files['native']);binary_sha=native['nativeBinarySha256'];assert sha(managed(qa,native['nativeBinary']))==binary_sha
    assert all(r['nativeBinarySha256']==binary_sha and r['nativeReceiptSha256']==native_sha for r in [mcp,ui,cached])
    expected={(kind,policy,snow) for kind in ['original','single','polygon'] for policy,snow in [(None,False),('cloud_free',False),('cloud_free_conservative',False),('cloud_free_conservative',True)]}
    assert len(native['cases'])==12 and {(r['case'],r['policy'],r['excludeSnow']) for r in native['cases']}==expected
    assert native['originalSourceFilesUnchanged'] and native['restartRulesAndResultsUnchanged'] and len(native['controls'])==4
    assert native['definition']==evidence.DEFINITION and len(native['originals'])==5
    previous=load(workspace/'prototype/qa/landsat-quality-processing-verification.json')
    assert previous['status']=='passed' and previous['evidence']['native']['sha256']==native['sourceReceiptSha256']
    for job in native['originals']:
        path=managed(qa,job['outputPath']);assert sha(path)==job['sha256'] and path.stat().st_size==job['bytesDownloaded']
        assert job['kind']=='download' and job['status']=='succeeded' and job['error'] is None and job['itemId']=='LC09_L2SP_044034_20250628_02_T1'
        assert '?' not in job['href'] and '#' not in job['href']
    for entry in native['cases']:
        job=entry['job'];g=job['rgbSpec']['grid'];mask=job['rgbSpec'].get('qualityMask');counts=job['rgbOutput'].get('qualityMask');pixels=g['width']*g['height']
        assert job['kind']=='raster_rgb' and job['status']=='succeeded' and job['error'] is None
        # CLI run waits for worker settlement, then serializes Job without the
        # transient HTTP flag. Resumed GET snapshots retain that flag.
        if 'settled' in job:assert job['settled'] is True
        assert job['rgbSpec']==entry['plan']['spec'] and entry['allDnCompared']==pixels*3
        assert job['rgbSpec']['profile']['product']=='landsat-c2-l2' and job['rgbSpec']['profile']['nodata']==0 and job['rgbSpec']['profile']['signed'] is False
        assert sha(managed(qa,job['outputPath']))==job['sha256'] and sha(managed(qa,entry['package']['path']))==entry['package']['sha256']
        assert sha(qa/f"{entry['case']}-{entry['policy'] or 'unmasked'}-{entry['excludeSnow']}-preview.png")==entry['preview']['pngSha256']
        if entry['policy']:
            assert mask['schemaVersion']=='geod-landsat-rgb-mask/v1' and mask['policy']==entry['policy'] and mask['excludeSnow']==entry['excludeSnow'] and mask['definition']==native['definition']
            assert [p['band'] for p in mask['sources']]==['qa_pixel','qa_radsat']
            assert counts==entry['counts'] and counts['examinedPixels']==pixels and counts['inputCommonValidPixels']-counts['removedValidPixels']==job['rgbOutput']['commonValidPixels']
        else:assert not mask and not counts and entry['counts'] is None
    assert len(native['clipChecks'])==2 and {c['case'] for c in native['clipChecks']}=={'single','polygon'}
    assert all(len(c['proofs'])==5 and len(c['layers'])==5 for c in native['clipChecks'])
    assert len(mcp['cases'])==2 and {c['mode'] for c in mcp['cases']}=={'direct','loopback'} and mcp['writesDeniedInReadOnlyMode'] and mcp['invalidPolicyAndDuplicateFlagsRejected']
    assert mcp['cleanProtocolEof'] and next(c for c in mcp['cases'] if c['mode']=='loopback')['reconnected']
    for c in mcp['cases']:assert sha(managed(qa,c['job']['outputPath']))==c['job']['sha256']
    assert not ui['diagnostic'] and len(ui['cases'])==3 and {c['kind'] for c in ui['cases']}=={'original','single','polygon'}
    assert ui['errors']==[] and ui['remoteRequests']==[] and ui['nativeWindowTested'] is False
    assert all(c['actualDraw']['allSourceRgbaIdentical'] and not c['horizontalOverflow'] and c['sourceDetails']['qualityPins']==2
        and c['sourceDetails']['displayedQualityCounts']=={k:c['counts'][k] for k in ['rejectedPixels','removedValidPixels']} for c in ui['cases'])
    assert ui['busyWorker']['id']==ui['createdByUi']['job']['id']
    assert any(r['id']==ui['busyWorker']['id'] and r['status']==400 and r['error']=='Preview worker is busy. Try again shortly.' for r in ui['thumbnailResponses'])
    created=ui['createdByUi']['job'];reference=next(c['job'] for c in native['cases'] if c['case']=='original' and c['policy']=='cloud_free_conservative' and c['excludeSnow'])
    assert created['status']=='succeeded' and created['rgbOutput']==reference['rgbOutput'] and sha(managed(qa,created['outputPath']))==created['sha256']
    assert len(cached['cases'])==12 and cached['allParentsAbsent'] and cached['restarted'] and len(cached['controls'])==3
    assert {c['id'] for c in cached['cases']}=={c['job']['id'] for c in native['cases']}
    assert all(c['readWithoutParents'] and c['cacheBytesFileIdentityCreationUnchanged'] for c in cached['cases'])
    for shown in ui['cases']:
        reference=next(c for c in native['cases'] if c['case']==shown['kind'] and c['policy']==shown['mask']['policy'] and c['excludeSnow']==shown['mask']['excludeSnow'])
        stored=next(c for c in cached['cases'] if c['id']==reference['job']['id'])['thumbnail']
        image=shown['libraryThumbnail']
        assert image['complete'] and all(image[k]==stored[k] for k in ['width','height','pngSha256'])
        assert any(r['id']==shown['id'] and r['status']==200 and r['sha256']==shown['sha256'] for r in ui['thumbnailResponses'])
    frozen,file_count=evidence.verify_renderer(snapshot,workspace/'prototype/dist');assert frozen['uiReceiptSha256']==sha(files['ui'])
    for filename,digest in ui['resources'].items():assert sha(workspace/filename)==digest
    archive=qa/'accepted-evidence';archive.mkdir(exist_ok=True);references={name:{'path':str(file),**preserve(file,archive,name)} for name,file in files.items()}
    for image in (qa/'ui').glob('*.png'):preserve(image,archive,image.stem)
    previous_renderers=[]
    for folder in ['renderer-modis-coupled-20261004','renderer-landsat-quality-20261004','renderer-landsat-quality-processing-accepted-20261004']:
        path=workspace/'.verification'/folder;_,count=evidence.verify_renderer(path)
        previous_renderers.append({'snapshot':str(path),'files':count,'receiptSha256':sha(path/'receipt.json')})
    canonical=workspace/'target/debug/geod-runtime.exe';assert sha(canonical)==evidence.CANONICAL_RUNTIME
    result={'schema':'geod-landsat-rgb-mask-verification/v1','status':'passed','verifiedAt':datetime.now(timezone.utc).isoformat(),'scope':native['scope'],'definition':native['definition'],
        'nativeBinary':{'path':native['nativeBinary'],'sha256':binary_sha},'evidence':references,
        'originals':[{'id':j['id'],'itemId':j['itemId'],'key':j['assetKey'],'bytes':j['bytesDownloaded'],'sha256':j['sha256'],'href':j['href']} for j in native['originals']],
        'native':{'outputs':12,'allDnCompared':sum(c['allDnCompared'] for c in native['cases']),'previewRgbaPixelsCompared':sum(c['preview']['rgbaPixelsCompared'] for c in native['cases']),'rawQueriesCompared':sum(len(c['pixels']) for c in native['cases']),'cases':[{'case':c['case'],'policy':c['policy'],'excludeSnow':c['excludeSnow'],'id':c['job']['id'],'sha256':c['job']['sha256'],'bytes':c['job']['bytesDownloaded'],'grid':c['job']['rgbSpec']['grid'],'counts':c['counts'],'preview':c['preview'],'packageSha256':c['package']['sha256']} for c in native['cases']],'controls':native['controls'],'clipChecks':native['clipChecks'],'originalSourcesUnchanged':True,'restartPassed':True},
        'mcp':{'modes':[c['mode'] for c in mcp['cases']],'allDnCompared':sum(c['samplesCompared'] for c in mcp['cases']),'rawQueriesCompared':sum(len(c['pixels']) for c in mcp['cases']),'writesDeniedInReadOnlyMode':True,'cleanProtocolEof':True,'loopbackDisconnectReconnectPassed':True},
        'ui':{'cases':ui['cases'],'createdByUi':{'id':created['id'],'sha256':created['sha256']},'drawnSourceRgbaPixelsCompared':sum(c['actualDraw']['width']*c['actualDraw']['height'] for c in ui['cases']),'resources':ui['resources'],'renderer':ui['renderer'],'nativeWindowTested':False,'errors':[],'remoteRequests':[],'spatialCanvasAllPixelsCompared':False,'libraryThumbnailsCompared':3,'realBusyWorkerRecovery':ui['busyWorker'],'handledBusyReplies':sum(r['status']==400 and r.get('error')=='Preview worker is busy. Try again shortly.' for r in ui['thumbnailResponses'])},
        'cache':{'filesRestored':12,'missingParentJobs':len(cached['missingParentJobs']),'thumbnailRgbaPixelsCompared':sum(c['thumbnail']['rgbaPixelsCompared'] for c in cached['cases']),'cacheEntriesReused':12,'controls':cached['controls']},
        'acceptedRenderer':{'snapshot':str(snapshot),'files':file_count,'receiptSha256':sha(snapshot/'receipt.json')},'developmentDesktopBuild':{**frozen['desktop'],'customProtocol':True,'nativeWindowTested':False,'installerBuilt':False},'previousRenderersPreserved':previous_renderers,'canonicalRuntimeUnchanged':True,
        'boundaries':['Previously downloaded complete public files are reused; this cohort makes no new provider download.','Real Landsat 9 same-scene originals and clips; Landsat 8 RGB and coherent multi-scene quality selection remain pending.','Exact flag selection does not establish atmospheric accuracy or include SR_QA_AEROSOL.','Synthetic flag and high unsigned DN controls remain separately identified as unit tests.','Headless production renderer under exact desktop CSP is not native WebView/window acceptance.','NASA/Copernicus account entries exist; successful production authorization and protected original downloads remain pending.']}
    target=Path(a.output).resolve();assert target.parent==workspace/'prototype/qa';target.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'status':'passed','output':str(target),'nativeOutputs':12,'allDnCompared':result['native']['allDnCompared'],'uiCases':3,'offlineFiles':12}))
if __name__=='__main__':main()
