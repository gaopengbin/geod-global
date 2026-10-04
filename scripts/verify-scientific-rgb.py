"""Native offline scientific RGB acceptance using previously downloaded public originals.

Creates only a fresh QA workspace. Rasterio/GDAL independently reads every DN,
calibration, geometry and package member. No provider request is made.
"""
import argparse, atexit, hashlib, json, shutil, subprocess, time, zipfile, urllib.request
from pathlib import Path
import numpy as np
import rasterio
from rasterio.windows import Window

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''): h.update(block)
    return h.hexdigest()

def verify_processed_band(source, output, path=None):
    """Compare a real rectangular MODIS clip directly with its original band."""
    result_path = Path(path or output['outputPath'])
    assert sha(result_path) == output['sha256']
    with rasterio.open(source['outputPath']) as original, rasterio.open(result_path) as result:
        # Older single-band writers normalize custom-CRS citation names; all
        # mathematical projection, sphere, axes and unit parameters must match.
        assert result.crs.to_dict() == original.crs.to_dict()
        assert result.count == 1 and result.dtypes == original.dtypes
        assert result.nodata == original.nodata and result.scales == original.scales and result.offsets == original.offsets
        window = rasterio.windows.from_bounds(*result.bounds, transform=original.transform)
        numbers = [window.col_off, window.row_off, window.width, window.height]
        assert all(abs(v - round(v)) < 1e-6 for v in numbers)
        col, row, width, height = map(round, numbers)
        assert width == result.width and height == result.height
        assert 0 <= col and 0 <= row and col + width <= original.width and row + height <= original.height
        expected = original.read(1, window=Window(col, row, width, height))
        assert np.array_equal(result.read(1), expected)
        return {'jobId': output['id'], 'band': source['assetKey'], 'sha256': output['sha256'], 'originalSha256': source['sha256'], 'originalWindow': [col, row, width, height], 'samplesCompared': width * height}

def main():
    p=argparse.ArgumentParser();p.add_argument('--landsat-root',required=True);p.add_argument('--modis-root',required=True);p.add_argument('--output',required=True);p.add_argument('--exe',default='target/debug/geod-runtime.exe');p.add_argument('--port',type=int,default=4598);p.add_argument('--reuse-inputs',action='store_true');a=p.parse_args()
    root=Path(a.output).resolve()
    if root.exists():
        assert a.reuse_inputs and root.parent.name=='.verification' and root.name.startswith('scientific-rgb-'), 'Use a fresh isolated QA output directory'
        old=json.loads((root/'jobs.json').read_text(encoding='utf-8'));assert all(j['kind']=='download' and j['status']=='succeeded' for j in old.values()), 'Reuse only an input-only QA workspace'
    root.mkdir(parents=True,exist_ok=a.reuse_inputs);(root/'assets').mkdir(exist_ok=a.reuse_inputs); sources={};jobs={};project={};before={}
    for label,folder in [('landsat',a.landsat_root),('modis',a.modis_root)]:
        data=json.loads((Path(folder)/'jobs.json').read_text(encoding='utf-8'))
        triplet=[next(j for j in data.values() if j['kind']=='download' and j['status']=='succeeded' and j['assetKey']==key and (label=='landsat' or 'h08v05' in j['itemId'])) for key in ['red','green','blue']]
        sources[label]=triplet
        for job in triplet:
            original=Path(job['outputPath']);before[str(original)]={'sha256':sha(original),'mtimeNs':original.stat().st_mtime_ns}
            assert before[str(original)]['sha256']==job['sha256']
            target=root/'assets'/f"{job['id']}.tif";shutil.copy2(original,target);job['outputPath']=str(target);jobs[job['id']]=job
        if label=='modis':
            project=next(v for v in json.loads((Path(folder)/'projects.json').read_text(encoding='utf-8')).values() if len(v['scenes'])==1 and v['name']=='QA · MODIS single real band clip')
    (root/'jobs.json').write_text(json.dumps(jobs,ensure_ascii=False,indent=2),encoding='utf-8');(root/'projects.json').write_text(json.dumps({project['id']:project}),encoding='utf-8')
    # All work is offline, including the later direct MCP session.
    (root/'proxy-settings.json').write_text(json.dumps({'mode':'custom','url':'http://127.0.0.1:9'}),encoding='utf-8')
    exe=str(Path(a.exe).resolve()); receipts=[];base=f'http://127.0.0.1:{a.port}';process=None
    def stop():
        nonlocal process
        if process is not None and process.poll() is None:process.terminate();process.wait(timeout=20)
        process=None
    atexit.register(stop)
    def http(path,body=None):
        req=urllib.request.Request(base+path,data=json.dumps(body).encode() if body is not None else None,headers={'X-GeoD-Client':'geod-global','Content-Type':'application/json'})
        with urllib.request.urlopen(req,timeout=120) as response:return json.load(response)
    def start_service():
        nonlocal process
        process=subprocess.Popen([exe,'serve','--data-dir',str(root),'--port',str(a.port)],stdout=(root/'runtime.stdout.log').open('ab'),stderr=(root/'runtime.stderr.log').open('ab'),creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        for _ in range(100):
            assert process.poll() is None,(root/'runtime.stderr.log').read_text(encoding='utf-8')
            try:
                health=http('/health');assert Path(health['storageRoot']).samefile(root);return
            except OSError:time.sleep(.1)
        raise AssertionError('Isolated QA service did not start')
    start_service()
    def cli(*args):
        run=subprocess.run([exe,*args,'--server',base],capture_output=True,text=True,encoding='utf-8',timeout=600,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        assert run.returncode==0,(args,run.stderr[:3000]);return json.loads(run.stdout)
    def combine(label,inputs,project_id=None):
        request={'jobIds':[j['id'] for j in inputs],'name':f'QA · {label} scientific RGB'}
        if project_id:request['projectId']=project_id
        path=root/f'{label}-request.json';path.write_text(json.dumps(request),encoding='utf-8')
        planned=cli('scientific-rgb','plan','--request',str(path));start=time.time();job=cli('scientific-rgb','run','--request',str(path));seconds=time.time()-start
        assert job['kind']=='raster_rgb' and job['status']=='succeeded';output=Path(job['outputPath']);assert sha(output)==job['sha256']
        originals=[rasterio.open(j['outputPath']) for j in inputs];counts=[0,0,0];joint=0;hashes=[hashlib.sha256() for _ in range(3)];total=0
        with rasterio.open(output) as result:
            expected=originals[0];assert result.count==3 and result.crs==expected.crs and result.transform==expected.transform and result.bounds==expected.bounds and result.res==expected.res
            assert result.dtypes==(expected.dtypes[0],)*3 and result.nodata==expected.nodata
            profile=planned['spec']['profile'];assert result.scales==(profile['scale'],)*3 and result.offsets==(profile['offset'],)*3
            assert result.descriptions==('red','green','blue'); assert [v.name for v in result.colorinterp]==['red','green','blue']
            for row in range(0,result.height,64):
                window=Window(0,row,result.width,min(64,result.height-row));actual=result.read(window=window);valid=[]
                for c,source in enumerate(originals):
                    pixels=source.read(1,window=window);assert np.array_equal(actual[c],pixels),(label,c,row)
                    hashes[c].update(actual[c].astype('<i2' if profile['signed'] else '<u2',copy=False).tobytes());flags=pixels!=profile['nodata'];counts[c]+=int(flags.sum());valid.append(flags)
                joint+=int(np.logical_and.reduce(valid).sum());total+=actual.size
            assert counts==job['rgbOutput']['channelValidPixels'] and joint==job['rgbOutput']['commonValidPixels']
            assert [h.hexdigest() for h in hashes]==job['rgbOutput']['samplesSha256']
            point=[expected.bounds.left+expected.res[0]*100.5,expected.bounds.top-expected.res[1]*100.5]
            if expected.width<101 or expected.height<101:point=[expected.bounds.left+expected.res[0]*.5,expected.bounds.top-expected.res[1]*.5]
            sample=cli('scientific-rgb','pixel','--id',job['id'],'--x',str(point[0]),'--y',str(point[1]));assert sample['values']==result.read(window=Window(*sample['pixel'],1,1))[:,0,0].tolist()
            grid={'width':result.width,'height':result.height,'crs':result.crs.to_string(),'nodata':result.nodata,'scales':result.scales,'offsets':result.offsets,'dtypes':result.dtypes}
        for source in originals:source.close()
        metadata=cli('scientific-rgb','inspect','--id',job['id']);assert metadata['artifact']['sha256']==job['sha256'] and metadata['previewOmitted']
        package=cli('scientific-rgb','package','--id',job['id']);repeat=cli('scientific-rgb','package','--id',job['id']);assert package['sha256']==repeat['sha256']
        with zipfile.ZipFile(package['path']) as archive:
            assert archive.testzip() is None;assert len(archive.namelist())==5
            checks=archive.read('checksums.sha256').decode().splitlines()
            for line in checks:
                digest,name=line.split('  ',1);h=hashlib.sha256()
                with archive.open(name) as stream:
                    for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
                assert h.hexdigest()==digest
            manifest=json.loads(archive.read(f"{job['id']}.metadata.json"));assert manifest['spec']==planned['spec'] and manifest['output']['sha256']==job['sha256']
        record={'label':label,'jobId':job['id'],'sourceJobs':[{'id':j['id'],'sha256':j['sha256'],'itemId':j['itemId'],'href':j['href']} for j in inputs], 'sha256':job['sha256'],'bytes':job['bytesDownloaded'],'elapsedSeconds':round(seconds,3),'grid':grid,'channelsCompared':total,'outputSamples':job['rgbOutput'],'package':{k:package[k] for k in ['sha256','bytes','files']},'pixel':sample}
        receipts.append(record);print(json.dumps({'label':label,'jobId':job['id'],'channelsCompared':total,'seconds':round(seconds,2)},ensure_ascii=False),flush=True);return job
    combine('landsat-original',sources['landsat'])
    combine('modis-original',sources['modis'])
    processed=[];processing_checks=[]
    for source in sources['modis']:
        submitted=http(f"/projects/{project['id']}/mosaics",{'assetKey':source['assetKey']})
        for _ in range(1200):
            result=http(f"/jobs/{submitted['id']}")
            if result.get('settled') and result['status'] not in ['queued','running']:break
            time.sleep(.1)
        assert result['status']=='succeeded',result.get('error');processing_checks.append(verify_processed_band(source,result));processed.append(result)
    derived=combine('modis-project-clip',processed,project['id'])
    # Move only fresh QA input files away, then reopen via CLI. Originals elsewhere remain untouched.
    moved=root/'removed-inputs';moved.mkdir()
    for source in processed:Path(source['outputPath']).rename(moved/Path(source['outputPath']).name)
    stop();start_service()
    independent=cli('scientific-rgb','inspect','--id',derived['id']);assert independent['artifact']['sha256']==derived['sha256']
    cli('scientific-rgb','package','--id',derived['id'])
    for path,state in before.items():assert sha(path)==state['sha256'] and Path(path).stat().st_mtime_ns==state['mtimeNs']
    receipt={'schema':'geod-scientific-rgb-acceptance/v1','nativeBinarySha256':sha(exe),'offlineProxy':'http://127.0.0.1:9','cases':receipts,'processedBandChecks':processing_checks,'derivedWorksWithoutParentFiles':True,'originalsUnchanged':True,'reference':'Rasterio '+rasterio.__version__+' / GDAL '+rasterio.__gdal_version__,'qaOnly':True}
    (root/'verification.json').write_text(json.dumps(receipt,ensure_ascii=False,indent=2),encoding='utf-8')
    (root/'ui-fixture.json').write_text(json.dumps({'derived':derived,'projectId':project['id'],'sourceIds':[j['id'] for j in sources['modis']]}),encoding='utf-8')
    stop();atexit.unregister(stop)

if __name__=='__main__':main()
