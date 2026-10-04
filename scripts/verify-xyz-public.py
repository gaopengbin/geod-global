"""Small real XYZ acquisitions and independent pixel / coordinate verification.

Run against an isolated native server. Never fetch a whole pyramid or use the
standard OSM tile service. TMS row direction has synthetic unit evidence, not
public-service evidence from this script.
"""
import argparse,base64,hashlib,io,json,math,pathlib,urllib.request,urllib.error,zipfile
import numpy as np,rasterio
from PIL import Image

def sha(b):return hashlib.sha256(b).hexdigest()
def http(url,payload=None):
    h={'Content-Type':'application/json','X-GeoD-Client':'geod-global'} if payload is not None else {}
    return urllib.request.urlopen(urllib.request.Request(url,data=json.dumps(payload).encode() if payload is not None else None,headers=h),timeout=140).read()
def plan(bounds,size,z):
    # Independent slippy-map global pixel model, not the native planner.
    n=size*2**z
    x=lambda lon:(lon+180)/360*n
    y=lambda lat:(1-math.asinh(math.tan(math.radians(lat)))/math.pi)/2*n
    def snap(v):return round(v) if abs(v-round(v))<1e-7 else v
    left,top=math.floor(snap(x(bounds[0]))),math.floor(snap(y(bounds[3])))
    right,bottom=math.ceil(snap(x(bounds[2]))),math.ceil(snap(y(bounds[1])))
    return [left,top,right-left,bottom-top]
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4370');p.add_argument('--out',default='.verification/xyz-public');a=p.parse_args()
    out=pathlib.Path(a.out);out.mkdir(parents=True,exist_ok=True);report={'type':'real-NASA-GIBS-XYZ-rendered-tiles','publicTmsAcquisitionVerified':False,'cases':[]}
    assert json.loads(http(a.server+'/map-images'))==[],'Use fresh isolated storage'
    base='https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/'
    cases=[('png','MODIS_Terra_Land_Surface_Temp_Day',7,'png','image/png',[-125,35,-120,40],6),('jpeg','MODIS_Terra_CorrectedReflectance_TrueColor',9,'jpeg','image/jpeg',[-125,35,-120,40],7)]
    for key,layer,maxz,ext,fmt,bounds,z in cases:
        template=base+layer+'/default/2025-06-27/GoogleMapsCompatible_Level'+str(maxz)+'/{z}/{y}/{x}.'+ext
        grid=dict(tileSize=256,minZoom=0,maxZoom=maxz,zoomOffset=0,format=fmt,attribution='NASA GIBS / MODIS Terra',accessConstraints='NASA GIBS visualization; dataset reuse terms apply. https://nasa-gibs.github.io/gibs-api-docs/')
        service=json.loads(http(a.server+'/map-services',dict(name='NASA XYZ '+key,url=template,protocol='XYZ',tileConfig=grid)))
        win=plan(bounds,256,z)
        request=dict(serviceId=service['id'],layerName='tiles',style='',time=None,bounds=bounds,width=win[2],height=win[3],tileMatrixSet='WebMercator',tileMatrix=str(z),areaGeometry=None)
        asset=json.loads(http(a.server+'/map-images',request));assert asset['source']['xyz']['pixelWindow']==win
        inspection=json.loads(http(a.server+'/map-images/'+asset['id']));assert inspection['asset']==asset
        png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert sha(png)==asset['sha256'] and len(png)==asset['bytes']
        decoded=np.array(Image.open(io.BytesIO(png)).convert('RGBA'));assert decoded.shape==(win[3],win[2],4)
        package=http(a.server+'/map-images/'+asset['id']+'/export');(out/(key+'.zip')).write_bytes(package)
        s=asset['source']['xyz'];assert s['matrixSet']=='WebMercator' and s['matrix']['tileWidth']==256 and s['matrix']['matrixWidth']==2**z;expected=np.zeros(decoded.shape,dtype=np.uint8);original_bytes=0
        with zipfile.ZipFile(io.BytesIO(package)) as pkg:
            assert pkg.read('map.png')==png and json.loads(pkg.read('source.json'))==asset
            for line in pkg.read('checksums.sha256').decode().splitlines():
                digest,file=line.split('  ');assert sha(pkg.read(file))==digest
            archive=pkg.read('source-tiles.zip');assert sha(archive)==s['archiveSha256'] and len(archive)==s['archiveBytes']
            with zipfile.ZipFile(io.BytesIO(archive)) as tiles:
                assert len(tiles.namelist())==len(s['tiles'])
                for receipt in s['tiles']:
                    row,col=receipt['row'],receipt['col'];expected_url=template.replace('{z}',str(z)).replace('{y}',str(row)).replace('{x}',str(col));assert expected_url==receipt['requestUrl']
                    exact=tiles.read(f'{row}-{col}.'+('png' if key=='png' else 'jpg'));assert exact==http(receipt['requestUrl'])
                    assert len(exact)==receipt['bytes'] and sha(exact)==receipt['sha256'];original_bytes+=len(exact)
                    rgba=np.array(Image.open(io.BytesIO(exact)).convert('RGBA'));assert rgba.shape==(256,256,4)
                    tx,ty=col*256,row*256;l,r=max(win[0],tx),min(win[0]+win[2],tx+256);t,b=max(win[1],ty),min(win[1]+win[3],ty+256)
                    expected[t-win[1]:b-win[1],l-win[0]:r-win[0]]=rgba[t-ty:b-ty,l-tx:r-tx]
            half=math.pi*6378137;res=2*half/(256*2**z);extent=[-half+win[0]*res,half-(win[1]+win[3])*res,-half+(win[0]+win[2])*res,half-win[1]*res]
            assert np.allclose(asset['imageExtent'],extent,rtol=0,atol=1e-8)
            world=np.array([float(v) for v in pkg.read('map.pgw').splitlines()]);assert np.allclose(world,[res,0,0,-res,extent[0]+res/2,extent[3]-res/2],rtol=0,atol=1e-8)
            extracted=out/(key+'-package');extracted.mkdir(exist_ok=True);pkg.extractall(extracted)
        with rasterio.open(extracted/'map.png') as ds:
            assert ds.crs.to_epsg()==3857 and np.allclose(tuple(ds.bounds),extent,rtol=0,atol=1e-8)
            assert np.array_equal(np.moveaxis(ds.read(),0,-1),decoded)
        diff=np.abs(decoded.astype(np.int16)-expected.astype(np.int16));maxdiff=int(diff.max());assert int(decoded[:,:,3].max())>0
        if key=='png':assert maxdiff==0
        else:assert np.array_equal(decoded[:,:,3],expected[:,:,3]) and maxdiff<=4,'JPEG decoder differences exceed verified WMTS tolerance'
        (out/(key+'-asset.json')).write_text(json.dumps(asset,ensure_ascii=False,indent=2),encoding='utf-8');(out/(key+'-image.png')).write_bytes(png)
        report['cases'].append(dict(kind=key,serviceId=service['id'],imageId=asset['id'],tiles=len(s['tiles']),originalBytes=original_bytes,size=[asset['width'],asset['height']],pixels=asset['width']*asset['height'],sha256=asset['sha256'],requestedBounds=bounds,actualExtent=extent,fullReferencePixelComparison=True,maxIndependentDecoderChannelDifference=maxdiff,nativeGdalCrsAndGrid=True,exactTileHttpBytes=True,exactPackageReceipts=True))
        print(key,asset['id'],report['cases'][-1]['size'],maxdiff,flush=True)
    failures=[]
    # An otherwise valid source with a deliberately incorrect declared tile size.
    badgrid={**grid,'tileSize':512};bad=json.loads(http(a.server+'/map-services',dict(name='Incorrect size test',url=template,protocol='XYZ',tileConfig=badgrid)));badwin=plan(bounds,512,6)
    badrequest={**request,'serviceId':bad['id'],'tileMatrix':'6','width':badwin[2],'height':badwin[3]}
    try:http(a.server+'/map-images',badrequest);raise AssertionError('Wrong actual tile size accepted')
    except urllib.error.HTTPError as e:failures.append(json.loads(e.read())['error'])
    assert 'dimensions' in failures[-1].lower();http(a.server+'/map-services/'+bad['id']+'/forget',{})
    for raw in ['https://localhost/{z}/{x}/{y}.png','https://tile.openstreetmap.org/{z}/{x}/{y}.png','https://example.com/{z}/{x}/{y}.png?token=credential']:
        try:http(a.server+'/map-services',dict(name='Invalid template',url=raw,protocol='XYZ',tileConfig=grid));raise AssertionError('Invalid template accepted')
        except urllib.error.HTTPError as e:failures.append(json.loads(e.read())['error'])
    assert len(json.loads(http(a.server+'/map-images')))==2 and len(json.loads(http(a.server+'/map-services')))==2
    report['rejections']=failures;report['pixels']=sum(c['pixels'] for c in report['cases']);report['passed']=True
    (out/'runtime.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps({'passed':True,'pixels':report['pixels']}))
if __name__=='__main__':main()
