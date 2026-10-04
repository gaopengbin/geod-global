"""Independent GDAL/TIFF check of an actual native-managed RTC download.

Uses an already-running isolated runtime; never downloads or overwrites a source.
Requires rasterio, numpy, tifffile and Pillow in the verification environment,
not in the desktop application. Output paths and previews are local evidence.
"""
import argparse, base64, hashlib, io, json, math, time, urllib.request
from pathlib import Path
import numpy as np
import rasterio
from rasterio.windows import Window
from PIL import Image
import tifffile

def request(server,path):
    start=time.monotonic()
    with urllib.request.urlopen(server+path,timeout=90) as response:
        data=json.load(response)
    return data, round(time.monotonic()-start,3)

def verify_preview(path,data):
    edge=max(data['previewWidth'],data['previewHeight'])
    with tifffile.TiffFile(path) as source:
        page=source.pages[0]; best=0
        for index,candidate in enumerate(source.pages[1:17],1):
            if max(candidate.shape)>=edge and max(candidate.shape)<max(page.shape):
                page=candidate;best=index
        dimensions=page.shape
    # GDAL's Float32 predictor decoder is independent from Rust tiff. The
    # reviewed IFD dimensions identify exactly which embedded overview to read.
    with rasterio.open(path,**({'overview_level':best-1} if best else {})) as source:
        assert (source.height,source.width)==dimensions
        values=source.read(1)
    w,h=data['previewWidth'],data['previewHeight']
    values=values[(np.arange(h,dtype=np.uint64)*values.shape[0]//h).astype(int)[:,None],(np.arange(w,dtype=np.uint64)*values.shape[1]//w).astype(int)[None,:]]
    positive=values[values>0].astype(np.float64)
    positive=np.sort(10*np.log10(positive)); count=len(positive)
    limits=[float(positive[(count-1)*2//100]),float(positive[(count-1)*98//100])] if count else [0.,0.]
    gray=np.zeros(values.shape,dtype=np.uint8); valid=values>0
    if limits[0]==limits[1]: gray[valid]=128
    else: gray[valid]=np.floor(np.clip((10*np.log10(values[valid].astype(np.float64))-limits[0])/(limits[1]-limits[0]),0,1)*255+.5).astype(np.uint8)
    rgba=np.stack([gray,gray,gray,np.where(values==-32768,0,255).astype(np.uint8)],axis=2)
    actual=np.asarray(Image.open(io.BytesIO(base64.b64decode(data['previewDataUrl'].split(',')[1]))).convert('RGBA'))
    assert np.array_equal(actual,rgba),'Native dB preview differs from independently decoded TIFF overview'
    if 'radar' in data:
        assert np.allclose(data['radar']['displayRange'],limits,rtol=0,atol=1e-12)
        assert data['radar']['validSampleCount']==int(np.count_nonzero(values!=-32768))
        assert data['radar']['overview']==bool(best)
    return {'independentDecoder':'GDAL/rasterio, tifffile metadata, Pillow/numpy','overviewIndex':best,'previewPixelsCompared':w*h,'exactRgbaMatch':True}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',default='http://127.0.0.1:4328');parser.add_argument('--job',required=True);parser.add_argument('--out',required=True);args=parser.parse_args()
    jobs,_=request(args.server,'/jobs');job=next(j for j in jobs if j['id']==args.job)
    assert job['status']=='succeeded' and job['assetKey'] in ['vv','vh','hh','hv']
    path=Path(job['outputPath'])
    with path.open('rb') as handle: digest=hashlib.file_digest(handle,'sha256').hexdigest()
    assert digest==job['sha256'];assert path.stat().st_size==job['bytesDownloaded']==job['totalBytes']
    metadata,inspect_seconds=request(args.server,f'/jobs/{args.job}/raster')
    results=[]
    with rasterio.open(path) as source:
        assert source.count==1 and source.dtypes==('float32',) and source.nodata==-32768
        assert source.crs.to_string()==metadata['crs'];assert source.width==metadata['width'];assert source.height==metadata['height']
        assert list(source.bounds)==metadata['bounds'];assert list(source.res)==metadata['pixelSize']
        for col,row in [(0,0),(source.width//2,source.height//2),(source.width//3,source.height//2),(source.width-1,source.height-1)]:
            value=float(source.read(1,window=Window(col,row,1,1))[0,0]);x,y=source.xy(row,col)
            pixel,seconds=request(args.server,f'/jobs/{args.job}/pixel?x={x}&y={y}')
            assert pixel['value']==value and pixel['pixel']==[col,row] and pixel['sha256']==digest and pixel['crs']==source.crs.to_string()
            assert pixel['isNoData']==(value==-32768)
            if value>0: assert abs(pixel['decibels']-10*math.log10(value))<1e-12
            else: assert 'decibels' not in pixel
            results.append({'pixel':[col,row],'originalGamma0':value,'decibels':pixel.get('decibels'),'nativeSeconds':seconds,'exactFloat32Match':True})
    preview=verify_preview(path,metadata)
    thumb,first_seconds=request(args.server,f'/jobs/{args.job}/thumbnail');cached,cache_seconds=request(args.server,f'/jobs/{args.job}/thumbnail')
    assert thumb==cached;assert thumb['sha256']==digest
    cache_entries=list(path.parent.parent.glob('cache/thumbnails/v1/*.json'));assert cache_entries
    verify_preview(path,{'previewWidth':thumb['width'],'previewHeight':thumb['height'],'previewDataUrl':thumb['dataUrl']})
    report={'checkedAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'scope':'One actual Planetary Computer Sentinel-1C VV RTC original; original transfer, geometry, sampled pixels, preview and persistent thumbnail only',
        'jobId':job['id'],'itemId':job['itemId'],'href':job['href'],'bytes':job['bytesDownloaded'],'sha256':digest,'raster':{k:metadata[k] for k in ['width','height','crs','dataType','bounds','pixelSize','nodata','radar']},'pixels':results,'preview':preview,
        'inspectSeconds':inspect_seconds,'thumbnail':{'firstSeconds':first_seconds,'cacheSeconds':cache_seconds,'persistentEntryCount':len(cache_entries),'identicalCacheHit':True},'terrainCorrectionAccuracyAssessed':False,'additionalCalibrationApplied':False,'speckleFilteringApplied':False,'otherPolarizationsAcceptedByOriginalTest':False,'usedUserDesktop':False}
    output=Path(args.out);output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps({'output':str(output),'pixels':len(results),'previewPixels':preview['previewPixelsCompared'],'cachedThumbnail':True,'bytes':job['bytesDownloaded']}))

if __name__=='__main__':main()
