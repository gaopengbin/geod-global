"""Independently verify the small public XYZ image saved from the real UI."""
import argparse,base64,hashlib,io,json,math,pathlib,urllib.request,zipfile
import numpy as np,rasterio
from PIL import Image
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4370');p.add_argument('--out',default='.verification/xyz-public');args=p.parse_args();out=pathlib.Path(args.out)
    record=json.loads((out/'ui-acquisition.json').read_text('utf-8'));a=record['asset'];s=a['source']['xyz'];c=s['configuration'];assert c['scheme']=='XYZ' and c['grid']['tileSize']==256 and c['grid']['zoomOffset']==0
    def get(url):return urllib.request.urlopen(url,timeout=40).read()
    inspection=json.loads(get(args.server+'/map-images/'+a['id']));assert inspection['asset']==a
    image=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert sha(image)==a['sha256']
    package=get(args.server+'/map-images/'+a['id']+'/export');(out/'ui.zip').write_bytes(package)
    z=s['logicalZoom'];n=256*2**z;b=s['requestedBounds'];x=lambda lon:(lon+180)/360*n;y=lambda lat:(1-math.asinh(math.tan(math.radians(lat)))/math.pi)/2*n
    left,top=math.floor(x(b[0])),math.floor(y(b[3]));right,bottom=math.ceil(x(b[2])),math.ceil(y(b[1]));assert s['pixelWindow']==[left,top,right-left,bottom-top]
    decoded=np.array(Image.open(io.BytesIO(image)).convert('RGBA'));reference=np.zeros(decoded.shape,dtype=np.uint8)
    with zipfile.ZipFile(io.BytesIO(package)) as pkg:
        assert pkg.read('map.png')==image and json.loads(pkg.read('source.json'))==a
        for line in pkg.read('checksums.sha256').decode().splitlines():
            digest,name=line.split('  ');assert sha(pkg.read(name))==digest
        archive=pkg.read('source-tiles.zip');assert sha(archive)==s['archiveSha256'] and len(archive)==s['archiveBytes']
        with zipfile.ZipFile(io.BytesIO(archive)) as tiles:
            assert len(tiles.namelist())==len(s['tiles'])
            for r in s['tiles']:
                row,col=r['row'],r['col'];raw=tiles.read(f'{row}-{col}.jpg');assert raw==get(r['requestUrl']) and sha(raw)==r['sha256'] and len(raw)==r['bytes']
                assert r['requestUrl']==c['urlTemplate'].replace('{z}',str(z)).replace('{x}',str(col)).replace('{y}',str(row))
                rgba=np.array(Image.open(io.BytesIO(raw)).convert('RGBA'));tx,ty=256*col,256*row;l,t=max(left,tx),max(top,ty);e,d=min(right,tx+256),min(bottom,ty+256)
                reference[t-top:d-top,l-left:e-left]=rgba[t-ty:d-ty,l-tx:e-tx]
        extracted=out/'ui-package';extracted.mkdir(exist_ok=True);pkg.extractall(extracted)
    diff=np.abs(decoded.astype(np.int16)-reference.astype(np.int16));maxdiff=int(diff.max());assert maxdiff<=4 and np.array_equal(decoded[:,:,3],reference[:,:,3])
    half=math.pi*6378137;res=2*half/n;extent=[-half+left*res,half-bottom*res,-half+right*res,half-top*res]
    with rasterio.open(extracted/'map.png') as ds:
        assert ds.crs.to_epsg()==3857 and np.allclose(tuple(ds.bounds),extent,rtol=0,atol=1e-8)
        assert np.array_equal(np.moveaxis(ds.read(),0,-1),decoded)
    world=[float(v) for v in (extracted/'map.pgw').read_text().splitlines()];assert np.allclose(world,[res,0,0,-res,extent[0]+res/2,extent[3]-res/2],rtol=0,atol=1e-8)
    report={'passed':True,'imageId':a['id'],'size':[a['width'],a['height']],'pixels':a['width']*a['height'],'originalTiles':len(s['tiles']),'maxIndependentDecoderChannelDifference':maxdiff,'allPixelChannelsCompared':True,'originalTileHttpBytesExact':True,'nativeGdalCrsExtentAndPixels':True,'packageReceiptsExact':True}
    (out/'ui-file-reference.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))
if __name__=='__main__':main()
