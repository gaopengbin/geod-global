"""Independent MOD13 C61 same-observation quality-selection acceptance.

Uses actual application-downloaded COGs and a new private store with blocked
upstream. GDAL/NumPy/Shapely are independent of the native TIFF/QA code. This
does not operate or accept the user's native desktop window.
"""
import argparse, base64, copy, hashlib, io, json, math, shutil, subprocess, time, urllib.request, urllib.error
from pathlib import Path
from datetime import datetime, timezone
import numpy as np
import rasterio
from rasterio.transform import Affine
from PIL import Image
from pyproj import Transformer
import shapely
from shapely.geometry import shape

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('indices',type=Path);parser.add_argument('science',type=Path);parser.add_argument('root',type=Path);parser.add_argument('binary',type=Path)
parser.add_argument('--port',type=int,default=4651);args=parser.parse_args()
root=args.root.resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('modis-vi-quality-');root.mkdir(exist_ok=False)
keys=['ndvi','evi','vi_quality','vi_reliability'];sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report={'schema':'geod-modis-vi-quality-acceptance/v1','checkedAt':datetime.now(timezone.utc).isoformat(),'status':'pending','originals':[],'outputs':[],'cases':[],'negativeControls':[],'upstreamBlocked':True,'nativeWindowTested':False,'sourceReceipts':[]}
binary_sha=sha(args.binary);binary=root/f'runtime-{binary_sha[:16]}.exe';shutil.copy2(args.binary,binary);report['nativeBinarySha256']=binary_sha
jobs={};(root/'assets').mkdir();source_fingerprints={}
for folder in [args.indices,args.science]:
 receipt=folder/'source-verification.json';data=json.loads(receipt.read_text(encoding='utf-8'));assert data['status']=='passed';report['sourceReceipts'].append({'path':str(receipt.resolve()),'sha256':sha(receipt)})
 for entry in data['cases']:
  j=copy.deepcopy(entry['job'])
  if j['assetKey'] not in keys:continue
  path=Path(j['outputPath']);assert sha(path)==j['sha256'] and path.stat().st_size==j['bytesDownloaded']==j['totalBytes'];source_fingerprints[str(path)]=(sha(path),path.stat().st_mtime_ns)
  out=root/'assets'/f"{j['id']}.tif";shutil.copy2(path,out);j['outputPath']=str(out);jobs[j['id']]=j
assert len(jobs)==12
(root/'jobs.json').write_text(json.dumps(jobs,indent=2));(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}))
originals={j['id']:j for j in jobs.values()};base=f'http://127.0.0.1:{args.port}'
def dump(): (root/'verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
def api(route,body=None,expected=200):
 request=urllib.request.Request(base+route,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'},method='GET' if body is None else 'POST')
 try:r=urllib.request.urlopen(request,timeout=65)
 except urllib.error.HTTPError as e:r=e
 with r:status=r.status;raw=r.read()
 try:data=json.loads(raw)
 except json.JSONDecodeError:data={'error':raw.decode('utf-8')}
 assert status==expected,(route,status,data);return data
def start():
 p=subprocess.Popen([str(binary),'serve','--data-dir',str(root),'--port',str(args.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
 for _ in range(100):
  try:api('/health');return p
  except Exception:assert p.poll() is None;time.sleep(.1)
 raise AssertionError('Private runtime failed to start')
def stop(p):p.terminate();p.wait(timeout=20)
def settled(job,status='succeeded'):
 for _ in range(1200):
  j=api('/jobs/'+job['id'])
  if j.get('settled') and j['status'] not in ['queued','running']:assert j['status']==status,j;return j
  time.sleep(.1)
 raise AssertionError('Native processing did not settle')

# Reviewed NASA C61 table 1 / 4 / 5, expressed independently of app modules.
def qualifies(ndvi,evi,qa,rank,policy):
 complete=(ndvi>=-2000)&(ndvi<=10000)&(evi>=-2000)&(evi<=10000)
 common=(qa!=65535)&np.isin((qa>>2)&15,[0,1,2,4,8,9,10])&(((qa>>6)&3)!=3)&((qa&((1<<8)|(1<<10)|(1<<14)|(1<<15)))==0)
 accepted=complete&common&(((qa&3)==0)&(rank==0) if policy=='good' else ((qa&3)<=1)&np.isin(rank,[0,1]))
 return complete,accepted

def verify_png(data,url):
 actual=np.asarray(Image.open(io.BytesIO(base64.b64decode(url.split(',')[1]))).convert('RGBA'));h,w=actual.shape[:2]
 v=data[np.ix_(np.arange(h,dtype=np.uint64)*data.shape[0]//h,np.arange(w,dtype=np.uint64)*data.shape[1]//w)].astype('int64')
 stops=[(-2000,[42,91,135]),(0,[208,169,104]),(2000,[235,223,160]),(4000,[161,196,112]),(7000,[75,143,76]),(10000,[24,86,51])]
 bounded=np.clip(v,-2000,10000);rgb=np.zeros((h,w,3),dtype='uint8')
 for (lo,lc),(hi,hc) in zip(stops,stops[1:]):
  mask=(bounded>=lo)&(bounded<=hi)
  for channel in range(3):rgb[...,channel][mask]=((lc[channel]*(hi-bounded)+hc[channel]*(bounded-lo)+(hi-lo)//2)//(hi-lo))[mask]
 expected=np.concatenate([rgb,np.where(v==-3000,0,255).astype('uint8')[...,None]],axis=2)
 assert np.array_equal(actual,expected);return h*w

source_data={};grids={}
for j in jobs.values():
 with rasterio.open(j['outputPath']) as ds:
  assert (ds.width,ds.height)==(4800,4800) and ds.count==1 and ds.tags()['AREA_OR_POINT']=='Area'
  key=j['assetKey'];assert ds.dtypes==({'ndvi':'int16','evi':'int16','vi_quality':'uint16','vi_reliability':'int8'}[key],)
  assert ds.nodata==({'ndvi':-3000,'evi':-3000,'vi_quality':65535,'vi_reliability':-1}[key])
  source_data[j['id']]=ds.read(1);grids[j['id']]=(ds.transform,ds.crs,tuple(ds.bounds))
  assert ds.crs.to_dict()=={'proj':'sinu','lon_0':0,'x_0':0,'y_0':0,'R':6371007.181,'units':'m','no_defs':True}
crs=next(iter(grids.values()))[1];forward=Transformer.from_crs('EPSG:4326',crs,always_xy=True);inverse=Transformer.from_crs(crs,'EPSG:4326',always_xy=True)
def aligned_grid(project,primary):
 tr=grids[primary[0]['id']][0];union=[min(grids[j['id']][2][0] for j in primary),min(grids[j['id']][2][1] for j in primary),max(grids[j['id']][2][2] for j in primary),max(grids[j['id']][2][3] for j in primary)]
 west,south,east,north=project['bounds'];edge=np.linspace(0,1,65);lon=np.concatenate([west+(east-west)*edge,west+(east-west)*edge,np.full(65,west),np.full(65,east)]);lat=np.concatenate([np.full(65,south),np.full(65,north),south+(north-south)*edge,south+(north-south)*edge]);x,y=forward.transform(lon,lat)
 bounds=[max(min(x),union[0]),max(min(y),union[1]),min(max(x),union[2]),min(max(y),union[3])]
 snap=lambda n:round(n) if abs(n-round(n))<1e-7 else n
 x0=math.floor(snap((bounds[0]-tr.c)/tr.a));x1=math.ceil(snap((bounds[2]-tr.c)/tr.a));y0=math.floor(snap((bounds[3]-tr.f)/tr.e));y1=math.ceil(snap((bounds[1]-tr.f)/tr.e))
 return x1-x0,y1-y0,Affine(tr.a,0,tr.c+x0*tr.a,0,tr.e,tr.f+y0*tr.e)
def read_on_grid(job,dst,fill):
 tr=grids[job['id']][0];dx=(tr.c-dst.transform.c)/tr.a;dy=(tr.f-dst.transform.f)/tr.e;assert abs(dx-round(dx))<1e-5 and abs(dy-round(dy))<1e-5;dx,dy=round(dx),round(dy)
 out=np.full((dst.height,dst.width),fill,dtype=source_data[job['id']].dtype);x0,x1=max(0,dx),min(dst.width,dx+4800);y0,y1=max(0,dy),min(dst.height,dy+4800)
 if x1>x0 and y1>y0:out[y0:y1,x0:x1]=source_data[job['id']][y0-dy:y1-dy,x0-dx:x1-dx]
 return out
def oracle(job,project,dst):
 spec=job['mosaic']['viSelection'];result=job['mosaicOutput']['viQuality'];policy=spec['policy'];shape_=(dst.height,dst.width)
 ndvi=np.full(shape_,-3000,dtype='int16');evi=ndvi.copy();winner=np.zeros(shape_,dtype='uint32');latest=np.zeros(shape_,dtype='uint32')
 primary=[originals[p['jobId']] for p in job['mosaic']['sources']];w,h,tr=aligned_grid(project,primary);assert (w,h)==(dst.width,dst.height) and np.allclose(tuple(tr),tuple(dst.transform),rtol=0,atol=1e-7)
 outside=np.zeros(shape_,dtype=bool)
 if project.get('geometry'):
  rows,cols=np.indices(shape_);lon,lat=inverse.transform(tr.c+(cols+.5)*tr.a,tr.f+(rows+.5)*tr.e);outside=~shapely.contains_xy(shape(project['geometry']),lon,lat)
 order=[(s['compositeStart'],s['itemId']) for s in spec['scenes']];assert order==sorted(set(order))
 for index,scene in enumerate(spec['scenes'],1):
  selected=[originals[p['jobId']] for p in scene['sources']];assert [j['assetKey'] for j in selected]==keys and all(j['itemId']==scene['itemId'] for j in selected)
  for pin,j in zip(scene['sources'],selected):assert pin['sha256']==j['sha256'] and pin['bytes']==j['bytesDownloaded'] and pin['href']==j['href']
  a,b,q,r=[read_on_grid(j,dst,fill) for j,fill in zip(selected,[-3000,-3000,65535,-1])];complete,accepted=qualifies(a,b,q,r,policy);complete&=~outside;accepted&=~outside
  latest[complete]=index;winner[accepted]=index;ndvi[accepted]=a[accepted];evi[accepted]=b[accepted]
 valid=winner>0;covered=int(valid.sum());input_count=int(np.count_nonzero(latest));fallback=int(np.count_nonzero(valid&(winner<latest)));expected=ndvi if job['assetKey']=='ndvi' else evi
 assert np.array_equal(dst.read(1),expected)
 assert job['mosaicOutput']['coveredPixels']==covered and job['mosaicOutput']['maskedPixels']==int(outside.sum())
 assert result['countsFullResolution'] and result['inputCommonValidPixels']==input_count and result['removedValidPixels']==input_count-covered and result['rejectedPixels']==expected.size-covered and result['fallbackPixels']==fallback
 assert result['sceneValidPixels']==[int(np.count_nonzero(winner==i)) for i in range(1,len(spec['scenes'])+1)]
 assert result['indicesSha256']==[hashlib.sha256(v.astype('<i2').tobytes()).hexdigest() for v in [ndvi,evi]] and result['selectionSha256']==hashlib.sha256(winner.astype('<u4').tobytes()).hexdigest()
 assert json.loads(dst.tags()['TIFFTAG_IMAGEDESCRIPTION'])==result
 assert dst.dtypes==('int16',) and dst.nodata==-3000 and dst.scales==(.0001,) and dst.offsets==(0.,)
 return expected,winner,{'dnPixelsCompared':int(expected.size),'fallbackPixels':fallback,'inputCommonValidPixels':input_count,'removedValidPixels':input_count-covered,'sceneValidPixels':result['sceneValidPixels'],'maskedPixels':int(outside.sum())}
def inspect(job,data,ds,winner=None):
 m=api('/jobs/'+job['id']+'/raster');thumb=api('/jobs/'+job['id']+'/thumbnail');v=m['vegetation'];r=(job.get('mosaicOutput') or {}).get('viQuality');assert v.get('qualitySelection')==r
 assert m['dataType']=='Int16' and m['bandCount']==1 and m['nodata']==-3000 and v['scale']==.0001 and v['offset']==0 and v['index']==job['assetKey']
 assert not any(k in m for k in ['reflectance','science','quality','composite'])
 checked=verify_png(data,m['previewDataUrl'])+verify_png(data,thumb['dataUrl'])
 coords={(0,0),(ds.height//2,ds.width//2),(ds.height-1,ds.width-1)}
 for mask in [data==-3000,(data!=-3000)&(data<0),data==0,*([winner==i for i in np.unique(winner)] if winner is not None else [])]:
  if np.any(mask):coords.add(tuple(map(int,np.unravel_index(np.argmax(mask),data.shape))))
 pixels=[]
 for row,col in sorted(coords):
  x,y=ds.xy(row,col);p=api(f"/jobs/{job['id']}/pixel?x={x}&y={y}");raw=int(data[row,col]);assert p['pixel']==[col,row] and p['value']==raw and p['isNoData']==(raw==-3000) and p['sha256']==job['sha256'];assert p.get('indexValue')==(None if raw==-3000 else raw*.0001);pixels.append(p)
 return {'job':job,'metadata':m,'thumbnail':thumb,'pixels':pixels,'pngPixelsCompared':checked}

process=None
try:
 process=start();assert api('/proxy')['url']=='http://127.0.0.1:9'
 for j in jobs.values():
  m=api('/jobs/'+j['id']+'/raster');thumb=api('/jobs/'+j['id']+'/thumbnail')
  with rasterio.open(j['outputPath']) as ds:
   if j['assetKey'] in ['ndvi','evi']:record=inspect(j,source_data[j['id']],ds)
   else:
    y,x=2400,2400;cx,cy=ds.xy(y,x);p=api(f"/jobs/{j['id']}/pixel?x={cx}&y={cy}");assert p['value']==int(source_data[j['id']][y,x]);record={'job':j,'metadata':m,'thumbnail':thumb,'pixels':[p]}
   record['dnPixelsRead']=int(source_data[j['id']].size);report['originals'].append(record)
 scenes=json.loads((args.science/'source-verification.json').read_text(encoding='utf-8'))['project']['scenes'];west=next(s for s in scenes if s['itemId'].startswith('MOD13Q1') and '.h08v05.' in s['itemId']);temporal=[s for s in scenes if '.h08v05.' in s['itemId']]
 # A predeclared real cloud/marginal window from the two actual originals.
 bounds=[-118.40422472210591,36.58333333005001,-116.54345786104489,37.24999999665701]
 hole=[-117.9,36.8,-117.4,37.05]
 rectangle=lambda b:[[b[0],b[1]],[b[2],b[1]],[b[2],b[3]],[b[0],b[3]],[b[0],b[1]]]
 adjacent=[-114.1,37.78,-113.5,37.9]
 drafts=[('single',{'bounds':[-122.55,37.68,-122.32,37.84],'scenes':[west]}),('temporal',{'bounds':bounds,'scenes':temporal}),('polygon',{'bounds':bounds,'scenes':temporal,'geometry':{'type':'Polygon','coordinates':[rectangle(bounds),list(reversed(rectangle(hole)))]}}),('adjacent',{'bounds':adjacent,'scenes':[s for s in scenes if s['itemId'].startswith('MOD13Q1')]}),('three_scene_polygon',{'bounds':adjacent,'scenes':scenes,'geometry':{'type':'Polygon','coordinates':[rectangle(adjacent),list(reversed(rectangle([-113.9,37.82,-113.7,37.86])))]}})]
 for case,draft in drafts:
  project=api('/projects',{'name':'QA · vegetation quality '+case,**draft},201);report['cases'].append({'name':case,'project':project})
  for policy in ['good','usable']:
   pair=[]
   for key in ['ndvi','evi']:
    j=settled(api('/projects/'+project['id']+'/mosaics',{'assetKey':key,'viQuality':{'policy':policy}},202));assert sha(j['outputPath'])==j['sha256']
    with rasterio.open(j['outputPath']) as ds:
     data,winner,comparison=oracle(j,project,ds);entry=inspect(j,data,ds,winner);entry.update(comparison);entry.update(case=case,policy=policy);pair.append(j['mosaicOutput']['viQuality'])
    manifest=api('/jobs/'+j['id']+'/metadata');assert manifest['viSelection']==j['mosaic']['viSelection'] and manifest['plan']==j['mosaicOutput'];entry['manifest']=manifest;report['outputs'].append(entry);dump();print(json.dumps({'case':case,'policy':policy,'key':key,**comparison}),flush=True)
   assert pair[0]==pair[1]
 assert all(e['fallbackPixels']>0 for e in report['outputs'] if e['case'] in ['temporal','polygon'])
 before=len(api('/jobs'));project=report['cases'][0]['project']
 for payload in [{'assetKey':'ndvi','viQuality':{'policy':'unknown'}},{'assetKey':'vi_quality','viQuality':{'policy':'good'}},{'assetKey':'ndvi','viQuality':{'policy':'good','maskHref':'https://example.com'}},{'assetKey':'ndvi','viQuality':{'policy':'good'},'arbitraryPath':'C:/temp/a.tif'}]:
  api('/projects/'+project['id']+'/mosaics',payload,400 if payload['assetKey']=='vi_quality' else 422);report['negativeControls'].append({'name':'invalid request','payload':payload})
 assert len(api('/jobs'))==before
 bad=copy.deepcopy(drafts[0][1]);del bad['scenes'][0]['assets']['vi_quality'];bad=api('/projects',{'name':'QA · missing quality asset',**bad},201);before=len(api('/jobs'));api('/projects/'+bad['id']+'/mosaics',{'assetKey':'ndvi','viQuality':{'policy':'good'}},400);assert len(api('/jobs'))==before;report['negativeControls'].append({'name':'missing same-scene QA refused before enqueue'})
 original=next(j for j in jobs.values() if j['assetKey']=='vi_quality' and j['itemId']==west['itemId']);path=Path(original['outputPath']);unchanged=path.read_bytes();damaged=bytearray(unchanged);damaged[-1]^=1;path.write_bytes(damaged)
 failed=settled(api('/projects/'+project['id']+'/mosaics',{'assetKey':'ndvi','viQuality':{'policy':'good'}},202),'failed');assert 'SHA-256 changed' in failed['error'] and not (root/'assets'/f"{failed['id']}.tif").exists();path.write_bytes(unchanged)
 retried=settled(api('/jobs/'+failed['id']+'/retry',{},200));recorded=next(e for e in report['outputs'] if e['case']=='single' and e['policy']=='good' and e['job']['assetKey']=='ndvi');assert retried['sha256']==recorded['job']['sha256'] and retried['mosaicOutput']==recorded['job']['mosaicOutput'];report['negativeControls'].append({'name':'changed original QA refused; restored pinned source retry identical','job':retried})
 submitted=api('/projects/'+project['id']+'/mosaics',{'assetKey':'evi','viQuality':{'policy':'good'}},202);api('/jobs/'+submitted['id']+'/cancel',{},200);cancelled=settled(submitted,'cancelled');assert cancelled['outputPath'] is None and not (root/'assets'/f"{cancelled['id']}.tif").exists();assert not list((root/'assets').glob('*.part'));report['negativeControls'].append({'name':'native cancellation leaves no partial output','job':cancelled})
 # Restoring the damaged QA bytes intentionally changes its source timestamp.
 # Prove that this one stale fingerprint refreshes before measuring an ordinary
 # restart. A normal disk hit changes last-use time, but not content/identity.
 restored_entry=next(p for p in (root/'cache/thumbnails/v1').glob('*.json') if json.loads(p.read_text())['preview']['jobId']==original['id'])
 old_fingerprint=json.loads(restored_entry.read_text())['source']
 expected_preview=next(e['thumbnail'] for e in report['originals'] if e['job']['id']==original['id'])
 assert api('/jobs/'+original['id']+'/thumbnail')==expected_preview
 new_fingerprint=json.loads(restored_entry.read_text())['source']
 assert old_fingerprint['modified']!=new_fingerprint['modified']
 report['negativeControls'].append({'name':'restored original QA refreshes stale source fingerprint with identical preview'})
 cache={p.name:(sha(p),p.stat().st_ino,p.stat().st_ctime_ns) for p in (root/'cache/thumbnails/v1').glob('*.json')};assert len(cache)==32
 stop(process);process=start()
 for e in report['originals']+report['outputs']:
  j=e['job'];m=api('/jobs/'+j['id']+'/raster');assert m==e['metadata'];assert api('/jobs/'+j['id']+'/thumbnail')==e['thumbnail']
 assert cache=={p.name:(sha(p),p.stat().st_ino,p.stat().st_ctime_ns) for p in (root/'cache/thumbnails/v1').glob('*.json')};report['restart']={'rastersChecked':32,'cacheEntriesUnchanged':32}
 for path,expected in source_fingerprints.items():assert (sha(path),Path(path).stat().st_mtime_ns)==expected
 report.update(status='passed',outputDnPixelsCompared=sum(e['dnPixelsCompared'] for e in report['outputs']),previewAndThumbnailPixelsCompared=sum(e['pngPixelsCompared'] for e in report['outputs']),pairedSelectionsIdentical=True,originalDownloads=12)
 print(json.dumps({k:report[k] for k in ['status','outputDnPixelsCompared','previewAndThumbnailPixelsCompared','pairedSelectionsIdentical']}),flush=True)
except Exception as e:report['status']='failed';report['failure']=repr(e);raise
finally:
 if process is not None and process.poll() is None:stop(process)
 dump()
