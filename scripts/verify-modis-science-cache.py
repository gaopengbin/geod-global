"""Actual ancillary science outputs with all source parents absent.
Uses accepted copies in a private store; blocked upstream, no user window.
"""
import argparse,copy,hashlib,json,shutil,subprocess,time,urllib.request,urllib.error
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('accepted',type=Path);parser.add_argument('root',type=Path);parser.add_argument('--port',type=int,default=4643);args=parser.parse_args()
root=args.root.resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('modis-science-');root.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
receipt=args.accepted/'verification.json';accepted=json.loads(receipt.read_text(encoding='utf-8'));assert accepted['status']=='passed'
binary_sha=accepted['nativeBinarySha256'];binary=root/f'runtime-{binary_sha[:16]}.exe';shutil.copy2(args.accepted/binary.name,binary);assert sha(binary)==binary_sha
(root/'assets').mkdir();entries=accepted['outputs'];assert len(entries)==40 and len(accepted['originals'])==30
jobs={e['job']['id']:copy.deepcopy(e['job']) for e in accepted['originals']+entries}
for j in jobs.values():
 j['outputPath']=str(root/'assets'/f"{j['id']}.tif")
 if j.get('manifestPath'):j['manifestPath']=str(root/'assets'/f"{j['id']}.metadata.json")
for e in entries:
 j=e['job'];assert sha(j['outputPath'])==j['sha256'];shutil.copy2(j['outputPath'],jobs[j['id']]['outputPath']);shutil.copy2(j['manifestPath'],jobs[j['id']]['manifestPath'])
(root/'jobs.json').write_text(json.dumps(jobs,indent=2),encoding='utf-8')
(root/'projects.json').write_text(json.dumps({e['project']['id']:e['project'] for e in accepted['cases']},indent=2),encoding='utf-8')
(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}),encoding='utf-8')
base=f'http://127.0.0.1:{args.port}';process=None
report={'schema':'geod-modis-science-cache/v1','sourceReceiptSha256':sha(receipt),'nativeBinarySha256':binary_sha,'upstreamBlocked':True,'usedUserDesktop':False,'nativeWindowTested':False,'status':'pending'}
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
 raise AssertionError('Private cache runtime failed to start')
def stop(p):p.terminate();p.wait(timeout=15)
def check():
 for e in entries:
  assert api(f"/jobs/{e['job']['id']}/raster")==e['metadata'];assert api(f"/jobs/{e['job']['id']}/thumbnail")==e['thumbnail']
  for p in e['pixels']:
   x,y=p['coordinate'];assert api(f"/jobs/{e['job']['id']}/pixel?x={x}&y={y}")==p
try:
 assert not any(Path(jobs[e['job']['id']]['outputPath']).exists() for e in accepted['originals'])
 process=start();check();cache_dir=root/'cache/thumbnails/v1'
 cache={p.name:(p.read_bytes(),p.stat().st_ino,p.stat().st_ctime_ns) for p in cache_dir.glob('*.json')};assert len(cache)==40
 stop(process);process=start();check()
 for name,(data,inode,created) in cache.items():
  p=cache_dir/name;assert p.read_bytes()==data and p.stat().st_ino==inode and p.stat().st_ctime_ns==created
 report.update(originalsAbsentDuringColdAndRestartReads=30,derivedFilesRestored=40,persistentEntriesNotRegenerated=40,pixelsCompared=sum(len(e['pixels']) for e in entries)*2)
 # Damage cached PNG metadata for every sample type and kind; no parents can rebuild it.
 kinds={key:next(e for e in entries if e['job']['assetKey']==key) for key in ['vi_quality','vi_reliability','vi_doy','vi_red','vi_relative_azimuth']}
 for key,e in kinds.items():
  p=next(p for p in cache_dir.glob('*.json') if json.loads(p.read_text())['preview']['jobId']==e['job']['id'])
  data=json.loads(p.read_text());data['png_sha256']='0'*64;p.write_text(json.dumps(data),encoding='utf-8')
  assert api(f"/jobs/{e['job']['id']}/thumbnail")==e['thumbnail'];assert json.loads(p.read_text())['png_sha256']!='0'*64
 report['corruptedCacheRebuiltWithoutParents']=list(kinds)
 for key,e in kinds.items():
  path=Path(jobs[e['job']['id']]['outputPath']);contents=path.read_bytes()
  try:
   with path.open('ab') as f:f.write(b'\0')
   for route in ['raster','thumbnail']:api(f"/jobs/{e['job']['id']}/{route}",400)
  finally:path.write_bytes(contents)
  assert sha(path)==e['job']['sha256'];assert api(f"/jobs/{e['job']['id']}/thumbnail")==e['thumbnail']
 report['changedDerivedRejectsCachedPreview']=list(kinds);report['status']='passed'
finally:
 if process and process.poll() is None:stop(process)
 (root/'cache-verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report))
