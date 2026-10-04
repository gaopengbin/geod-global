"""Bounded real NASA GIBS WMTS acceptance via native runtime and independent readers."""
import argparse, base64, hashlib, io, json, math, pathlib, urllib.request, urllib.error, xml.etree.ElementTree as E, zipfile
import numpy as np
from PIL import Image
import rasterio

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',required=True);parser.add_argument('--output',required=True);args=parser.parse_args()
    out=pathlib.Path(args.output);out.mkdir(parents=True,exist_ok=True)
    local=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def request(path,data=None,binary=False):
        body=json.dumps(data).encode() if data is not None else None
        headers={'X-GeoD-Client':'geod-global','Content-Type':'application/json'} if body is not None else {}
        try:
            with local.open(urllib.request.Request(args.server+path,data=body,headers=headers),timeout=160) as r: raw=r.read()
        except urllib.error.HTTPError as e:
            raise RuntimeError(e.read().decode('utf-8')) from e
        return raw if binary else json.loads(raw)
    def sha(b):return hashlib.sha256(b).hexdigest()
    receipts=[];bounds=[-125,30,-110,43];ns={'w':'http://www.opengis.net/wmts/1.0','o':'http://www.opengis.net/ows/1.1'}
    for projection,layer_name in [('4326','MODIS_Terra_CorrectedReflectance_TrueColor'),('3857','MODIS_Terra_CorrectedReflectance_TrueColor'),('4326','MODIS_Terra_Aerosol')]:
        endpoint=f'https://gibs.earthdata.nasa.gov/wmts/epsg{projection}/best/wmts.cgi'
        service=request('/map-services',{'name':f'NASA GIBS {projection}','url':endpoint,'protocol':'WMTS'})
        layer=next(l for l in service['layers'] if l['name']==layer_name)
        # Independently inspect current public XML, including axis order and sizes.
        with urllib.request.urlopen(endpoint+'?SERVICE=WMTS&REQUEST=GetCapabilities&VERSION=1.0.0',timeout=60) as r: xml=r.read()
        doc=E.fromstring(xml);(out/f'capabilities-{projection}.xml').write_bytes(xml)
        set_id=layer['wmts']['links'][0]['matrixSet'];matrix_set=next(s for s in service['wmts']['matrixSets'] if s['id']==set_id)
        declared=next(s for s in doc.findall('w:Contents/w:TileMatrixSet',ns) if s.findtext('o:Identifier',namespaces=ns)==set_id)
        assert declared.findtext('o:SupportedCRS',namespaces=ns)==matrix_set['declaredCrs']
        candidates=[]
        for m in matrix_set['matrices']:
            xm=next(s for s in declared.findall('w:TileMatrix',ns) if s.findtext('o:Identifier',namespaces=ns)==m['id'])
            assert float(xm.findtext('w:ScaleDenominator',namespaces=ns))==m['scaleDenominator']
            raw_origin=[float(s) for s in xm.findtext('w:TopLeftCorner',namespaces=ns).split()]
            if '4326' in matrix_set['declaredCrs'] and 'CRS84' not in matrix_set['declaredCrs']:raw_origin.reverse()
            assert raw_origin==m['topLeft']
            for key,tag in [('tileWidth','TileWidth'),('tileHeight','TileHeight'),('matrixWidth','MatrixWidth'),('matrixHeight','MatrixHeight')]:assert m[key]==int(xm.findtext('w:'+tag,namespaces=ns))
            r=m['scaleDenominator']*.00028/(1 if projection=='3857' else math.pi*6378137/180)
            b=bounds[:]
            if projection=='3857':
                y=lambda lat:6378137*math.log(math.tan(math.pi/4+lat*math.pi/360))
                b=[bounds[0]*math.pi*6378137/180,y(bounds[1]),bounds[2]*math.pi*6378137/180,y(bounds[3])]
            snap=lambda n:round(n) if abs(n-round(n))<1e-7 else n
            x=math.floor(snap((b[0]-m['topLeft'][0])/r));y=math.floor(snap((m['topLeft'][1]-b[3])/r));right=math.ceil(snap((b[2]-m['topLeft'][0])/r));bottom=math.ceil(snap((m['topLeft'][1]-b[1])/r))
            if 0<right-x<=1024 and 0<bottom-y<=1024:
                rows=range(y//m['tileHeight'],(bottom-1)//m['tileHeight']+1);cols=range(x//m['tileWidth'],(right-1)//m['tileWidth']+1)
                if len(rows)*len(cols)<=16:candidates.append((max(right-x,bottom-y),m,[x,y,right-x,bottom-y],r))
        _,matrix,window,resolution=max(candidates,key=lambda v:v[0]);x,y,width,height=window
        asset=request('/map-images',{'serviceId':service['id'],'layerName':layer_name,'style':layer['wmts']['defaultStyle'],'time':'2025-06-27','bounds':bounds,'width':width,'height':height,'tileMatrixSet':set_id,'tileMatrix':matrix['id'],'areaGeometry':None})
        result=request('/map-images/'+asset['id']);assert result['asset']==asset;png=base64.b64decode(result['imageUrl'].split(',',1)[1]);assert sha(png)==asset['sha256']
        rendered=np.asarray(Image.open(io.BytesIO(png)).convert('RGBA'));assert rendered.shape==(height,width,4)
        package=request('/map-images/'+asset['id']+'/export',binary=True)
        case=out/f'{projection}-{layer_name}';case.mkdir(exist_ok=True);(case/'map.zip').write_bytes(package)
        with zipfile.ZipFile(io.BytesIO(package)) as z:
            assert z.read('map.png')==png;assert json.loads(z.read('source.json'))==asset
            for line in z.read('checksums.sha256').decode().splitlines():
                digest,name=line.split('  ',1);assert sha(z.read(name))==digest
            archived=z.read('source-tiles.zip');assert sha(archived)==asset['source']['wmts']['archiveSha256'];z.extractall(case/'export')
        expected=np.zeros_like(rendered);snapshot=asset['source']['wmts'];tiles=[]
        with zipfile.ZipFile(io.BytesIO(archived)) as z:
            assert len(z.namelist())==len(snapshot['tiles'])
            for tile in snapshot['tiles']:
                ext='png' if snapshot['format']=='image/png' else 'jpg';b=z.read(f"{tile['row']}-{tile['col']}.{ext}")
                assert sha(b)==tile['sha256'] and len(b)==tile['bytes']
                with urllib.request.urlopen(tile['requestUrl'],timeout=60) as response:independent=response.read()
                assert b==independent  # exact public response, not a fixture or thumbnail
                pixels=np.asarray(Image.open(io.BytesIO(b)).convert('RGBA'));assert pixels.shape==(matrix['tileHeight'],matrix['tileWidth'],4)
                tx=tile['col']*matrix['tileWidth'];ty=tile['row']*matrix['tileHeight'];left=max(x,tx);right=min(x+width,tx+matrix['tileWidth']);top=max(y,ty);bottom=min(y+height,ty+matrix['tileHeight'])
                expected[top-y:bottom-y,left-x:right-x]=pixels[top-ty:bottom-ty,left-tx:right-tx]
                tiles.append({'row':tile['row'],'col':tile['col'],'sha256':tile['sha256'],'independentPublicBytesEqual':True})
        delta=np.abs(rendered.astype(np.int16)-expected.astype(np.int16));max_delta=int(delta[:,:,:3].max())
        # Independent JPEG implementations can differ by a few integer display levels.
        if snapshot['format']=='image/png':assert np.array_equal(rendered,expected)
        else:assert max_delta<=6 and float(delta[:,:,:3].mean())<.5
        assert np.array_equal(rendered[:,:,3],expected[:,:,3]);assert np.any(rendered[:,:,:3]!=0)
        extent=[matrix['topLeft'][0]+x*resolution,matrix['topLeft'][1]-(y+height)*resolution,matrix['topLeft'][0]+(x+width)*resolution,matrix['topLeft'][1]-y*resolution]
        assert np.allclose(asset['imageExtent'],extent,rtol=0,atol=1e-8)
        with rasterio.open(case/'export/map.png') as ds:
            assert str(ds.crs)==f'EPSG:{projection}';assert ds.width==width and ds.height==height and ds.count==4
            assert np.allclose(list(ds.bounds),extent,rtol=0,atol=1e-8);assert abs(ds.transform.a-resolution)<1e-9 and abs(ds.transform.e+resolution)<1e-9
            assert np.array_equal(np.moveaxis(ds.read(),0,-1),rendered)
        receipts.append({'serviceId':service['id'],'compatibleLayers':len(service['layers']),'asset':asset,'tileReceipts':tiles,'checkedChannels':int(rendered.size),'independentPngChannelsExact':snapshot['format']=='image/png','jpegMaximumChannelDelta':max_delta,'jpegMeanChannelDelta':float(delta[:,:,:3].mean()),'independentGdalGridAndPixels':True,'exportHashesAndOriginalTileBytes':True})
        print(json.dumps({'projection':projection,'layer':layer_name,'assetId':asset['id'],'tiles':len(tiles),'size':[width,height],'maximumDelta':max_delta},ensure_ascii=False),flush=True)
    report={'checkedAt':receipts[-1]['asset']['source']['requestedAt'],'mode':'Actual public NASA GIBS WMTS through isolated native runtime','publicSource':'https://nasa-gibs.github.io/gibs-api-docs/access-basics/','receipts':receipts,'fixturesUsed':False,'scientificOriginalBands':False,'desktopWebViewAccepted':False}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
if __name__=='__main__':main()
