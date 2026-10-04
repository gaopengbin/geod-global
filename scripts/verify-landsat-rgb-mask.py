"""Real, offline Landsat 8/9 C2 RGB screening acceptance. Reuses complete,
hash-pinned official downloads, creates a fresh private workspace, and compares
every DN, mask/count, calibrated grid, preview and ZIP with Rasterio/GDAL/NumPy.
Synthetic flags are covered separately by Rust tests, never by this receipt.
"""
import argparse, atexit, base64, copy, hashlib, importlib.util, io, json, shutil, subprocess, time, urllib.error, urllib.request, zipfile
from datetime import datetime, timezone
from pathlib import Path
import numpy as np
import rasterio
from PIL import Image
from rasterio.windows import Window
_mask_module=importlib.util.spec_from_file_location('landsat_processing_reference',Path(__file__).with_name('verify-landsat-quality-processing.py'))
_mask_reference=importlib.util.module_from_spec(_mask_module)
_mask_module.loader.exec_module(_mask_reference)
polygon_mask=_mask_reference.polygon_mask

KEYS = ['red','green','blue','qa_pixel','qa_radsat']
ITEM = 'LC09_L2SP_044034_20250628_02_T1'
DEFINITION = 'https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands'
PINNED = [
    '3642840ee224c5af67a398f12532adc0f8d09d0eded5e8a1e512e01c09e4f546',
    '679c701ee7e0b0d9f6abc3dd49ebb0a58a9d4692b527f9e4643b7f35a4b2c1db',
    'e350eebc6dc62e40fd0f6ed7a8b4419be4a2787a897391e8ddbfaf59d2fb9cd1',
    'c56b8eba5bc5df6442892663eed5b7d3bf84915dde2c009aa6386fb8b7c50f2f',
    'b2e4ec6b6594effe5ac166c877de41eabd93fdbb028914a8cc788b27bab916f4',
]
def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''): h.update(block)
    return h.hexdigest()
def dump(path,value): Path(path).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def read_layers(jobs):
    arrays=[];coverage=[]
    for j in jobs:
        assert sha(j['outputPath'])==j['sha256']
        with rasterio.open(j['outputPath']) as ds:
            assert ds.dtypes==('uint16',) and ds.res==(30.,30.) and ds.crs.to_string()=='EPSG:32610'
            arrays.append(ds.read(1));coverage.append(ds.read_masks(1)!=0)
    return arrays,coverage
def expected_rgb(arrays,coverage,mask):
    rgb=np.stack(arrays[:3]);input_valid=np.all(rgb!=0,axis=0)
    if not mask:return rgb,None
    q,r=arrays[3:];accepted=(q&31)==0
    accepted &= (r&(2|4|8|2048))==0
    accepted &= coverage[3]&coverage[4]
    if mask['excludeSnow']:accepted &= (q&32)==0
    if mask['policy']=='cloud_free_conservative':
        accepted &= (q&64)!=0
        for shift in [8,10,14]:accepted &= ((q>>shift)&3)==1
        if mask['excludeSnow']:accepted &= ((q>>12)&3)==1
        accepted &= (r&(128|512|1024|61440))==0
    rgb[:,~accepted]=0
    return rgb,{'examinedPixels':accepted.size,'rejectedPixels':int((~accepted).sum()),'inputCommonValidPixels':int(input_valid.sum()),'removedValidPixels':int((input_valid&~accepted).sum())}
def preview_check(info,rgb,path):
    w,h=info['previewWidth'],info['previewHeight'];sy=np.arange(h,dtype=np.int64)*rgb.shape[1]//h;sx=np.arange(w,dtype=np.int64)*rgb.shape[2]//w
    samples=rgb[:,sy[:,None],sx[None,:]].astype(np.int32);valid=np.all(samples!=0,axis=0);rgba=np.zeros((h,w,4),dtype=np.uint8)
    for c in range(3):
        values=np.sort(samples[c][valid]);bounds=[0,0] if not values.size else values[[(values.size-1)*2//100,(values.size-1)*98//100]].tolist()
        assert bounds==info['composite']['displayRanges'][c];lo,hi=bounds
        rgba[:,:,c][valid]=128 if lo==hi else np.floor(np.clip((samples[c][valid]-lo)/(hi-lo),0,1)*255+.5).astype(np.uint8)
    rgba[:,:,3][valid]=255;png=base64.b64decode(info['previewDataUrl'].split(',',1)[1]);Path(path).write_bytes(png)
    assert np.array_equal(np.asarray(Image.open(io.BytesIO(png)).convert('RGBA')),rgba)
    assert info['composite']['validSampleCount']==int(valid.sum())
    return {'width':w,'height':h,'rgbaPixelsCompared':w*h,'pngSha256':hashlib.sha256(png).hexdigest()}
def check_clip(originals,layers,project):
    proofs=[]
    for original,job in zip(originals,layers):
        g=job['mosaicOutput'];w,h=g['width'],g['height'];area=polygon_mask(g,project.get('geometry'))
        with rasterio.open(original['outputPath']) as src,rasterio.open(job['outputPath']) as out:
            x=(g['bounds'][0]-src.bounds.left)/30;y=(src.bounds.top-g['bounds'][3])/30;ox,oy=round(x),round(y)
            assert abs(x-ox)<1e-7 and abs(y-oy)<1e-7 and out.tags()['AREA_OR_POINT']=='Area'
            expected=src.read(1,window=Window(ox,oy,w,h));assert expected.shape==(h,w)
            if job['assetKey'].startswith('qa_'):
                with rasterio.open(originals[3]['outputPath']) as qa: covered=(qa.read(1,window=Window(ox,oy,w,h))&1)==0
                covered &= area;expected[~area]=1 if job['assetKey']=='qa_pixel' else 0
                assert np.array_equal(out.read_masks(1)!=0,covered)
                assert out.nodata is None
            else:expected[~area]=0
            assert np.array_equal(out.read(1),expected)
            proofs.append({'key':job['assetKey'],'id':job['id'],'sha256':job['sha256'],'pixelsCompared':w*h,'maskPixelsCompared':w*h if job['assetKey'].startswith('qa_') else 0,'polygonOutsidePixels':int((~area).sum())})
    return proofs
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',required=True);p.add_argument('--exe',required=True);p.add_argument('--port',type=int,default=4631);p.add_argument('--rgb-source',default='.verification/local-rgb-native-20261002');p.add_argument('--quality-source',default='.verification/landsat-quality-processing-20261004');p.add_argument('--resume',action='store_true');a=p.parse_args()
    root=Path(a.root).resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-rgb-mask-') and (a.resume or not root.exists())
    rgb_source=Path(a.rgb_source).resolve();qa_source=Path(a.quality_source).resolve();exe_source=Path(a.exe).resolve()
    rgb_jobs=json.loads((rgb_source/'jobs.json').read_text(encoding='utf-8'));qa_jobs=json.loads((qa_source/'jobs.json').read_text(encoding='utf-8'))
    previous=json.loads((qa_source/'native-processing-verification.json').read_text(encoding='utf-8'));assert previous['status']=='passed'
    originals=[copy.deepcopy(next(j for j in (rgb_jobs if i<3 else qa_jobs).values() if j['kind']=='download' and j['status']=='succeeded' and j['itemId']==ITEM and j['assetKey']==key)) for i,key in enumerate(KEYS)]
    wrong_qa=copy.deepcopy(next(j for j in qa_jobs.values() if j['kind']=='download' and j['assetKey']=='qa_pixel' and '20250612' in j['itemId']))
    binary_sha=sha(exe_source);exe=root/f'runtime-{binary_sha[:16]}.exe'
    partial=json.loads((root/'native-verification.json').read_text(encoding='utf-8')) if a.resume else None
    if partial:
        assert partial['status']=='running' and partial['nativeBinarySha256']==binary_sha and sha(exe)==binary_sha
        diagnostic=root/f"interrupted-native-{sha(root/'native-verification.json')}.json";shutil.copy2(root/'native-verification.json',diagnostic)
    else:
        root.mkdir();(root/'assets').mkdir();shutil.copy2(exe_source,exe);assert sha(exe)==binary_sha
    selected={};source_files=[]
    for i,j in enumerate([*originals,wrong_qa]):
        src=Path(j['outputPath']);assert sha(src)==j['sha256'] and (i>=5 or j['sha256']==PINNED[i])
        source_files.append({'path':str(src),'id':j['id'],'sha256':j['sha256'],'bytes':src.stat().st_size,'mtimeNs':src.stat().st_mtime_ns})
        target=root/'assets'/f"{j['id']}.tif"
        if partial:assert sha(target)==j['sha256']
        else:shutil.copy2(src,target)
        j['outputPath']=str(target);selected[j['id']]=j
    if not partial:dump(root/'jobs.json',selected)
    dump(root/'proxy-settings.json',{'mode':'custom','url':'http://127.0.0.1:9'})
    report={'schema':'geod-landsat-rgb-mask-native/v1','status':'running','checkedAt':datetime.now(timezone.utc).isoformat(),'qaOnly':True,'nativeBinary':str(exe),'nativeBinarySha256':binary_sha,'sourceReceiptSha256':sha(qa_source/'native-processing-verification.json'),'definition':DEFINITION,'network':'provider requests disabled by loopback:9','independentReader':f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}','originals':originals,'sourceFiles':source_files,'cases':[],'controls':[],'clipChecks':[]}
    if not partial:dump(root/'input-snapshots.json',report)
    else:report['retainedInterruptedReceipt']={'file':diagnostic.name,'sha256':sha(diagnostic)}
    process=None;base=f'http://127.0.0.1:{a.port}';opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def stop():
        nonlocal process
        if process and process.poll() is None:process.terminate();process.wait(timeout=20)
        process=None
    atexit.register(stop)
    def api(path,body=None):
        req=urllib.request.Request(base+path,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'})
        with opener.open(req,timeout=180) as res:return json.load(res)
    def start():
        nonlocal process
        process=subprocess.Popen([str(exe),'serve','--data-dir',str(root),'--port',str(a.port)],stdout=(root/'runtime.stdout.log').open('ab'),stderr=(root/'runtime.stderr.log').open('ab'),creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        for _ in range(100):
            assert process.poll() is None,(root/'runtime.stderr.log').read_text(encoding='utf-8')
            try:assert Path(api('/health')['storageRoot']).samefile(root);return
            except OSError:time.sleep(.1)
        raise AssertionError('QA service did not start')
    def wait(job):
        deadline=time.monotonic()+600
        while time.monotonic()<deadline:
            job=api(f"/jobs/{job['id']}");assert job['status'] not in ['failed','cancelled','interrupted'],job
            if job['status']=='succeeded' and job['settled']:return job
            time.sleep(.2)
        raise AssertionError('RGB did not settle')
    def cli(*args,success=True):
        r=subprocess.run([str(exe),'scientific-rgb',*args,'--server',base],capture_output=True,text=True,encoding='utf-8',timeout=600,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        if success:assert r.returncode==0,r.stderr;return json.loads(r.stdout)
        assert r.returncode!=0;return r.stderr.strip()
    try:
        start();project=next(c['project'] for c in previous['cases'] if c['name']=='single')
        geometry=next(c['project']['geometry'] for c in previous['cases'] if c['name']=='polygon')
        derived={};projects={}
        for label in ['single','polygon']:
            draft={'name':f'QA · Landsat same-scene RGB · {label}','bounds':project['bounds'],'scenes':project['scenes']}
            if label=='polygon':draft['geometry']=geometry
            prior=next((c for c in partial['clipChecks'] if c['case']==label),None) if partial else None
            if prior:
                created=next(p for p in api('/projects') if p['id']==prior['project']['id']);assert all(created[k]==v for k,v in draft.items())
                layers=[wait(j) for j in prior['layers']]
            else:created=api('/projects',draft);layers=[wait(api(f"/projects/{created['id']}/mosaics",{'assetKey':key})) for key in KEYS]
            report['clipChecks'].append({'case':label,'project':created,'layers':layers,'proofs':check_clip(originals,layers,created)})
            derived[label]=layers;projects[label]=created;dump(root/'native-verification.json',report)
        for label,layers,project_id in [('original',originals,None),('single',derived['single'],projects['single']['id']),('polygon',derived['polygon'],projects['polygon']['id'])]:
            arrays,coverage=read_layers(layers)
            for policy,snow in [(None,False),('cloud_free',False),('cloud_free_conservative',False),('cloud_free_conservative',True)]:
                slug=f'{label}-{policy or "unmasked"}-{snow}';request={'jobIds':[j['id'] for j in layers[:3]],'name':f'QA · Landsat · {slug}'}
                if project_id:request['projectId']=project_id
                mask={'qaPixelJobId':layers[3]['id'],'qaRadsatJobId':layers[4]['id'],'policy':policy,'excludeSnow':snow} if policy else None
                if mask:request['qualityMask']=mask
                request_path=root/f'{slug}-request.json';dump(request_path,request);plan=cli('plan','--request',str(request_path))
                reused=next((j for j in api('/jobs') if j.get('rgbSpec')==plan['spec'] and j['status']=='succeeded'),None) if partial else None
                job=wait(reused) if reused else cli('run','--request',str(request_path));assert job['status']=='succeeded' and sha(job['outputPath'])==job['sha256']
                expected,counts=expected_rgb(arrays,coverage,mask)
                with rasterio.open(layers[0]['outputPath']) as src,rasterio.open(job['outputPath']) as out:
                    assert out.crs==src.crs and out.transform==src.transform and out.bounds==src.bounds and out.tags()['AREA_OR_POINT']==src.tags()['AREA_OR_POINT']
                    assert out.count==3 and out.dtypes==('uint16',)*3 and out.nodata==0 and out.scales==(.0000275,)*3 and out.offsets==(-.2,)*3
                    for y in range(0,out.height,64):assert np.array_equal(out.read(window=Window(0,y,out.width,min(64,out.height-y))),expected[:,y:y+64,:])
                result=job['rgbOutput'];assert result.get('qualityMask')==counts
                assert result['channelValidPixels']==[int((band!=0).sum()) for band in expected] and result['commonValidPixels']==int(np.all(expected!=0,axis=0).sum())
                assert result['samplesSha256']==[hashlib.sha256(band.astype('<u2').tobytes()).hexdigest() for band in expected]
                summary=cli('inspect','--id',job['id']);assert summary['artifact']['sha256']==job['sha256'] and summary['previewOmitted']
                info=api(f"/jobs/{job['id']}/rgb");assert info['artifact']['sha256']==job['sha256'];preview=preview_check(info,expected,root/f'{slug}-preview.png')
                samples=[]
                conditions=[np.all(expected!=0,axis=0),~np.all(expected!=0,axis=0)]
                if mask:
                    conditions.extend([((arrays[3]&128)!=0)&np.all(expected!=0,axis=0),((arrays[4]&0x171)!=0)&np.all(expected!=0,axis=0)])
                for choose in conditions:
                    coords=np.argwhere(choose)
                    if len(coords):
                        for row,col in [coords[0],coords[len(coords)//2],coords[-1]]:
                            xy=[info['bounds'][0]+(int(col)+.5)*30,info['bounds'][3]-(int(row)+.5)*30];sample=cli('pixel','--id',job['id'],'--x',str(xy[0]),'--y',str(xy[1]))
                            assert sample['values']==expected[:,row,col].tolist();assert sample['channelNoData']==[v==0 for v in sample['values']]
                            for v,r in zip(sample['values'],sample['reflectances']):assert r is None if v==0 else abs(r-(v*.0000275-.2))<1e-12
                            samples.append(sample)
                package=cli('package','--id',job['id']);assert sha(package['path'])==package['sha256']
                with zipfile.ZipFile(package['path']) as z:
                    assert z.testzip() is None and len(z.namelist())==5
                    assert hashlib.sha256(z.read(f"{job['id']}.tif")).hexdigest()==job['sha256']
                    m=json.loads(z.read(f"{job['id']}.metadata.json"));assert m['spec']==job['rgbSpec'] and m['output']['samples']==result
                    for line in z.read('checksums.sha256').decode().splitlines():
                        h,name=line.split('  ',1);assert hashlib.sha256(z.read(name)).hexdigest()==h
                    assert ('quality-layer pins' in z.read('README.txt').decode())==bool(mask)
                report['cases'].append({'case':label,'policy':policy,'excludeSnow':snow,'request':request,'plan':plan,'job':job,'allDnCompared':expected.size,'counts':counts,'preview':preview,'pixels':samples,'package':package})
                dump(root/'native-verification.json',report);print(json.dumps({'case':label,'policy':policy,'snow':snow,'samples':expected.size,'removed':counts and counts['removedValidPixels']}),flush=True)
        for label,patch in [('wrong scene',{'qaPixelJobId':wrong_qa['id']}),('duplicate flags',{'qaRadsatJobId':originals[3]['id']}),('RGB as flag',{'qaPixelJobId':originals[0]['id']}),('wrong product policy',{'policy':'clear_best'})]:
            bad={'jobIds':[j['id'] for j in originals[:3]],'qualityMask':{'qaPixelJobId':originals[3]['id'],'qaRadsatJobId':originals[4]['id'],'policy':'cloud_free','excludeSnow':False,**patch}}
            path=root/f"{label.replace(' ','-')}-control.json";dump(path,bad);report['controls'].append({'name':label,'message':cli('plan','--request',str(path),success=False)})
        stop();start()
        for entry in report['cases']:
            j=api(f"/jobs/{entry['job']['id']}");assert j['rgbSpec']==entry['job']['rgbSpec'] and j['rgbOutput']==entry['job']['rgbOutput']
            assert cli('inspect','--id',j['id'])['artifact']['sha256']==j['sha256']
        for f in source_files:assert sha(f['path'])==f['sha256'] and Path(f['path']).stat().st_mtime_ns==f['mtimeNs']
        report.update(status='passed',originalSourceFilesUnchanged=True,restartRulesAndResultsUnchanged=True,scope='Real Landsat 9 original, same-scene rectangle and polygon with hole; explicit flag policies only. Landsat 8 real RGB and coupled multi-scene screening are not claimed.')
        dump(root/'native-verification.json',report);dump(root/'ui-fixture.json',{'originals':originals,'single':derived['single'],'polygon':derived['polygon'],'singleProject':projects['single'],'polygonProject':projects['polygon'],'cases':report['cases'],'nativeBinary':str(exe),'nativeBinarySha256':binary_sha})
        print(json.dumps({'status':'passed','cases':len(report['cases']),'dnSamples':sum(c['allDnCompared'] for c in report['cases']),'previewPixels':sum(c['preview']['rgbaPixelsCompared'] for c in report['cases'])}),flush=True)
    finally:stop()
if __name__=='__main__':main()
