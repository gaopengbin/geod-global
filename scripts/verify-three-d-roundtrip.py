"""Actual local import and repeated ZIP round-trip against an isolated native store.

Use only a QA store whose service/desktop has been stopped. Source evidence must
come from verify-three-d-public.py, not synthetic parser controls.
"""
import argparse,base64,hashlib,json,shutil,subprocess,urllib.parse,zipfile
from datetime import datetime,timezone
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,value):p.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def require(value,message):
    if not value:raise AssertionError(message)
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for flag in ['data-dir','nested-evidence','texture-evidence','output']:parser.add_argument('--'+flag,required=True,type=Path)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/geod-runtime.exe')
    a=parser.parse_args();a.output.mkdir(parents=True,exist_ok=True);commands=[]
    def call(action,request=None,ident=None,extra=None):
        args=[str(a.binary),'three-d',action,'--data-dir',str(a.data_dir)]
        if request is not None:
            file=a.output/f'{len(commands):03d}-{action}-request.json';save(file,request);args+=['--request',str(file)]
        if ident:args+=['--id',ident]
        args+=extra or [];result=subprocess.run(args,capture_output=True,timeout=210)
        label=f'{len(commands):03d}-{action}';(a.output/(label+'.stdout.json')).write_bytes(result.stdout);(a.output/(label+'.stderr.txt')).write_bytes(result.stderr)
        commands.append({'arguments':args[1:],'exitCode':result.returncode});save(a.output/'commands.json',commands)
        require(result.returncode==0,result.stderr.decode('utf-8',errors='replace'));return json.loads(result.stdout)
    def bytes_for(asset,resource):
        reply=call('resource',{'id':asset['id'],'resourceId':resource['id']});require(reply['resource']==resource,'Resource receipt changed')
        b=base64.b64decode(reply['dataBase64'],validate=True);require(sha(b)==resource['sha256']and len(b)==resource['bytes'],'Original bytes changed');return b
    records=[]
    for evidence in [a.nested_evidence,a.texture_evidence]:
        parent=json.loads((evidence/'asset.json').read_text(encoding='utf-8'));source_export=evidence/'geod-3d-export.zip'
        require(call('inspect',ident=parent['id'])==parent,'Source package changed since public verification')
        current=parent
        for generation in range(1,3):
            imported=call('open',{'name':f'QA 3D original round-trip {generation} {parent["resources"][0]["kind"]}','rights':parent['rights']},extra=['--file',str(source_export)])
            require(imported['resources']==parent['resources']and imported['entry']==parent['entry'],'Import altered original locators, graph or receipt members')
            require(imported['bytes']==parent['bytes'],'Import changed original byte total')
            require(imported['importedFrom']['sourceReceiptSha256']==current['receiptSha256'],'Prior source receipt was lost')
            if generation==2:require(imported['importedFrom']['previous']['sourceReceiptSha256']==parent['receiptSha256'],'Original source history was lost')
            exported=a.output/f'export-{parent["id"]}-{generation}.zip';call('export',ident=imported['id'],extra=['--out',str(exported)])
            compared=0
            with zipfile.ZipFile(exported)as actual,zipfile.ZipFile(evidence/'geod-3d-export.zip')as original:
                for member in original.namelist():
                    if member=='manifest.json':continue
                    require(actual.read(member)==original.read(member),'Re-export changed original or localized source member');compared+=1
            for resource in imported['resources']:bytes_for(imported,resource)
            records.append({'id':imported['id'],'parent':current['id'],'generation':generation,'originalResources':len(imported['resources']),'originalBytes':imported['bytes'],'comparedExportMembers':compared,'resourceLocatorsAndOriginalHashesUnchanged':True,'exportSha256':sha(exported.read_bytes())})
            current=imported;source_export=exported
    texture=json.loads((a.texture_evidence/'asset.json').read_text(encoding='utf-8'))
    folder=(a.output/'owned-test-input').resolve()
    require(folder.is_relative_to(a.output.resolve())and folder.is_relative_to(ROOT/'.verification'),'Local test input must stay inside this repository QA directory')
    require(not folder.exists(),'Use a fresh output directory; never delete pre-existing input')
    folder.mkdir();entry=None
    for resource in texture['resources']:
        name=Path(urllib.parse.unquote(urllib.parse.urlsplit(resource['locator']).path)).name
        require(name not in ['', '.', '..']and not(folder/name).exists(),'Ambiguous source filenames')
        (folder/name).write_bytes(bytes_for(texture,resource))
        if resource['id']==texture['entry']:entry=folder/name
    local=call('open',{'name':'QA local BoxTextured unchanged files','rights':texture['rights']},extra=['--file',str(entry)])
    require(local['origin']=='local-files','Local source origin changed')
    require(sorted((r['sha256'],r['bytes'],r['kind'])for r in local['resources'])==sorted((r['sha256'],r['bytes'],r['kind'])for r in texture['resources']),'Local import changed original files')
    # Only remove files created above in the validated, fresh QA input folder.
    require(folder.resolve().is_relative_to(a.output.resolve())and not folder.is_symlink(),'Test input path escaped before removal')
    shutil.rmtree(folder)
    require(call('inspect',ident=local['id'])==local,'Saved local scene depends on removed input')
    for resource in local['resources']:bytes_for(local,resource)
    save(a.output/'report.json',{'verifiedAt':datetime.now(timezone.utc).isoformat(),'nativeBinarySha256':sha(a.binary.read_bytes()),'demonstrationData':True,'originalBytesAndLocalizedExportViewsUnchanged':True,'archiveCopies':records,'localInput':{'assetId':local['id'],'resources':len(local['resources']),'bytes':local['bytes'],'inputRemoved':True,'nativeReadbackStillWorks':True},'commands':len(commands)})
if __name__=='__main__':main()
