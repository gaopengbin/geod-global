"""Read accepted quality-screened indices without any source parents.
Private offline store, actual files, persistent cache and refusal controls.
"""
import argparse, copy, hashlib, json, shutil, subprocess, time, urllib.request, urllib.error
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('accepted',type=Path);parser.add_argument('root',type=Path);parser.add_argument('--port',type=int,default=4653);args=parser.parse_args()
root=args.root.resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('modis-vi-quality-');root.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();receipt=args.accepted/'verification.json';accepted=json.loads(receipt.read_text(encoding='utf-8'));assert accepted['status']=='passed'
binary_sha=accepted['nativeBinarySha256'];binary=root/f'runtime-{binary_sha[:16]}.exe';shutil.copy2(args.accepted/binary.name,binary);assert sha(binary)==binary_sha
entries=accepted['outputs'];assert len(entries)==20 and len(accepted['originals'])==12
(root/'assets').mkdir();jobs={}
for e in entries:
 j=copy.deepcopy(e['job']);assert sha(j['outputPath'])==j['sha256'];j['outputPath']=str(root/'assets'/f"{j['id']}.tif");j['manifestPath']=str(root/'assets'/f"{j['id']}.metadata.json");shutil.copy2(e['job']['outputPath'],j['outputPath']);shutil.copy2(e['job']['manifestPath'],j['manifestPath']);jobs[j['id']]=j
# Both original files AND job records are absent, not only inaccessible paths.
assert not any(e['job']['id'] in jobs for e in accepted['originals'])
(root/'jobs.json').write_text(json.dumps(jobs,indent=2),encoding='utf-8');(root/'projects.json').write_text('{}');(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}))
base=f'http://127.0.0.1:{args.port}';process=None
report={'schema':'geod-modis-vi-quality-cache/v1','nativeBinarySha256':binary_sha,'nativeReceiptSha256':sha(receipt),'upstreamBlocked':True,'usedUserDesktop':False,'nativeWindowTested':False,'status':'pending','negativeControls':[]}
def api(route,expected=200):
 try:r=urllib.request.urlopen(base+route,timeout=65)
 except urllib.error.HTTPError as e:r=e
 with r:status=r.status;data=json.loads(r.read())
 assert status==expected,(route,status,data);return data
def start():
 p=subprocess.Popen([str(binary),'serve','--data-dir',str(root),'--port',str(args.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
 for _ in range(100):
  try:api('/health');return p
  except Exception:assert p.poll() is None;time.sleep(.1)
 raise AssertionError('Private cache service did not start')
def stop(p):p.terminate();p.wait(timeout=15)
def check():
 for e in entries:
  j=e['job'];assert api('/jobs/'+j['id']+'/raster')==e['metadata'];assert api('/jobs/'+j['id']+'/thumbnail')==e['thumbnail'];assert api('/jobs/'+j['id']+'/metadata')==e['manifest']
  for pixel in e['pixels']:
   x,y=pixel['coordinate'];assert api(f"/jobs/{j['id']}/pixel?x={x}&y={y}")==pixel
try:
 process=start();assert api('/projects')==[];check();cache_dir=root/'cache/thumbnails/v1';cache={p.name:(sha(p),p.stat().st_ino,p.stat().st_ctime_ns) for p in cache_dir.glob('*.json')};assert len(cache)==20
 stop(process);process=start();check();assert cache=={p.name:(sha(p),p.stat().st_ino,p.stat().st_ctime_ns) for p in cache_dir.glob('*.json')}
 report.update(originalRecordsAndFilesAbsent=12,projectsAbsent=True,derivedFilesRestored=20,persistentEntriesNotRegenerated=20,pixelsCompared=sum(len(e['pixels']) for e in entries)*2)
 for key in ['ndvi','evi']:
  e=next(e for e in entries if e['job']['assetKey']==key and e['case']=='temporal' and e['policy']=='good');j=jobs[e['job']['id']];p=next(p for p in cache_dir.glob('*.json') if json.loads(p.read_text())['preview']['jobId']==j['id'])
  damaged=json.loads(p.read_text());damaged['png_sha256']='0'*64;p.write_text(json.dumps(damaged));assert api('/jobs/'+j['id']+'/thumbnail')==e['thumbnail'];assert json.loads(p.read_text())['png_sha256']!='0'*64;report['negativeControls'].append(key+': corrupt cache rebuilt without parents')
  path=Path(j['outputPath']);content=path.read_bytes();path.write_bytes(content+b'\0')
  for operation in ['raster','thumbnail']:api('/jobs/'+j['id']+'/'+operation,400)
  path.write_bytes(content);assert sha(path)==j['sha256'];assert api('/jobs/'+j['id']+'/thumbnail')==e['thumbnail'];report['negativeControls'].append(key+': changed result refuses cached preview')
 # Move one contribution while keeping all total-count equations valid. Both
 # the inspector and disk-cache miss must compare the actual TIFF description.
 stop(process);pristine=copy.deepcopy(jobs);target=next(j for j in jobs.values() if j['assetKey']=='ndvi' and len(j['mosaic']['viSelection']['scenes'])==2 and j['mosaicOutput']['viQuality']['sceneValidPixels'][0]>1)
 target['mosaicOutput']['viQuality']['sceneValidPixels'][0]-=1;target['mosaicOutput']['viQuality']['sceneValidPixels'][1]+=1;(root/'jobs.json').write_text(json.dumps(jobs,indent=2));process=start()
 for operation in ['raster','thumbnail','metadata']:api('/jobs/'+target['id']+'/'+operation,400)
 report['negativeControls'].append('changed contribution counts refuse raster, cached thumbnail and provenance')
 stop(process);(root/'jobs.json').write_text(json.dumps(pristine,indent=2));process=start();check();report['status']='passed'
except Exception as e:report.update(status='failed',failure=repr(e));raise
finally:
 if process and process.poll() is None:stop(process)
 (root/'cache-verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report))
