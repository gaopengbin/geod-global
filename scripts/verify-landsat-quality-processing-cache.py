"""Offline derived-QA persistence, with all six parent originals/jobs absent.

Corruption controls mutate only disposable isolated copies, then restore them.
"""
import argparse,base64,copy,hashlib,json,shutil,subprocess,time,urllib.request,urllib.error,zipfile
from pathlib import Path
def sha(data):return hashlib.sha256(data).hexdigest()
def dump(p,data):p.write_text(json.dumps(data,indent=2),encoding='utf-8')
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--source',required=True);parser.add_argument('--root',required=True);parser.add_argument('--port',type=int,default=4647);a=parser.parse_args()
    source,root=Path(a.source).resolve(),Path(a.root).resolve()
    for p in [source,root]:assert p.parent==Path('.verification').resolve() and p.name.startswith('landsat-quality-processing-')
    assert source!=root and not root.exists();receipt=(source/'native-processing-verification.json').read_bytes();native=json.loads(receipt);oracle=json.loads((source/'independent-processing-verification.json').read_text(encoding='utf-8'));assert native['status']==oracle['status']=='passed' and oracle['nativeReceiptSha256']==sha(receipt)
    root.mkdir();(root/'assets').mkdir();exe=root/Path(native['nativeBinary']).name;shutil.copy2(native['nativeBinary'],exe);assert sha(exe.read_bytes())==native['nativeBinarySha256']
    jobs={};parents=set()
    for entry in native['outputs']:
        job=copy.deepcopy(entry['job']);parents.update(pin['jobId'] for pin in job['mosaic']['sources']+job['mosaic'].get('coverageSources',[]));target=root/'assets'/f"{job['id']}.tif";metadata=root/'assets'/f"{job['id']}.metadata.json"
        shutil.copy2(job['outputPath'],target);shutil.copy2(job['manifestPath'],metadata);assert sha(target.read_bytes())==job['sha256'];job.update(outputPath=str(target),manifestPath=str(metadata));job.pop('settled',None);jobs[job['id']]=job
    assert not parents.intersection(jobs);dump(root/'jobs.json',jobs);shutil.copy2(source/'projects.json',root/'projects.json');dump(root/'proxy-settings.json',{'mode':'custom','url':'http://127.0.0.1:9'})
    report={'schema':'geod-landsat-quality-processing-cache/v1','qaOnly':True,'nativeBinarySha256':native['nativeBinarySha256'],'nativeReceiptSha256':sha(receipt),'allParentJobsAndFilesAbsent':True,'missingParentJobs':sorted(parents),'network':'unreachable upstream proxy','cases':[],'controls':[]}
    base=f'http://127.0.0.1:{a.port}';opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));process=None
    def api(route,body=None,rejected=False):
        request=urllib.request.Request(base+route,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json','X-GeoD-Client':'geod-global'})
        try:
            with opener.open(request,timeout=120) as response:data=json.load(response)
        except urllib.error.HTTPError as e:
            reply=json.loads(e.read());assert rejected,reply;return {'status':e.code,'reply':reply}
        assert not rejected;return data
    def stop():
        nonlocal process
        if process and process.poll() is None:process.terminate();process.wait(timeout=10)
        process=None
    def start():
        nonlocal process
        process=subprocess.Popen([str(exe),'serve','--data-dir',str(root),'--port',str(a.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        for i in range(100):
            try:
                health=api('/health');assert Path(health['storageRoot']).samefile(root);assert api('/proxy')=={'mode':'custom','url':'http://127.0.0.1:9'};assert len(api('/jobs'))==len(jobs);return
            except OSError:
                assert process.poll() is None
                if i==99:raise
                time.sleep(.1)
    def entries():
        result={}
        for p in (root/'cache/thumbnails/v1').glob('*.json'):
            content=p.read_bytes();entry=json.loads(content);job_id=entry['preview']['jobId']
            if job_id in jobs:
                stat=p.stat();result[job_id]={'path':p,'bytes':content,'inode':stat.st_ino,'createdNs':stat.st_ctime_ns}
        assert len(result)==len(jobs);return result
    thumbnails={}
    try:
        start()
        for entry in native['outputs']:
            job=jobs[entry['job']['id']];identifier=job['id'];meta=api(f'/jobs/{identifier}/raster');assert meta['quality']==entry['metadata']['quality'] and meta['classes']==entry['metadata']['classes'];assert sha(base64.b64decode(meta['previewDataUrl'].split(',',1)[1]))==sha(base64.b64decode(entry['metadata']['previewDataUrl'].split(',',1)[1]))
            thumb=api(f'/jobs/{identifier}/thumbnail');assert thumb==entry['thumbnail'];thumbnails[identifier]=thumb
            for expected in entry['pixels']:
                x,y=expected['coordinate'];assert api(f'/jobs/{identifier}/pixel?x={x}&y={y}')==expected
            report['cases'].append({'case':entry['case'],'key':entry['key'],'id':identifier,'sha256':job['sha256'],'readWithoutParents':True,'rawSamplesCompared':len(entry['pixels'])});print(json.dumps({'case':entry['case'],'key':entry['key'],'readWithoutParents':True}),flush=True)
        cache=entries();pinned=next(iter(jobs.values()));tiff=Path(pinned['outputPath']);original=tiff.read_bytes()
        try:
            tiff.write_bytes(original+b'\0');failures={route:api(f"/jobs/{pinned['id']}/{route}",rejected=True) for route in ['raster','thumbnail']};report['controls'].append({'name':'Changed output rejected despite existing thumbnail cache','failures':failures})
        finally:tiff.write_bytes(original)
        assert api(f"/jobs/{pinned['id']}/thumbnail")==thumbnails[pinned['id']]
        damaged=cache[pinned['id']]['path'];damaged.write_bytes(b'{invalid cache');assert api(f"/jobs/{pinned['id']}/thumbnail")==thumbnails[pinned['id']];report['controls'].append({'name':'Corrupt thumbnail cache rebuilt','passed':True})
        before=entries();stop();start()
        for row in report['cases']:
            started=time.perf_counter();assert api(f"/jobs/{row['id']}/thumbnail")==thumbnails[row['id']];prior=before[row['id']];stat=prior['path'].stat();assert prior['path'].read_bytes()==prior['bytes'] and stat.st_ino==prior['inode'] and stat.st_ctime_ns==prior['createdNs'];row.update(cacheBytesFileIdentityCreationUnchanged=True,warmRestartMilliseconds=(time.perf_counter()-started)*1000)
        report.update(status='passed',restarted=True,originalAcceptedStoreUnchanged=True,boundaries=['Project quality GeoTIFFs are self-contained; delivery ZIP packages for generic project mosaics are not enabled.']);dump(root/'cache-verification.json',report);print(json.dumps({'status':'passed','readWithoutParents':len(jobs),'cacheEntriesReused':len(jobs)}))
    except Exception as e:report.update(status='failed',failure=str(e));dump(root/'cache-verification.json',report);raise
    finally:stop()
if __name__=='__main__':main()
