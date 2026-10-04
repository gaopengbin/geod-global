"""Actual native 3D source acquisition, independent byte/graph checks and export.

The public Cesium assets are generated demonstrations, not production geography.
Run only against an isolated QA data directory, with its service stopped.
"""
import argparse,base64,hashlib,json,struct,subprocess,urllib.request,zipfile
from datetime import datetime,timezone
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SAMPLE='https://raw.githubusercontent.com/CesiumGS/cesium/main/Specs/Data/Cesium3DTiles/Tilesets/TilesetOfTilesets/tileset.json'
LICENSE='https://github.com/CesiumGS/3d-tiles-samples-generator/blob/main/README.md'
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def require(v,message):
    if not v:raise AssertionError(message)
def document(b,kind):
    if kind=='b3dm':
        require(b[:4]==b'b3dm' and struct.unpack_from('<II',b,4)==(1,len(b)),'b3dm header')
        start=28+sum(struct.unpack_from('<IIII',b,12))
        b=b[start:]
    if kind in ['glb','b3dm']:
        require(b[:4]==b'glTF' and struct.unpack_from('<II',b,4)==(2,len(b)),'GLB header')
        n,k=struct.unpack_from('<II',b,12);require(k==0x4e4f534a,'GLB JSON');return json.loads(b[20:20+n])
    if kind in ['tileset','gltf','schema']:return json.loads(b)
    return None
def pointer(v,path):
    keys=[s.replace('~1','/').replace('~0','~')for s in path[1:].split('/')]
    for k in keys[:-1]:v=v[int(k)]if isinstance(v,list)else v[k]
    return v,int(keys[-1])if isinstance(v,list)else keys[-1]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--data-dir',required=True,type=Path);p.add_argument('--output',required=True,type=Path);p.add_argument('--binary',type=Path,default=ROOT/'target/debug/geod-runtime.exe');p.add_argument('--offline',action='store_true');p.add_argument('--source',default=SAMPLE);p.add_argument('--name',default='Cesium nested tiles · public demo');p.add_argument('--license',default='CC0-1.0');p.add_argument('--attribution',default='CesiumGS · generated 3D Tiles demonstration assets');p.add_argument('--license-url',default=LICENSE);p.add_argument('--min-resources',type=int,default=8);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    commands=[]
    def call(action,request=None,ident=None,extra=None):
        cmd=[str(a.binary),'three-d',action,'--data-dir',str(a.data_dir)]
        if request is not None:
            path=a.output/f'{len(commands):03d}-{action}-request.json';save(path,request);cmd+=['--request',str(path)]
        if ident:cmd+=['--id',ident]
        cmd+=extra or [];r=subprocess.run(cmd,capture_output=True,timeout=210);label=f'{len(commands):03d}-{action}';(a.output/(label+'.stdout.json')).write_bytes(r.stdout);(a.output/(label+'.stderr.txt')).write_bytes(r.stderr);commands.append({'arguments':cmd[1:],'exitCode':r.returncode});save(a.output/'commands.json',commands);require(r.returncode==0,r.stderr.decode('utf-8',errors='replace'));return json.loads(r.stdout)
    if a.offline:
        def snapshot():
            files=[p for p in (a.data_dir/'three-d').iterdir()if p.is_file()]+[a.data_dir/'three-d.json']
            return{str(p.relative_to(a.data_dir)):{'bytes':p.stat().st_size,'modifiedNs':p.stat().st_mtime_ns,'sha256':sha(p.read_bytes())}for p in files}
        before=snapshot();packages=call('list');require(packages,'No actual source packages were restored')
        for asset in packages:
            restored=call('inspect',ident=asset['id']);require(restored==asset,'Offline receipt mismatch')
            for resource in asset['resources']:
                result=call('resource',{'id':asset['id'],'resourceId':resource['id']});b=base64.b64decode(result['dataBase64'],validate=True);require(sha(b)==resource['sha256'],'Offline resource mismatch')
        after=snapshot();require(before==after,'Offline read modified original bytes, mtimes or registry');save(a.output/'readonly-storage.json',before);save(a.output/'report.json',{'mode':'offline-restart','assets':len(packages),'resources':sum(len(p['resources'])for p in packages),'originalsUnchanged':True,'registryUnchanged':True,'originalMtimesUnchanged':True,'nativeBinarySha256':sha(a.binary.read_bytes()),'commands':len(commands)});return
    discovery=call('discover',{'url':a.source});request={'url':a.source,'discoverySha256':discovery['sha256'],'name':a.name,'rights':{'license':a.license,'attribution':a.attribution,'licenseUrl':a.license_url,'permissionConfirmed':True}};asset=call('save',request);save(a.output/'asset.json',asset);require(asset['origin']=='public-https','Source origin');require(len(asset['resources'])>=a.min_resources,'Complete dependency graph expected')
    controls=[];originals={};triangles=0
    for r in asset['resources']:
        req=urllib.request.Request(r['locator'],headers={'Accept-Encoding':'identity'});response=urllib.request.urlopen(req,timeout=45);b=response.read(32*1024*1024+1);require(sha(b)==r['sha256']and len(b)==r['bytes'],'Independent source bytes differ');originals[r['id']]=b;doc=document(b,r['kind'])
        if r['kind']in['gltf','glb','b3dm']:
            for mesh in doc.get('meshes',[]):
                for primitive in mesh['primitives']:
                    if primitive.get('mode',4)==4:
                        accessor=primitive.get('indices',primitive['attributes']['POSITION']);triangles+=doc['accessors'][accessor]['count']//3
        controls.append({'locator':r['locator'],'kind':r['kind'],'bytes':len(b),'sha256':sha(b),'responseEtag':response.headers.get('ETag')})
        native=call('resource',{'id':asset['id'],'resourceId':r['id']});require(native['resource']==r,'Native resource receipt mismatch');require(base64.b64decode(native['dataBase64'],validate=True)==b,'Native saved bytes differ')
    require(triangles>0,'No actual meshes found');require(call('inspect',ident=asset['id'])==asset,'Inspection mismatch')
    out=a.output/'geod-3d-export.zip';call('export',ident=asset['id'],extra=['--out',str(out)])
    with zipfile.ZipFile(out)as z:
        require(json.loads(z.read('manifest.json'))==asset,'Export manifest mismatch');require(len(z.namelist())==1+2*len(asset['resources']),'Export member mismatch')
        by_id={r['id']:r for r in asset['resources']}
        for r in asset['resources']:
            extension={'tileset':'json','gltf':'json','schema':'json','jpeg':'jpg','buffer':'bin'}.get(r['kind'],r['kind']);name=r['id']+'.'+extension;require(z.read('originals/'+name)==originals[r['id']],'Original changed on export');original=document(originals[r['id']],r['kind']);localized=document(z.read('scene/'+name),r['kind'])
            if original is not None:
                for link in r['links']:
                    parent,key=pointer(original,link['reference']['pointer']);require(parent[key]==link['reference']['uri'],'Dependency pointer mismatch');target=by_id[link['target']];ext={'tileset':'json','gltf':'json','schema':'json','jpeg':'jpg','buffer':'bin'}.get(target['kind'],target['kind']);parent[key]=target['id']+'.'+ext;require('scene/'+parent[key]in z.namelist(),'Export dependency missing')
                require(original==localized,'Transforms, metadata or non-URI glTF fields changed during localization')
    save(a.output/'independent.json',controls);save(a.output/'report.json',{'verifiedAt':datetime.now(timezone.utc).isoformat(),'source':a.source,'licenseReference':a.license_url,'assetId':asset['id'],'sourceReceiptSha256':asset['receiptSha256'],'originalBytes':asset['bytes'],'resources':len(asset['resources']),'tiles':asset['tileCount'],'independentMeshTriangles':triangles,'exportSha256':sha(out.read_bytes()),'allOriginalBytesMatch':True,'localizedMetadataAndTransformsMatch':True,'productionGeographicData':False,'commands':len(commands)})
if __name__=='__main__':main()
