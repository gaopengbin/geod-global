"""Independent GDAL/NumPy acceptance of actual science COGs and native outputs.
The upstream is blocked for all local processing and restart checks. This does
not operate or accept a native desktop window. Inputs are actual application
downloads; no synthetic raster is used as positive scientific evidence.
"""
import argparse, copy, hashlib, json, shutil, subprocess, time, urllib.request, urllib.error
from pathlib import Path
from datetime import datetime, timezone, timedelta
import numpy as np
import rasterio
from PIL import Image
from pyproj import Transformer
import shapely
from shapely.geometry import shape

parser=argparse.ArgumentParser();parser.add_argument('sources',type=Path);parser.add_argument('root',type=Path);parser.add_argument('binary',type=Path);parser.add_argument('--port',type=int,default=4641);args=parser.parse_args()
root=args.root.resolve();source_root=args.sources.resolve();assert root.parent==source_root.parent==Path('.verification').resolve();assert root.name.startswith('modis-science-');root.mkdir(exist_ok=False)
sources=json.loads((source_root/'source-verification.json').read_text(encoding='utf-8'));assert sources['status']=='passed' and len(sources['cases'])==30
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
binary_sha=sha(args.binary);binary=root/f'runtime-{binary_sha[:16]}.exe';shutil.copy2(args.binary,binary)
shutil.copytree(source_root/'assets',root/'assets');jobs=json.loads((source_root/'jobs.json').read_text(encoding='utf-8'))
for job in jobs.values():job['outputPath']=str(root/'assets'/f"{job['id']}.tif")
(root/'jobs.json').write_text(json.dumps(jobs,indent=2));(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}))
originals={j['id']:j for j in jobs.values()};base=f'http://127.0.0.1:{args.port}'
# Independent definitions checked against NASA C61 table 1 / 4 / 5 and actual original TIFF tags.
layers={
 'vi_quality':('uint16',1,65535,0,65534), 'vi_reliability':('int8',1,-1,0,3), 'vi_doy':('int16',1,-1,1,366),
 'vi_red':('int16',.0001,-1000,0,10000), 'vi_nir':('int16',.0001,-1000,0,10000), 'vi_blue':('int16',.0001,-1000,0,10000), 'vi_mir':('int16',.0001,-1000,0,10000),
 'vi_view_zenith':('int16',.01,-10000,0,18000), 'vi_sun_zenith':('int16',.01,-10000,0,18000), 'vi_relative_azimuth':('int16',.01,-4000,-18000,18000),
}
report={'schema':'geod-modis-science-acceptance/v1','checkedAt':datetime.now(timezone.utc).isoformat(),'nativeBinarySha256':binary_sha,'sourceReceiptSha256':sha(source_root/'source-verification.json'),'originals':[],'outputs':[],'cases':[],'negativeControls':[],'status':'pending','nativeWindowTested':False}
def dump(): (root/'verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
def api(route,body=None,expected=200):
 req=urllib.request.Request(base+route,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'},method='GET' if body is None else 'POST')
 try:r=urllib.request.urlopen(req,timeout=65)
 except urllib.error.HTTPError as e:r=e
 with r:status=r.status;data=json.loads(r.read())
 assert status==expected,(route,status,data)
 return data
def start():
 p=subprocess.Popen([str(binary),'serve','--data-dir',str(root),'--port',str(args.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
 for _ in range(100):
  try:api('/health');return p
  except Exception:assert p.poll() is None;time.sleep(.1)
 raise AssertionError('Private runtime failed to start')
def stop(p):p.terminate();p.wait(timeout=20)
def wait(job):
 for _ in range(900):
  j=api('/jobs/'+job['id']);assert j['status'] not in ['failed','interrupted','cancelled'],j
  if j['status']=='succeeded' and j.get('settled'):return j
  time.sleep(.1)
 raise AssertionError('Native processing did not settle')
def png_check(key,data,url):
 import base64,io
 a=np.asarray(Image.open(io.BytesIO(base64.b64decode(url.split(',')[1]))).convert('RGBA'));h,w=a.shape[:2]
 selected=data[np.ix_(np.arange(h,dtype=np.uint64)*data.shape[0]//h,np.arange(w,dtype=np.uint64)*data.shape[1]//w)].astype('int64');_,_,fill,low,high=layers[key]
 if key in ['vi_quality','vi_reliability']:
  index=selected&3 if key=='vi_quality' else selected
  palette=np.array([[37,99,235],[217,119,6],[139,92,246],[100,116,139]],dtype='uint8');rgb=palette[np.clip(index,0,3)]
  if key=='vi_reliability':rgb[(index<0)|(index>3)]=[100,116,139]
 else:
  gray=np.floor(np.clip((selected-low)/(high-low),0,1)*255+.5).astype('uint8');rgb=np.repeat(gray[...,None],3,axis=2)
 rgba=np.concatenate([rgb,np.where(selected==fill,0,255).astype('uint8')[...,None]],axis=-1)
 assert np.array_equal(a,rgba),(key,int(np.count_nonzero(a!=rgba)))
 return h*w
def inspect(job,data,ds,case):
 key=job['assetKey'];dtype,scale,fill,low,high=layers[key];m=api(f"/jobs/{job['id']}/raster");t=api(f"/jobs/{job['id']}/thumbnail");v=m['science']
 assert m['dataType']=={'int8':'Int8','int16':'Int16','uint16':'UInt16'}[dtype] and m['bandCount']==1 and m['nodata']==fill
 assert not any(k in m for k in ['reflectance','vegetation','quality','radar','elevation'])
 assert v['band']==key and v['scale']==scale and v['offset']==0 and v['validRange']==[low,high] and v['displayRange']==[low,high] and v['countsFullResolution'] is False
 assert ds.dtypes==(dtype,) and ds.nodata==fill and ds.tags()['AREA_OR_POINT']=='Area'
 assert [ds.width,ds.height]==[m['width'],m['height']] and np.allclose(ds.bounds,m['bounds'],atol=1e-7,rtol=0)
 assert ds.crs.to_dict()=={'proj':'sinu','lon_0':0,'x_0':0,'y_0':0,'R':6371007.181,'units':'m','no_defs':True}
 h,w=m['previewHeight'],m['previewWidth'];sampled=data[np.ix_(np.arange(h,dtype=np.uint64)*ds.height//h,np.arange(w,dtype=np.uint64)*ds.width//w)];valid=sampled!=fill
 assert v['sampleCount']==h*w and v['validSampleCount']==int(valid.sum()) and v['outOfRangeSampleCount']==int(np.count_nonzero(valid&((sampled<low)|(sampled>high))))
 checked=png_check(key,data,m['previewDataUrl'])+png_check(key,data,t['dataUrl'])
 coordinates={(0,0),(ds.height-1,ds.width-1),(ds.height//2,ds.width//2)}
 for mask in [data==fill,(data!=fill)&(data<0),(data!=fill)&(data==0),(data!=fill)&((data.astype('uint16')&32768)!=0) if key=='vi_quality' else data==fill,data==data[data!=fill].min(),data==data[data!=fill].max()]:
  if np.any(mask):coordinates.add(tuple(map(int,np.unravel_index(np.argmax(mask),mask.shape))))
 pixels=[]
 for row,col in sorted(coordinates):
  x,y=ds.xy(row,col);p=api(f"/jobs/{job['id']}/pixel?x={x}&y={y}");raw=int(data[row,col]);s=p['science'];missing=raw==fill
  assert p['pixel']==[col,row] and p['value']==raw and p['sha256']==job['sha256'] and p['isNoData']==missing
  assert s['withinRange']==(not missing and low<=raw<=high)
  if key=='vi_quality' and not missing:
   fields=s['flags']['fields'];assert len(fields)==9 and s['flags']['hex']==f'0x{raw:04X}' and s['flags']['binary']==f'{raw:016b}'
   for field,(begin,end) in zip(fields,[(0,1),(2,5),(6,7),(8,8),(9,9),(10,10),(11,13),(14,14),(15,15)]):assert [field['startBit'],field['endBit']]==[begin,end] and field['value']==(raw>>begin)&((1<<(end-begin+1))-1)
   assert fields[1]['defined']==(fields[1]['value'] in [0,1,2,4,8,9,10,12,13,14,15])
  elif not missing and key=='vi_doy':assert s['date']==(datetime(v['calendarYear'],1,1)+timedelta(days=raw-1)).date().isoformat()
  elif not missing and scale!=1:assert abs(s['convertedValue']-raw*scale)<1e-12
  if missing:assert not any(k in s for k in ['date','convertedValue','flags'])
  pixels.append(p)
 return {'job':job,'metadata':m,'thumbnail':t,'pixels':pixels,'pngPixelsCompared':checked,'dnPixelsRead':int(data.size),'noDataPixels':int(np.count_nonzero(data==fill)),'minimumDn':int(data[data!=fill].min()),'maximumDn':int(data[data!=fill].max())}
inverse=Transformer.from_pipeline('+proj=pipeline +step +inv +proj=sinu +R=6371007.181 +lon_0=0 +x_0=0 +y_0=0 +step +proj=unitconvert +xy_in=rad +xy_out=deg')
def oracle(job,project,dst):
 key=job['assetKey'];dtype,scale,fill,_,_=layers[key];expected=np.full((dst.height,dst.width),fill,dtype=dtype);covered=np.zeros(expected.shape,dtype=bool);winner=np.full(expected.shape,-1,dtype='int16')
 ordered=[originals[p['jobId']] for p in job['mosaic']['sources']];dates={s['itemId']:s['date'] for s in project['scenes']}
 assert [(dates[j['itemId']],j['itemId']) for j in ordered]==sorted((dates[j['itemId']],j['itemId']) for j in ordered)
 for i,j in enumerate(ordered):
  assert j['assetKey']==key and sha(j['outputPath'])==j['sha256']
  with rasterio.open(j['outputPath']) as src:
   dx,dy=(src.transform.c-dst.transform.c)/dst.transform.a,(src.transform.f-dst.transform.f)/dst.transform.e
   assert abs(dx-round(dx))<1e-5 and abs(dy-round(dy))<1e-5;dx,dy=round(dx),round(dy)
   x0,x1=max(0,dx),min(dst.width,dx+src.width);y0,y1=max(0,dy),min(dst.height,dy+src.height)
   if x1<=x0 or y1<=y0:continue
   a=src.read(1,window=rasterio.windows.Window(x0-dx,y0-dy,x1-x0,y1-y0));valid=a!=fill
   expected[y0:y1,x0:x1][valid]=a[valid];covered[y0:y1,x0:x1][valid]=True;winner[y0:y1,x0:x1][valid]=i
 masked=np.zeros(expected.shape,dtype=bool)
 if project.get('geometry'):
  rows,cols=np.indices(expected.shape);lon,lat=inverse.transform(dst.transform.c+(cols+.5)*dst.transform.a,dst.transform.f+(rows+.5)*dst.transform.e)
  masked=~shapely.contains_xy(shape(project['geometry']),lon,lat);expected[masked]=fill;covered[masked]=False;winner[masked]=-1
 assert np.array_equal(dst.read(1),expected)
 assert job['mosaicOutput']['coveredPixels']==int(covered.sum()) and job['mosaicOutput']['maskedPixels']==int(masked.sum())
 assert dst.scales==(scale,) and dst.offsets==(0.,) and dst.tags()['PRODUCT']=='modis-13q1-v061' and dst.tags()['SCIENCE_KEY']==key
 return {'dnPixelsCompared':int(expected.size),'winnerCounts':{j['itemId']:int(np.count_nonzero(winner==i)) for i,j in enumerate(ordered)},'maskedPixels':int(masked.sum())}
process=None
try:
 process=start();assert api('/proxy')['mode']=='custom'
 for entry in sources['cases']:
  j=originals[entry['job']['id']];assert sha(j['outputPath'])==j['sha256'] and Path(j['outputPath']).stat().st_size==j['bytesDownloaded']==j['totalBytes']
  with rasterio.open(j['outputPath']) as ds:
   assert (ds.width,ds.height)==(4800,4800);result=inspect(j,ds.read(1),ds,'original');report['originals'].append(result)
  dump();print(json.dumps({'stage':'independent-original','key':j['assetKey'],'dn':result['dnPixelsRead']}),flush=True)
 scenes=sources['project']['scenes'];west=next(s for s in scenes if s['itemId'].startswith('MOD13Q1') and '.h08v05.' in s['itemId']);bounds=[-114.1,37.78,-113.5,37.9]
 drafts=[{'name':'QA · science single','bounds':[-122.55,37.68,-122.32,37.84],'scenes':[west]},
  {'name':'QA · science adjacent','bounds':bounds,'scenes':scenes},
  {'name':'QA · science polygon with hole','bounds':bounds,'scenes':scenes,'geometry':{'type':'Polygon','coordinates':[[[-114.1,37.78],[-113.5,37.78],[-113.5,37.9],[-114.1,37.9],[-114.1,37.78]],[[-113.9,37.82],[-113.9,37.86],[-113.7,37.86],[-113.7,37.82],[-113.9,37.82]]]}},
  {'name':'QA · science temporal','bounds':[-122.42,37.96,-122.30,38.06],'scenes':[s for s in scenes if '.h08v05.' in s['itemId']]},]
 for case,draft in zip(['single','mosaic','polygon','temporal'],drafts):
  project=api('/projects',draft,201);report['cases'].append({'name':case,'project':project})
  for key in layers:
   job=wait(api(f"/projects/{project['id']}/mosaics",{'assetKey':key},202));assert sha(job['outputPath'])==job['sha256']
   with rasterio.open(job['outputPath']) as ds:
    comparison=oracle(job,project,ds);result=inspect(job,ds.read(1),ds,case);result.update(comparison);result['case']=case;report['outputs'].append(result)
   dump();print(json.dumps({'stage':'independent-output','case':case,'key':key,**comparison}),flush=True)
 before=(len(api('/projects')),len(api('/jobs')))
 for key in layers:
  for field,value in [('nodata',0 if layers[key][2]!=0 else -9999),('scale',100),('dataType','uint8')]:
   bad=copy.deepcopy(drafts[0]);bad['name']='QA invalid science';bad['scenes'][0]['assets'][key]['rasterBand'][field]=value;api('/projects',bad,400);report['negativeControls'].append(key+':'+field)
 for field in ['date','crs','channel']:
  bad=copy.deepcopy(drafts[0]);bad['name']='QA invalid science'
  if field=='date':bad['scenes'][0]['date']='2025-06-27T00:00:00Z'
  if field=='crs':bad['scenes'][0]['crs']='EPSG:32610'
  if field=='channel':bad['scenes'][0]['assets']['vi_red']['href']=bad['scenes'][0]['assets']['vi_nir']['href']
  api('/projects',bad,400);report['negativeControls'].append(field)
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
