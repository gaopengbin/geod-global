"""Independently verify actual MODIS index originals and native project outputs.

Uses previously downloaded unchanged files, a fresh private store, and a
blocked upstream proxy. GDAL/NumPy are independent of the application's TIFF
decoder. This is not a native-window acceptance test.
"""
import argparse,copy,hashlib,json,shutil,subprocess,time,urllib.request,urllib.error
from pathlib import Path
from datetime import datetime,timezone
import numpy as np
import rasterio
from PIL import Image
from pyproj import Transformer
import shapely
from shapely.geometry import shape

parser=argparse.ArgumentParser();parser.add_argument('sources',type=Path);parser.add_argument('root',type=Path);parser.add_argument('binary',type=Path);parser.add_argument('--port',type=int,default=4633);parser.add_argument('--original-receipt',type=Path,help='Add only the real temporal fallback case, using previously accepted unchanged originals');args=parser.parse_args()
root=args.root.resolve();source_root=args.sources.resolve();assert root.parent==source_root.parent==Path('.verification').resolve();assert root.name.startswith('modis-vegetation-');root.mkdir(exist_ok=False)
sources=json.loads((source_root/'source-verification.json').read_text());assert sources['status']=='passed' and len(sources['cases'])==6
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
binary_sha=sha(args.binary);binary=root/f'runtime-{binary_sha[:16]}.exe';shutil.copy2(args.binary,binary)
shutil.copytree(source_root/'assets',root/'assets')
jobs=json.loads((source_root/'jobs.json').read_text())
for job in jobs.values():job['outputPath']=str(root/'assets'/f"{job['id']}.tif")
(root/'jobs.json').write_text(json.dumps(jobs,indent=2));(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}))
originals={j['id']:j for j in jobs.values()};base=f'http://127.0.0.1:{args.port}'
report={'schema':'geod-modis-vegetation-acceptance/v1','checkedAt':datetime.now(timezone.utc).isoformat(),'nativeBinarySha256':binary_sha,'sourceReceiptSha256':sha(source_root/'source-verification.json'),'originals':[],'outputs':[],'cases':[],'negativeControls':[],'status':'pending','nativeWindowTested':False}
if args.original_receipt:
    reference=json.loads(args.original_receipt.read_text(encoding='utf-8'))
    assert reference['status']=='passed' and reference['nativeBinarySha256']==binary_sha and reference['sourceReceiptSha256']==report['sourceReceiptSha256']
    assert len(reference['originals'])==6
    for entry in reference['originals']:
        j=originals[entry['job']['id']];assert j['sha256']==entry['job']['sha256']==sha(j['outputPath']) and j['href']==entry['job']['href']
    report['originalReference']={'path':str(args.original_receipt.resolve()),'sha256':sha(args.original_receipt),'filesReusedUnchanged':6}
def dump(): (root/'verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
def api(route,body=None,expected=200):
    req=urllib.request.Request(base+route,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'},method='GET' if body is None else 'POST')
    try:r=urllib.request.urlopen(req,timeout=65)
    except urllib.error.HTTPError as e:r=e
    with r:status=r.status;data=json.loads(r.read())
    assert status==expected,(route,status,data)
    return data
def start():
    process=subprocess.Popen([str(binary),'serve','--data-dir',str(root),'--port',str(args.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
    for _ in range(100):
        try:api('/health');return process
        except Exception:assert process.poll() is None;time.sleep(.1)
    raise AssertionError('Private runtime failed to start')
def stop(p):p.terminate();p.wait(timeout=20)
def wait(job):
    for _ in range(600):
        j=api('/jobs/'+job['id']);assert j['status'] not in ['failed','interrupted','cancelled'],j
        if j['status']=='succeeded' and j.get('settled'):return j
        time.sleep(.1)
    raise AssertionError('Native processing did not settle')
stops=np.array([-2000,0,2000,4000,7000,10000]);colors=np.array([[42,91,135],[208,169,104],[235,223,160],[161,196,112],[75,143,76],[24,86,51]],dtype=np.int64)
def png_check(data,url):
    import base64,io
    actual=np.asarray(Image.open(io.BytesIO(base64.b64decode(url.split(',')[1]))).convert('RGBA'));h,w=actual.shape[:2]
    selected=data[np.ix_(np.arange(h,dtype=np.uint64)*data.shape[0]//h,np.arange(w,dtype=np.uint64)*data.shape[1]//w)]
    clipped=np.clip(selected.astype(np.int64),-2000,10000);segment=np.searchsorted(stops[1:],clipped,side='left');span=stops[segment+1]-stops[segment]
    # Exact rational arithmetic is independent of the native implementation
    # and avoids a binary-float ambiguity at half-color values.
    numerator=colors[segment]*(stops[segment+1]-clipped)[...,None]+colors[segment+1]*(clipped-stops[segment])[...,None]
    rgb=((numerator+span[...,None]//2)//span[...,None]).astype('uint8')
    rgba=np.concatenate([rgb,np.where(selected==-3000,0,255).astype('uint8')[...,None]],axis=-1)
    # RGB under transparent pixels is deliberately retained in the application.
    assert np.array_equal(actual,rgba),int(np.count_nonzero(actual!=rgba))
    return h*w
def inspect(job,data,ds,case):
    metadata=api(f"/jobs/{job['id']}/raster");thumb=api(f"/jobs/{job['id']}/thumbnail");v=metadata['vegetation']
    assert metadata['dataType']=='Int16' and metadata['bandCount']==1 and metadata['nodata']==-3000 and 'reflectance' not in metadata
    assert v['index']==job['assetKey'] and v['scale']==.0001 and v['offset']==0 and v['validRange']==[-2000,10000] and v['displayRange']==[-2000,10000]
    assert [ds.width,ds.height]==[metadata['width'],metadata['height']] and np.allclose(ds.bounds,metadata['bounds'],atol=1e-7,rtol=0)
    assert ds.dtypes==('int16',) and ds.nodata==-3000 and ds.tags()['AREA_OR_POINT']=='Area'
    assert ds.crs.to_dict()=={'proj':'sinu','lon_0':0,'x_0':0,'y_0':0,'R':6371007.181,'units':'m','no_defs':True}
    assert abs(ds.transform.a-231.656358263889)<1e-6 and abs(ds.transform.e+231.656358263889)<1e-6
    h,w=metadata['previewHeight'],metadata['previewWidth'];sampled=data[np.ix_(np.arange(h,dtype=np.uint64)*ds.height//h,np.arange(w,dtype=np.uint64)*ds.width//w)];valid=sampled!=-3000
    assert v['sampleCount']==h*w and v['validSampleCount']==int(valid.sum()) and v['outOfRangeSampleCount']==int(np.count_nonzero(valid&((sampled<-2000)|(sampled>10000))))
    checked=png_check(data,metadata['previewDataUrl'])+png_check(data,thumb['dataUrl'])
    import base64
    for suffix,url in [('preview',metadata['previewDataUrl']),('thumbnail',thumb['dataUrl'])]:(root/f"{case}-{job['assetKey']}-{job['id'][:8]}-{suffix}.png").write_bytes(base64.b64decode(url.split(',')[1]))
    # Explicit source extrema and NoData, plus edges. They are read from the
    # independent array, never selected from the application's display PNG.
    coordinates={(0,0),(ds.height-1,ds.width-1),(ds.height//2,ds.width//2)}
    for mask in [data==-3000,(data!=-3000)&(data<0),(data!=-3000)&(data==0),data==data[data!=-3000].min(),data==data[data!=-3000].max()]:
        if np.any(mask):coordinates.add(tuple(map(int,np.unravel_index(np.argmax(mask),mask.shape))))
    pixels=[]
    for row,col in sorted(coordinates):
        x,y=ds.xy(row,col);pixel=api(f"/jobs/{job['id']}/pixel?x={x}&y={y}");raw=int(data[row,col])
        assert pixel['pixel']==[col,row] and pixel['value']==raw and pixel['sha256']==job['sha256'] and pixel['isNoData']==(raw==-3000) and 'reflectance' not in pixel
        if raw==-3000:assert 'indexValue' not in pixel
        else:assert abs(pixel['indexValue']-raw*.0001)<1e-12
        pixels.append(pixel)
    return {'job':job,'metadata':metadata,'thumbnail':thumb,'pixels':pixels,'pngPixelsCompared':checked,'dnPixelsRead':int(data.size),'noDataPixels':int(np.count_nonzero(data==-3000)),'minimumDn':int(data[data!=-3000].min()),'maximumDn':int(data[data!=-3000].max())}
inverse=Transformer.from_pipeline('+proj=pipeline +step +inv +proj=sinu +R=6371007.181 +lon_0=0 +x_0=0 +y_0=0 +step +proj=unitconvert +xy_in=rad +xy_out=deg')
def oracle(job,project,dst):
    expected=np.full((dst.height,dst.width),-3000,dtype='int16');covered=np.zeros(expected.shape,dtype=bool);winner=np.full(expected.shape,-1,dtype='int16')
    ordered=[originals[p['jobId']] for p in job['mosaic']['sources']]
    dates={s['itemId']:s['date'] for s in project['scenes']};assert [(dates[j['itemId']],j['itemId']) for j in ordered]==sorted((dates[j['itemId']],j['itemId']) for j in ordered)
    for i,j in enumerate(ordered):
        assert j['assetKey']==job['assetKey'] and sha(j['outputPath'])==j['sha256']
        with rasterio.open(j['outputPath']) as src:
            assert src.crs.to_dict()==dst.crs.to_dict() and np.allclose(src.res,dst.res,atol=1e-8,rtol=0)
            dx,dy=(src.transform.c-dst.transform.c)/dst.transform.a,(src.transform.f-dst.transform.f)/dst.transform.e
            assert abs(dx-round(dx))<1e-5 and abs(dy-round(dy))<1e-5;dx,dy=round(dx),round(dy)
            x0,x1=max(0,dx),min(dst.width,dx+src.width);y0,y1=max(0,dy),min(dst.height,dy+src.height)
            if x1<=x0 or y1<=y0:continue
            values=src.read(1,window=rasterio.windows.Window(x0-dx,y0-dy,x1-x0,y1-y0));valid=values!=-3000
            expected[y0:y1,x0:x1][valid]=values[valid];covered[y0:y1,x0:x1][valid]=True;winner[y0:y1,x0:x1][valid]=i
    masked=np.zeros(expected.shape,dtype=bool)
    if project.get('geometry'):
        rows,cols=np.indices(expected.shape);lon,lat=inverse.transform(dst.transform.c+(cols+.5)*dst.transform.a,dst.transform.f+(rows+.5)*dst.transform.e)
        masked=~shapely.contains_xy(shape(project['geometry']),lon,lat);expected[masked]=-3000;covered[masked]=False;winner[masked]=-1
    assert np.array_equal(dst.read(1),expected)
    assert job['mosaicOutput']['coveredPixels']==int(covered.sum()) and job['mosaicOutput']['maskedPixels']==int(masked.sum())
    assert dst.scales==(.0001,) and dst.offsets==(0.,) and dst.tags()['PRODUCT']=='modis-13q1-v061'
    return {'dnPixelsCompared':int(expected.size),'winnerCounts':{j['itemId']:int(np.count_nonzero(winner==i)) for i,j in enumerate(ordered)},'maskedPixels':int(masked.sum())}
process=None
try:
    process=start();assert api('/proxy')['mode']=='custom'
    for entry in ([] if args.original_receipt else sources['cases']):
        j=originals[entry['job']['id']];assert sha(j['outputPath'])==j['sha256'];data_path=Path(j['outputPath'])
        assert data_path.stat().st_size==j['bytesDownloaded']==j['totalBytes']
        with rasterio.open(data_path) as ds:
            assert (ds.width,ds.height)==(4800,4800);result=inspect(j,ds.read(1),ds,'original');report['originals'].append(result)
        dump();print(json.dumps({'stage':'independent-original','id':j['id'],'key':j['assetKey'],'dn':result['dnPixelsRead']}),flush=True)
    scenes=sources['project']['scenes'];west=next(s for s in scenes if s['itemId'].startswith('MOD13Q1') and '.h08v05.' in s['itemId']);bounds=[-114.1,37.78,-113.5,37.9]
    drafts=[{'name':'QA · MODIS vegetation · single clip','bounds':[-122.55,37.68,-122.32,37.84],'scenes':[west]},
        {'name':'QA · MODIS vegetation · adjacent and shifted periods','bounds':bounds,'scenes':scenes},
        {'name':'QA · MODIS vegetation · polygon with hole','bounds':bounds,'scenes':scenes,'geometry':{'type':'Polygon','coordinates':[
            [[-114.1,37.78],[-113.5,37.78],[-113.5,37.9],[-114.1,37.9],[-114.1,37.78]],
            [[-113.9,37.82],[-113.9,37.86],[-113.7,37.86],[-113.7,37.82],[-113.9,37.82]]]}}]
    names=['single','mosaic','polygon']
    if args.original_receipt:
        drafts=[{'name':'QA · MODIS vegetation · real temporal fallback','bounds':[-122.42,37.96,-122.30,38.06],
                 'scenes':[s for s in scenes if '.h08v05.' in s['itemId']]}];names=['temporal']
    for case,draft in zip(names,drafts):
        project=api('/projects',draft,201);report['cases'].append({'name':case,'project':project})
        for key in ['ndvi','evi']:
            job=wait(api(f"/projects/{project['id']}/mosaics",{'assetKey':key},202));assert sha(job['outputPath'])==job['sha256']
            with rasterio.open(job['outputPath']) as ds:
                comparison=oracle(job,project,ds);result=inspect(job,ds.read(1),ds,case);result.update(comparison);result['case']=case;report['outputs'].append(result)
            if case=='temporal':
                assert len(comparison['winnerCounts'])==2 and all(count>0 for count in comparison['winnerCounts'].values()),comparison
            manifest=json.loads(Path(job['manifestPath']).read_text());assert all('compositePeriod' in s and 'compositeStart' in s and 'MOD13Q1/MYD13Q1' in s['distribution'] for s in manifest['sources'])
            dump();print(json.dumps({'stage':'independent-output','case':case,'key':key,**comparison}),flush=True)
    before=(len(api('/projects')),len(api('/jobs')))
    for case in ['nodata','date','crs','channel']:
        bad=copy.deepcopy(drafts[0]);bad['name']='QA invalid '+case
        if case=='nodata':bad['scenes'][0]['assets']['ndvi']['rasterBand']['nodata']=-28672
        if case=='date':bad['scenes'][0]['date']='2025-06-27T00:00:00Z'
        if case=='crs':bad['scenes'][0]['crs']='EPSG:32610'
        if case=='channel':bad['scenes'][0]['assets']['ndvi']['href']=bad['scenes'][0]['assets']['evi']['href']
        api('/projects',bad,400);report['negativeControls'].append(case)
    assert before==(len(api('/projects')),len(api('/jobs')))
    checked_count=len(report['originals'])+len(report['outputs']);cache={p.name:(p.read_bytes(),p.stat().st_ino,p.stat().st_ctime_ns) for p in (root/'cache/thumbnails/v1').glob('*.json')};assert len(cache)==checked_count
    stop(process);process=start()
    for result in report['originals']+report['outputs']:
        j=result['job'];assert api(f"/jobs/{j['id']}/raster")==result['metadata'] and api(f"/jobs/{j['id']}/thumbnail")==result['thumbnail']
    for name,(contents,inode,created) in cache.items():
        p=root/'cache/thumbnails/v1'/name;assert p.read_bytes()==contents and p.stat().st_ino==inode and p.stat().st_ctime_ns==created
    report['offlineRestart']={'upstreamBlocked':True,'rastersRestored':checked_count,'persistentThumbnailsReused':checked_count,'cacheFilesNotRegenerated':True}
    report['status']='passed';report['dnPixelsRead']=sum(r['dnPixelsRead'] for r in report['originals']);report['outputDnPixelsCompared']=sum(r['dnPixelsCompared'] for r in report['outputs']);report['pngPixelsCompared']=sum(r['pngPixelsCompared'] for r in report['originals']+report['outputs']);dump()
finally:
    if process and process.poll() is None:stop(process)
