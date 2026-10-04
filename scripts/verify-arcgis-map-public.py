"""Verify two live public render services against the real native runtime.

Uses an isolated, initially empty storage directory. Does not launch a desktop
window, modify user projects, or treat rendered colors as source band values.
"""
import argparse,base64,hashlib,io,json,pathlib,urllib.request,zipfile
from PIL import Image

def sha(b):return hashlib.sha256(b).hexdigest()
def http(url,payload=None):
    headers={'X-GeoD-Client':'geod-global','Content-Type':'application/json'} if payload is not None else {}
    req=urllib.request.Request(url,data=json.dumps(payload).encode() if payload is not None else None,headers=headers)
    return urllib.request.urlopen(req,timeout=90).read()
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',default='http://127.0.0.1:4369');p.add_argument('--out',default='.verification/arcgis-map-public');args=p.parse_args()
    out=pathlib.Path(args.out);out.mkdir(parents=True,exist_ok=True);report={'type':'live-public-ArcGIS-rendered-images','cases':[]}
    assert json.loads(http(args.server+'/map-images'))==[], 'Use fresh isolated storage'
    cases=[('map','Esri USA public sample','https://sampleserver6.arcgisonline.com/arcgis/rest/services/USA/MapServer','2',[-123.0,37.0,-122.0,38.0],512,256),('image','USGS NAIP','https://imagery.nationalmap.gov/arcgis/rest/services/USGSNAIPImagery/ImageServer','image',[-122.52,37.72,-122.40,37.80],256,256)]
    for name,title,url,layer,bounds,width,height in cases:
        service=json.loads(http(args.server+'/map-services',dict(name=title,url=url,protocol='ArcGIS')))
        a=json.loads(http(args.server+'/map-images',dict(serviceId=service['id'],layerName=layer,style='',time=None,bounds=bounds,width=width,height=height,areaGeometry=None)))
        inspection=json.loads(http(args.server+'/map-images/'+a['id']));assert inspection['asset']==a
        png=base64.b64decode(inspection['imageUrl'].split(',')[1]);assert len(png)==a['bytes'] and sha(png)==a['sha256']
        snapshot=a['source']['arcgis'];response=json.loads(snapshot['exportMetadata']);assert sha(snapshot['exportMetadata'].encode())==snapshot['exportSha256']
        raw=http(snapshot['imageUrl']);assert raw==png,'Independent exact href fetch differs'
        rgba=Image.open(io.BytesIO(png)).convert('RGBA');assert rgba.size==(width,height)
        assert rgba.getextrema()[3][1]>0,'Service returned only transparent pixels'
        exact=http(args.server+'/map-images/'+a['id']+'/export');(out/(name+'.zip')).write_bytes(exact)
        with zipfile.ZipFile(io.BytesIO(exact)) as z:
            assert z.read('map.png')==png
            assert z.read('service-metadata.json').decode()==snapshot['capabilities']['metadata']
            assert z.read('export-response.json').decode()==snapshot['exportMetadata']
            assert json.loads(z.read('source.json'))==a
            if name=='map':assert z.read('layers-metadata.json').decode()==snapshot['capabilities']['layersMetadata']
            for line in z.read('checksums.sha256').decode().splitlines():
                digest,file=line.split('  ');assert sha(z.read(file))==digest
            world=[float(n) for n in z.read('map.pgw').splitlines()]
            actual=[response['extent'][k] for k in ['xmin','ymin','xmax','ymax']];assert a['bounds']==actual
            assert a['bounds']!=bounds,'Verification must exercise actual server extent adjustment'
            expected=[(actual[2]-actual[0])/width,0,0,-(actual[3]-actual[1])/height,actual[0]+(actual[2]-actual[0])/width/2,actual[3]-(actual[3]-actual[1])/height/2]
            assert all(abs(x-y)<1e-12 for x,y in zip(world,expected))
            extracted=out/(name+'-package');extracted.mkdir(exist_ok=True);z.extractall(extracted)
        import numpy as np,rasterio
        with rasterio.open(extracted/'map.png') as ds:
            assert ds.crs.to_epsg()==4326 and (ds.width,ds.height)==(width,height)
            assert np.allclose(tuple(ds.bounds),actual,rtol=0,atol=1e-12)
            assert np.array_equal(np.moveaxis(ds.read(),0,-1),np.array(rgba))
        (out/(name+'-image.png')).write_bytes(png);(out/(name+'-asset.json')).write_text(json.dumps(a,ensure_ascii=False,indent=2),encoding='utf-8')
        report['cases'].append(dict(kind=name,serviceId=service['id'],imageId=a['id'],bytes=len(png),sha256=sha(png),size=[width,height],requestedBounds=bounds,actualBounds=a['bounds'],rgbaChannels=width*height*4,exactReturnedHrefBytes=True,packageReceiptsExact=True,worldfileActualExtent=True,independentGdalCrsExtentAndAllDecodedPixels=True,exportSha256=snapshot['exportSha256']))
        print(name,a['id'],len(png),a['bounds'],flush=True)
    # Invalid/public-protected inputs never add records. HTTP errors are expected.
    bad=[dict(name='Private',url='https://127.0.0.1/arcgis/rest/services/Bad/MapServer',protocol='ArcGIS'),dict(name='Not a root',url=cases[0][2]+'/2',protocol='ArcGIS')]
    failures=[]
    for request in bad:
        try:http(args.server+'/map-services',request);raise AssertionError('Invalid connection accepted')
        except urllib.error.HTTPError as e:failures.append(json.loads(e.read())['error'])
    assert len(json.loads(http(args.server+'/map-images')))==2
    report['rejectedConnections']=failures;report['pixelCount']=sum(c['size'][0]*c['size'][1] for c in report['cases'])
    (out/'runtime.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({'images':len(report['cases']),'pixels':report['pixelCount'],'status':'passed'}))
if __name__=='__main__':main()
