"""Real PMTiles acceptance using the independent Protomaps reader and Mapbox MVT decoder.
Install verification-only pmtiles==3.8.1 and mapbox-vector-tile==2.2.0 in --python-path.
No product module or generated fixture is used as the reference reader.
"""
import argparse, base64, gzip, hashlib, io, json, sys, urllib.request, zipfile
from pathlib import Path
from datetime import datetime, timezone

p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4372');p.add_argument('--data-dir',required=True);p.add_argument('--python-path',required=True);p.add_argument('--out',required=True);p.add_argument('--offline',action='store_true');a=p.parse_args()
sys.path.insert(0,str(Path(a.python_path).resolve()))
from pmtiles.reader import Reader,MemorySource,all_tiles
from pmtiles.tile import Compression,TileType
import mapbox_vector_tile
sha=lambda b:hashlib.sha256(b).hexdigest()
out=Path(a.out);out.mkdir(parents=True,exist_ok=True);cache=out/'source-ranges';cache.mkdir(exist_ok=True)
public=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def request(path):
    with urllib.request.urlopen(a.server+path,timeout=30) as r:return r.read()
packages=json.loads(request('/tile-packages'));sources=json.loads(request('/tile-sources'));assert packages
reports=[]
for asset in packages:
    sid=asset['id'];s=asset['source'];blobs={}
    for r in asset['ranges']:
        saved=cache/(r['sha256']+'.bin')
        if a.offline:raw=saved.read_bytes()
        else:
            req=urllib.request.Request(s['url'],headers={'Range':f"bytes={r['offset']}-{r['offset']+r['bytes']-1}",'Accept-Encoding':'identity','If-Match':s['etag'],'User-Agent':'GeoD-Verification/1.0'})
            with public.open(req,timeout=45) as response:
                assert response.status==206 and response.headers['ETag']==s['etag']
                assert response.headers['Content-Range']==f"bytes {r['offset']}-{r['offset']+r['bytes']-1}/{s['totalBytes']}"
                raw=response.read(r['bytes']+1)
            saved.write_bytes(raw)
        assert len(raw)==r['bytes'] and sha(raw)==r['sha256'];blobs[(r['offset'],r['bytes'])]=raw
    def get_bytes(offset,length):return blobs[(offset,length)]
    reference=Reader(get_bytes);h=reference.header();assert h['tile_type']==TileType.MVT and h['tile_compression'] in (Compression.NONE,Compression.GZIP)
    assert reference.metadata()==s['metadata']
    blob=(Path(a.data_dir)/'tiles'/(sid+'.pmtiles')).read_bytes();assert sha(blob)==asset['sha256'] and len(blob)==asset['bytes']
    local=Reader(MemorySource(blob));listed=dict(all_tiles(MemorySource(blob)));assert set(listed)=={tuple(t['coordinate'][k] for k in ('z','x','y')) for t in asset['tiles']}
    assert local.header()['addressed_tiles_count']==len(listed)
    native=json.loads(request('/tile-packages/'+sid));assert native['asset']==asset;assert local.metadata()==native['metadata']
    total_features=0
    for tile in asset['tiles']:
        c=tuple(tile['coordinate'][k] for k in ('z','x','y'));original=reference.get(*c);stored=local.get(*c)
        assert stored==original==blob[tile['packageOffset']:tile['packageOffset']+tile['bytes']]
        assert original==get_bytes(tile['sourceOffset'],tile['bytes']) and sha(original)==tile['sha256']
        decoded=gzip.decompress(original) if h['tile_compression']==Compression.GZIP else original
        layers=mapbox_vector_tile.decode(decoded,default_options={'y_coord_down':True})
        summary=[{'name':name,'extent':layer['extent'],'version':layer['version'],'features':len(layer['features'])} for name,layer in layers.items()];assert summary==tile['layers'];total_features+=sum(l['features'] for l in summary)
        read=json.loads(request(f'/tile-packages/{sid}/tiles/{c[0]}/{c[1]}/{c[2]}'));assert base64.b64decode(read['dataBase64'])==decoded and read['sha256']==sha(decoded) and read['layers']==summary
    for c in asset['absent']:
        coordinate=tuple(c[k] for k in ('z','x','y'));assert reference.get(*coordinate) is None
        read=json.loads(request(f'/tile-packages/{sid}/tiles/{coordinate[0]}/{coordinate[1]}/{coordinate[2]}'));assert read['dataBase64'] is None
    archive=request('/tile-packages/'+sid+'/export');z=zipfile.ZipFile(io.BytesIO(archive));assert z.read('tiles.pmtiles')==blob and json.loads(z.read('source.json'))==asset
    for line in z.read('checksums.sha256').decode().splitlines():digest,name=line.split('  ');assert sha(z.read(name))==digest
    baseline=out/(sid+'.zip')
    if a.offline:assert baseline.read_bytes()==archive
    else:baseline.write_bytes(archive)
    reports.append({'id':sid,'name':asset['name'],'bounds':asset['requestedBounds'],'levels':[asset['minZoom'],asset['maxZoom']],'tiles':len(asset['tiles']),'absent':len(asset['absent']),'bytes':asset['bytes'],'decodedFeaturesAcrossTilesAndLevels':total_features,'sha256':asset['sha256'],'exactSourceCompressedTiles':True,'independentPmtilesAndMvtDecode':True,'exportChecksumsVerified':True,'rangeCount':len(asset['ranges'])})
report={'checkedAt':datetime.now(timezone.utc).isoformat(),'mode':'offline restart' if a.offline else 'real public byte ranges','referenceVersions':{'pmtiles':'3.8.1','mapbox-vector-tile':'2.2.0'},'sourceCount':len(sources),'packages':reports}
(out/('restart-report.json' if a.offline else 'report.json')).write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8');print(json.dumps(report,ensure_ascii=False))
