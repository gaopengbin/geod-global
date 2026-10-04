"""Read persistent derived Landsat QA through direct and loopback stdio MCP."""
import argparse,hashlib,importlib.util,json,subprocess,time,urllib.request
from pathlib import Path
spec=importlib.util.spec_from_file_location('geod_verify_mcp',Path(__file__).with_name('verify-mcp.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
def sha(data):return hashlib.sha256(data).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('root');parser.add_argument('--port',type=int,default=4645);a=parser.parse_args()
    root=Path(a.root).resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-quality-processing-')
    receipt=(root/'native-processing-verification.json').read_bytes();native=json.loads(receipt);oracle=json.loads((root/'independent-processing-verification.json').read_text(encoding='utf-8'))
    assert native['status']==oracle['status']=='passed' and oracle['nativeReceiptSha256']==sha(receipt)
    exe=Path(native['nativeBinary']);assert sha(exe.read_bytes())==native['nativeBinarySha256']
    samples=json.loads((root/'sample-plan.json').read_text(encoding='utf-8'))
    report={'schema':'geod-landsat-quality-processing-mcp/v1','nativeBinarySha256':native['nativeBinarySha256'],'nativeReceiptSha256':sha(receipt),'readOnly':True,'cases':[]}
    def inspect(client,mode):
        tools=client.request('tools/list')['result']['tools']
        for name in ['geod_raster_inspect','geod_raster_pixel']:
            tool=next(t for t in tools if t['name']==name);assert tool['annotations']['readOnlyHint'] and 'Landsat' in tool['description']
        for case in native['cases']:assert client.call('geod_project_get',{'id':case['project']['id']})==case['project']
        for case in native['outputs']:
            job=case['job'];meta=client.call('geod_raster_inspect',{'id':job['id']});assert meta['sha256']==job['sha256'] and meta['quality']==case['metadata']['quality'] and meta['classes']==case['metadata']['classes'];assert meta['previewOmitted'] and 'previewDataUrl' not in meta
            for expected in case['pixels']:
                x,y=expected['coordinate'];assert client.call('geod_raster_pixel',{'id':job['id'],'x':x,'y':y})==expected
            for expected in samples[job['id']]:
                x,y=expected['coordinate'];raw=client.call('geod_raster_pixel',{'id':job['id'],'x':x,'y':y});assert raw['value']==expected['raw'] and raw['pixel']==expected['pixel'] and raw['isNoData']==(not expected['covered']) and raw['quality']['covered']==expected['covered']
            report['cases'].append({'mode':mode,'case':case['case'],'key':case['key'],'jobId':job['id'],'sha256':job['sha256'],'rawSamplesCompared':len(case['pixels'])+len(samples[job['id']]),'coverageMask':meta['quality']['flags']['coverageMask'],'previewOmitted':True})
    client=module.Client(exe,data_dir=root)
    try:inspect(client,'direct; unreachable upstream proxy')
    finally:client.close()
    base=f'http://127.0.0.1:{a.port}';runtime=subprocess.Popen([str(exe),'serve','--data-dir',str(root),'--port',str(a.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0));client=None
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        for i in range(100):
            try:
                with opener.open(base+'/health',timeout=2) as response:assert response.status==200
                break
            except OSError:
                assert runtime.poll() is None
                if i==99:raise
                time.sleep(.1)
        client=module.Client(exe,server=base);inspect(client,'loopback adapter; unreachable upstream proxy')
        runtime.terminate();runtime.wait(timeout=10);result=client.request('tools/call',{'name':'geod_raster_inspect','arguments':{'id':native['outputs'][0]['job']['id']}});assert result['result']['isError'];report['disconnectedAdapterRejected']=True
    finally:
        if client:client.close()
        if runtime.poll() is None:runtime.terminate();runtime.wait(timeout=10)
    report['status']='passed';(root/'mcp-processing-verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps({'status':'passed','cases':len(report['cases']),'rawSamplesCompared':sum(c['rawSamplesCompared'] for c in report['cases'])}))
if __name__=='__main__':main()
