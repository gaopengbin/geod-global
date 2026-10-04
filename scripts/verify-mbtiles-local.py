"""Independent SQLite + MVT/Pillow reference for local MBTiles import and replay.

Requires mapbox-vector-tile and Pillow in the verification environment, not in the product.
An existing report pins complete input bytes and native inventory for restart replay.
"""
import argparse,base64,gzip,hashlib,io,json,sqlite3,subprocess,tempfile,urllib.request,zipfile,platform
from importlib.metadata import version
from datetime import datetime,timezone
from pathlib import Path
import mapbox_vector_tile
from PIL import Image

def sha(data):return hashlib.sha256(data).hexdigest()
def get(url):
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(url,timeout=60) as r:return r.read()
def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',default='target/debug/geod-runtime.exe');p.add_argument('--data-dir',required=True);p.add_argument('--reference-dir',required=True);p.add_argument('--report',required=True);p.add_argument('--server');p.add_argument('--import-file',action='append',default=[]);a=p.parse_args()
    refs=Path(a.reference_dir);refs.mkdir(parents=True,exist_ok=True);records=json.loads((refs/'packages.json').read_text(encoding='utf-8')) if (refs/'packages.json').exists() else []
    def native(*parts):return subprocess.check_output([a.binary,'tile-packages',*parts,'--data-dir',a.data_dir],encoding='utf-8')
    for filename in a.import_file:
        assert not a.server,'Imports use stopped isolated stores'
        asset=json.loads(native('open','--file',filename));data=Path(filename).read_bytes();assert asset['sha256']==sha(data)
        (refs/(asset['id']+'.mbtiles')).write_bytes(data)
        if not any(old['id']==asset['id'] for old in records):records.append(asset)
    if records:(refs/'packages.json').write_text(json.dumps(records,ensure_ascii=False,indent=2),encoding='utf-8')
    else:records=json.loads((refs/'packages.json').read_text(encoding='utf-8'))
    results=[]
    for asset in records:
        identifier=asset['id'];b=(refs/(identifier+'.mbtiles')).read_bytes();assert sha(b)==asset['sha256'];db=sqlite3.connect((refs/(identifier+'.mbtiles')).resolve().as_uri()+'?mode=ro',uri=True)
        assert db.execute('PRAGMA integrity_check').fetchone()[0]=='ok';metadata=dict(db.execute('SELECT name,value FROM metadata'));entries=list(db.execute('SELECT zoom_level,tile_column,tile_row,tile_data FROM tiles'));assert len(entries)==len(asset['tiles'])
        checked=json.loads(get(a.server+'/tile-packages/'+identifier)) if a.server else json.loads(native('inspect','--id',identifier))
        assert checked['asset']==asset;assert checked['metadata']['mbtiles']==metadata
        receipts={(t['coordinate']['z'],t['coordinate']['x'],t['coordinate']['y']):t for t in asset['tiles']};features=0;pixels=0
        for z,x,tms,raw in entries:
            y=2**z-1-tms;t=receipts[(z,x,y)];assert t['sha256']==sha(raw);assert t['bytes']==len(raw);assert 'sourceOffset' not in t and 'packageOffset' not in t
            if a.server:response=json.loads(get(f'{a.server}/tile-packages/{identifier}/tiles/{z}/{x}/{y}'))
            else:
                request=refs/'tile-request.json';request.write_text(json.dumps({'id':identifier,'z':z,'x':x,'y':y}),encoding='utf-8');response=json.loads(native('tile','--request',str(request)))
            decoded=base64.b64decode(response['dataBase64']);assert sha(decoded)==response['sha256'];assert response['coordinate']=={'z':z,'x':x,'y':y}
            if metadata['format']=='pbf':
                assert decoded==gzip.decompress(raw);reference=mapbox_vector_tile.decode(decoded);counts={name:len(layer['features']) for name,layer in reference.items()};assert counts=={l['name']:l['features'] for l in response['layers']};assert response['layers']==t['layers'];assert 'contentType' not in response
                features+=sum(counts.values())
            else:
                assert decoded==raw;im=Image.open(io.BytesIO(raw));im.load();pix=im.convert('RGBA').tobytes();assert Image.open(io.BytesIO(decoded)).convert('RGBA').tobytes()==pix;assert list(im.size)==[t['image']['width'],t['image']['height']];assert response['contentType']==t['image']['contentType'];assert not response['layers'];pixels+=im.width*im.height
        db.close()
        if a.server:export=get(a.server+'/tile-packages/'+identifier+'/export')
        else:
            with tempfile.TemporaryDirectory(prefix='geod-mbtiles-export-') as temporary:
                dest=Path(temporary)/'map.zip';native('export','--id',identifier,'--out',str(dest));export=dest.read_bytes()
        with zipfile.ZipFile(io.BytesIO(export)) as z:
            assert set(z.namelist())=={'tiles.mbtiles','source.json','README.txt','checksums.sha256'};assert z.read('tiles.mbtiles')==b;assert json.loads(z.read('source.json'))==asset
            for line in z.read('checksums.sha256').decode().splitlines():checksum,name=line.split('  ',1);assert sha(z.read(name))==checksum
        baseline=refs/(identifier+'.zip')
        if baseline.exists():assert baseline.read_bytes()==export
        else:baseline.write_bytes(export)
        results.append({'id':identifier,'fileName':asset['source']['local']['fileName'],'format':metadata['format'],'bytes':len(b),'sha256':sha(b),'tiles':len(entries),'vectorRecordsAcrossTiles':features,'imagePixels':pixels,'fullOriginalUnchanged':True,'tmsToXyzVerified':True,'completeZipUnchanged':True})
    Path(a.report).write_text(json.dumps({'checkedAt':datetime.now(timezone.utc).isoformat(),'mode':'HTTP replay' if a.server else 'Native CLI import/replay','independentReferences':['Python sqlite3','mapbox-vector-tile','Pillow'],'referenceVersions':{'Python':platform.python_version(),'SQLite':sqlite3.sqlite_version,'mapbox-vector-tile':version('mapbox-vector-tile'),'Pillow':version('Pillow')},'results':results},ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps(results,ensure_ascii=False))
if __name__=='__main__':main()
