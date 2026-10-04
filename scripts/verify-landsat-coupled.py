"""Real Landsat 8/9 coherent RGB acceptance, independently checked with GDAL.

Fresh private cohort only. Official downloaded files remain unchanged; local
processing uses a blocked upstream proxy. Synthetic edge controls are Rust tests.
"""
import argparse, atexit, copy, hashlib, importlib.util, json, shutil, subprocess, time, urllib.request, zipfile
from datetime import datetime, timezone
from pathlib import Path
import numpy as np
import rasterio
from rasterio.windows import Window
from pyproj import Transformer

helper=importlib.util.spec_from_file_location('landsat_reference',Path(__file__).with_name('verify-landsat-rgb-mask.py'))
reference=importlib.util.module_from_spec(helper);helper.loader.exec_module(reference)
sha,dump,read_layers,expected_rgb,preview_check,polygon_mask=(getattr(reference,n) for n in ['sha','dump','read_layers','expected_rgb','preview_check','polygon_mask'])
KEYS=reference.KEYS
SELECTION='newest complete qualified RGB scene wins; acquisition date then item ID break ties'

def coherent_oracle(spec,jobs):
    g,selection=spec['grid'],spec['qualityMask']['coupled'];w,h=g['width'],g['height']
    rgb=np.zeros((3,h,w),dtype=np.uint16);winner=np.full((h,w),-1,dtype=np.int16);baseline=winner.copy()
    coverage=polygon_mask({**g,'pixelSize':g['pixelSize']},selection['geometry'])
    for index,scene in enumerate(selection['scenes']):
        layers=[jobs[p['jobId']] for p in scene['sources']]
        assert [j['assetKey'] for j in layers]==KEYS and len({j['itemId'] for j in layers})==1
        for pin,j in zip(scene['sources'],layers):
            assert pin['sha256']==j['sha256'] and pin['href']==j['href'] and pin['bytes']==j['bytesDownloaded'] and pin['itemId']==j['itemId']
        with rasterio.open(layers[0]['outputPath']) as ds:
            assert ds.crs.to_string()==g['crs'] and ds.res==(30.,30.) and ds.dtypes==('uint16',)
            assert ds.width==scene['grid']['width'] and ds.height==scene['grid']['height']
            assert list(ds.bounds)==scene['grid']['bounds']
            off=[(ds.transform.c-g['bounds'][0])/30,(g['bounds'][3]-ds.transform.f)/30]
            assert max(abs(v-round(v)) for v in off)<1e-6;dx,dy=map(round,off)
            x0,x1=max(0,dx),min(w,dx+ds.width);y0,y1=max(0,dy),min(h,dy+ds.height)
        if x0>=x1 or y0>=y1:continue
        window=Window(x0-dx,y0-dy,x1-x0,y1-y0);arrays=[];masks=[]
        for layer in layers:
            with rasterio.open(layer['outputPath']) as ds:
                arrays.append(ds.read(1,window=window));masks.append(ds.read_masks(1,window=window)!=0)
        complete=np.all(np.stack(arrays[:3])!=0,axis=0)&coverage[y0:y1,x0:x1]
        qualified,_=expected_rgb(arrays,masks,spec['qualityMask']);accepted=np.all(qualified!=0,axis=0)&complete
        baseline[y0:y1,x0:x1][complete]=index;winner[y0:y1,x0:x1][accepted]=index
        rgb[:,y0:y1,x0:x1][:,accepted]=qualified[:,accepted]
    valid=winner>=0
    counts={'examinedPixels':w*h,'rejectedPixels':int((~valid).sum()),'inputCommonValidPixels':int((baseline>=0).sum()),
        'removedValidPixels':int(((baseline>=0)&~valid).sum()),'coupled':{'sceneValidPixels':[int((winner==i).sum()) for i in range(len(selection['scenes']))],
        'fallbackPixels':int((valid&(winner!=baseline)).sum())}}
    return rgb,counts,winner,baseline,coverage

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',required=True);p.add_argument('--exe',required=True)
    p.add_argument('--source',default='.verification/landsat-coupled-sources-20261004');p.add_argument('--port',type=int,default=4659);a=p.parse_args()
    root,source,source_exe=(Path(v).resolve() for v in [a.root,a.source,a.exe])
    assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-coupled-') and not root.exists()
    old=json.loads((source/'source-verification.json').read_text(encoding='utf-8'));assert old['status']=='passed'
    jobs={j['id']:copy.deepcopy(j) for j in old['originals']};assert len(jobs)==15
    root.mkdir();(root/'assets').mkdir();binary_hash=sha(source_exe);exe=root/f'runtime-{binary_hash[:16]}.exe';shutil.copy2(source_exe,exe)
    before=[]
    for j in jobs.values():
        original=Path(j['outputPath']);assert sha(original)==j['sha256'];before.append({'path':str(original),'sha256':j['sha256'],'mtimeNs':original.stat().st_mtime_ns})
        target=root/'assets'/f"{j['id']}.tif";shutil.copy2(original,target);j['outputPath']=str(target)
    dump(root/'jobs.json',jobs);dump(root/'proxy-settings.json',{'mode':'custom','url':'http://127.0.0.1:9'})
    dump(root/'input-snapshots.json',{'jobs':jobs,'sourceFiles':before,'receiptSha256':sha(source/'source-verification.json')})
    report={'schema':'geod-landsat-coupled-native/v1','status':'running','qaOnly':True,'checkedAt':datetime.now(timezone.utc).isoformat(),
        'nativeBinary':str(exe),'nativeBinarySha256':binary_hash,'sourceReceiptSha256':sha(source/'source-verification.json'),
        'definition':reference.DEFINITION,'independentReader':f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}, NumPy, Shapely',
        'network':'provider traffic disabled via loopback:9','inputs':{},'cases':[],'controls':[]}
    process=None;base=f'http://127.0.0.1:{a.port}';opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def api(route,body=None):
        req=urllib.request.Request(base+route,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'})
        with opener.open(req,timeout=180) as r:return json.load(r)
    def stop():
        nonlocal process
        if process and process.poll() is None:process.terminate();process.wait(timeout=20)
        process=None
    atexit.register(stop)
    def start():
        nonlocal process
        process=subprocess.Popen([str(exe),'serve','--data-dir',str(root),'--port',str(a.port)],stdout=(root/'runtime.stdout.log').open('ab'),stderr=(root/'runtime.stderr.log').open('ab'),creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        for _ in range(100):
            assert process.poll() is None,(root/'runtime.stderr.log').read_text(encoding='utf-8')
            try:assert Path(api('/health')['storageRoot']).samefile(root);return
            except OSError:time.sleep(.1)
        raise AssertionError('Private runtime did not start')
    def wait(j):
        until=time.monotonic()+600
        while time.monotonic()<until:
            j=api('/jobs/'+j['id']);assert j['status'] not in ['failed','cancelled','interrupted'],j
            if j['status']=='succeeded' and j['settled']:return j
            time.sleep(.2)
        raise AssertionError('Native output did not settle')
    def cli(*args,success=True):
        r=subprocess.run([str(exe),'scientific-rgb',*args,'--server',base],capture_output=True,text=True,encoding='utf-8',timeout=600,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        if success:assert r.returncode==0,r.stderr;return json.loads(r.stdout)
        assert r.returncode!=0;return r.stderr.strip()
    def ring(bounds):
        w,s,e,n=bounds;return [[w,s],[e,s],[e,n],[w,n],[w,s]]
    def geometry(bounds):
        w,s,e,n=bounds;cx,cy=(w+e)/2,(s+n)/2;dx,dy=(e-w)/10,(n-s)/10
        return {'type':'Polygon','coordinates':[ring(bounds),[[cx-dx,cy-dy],[cx-dx,cy+dy],[cx+dx,cy+dy],[cx+dx,cy-dy],[cx-dx,cy-dy]]]}
    try:
        start();scenes=old['project']['scenes'];l9=[s for s in scenes if s['itemId'].startswith('LC09')];bounds=old['project']['bounds']
        # Locate actual quality fallback within the accepted common project area.
        full=[]
        for scene in l9:
            layers=[next(j for j in jobs.values() if j['itemId']==scene['itemId'] and j['assetKey']==k) for k in KEYS]
            with rasterio.open(layers[0]['outputPath']) as ds:
                window=ds.window(*Transformer.from_crs('EPSG:4326',ds.crs,always_xy=True).transform_bounds(*bounds,densify_pts=21)).round_offsets().round_lengths()
                if full:
                    ref_bounds=rasterio.windows.bounds(full[0][2],full[0][3])
                    aligned=ds.window(*ref_bounds)
                    assert max(abs(v-round(v)) for v in [aligned.col_off,aligned.row_off,aligned.width,aligned.height])<1e-6
                    window=aligned.round_offsets().round_lengths()
                transform=ds.transform;arrays=[];masks=[]
                for j in layers:
                    with rasterio.open(j['outputPath']) as d:arrays.append(d.read(1,window=window,boundless=True,fill_value=0));masks.append(d.read_masks(1,window=window,boundless=True)!=0)
                screened,_=expected_rgb(arrays,masks,{'policy':'cloud_free','excludeSnow':False})
                full.append((arrays,screened,window,transform))
        fallback=np.all(full[0][1]!=0,axis=0)&np.all(np.stack(full[1][0][:3])!=0,axis=0)&~np.all(full[1][1]!=0,axis=0)
        positions=np.argwhere(fallback);assert len(positions)>0;row,col=map(int,positions[len(positions)//2]);window,transform=full[0][2:]
        col+=int(window.col_off);row+=int(window.row_off);inverse=Transformer.from_crs('EPSG:32610','EPSG:4326',always_xy=True)
        xs=[transform.c+(col+d)*30 for d in [-20,20]];ys=[transform.f-(row+d)*30 for d in [-10,10]]
        lon,lat=inverse.transform([xs[0],xs[0],xs[1],xs[1]],[ys[0],ys[1],ys[0],ys[1]])
        small=[min(lon),min(lat),max(lon),max(lat)];report['fallbackRegion']={'centerOriginalPixel':[col,row],'observedFallbackPixels':int(fallback.sum()),'bounds':small};del full
        for label,area,chosen,polygon in [('fallback',small,l9,None),('fallback-polygon',small,l9,geometry(small)),('mosaic',bounds,scenes,None),('polygon',bounds,scenes,geometry(bounds))]:
            draft={'name':'QA · coherent Landsat · '+label,'bounds':area,'scenes':chosen}
            if polygon:draft['geometry']=polygon
            project=api('/projects',draft);layers=[wait(api(f"/projects/{project['id']}/mosaics",{'assetKey':k})) for k in KEYS]
            jobs.update({j['id']:j for j in layers});report['inputs'][label]={'project':project,'jobs':layers};dump(root/'native-verification.json',report)
            print(json.dumps({'input':label,'pixels':layers[0]['mosaicOutput']['width']*layers[0]['mosaicOutput']['height']}),flush=True)
        l8=[next(j for j in jobs.values() if j['kind']=='download' and j['itemId'].startswith('LC08') and j['assetKey']==k) for k in KEYS]
        report['inputs']['landsat8-original']={'jobs':l8};original_arrays,original_masks=read_layers(l8)
        for label in ['landsat8-original','fallback','fallback-polygon','mosaic','polygon']:
            layers=report['inputs'][label]['jobs'];multi=label!='landsat8-original'
            options=[('cloud_free',False),('cloud_free_conservative',False),('cloud_free_conservative',True)]
            if not multi:options.insert(0,(None,False))
            for policy,snow in options:
                req={'jobIds':[j['id'] for j in layers[:3]],'name':f'QA · coherent {label} · {policy} · snow {snow}'}
                if policy:req['qualityMask']={'qaPixelJobId':layers[3]['id'],'qaRadsatJobId':layers[4]['id'],'policy':policy,'excludeSnow':snow}
                slug=f'{label}-{policy}-{snow}';file=root/f'{slug}-request.json';dump(file,req);plan=cli('plan','--request',str(file));spec=plan['spec']
                if multi:
                    selection=spec['qualityMask']['coupled'];assert spec['qualityMask']['schemaVersion']=='geod-landsat-rgb-mask/v2' and selection['selection']==SELECTION
                    ids=[s['sources'][0]['itemId'] for s in selection['scenes']];assert ids==sorted(ids,key=lambda item:(item.split('_')[3],item))
                    expected,counts,winner,baseline,coverage=coherent_oracle(spec,jobs)
                    if label.startswith('fallback'):assert counts['coupled']['fallbackPixels']>0
                else:expected,counts=expected_rgb(original_arrays,original_masks,req.get('qualityMask'));winner=baseline=np.zeros(expected.shape[1:],dtype=np.int16);coverage=np.ones(expected.shape[1:],dtype=bool)
                job=cli('run','--request',str(file));assert job['status']=='succeeded' and job['rgbSpec']==spec and sha(job['outputPath'])==job['sha256']
                with rasterio.open(layers[0]['outputPath']) as parent,rasterio.open(job['outputPath']) as ds:
                    assert ds.crs==parent.crs and ds.transform==parent.transform and ds.bounds==parent.bounds
                    assert ds.dtypes==('uint16',)*3 and ds.nodata==0 and ds.scales==(.0000275,)*3 and ds.offsets==(-.2,)*3
                    actual=ds.read();assert np.array_equal(actual,expected),(label,policy,int(np.count_nonzero(actual!=expected)))
                out=job['rgbOutput'];assert out.get('qualityMask')==counts
                valid=np.all(expected!=0,axis=0);assert out['commonValidPixels']==int(valid.sum())
                assert out['channelValidPixels']==[int((b!=0).sum()) for b in expected]
                assert out['samplesSha256']==[hashlib.sha256(b.astype('<u2').tobytes()).hexdigest() for b in expected]
                metadata=api(f"/jobs/{job['id']}/rgb");preview=preview_check(metadata,expected,root/f'{slug}-preview.png');samples=[]
                choices={'fallback':valid&(winner!=baseline),'newest':valid&(winner==baseline),'nodata':~valid,'polygon-hole-or-outside':~coverage}
                for category,mask in choices.items():
                    points=np.argwhere(mask)
                    if not len(points):continue
                    y,x=map(int,points[len(points)//2]);xy=[spec['grid']['bounds'][0]+(x+.5)*30,spec['grid']['bounds'][3]-(y+.5)*30]
                    pixel=cli('pixel','--id',job['id'],'--x',str(xy[0]),'--y',str(xy[1]));assert pixel['values']==expected[:,y,x].tolist()
                    assert pixel['reflectances']==[None if v==0 else int(v)*.0000275-.2 for v in expected[:,y,x]]
                    samples.append({'category':category,'winnerSceneIndex':int(winner[y,x]),'baselineSceneIndex':int(baseline[y,x]),'sample':pixel})
                package=cli('package','--id',job['id']);assert sha(package['path'])==package['sha256']
                with zipfile.ZipFile(package['path']) as z:
                    assert z.testzip() is None and len(z.namelist())==5 and hashlib.sha256(z.read(f"{job['id']}.tif")).hexdigest()==job['sha256']
                    manifest=json.loads(z.read(f"{job['id']}.metadata.json"));assert manifest['spec']==spec and manifest['output']['samples']==out
                    if multi:assert 'original Landsat 8/9 scene' in z.read('README.txt').decode()
                    for line in z.read('checksums.sha256').decode().splitlines():digest,name=line.split('  ');assert hashlib.sha256(z.read(name)).hexdigest()==digest
                report['cases'].append({'case':label,'policy':policy or 'none','excludeSnow':snow,'request':req,'plan':plan,'job':job,'allDnCompared':expected.size,'counts':counts,'preview':preview,'pixels':samples,'package':package,'outsideOrHolePixels':int((~coverage).sum())});dump(root/'native-verification.json',report)
                print(json.dumps({'case':label,'policy':policy,'snow':snow,'dnSamples':expected.size,'counts':counts}),flush=True)
        good=next(c['request'] for c in report['cases'] if c['case']=='fallback');before_count=len(api('/jobs'))
        for label,change in [('invalid policy',{'policy':'clear'}),('different area QA',{'qaPixelJobId':report['inputs']['mosaic']['jobs'][3]['id']}),('duplicate quality',{'qaRadsatJobId':good['qualityMask']['qaPixelJobId']})]:
            wrong=copy.deepcopy(good);wrong['qualityMask'].update(change);file=root/f'control-{label.replace(" ","-")}.json';dump(file,wrong)
            message=cli('plan','--request',str(file),success=False);assert len(api('/jobs'))==before_count;report['controls'].append({'name':label,'message':message,'queued':False})
        stop();stored=json.loads((root/'jobs.json').read_text(encoding='utf-8'));missing=copy.deepcopy(stored)
        removed=next(c for c in report['cases'] if c['case']=='fallback')['job']['rgbSpec']['qualityMask']['coupled']['scenes'][0]['sources'][3]['jobId'];missing.pop(removed)
        dump(root/'jobs.json',missing);start();file=root/'missing-original-request.json';dump(file,good);message=cli('plan','--request',str(file),success=False)
        assert 'original' in message.lower() and len(api('/jobs'))==len(missing);report['controls'].append({'name':'missing original QA','message':message,'queued':False})
        stop();dump(root/'jobs.json',stored);start()
        for c in report['cases']:assert api(f"/jobs/{c['job']['id']}/rgb")['artifact']['sha256']==c['job']['sha256']
        for entry in before:assert sha(entry['path'])==entry['sha256'] and Path(entry['path']).stat().st_mtime_ns==entry['mtimeNs']
        report['status']='passed';report['sourceFilesUnchanged']=len(before);report['restartInspections']=len(report['cases']);report['finishedAt']=datetime.now(timezone.utc).isoformat();dump(root/'native-verification.json',report)
        dump(root/'ui-fixture.json',{'nativeBinary':str(exe),'cases':{label:report['inputs'][label] for label in ['fallback','fallback-polygon','polygon']}})
        print(json.dumps({'status':'passed','cases':len(report['cases']),'dnSamples':sum(c['allDnCompared'] for c in report['cases'])}),flush=True)
    except Exception as e:report['status']='failed';report['error']=repr(e);dump(root/'native-verification.json',report);raise
    finally:stop()

if __name__=='__main__':main()
