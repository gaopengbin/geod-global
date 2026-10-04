"""Small real NASA GIBS request through an isolated native runtime, never fixtures."""
import argparse, hashlib, io, json, pathlib, urllib.request, urllib.parse, zipfile
from PIL import Image

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',required=True);parser.add_argument('--output',required=True);args=parser.parse_args()
    output=pathlib.Path(args.output);output.mkdir(parents=True,exist_ok=True)
    local=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def request(path,data=None,binary=False):
        headers={'X-GeoD-Client':'geod-global','Content-Type':'application/json'} if data is not None else {}
        req=urllib.request.Request(args.server+path,data=json.dumps(data).encode() if data is not None else None,headers=headers)
        with local.open(req,timeout=100) as r: raw=r.read()
        return raw if binary else json.loads(raw)
    service=request('/map-services',{'name':'NASA GIBS','url':'https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi'})
    layer=next(l for l in service['layers'] if l['name']=='MODIS_Terra_CorrectedReflectance_TrueColor')
    bounds=[-125,30,-110,43]
    asset=request('/map-images',{'serviceId':service['id'],'layerName':layer['name'],'style':'','time':'2025-06-27','bounds':bounds,'width':512,'height':444,'areaGeometry':None})
    actual=request('/map-images/'+asset['id']);assert actual['asset']==asset
    import base64
    png=base64.b64decode(actual['imageUrl'].split(',',1)[1]);assert hashlib.sha256(png).hexdigest()==asset['sha256'];assert len(png)==asset['bytes']
    im=Image.open(io.BytesIO(png));im.load();assert im.size==(512,444);assert im.mode=='RGBA';assert im.getextrema()[3][1]>0
    q=urllib.parse.parse_qs(urllib.parse.urlparse(asset['source']['requestUrl']).query,keep_blank_values=True)
    assert q['BBOX']==['30,-125,43,-110'];assert q['CRS']==['EPSG:4326'];assert q['TIME']==['2025-06-27']
    package=request('/map-images/'+asset['id']+'/export',binary=True);(output/'map.zip').write_bytes(package)
    with zipfile.ZipFile(io.BytesIO(package)) as z:
        assert z.read('map.png')==png;assert json.loads(z.read('source.json'))==asset
        for line in z.read('checksums.sha256').decode().splitlines():
            sha,name=line.split('  ',1);assert hashlib.sha256(z.read(name)).hexdigest()==sha
        world=[float(v) for v in z.read('map.pgw').decode().splitlines()]
        dx=(bounds[2]-bounds[0])/512;dy=-(bounds[3]-bounds[1])/444
        assert world==[dx,0,0,dy,bounds[0]+dx/2,bounds[3]+dy/2]
        z.extractall(output/'export')
    # Independent GDAL reader verifies the PNG sidecars in a GIS consumer.
    import rasterio
    with rasterio.open(output/'export/map.png') as dataset:
        assert str(dataset.crs)=='EPSG:4326';assert list(dataset.bounds)==bounds
        assert dataset.width==512 and dataset.height==444 and dataset.count==4
        assert dataset.transform.a==dx and dataset.transform.e==dy
    report={'checkedAt':asset['source']['requestedAt'],'mode':'Actual NASA GIBS public WMS through native runtime','serviceUrl':service['url'],'version':service['version'],'compatibleLayers':len(service['layers']),'acceptedLayer':layer['name'],'asset':asset,'checks':{'actualPngReceived':True,'exactExportBytes':True,'sourceMetadataEqual':True,'allExportChecksums':True,'independentPillowDimensions':True,'independentGdalCrsBoundsTransform':True,'nonSymmetricAxisOrder':True},'originalScientificProduct':False,'accountRequired':False,'desktopWebViewAccepted':False}
    (output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8')
    print(json.dumps({'serviceId':service['id'],'assetId':asset['id'],'layers':len(service['layers']),'sha256':asset['sha256'],'checks':report['checks']},ensure_ascii=False))
if __name__=='__main__':main()
