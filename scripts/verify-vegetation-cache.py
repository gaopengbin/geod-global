"""Cold and persistent index previews with all original source files absent.

Copies only accepted actual outputs into a new private store. Corruption and
source-change controls operate on these copies, never on accepted originals.
"""
import argparse
import copy
import hashlib
import json
import shutil
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('base',type=Path);parser.add_argument('temporal',type=Path);parser.add_argument('root',type=Path)
parser.add_argument('--port',type=int,default=4637);args=parser.parse_args()
root=args.root.resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('modis-vegetation-');root.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reports=[json.loads((p/'verification.json').read_text(encoding='utf-8')) for p in [args.base,args.temporal]]
assert all(r['status']=='passed' for r in reports) and reports[0]['nativeBinarySha256']==reports[1]['nativeBinarySha256']
binary_sha=reports[0]['nativeBinarySha256'];binary=root/f'runtime-{binary_sha[:16]}.exe'
shutil.copy2(args.base/binary.name,binary);assert sha(binary)==binary_sha
(root/'assets').mkdir();entries=reports[0]['outputs']+reports[1]['outputs'];assert len(entries)==8
all_jobs={e['job']['id']:copy.deepcopy(e['job']) for e in reports[0]['originals']+entries}
for job in all_jobs.values():
    job['outputPath']=str(root/'assets'/f"{job['id']}.tif")
    if job.get('manifestPath'):job['manifestPath']=str(root/'assets'/f"{job['id']}.metadata.json")
for e in entries:
    job=e['job'];assert sha(job['outputPath'])==job['sha256'];shutil.copy2(job['outputPath'],all_jobs[job['id']]['outputPath'])
    shutil.copy2(job['manifestPath'],all_jobs[job['id']]['manifestPath'])
(root/'jobs.json').write_text(json.dumps(all_jobs,indent=2),encoding='utf-8')
(root/'projects.json').write_text(json.dumps({e['project']['id']:e['project'] for r in reports for e in r['cases']},indent=2),encoding='utf-8')
(root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}),encoding='utf-8')
base=f'http://127.0.0.1:{args.port}';process=None
report={'schema':'geod-modis-vegetation-cache/v1','nativeBinarySha256':binary_sha,'sourceReceipts':[{ 'path':str(p.resolve()/'verification.json'),'sha256':sha(p/'verification.json')} for p in [args.base,args.temporal]],'upstreamBlocked':True,'usedUserDesktop':False,'nativeWindowTested':False,'status':'pending'}
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
    raise AssertionError('Private cache runtime did not start')
def stop(p):p.terminate();p.wait(timeout=15)
def check():
    for e in entries:
        assert api(f"/jobs/{e['job']['id']}/raster")==e['metadata']
        assert api(f"/jobs/{e['job']['id']}/thumbnail")==e['thumbnail']
        for pixel in e['pixels']:
            x,y=pixel['coordinate'];assert api(f"/jobs/{e['job']['id']}/pixel?x={x}&y={y}")==pixel
        print(json.dumps({'stage':'parent-free-preview','key':e['job']['assetKey'],'case':e['case']}),flush=True)
try:
    assert not any(Path(all_jobs[e['job']['id']]['outputPath']).exists() for e in reports[0]['originals'])
    process=start();check();cache_dir=root/'cache/thumbnails/v1'
    cached={p.name:(p.read_bytes(),p.stat().st_ino,p.stat().st_ctime_ns) for p in cache_dir.glob('*.json')};assert len(cached)==8
    stop(process);process=start();check()
    for name,(data,inode,created) in cached.items():
        p=cache_dir/name;assert p.read_bytes()==data and p.stat().st_ino==inode and p.stat().st_ctime_ns==created
    report['originalsAbsentDuringColdAndRestartReads']=6
    report['derivedFilesRestored']=report['previewsCompared']=report['diskEntriesReusedWithoutRegeneration']=8
    report['pixelsCompared']=sum(len(e['pixels']) for e in entries)*2
    target=entries[-1];job=all_jobs[target['job']['id']]
    cached_path=next(p for p in cache_dir.glob('*.json') if json.loads(p.read_text())['preview']['jobId']==job['id'])
    damaged=json.loads(cached_path.read_text());damaged['png_sha256']='0'*64;cached_path.write_text(json.dumps(damaged),encoding='utf-8')
    assert api(f"/jobs/{job['id']}/thumbnail")==target['thumbnail']
    repaired=json.loads(cached_path.read_text());assert repaired['png_sha256']!='0'*64
    report['corruptedPngHashRegeneratedIdentically']=True
    assets_to_change=[target]+[reports[0]['originals'][0]]
    for e in assets_to_change:
        j=all_jobs[e['job']['id']];p=Path(j['outputPath'])
        if not p.exists():
            # The preceding parent-free startup correctly marks missing source
            # downloads unavailable. Seed this separate corruption control from
            # its accepted bytes and job record while the store is closed.
            stop(process);shutil.copy2(e['job']['outputPath'],p)
            saved=json.loads((root/'jobs.json').read_text(encoding='utf-8'));saved[j['id']]=j
            (root/'jobs.json').write_text(json.dumps(saved,indent=2),encoding='utf-8');process=start()
        assert sha(p)==j['sha256'];assert api(f"/jobs/{j['id']}/thumbnail")==e['thumbnail'];contents=p.read_bytes()
        try:
            with p.open('ab') as f:f.write(b'\0')
            for route in ['raster','thumbnail']:api(f"/jobs/{j['id']}/{route}",400)
        finally:p.write_bytes(contents)
        assert sha(p)==j['sha256'];assert api(f"/jobs/{j['id']}/raster")==e['metadata'];assert api(f"/jobs/{j['id']}/thumbnail")==e['thumbnail']
    report['changedOriginalAndDerivedRejectCachedPreview']=True
    report['restoredCopiesReturnIdenticalPreviews']=True;report['status']='passed'
finally:
    if process and process.poll() is None:stop(process)
    (root/'cache-verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k not in ['sourceReceipts']}))
