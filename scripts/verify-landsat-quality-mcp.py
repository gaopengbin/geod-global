"""Actual read-only MCP inspection of persistent original Landsat quality COGs.

Run after verify-landsat-quality.mjs, while its isolated native service is stopped.
Tests direct mode and the loopback adapter, plus disconnected-adapter rejection.
"""
import argparse
import hashlib
import contextlib
import importlib.util
import json
import subprocess
import time
import urllib.request
from pathlib import Path

spec = importlib.util.spec_from_file_location('geod_verify_mcp',Path(__file__).with_name('verify-mcp.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root')
    parser.add_argument('--port',type=int,default=4601)
    args=parser.parse_args()
    root=Path(args.root).resolve()
    assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-quality-')
    native_receipt=(root/'native-verification.json').read_bytes()
    source=json.loads(native_receipt)
    assert source['status']=='passed'
    executable=str(Path(source['nativeBinary']).resolve())
    assert hashlib.sha256(Path(executable).read_bytes()).hexdigest()==source['nativeBinarySha256']
    report={'schema':'geod-landsat-quality-mcp/v1','nativeBinarySha256':source['nativeBinarySha256'],
            'nativeReceiptSha256':hashlib.sha256(native_receipt).hexdigest(),'readOnly':True,'cases':[]}

    def inspect(client, mode):
        tools=client.request('tools/list')['result']['tools']
        for name in ['geod_raster_inspect','geod_raster_pixel']:
            tool=next(t for t in tools if t['name']==name)
            assert tool['annotations']['readOnlyHint'] and 'Landsat' in tool['description']
        project=client.call('geod_project_get',{'id':source['project']['id']})
        assert project['scenes'][0]['assets']['qa_pixel']['href']==source['project']['scenes'][0]['assets']['qa_pixel']['href']
        for case in source['cases']:
            job=case['job']; metadata=client.call('geod_raster_inspect',{'id':job['id']})
            assert metadata['sha256']==job['sha256'] and metadata['quality']==case['metadata']['quality']
            assert metadata['classes']==case['metadata']['classes'] and metadata['previewOmitted'] and 'previewDataUrl' not in metadata
            for expected in case['pixels']:
                x,y=expected['coordinate']; pixel=client.call('geod_raster_pixel',{'id':job['id'],'x':x,'y':y})
                assert pixel==expected
            report['cases'].append({'mode':mode,'assetKey':case['key'],'sha256':job['sha256'],'pixelsCompared':len(case['pixels']),'previewOmitted':True,'allBitFieldsRetained':True})

    client=module.Client(executable,data_dir=root)
    try:
        inspect(client,'direct read-only; rejected upstream proxy')
    finally:
        client.close()
    base=f'http://127.0.0.1:{args.port}'
    runtime=subprocess.Popen([executable,'serve','--data-dir',str(root),'--port',str(args.port)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
    client=None
    try:
        for i in range(100):
            try:
                with urllib.request.urlopen(base+'/health',timeout=2) as response:
                    assert response.status==200
                break
            except Exception:
                assert runtime.poll() is None
                if i==99:raise
                time.sleep(.1)
        client=module.Client(executable,server=base)
        inspect(client,'loopback read-only; rejected upstream proxy')
        runtime.terminate();runtime.wait(timeout=10)
        failed=client.request('tools/call',{'name':'geod_raster_inspect','arguments':{'id':source['cases'][0]['job']['id']}})
        assert failed['result']['isError']
        report['disconnectedAdapterRejected']=True
    finally:
        if client:client.close()
        if runtime.poll() is None:
            runtime.terminate();runtime.wait(timeout=10)
    report['status']='passed'
    (root/'mcp-verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report))


if __name__=='__main__':
    main()
