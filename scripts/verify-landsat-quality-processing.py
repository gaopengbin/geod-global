"""Independent GDAL/NumPy oracle for exact unsigned copying and internal masks.

No native implementation or renderer is imported. Every derived sample, mask
bit, full-resolution field bin, preview cell and recorded raw query is checked.
"""
import argparse,base64,hashlib,importlib.util,io,json
from pathlib import Path
import numpy as np
import rasterio,tifffile
from rasterio.windows import Window
from pyproj import Transformer
from PIL import Image

reference_spec=importlib.util.spec_from_file_location('usgs_reference',Path(__file__).with_name('verify-landsat-quality.py'))
reference=importlib.util.module_from_spec(reference_spec);reference_spec.loader.exec_module(reference)
DEFINITION='https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands'
COVERAGE='Coverage follows the independent internal mask, derived from matching QA_PIXEL bit 0 and the project geometry'
POLICY='newest scene with QA_PIXEL bit 0 unset wins; complete UInt16 flags retained; filled newer scenes do not erase covered older samples; independent internal mask; no quality ranking or bit merging'
def sha(data):return hashlib.sha256(data).hexdigest()
def managed(root,text):
    p=Path(text[4:] if text.startswith(chr(92)*2+'?'+chr(92)) else text).resolve();assert p.is_relative_to(root);return p
def inside_ring(x,y,ring):
    result=np.zeros(x.shape,dtype=bool)
    for (ax,ay),(bx,by) in zip(ring[:-1],ring[1:]):
        if by!=ay:result^=((ay>y)!=(by>y))&(x<(bx-ax)*(y-ay)/(by-ay)+ax)
    return result
def polygon_mask(plan,geometry):
    w,h=plan['width'],plan['height'];mask=np.ones((h,w),dtype=bool)
    if not geometry:return mask
    assert geometry['type']=='Polygon';rings=geometry['coordinates'];transform=Transformer.from_crs(plan['crs'],'EPSG:4326',always_xy=True)
    x=plan['bounds'][0]+(np.arange(w)+.5)*30
    for row in range(0,h,128):
        y=plan['bounds'][3]-(np.arange(row,min(row+128,h))+.5)*30
        xx,yy=np.meshgrid(x,y);lon,lat=transform.transform(xx,yy)
        block=inside_ring(lon,lat,rings[0])
        for hole in rings[1:]:block&=~inside_ring(lon,lat,hole)
        mask[row:row+block.shape[0]]=block
    return mask
def png_check(key,raw,covered,url,width,height):
    rows=np.arange(height)*raw.shape[0]//height;cols=np.arange(width)*raw.shape[1]//width
    sample=raw[rows[:,None],cols[None,:]];valid=covered[rows[:,None],cols[None,:]]
    rgba=np.zeros((height,width,4),dtype=np.uint8);rgba[:,:,:3]=np.array(reference.COLORS[key],dtype=np.uint8)[reference.display(key,sample)];rgba[:,:,3]=255;rgba[~valid]=0
    png=base64.b64decode(url.split(',',1)[1]);assert np.array_equal(np.array(Image.open(io.BytesIO(png)).convert('RGBA')),rgba)
    return {'rgbaPixelsCompared':width*height,'pngSha256':sha(png),'rgbaSha256':sha(rgba.tobytes())}
def bins(raw):
    hist=np.zeros(65536,dtype=np.int64)
    for row in range(0,raw.shape[0],64):hist+=np.bincount(raw[row:row+64].ravel(),minlength=65536)
    return hist
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('root');args=parser.parse_args()
    root=Path(args.root).resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-quality-processing-')
    receipt=root/'native-processing-verification.json';native=json.loads(receipt.read_text(encoding='utf-8'));assert native['status']=='passed'
    assert sha(Path(native['nativeBinary']).read_bytes())==native['nativeBinarySha256']
    originals={j['id']:j for j in native['originals']};assert len(originals)==6
    sources={};results=[];samples={};source_proofs=[]
    for job in originals.values():
        p=managed(root,job['outputPath']);assert sha(p.read_bytes())==job['sha256'];ds=rasterio.open(p);sources[job['id']]=ds
        assert ds.dtypes==('uint16',) and ds.count==1 and ds.res==(30,30) and ds.crs.to_string()=='EPSG:32610'
        assert ds.tags()['AREA_OR_POINT']=='Point';assert ds.nodata is None
        with tifffile.TiffFile(p) as tf:
            tie=tf.pages[0].tags[33922].value
            assert abs(tie[3]-15-ds.bounds.left)<1e-6 and abs(tie[4]+15-ds.bounds.top)<1e-6
        source_proofs.append({'id':job['id'],'itemId':job['itemId'],'key':job['assetKey'],'sha256':job['sha256'],'bytes':job['bytesDownloaded'],'dimensions':[ds.width,ds.height],'bounds':list(ds.bounds),'pixelInterpretation':'PixelIsPoint'})
    for entry in native['outputs']:
        key,job,meta=entry['key'],entry['job'],entry['metadata'];plan=job['mosaicOutput'];spec=job['mosaic'];case=next(c for c in native['cases'] if c['name']==entry['case']);w,h=plan['width'],plan['height']
        profile={'schemaVersion':'geod-landsat-quality-mosaic/v1','product':'landsat-c2-l2','band':key,'bits':16,'definition':DEFINITION,'coverage':COVERAGE}
        assert plan['landsatQuality']==profile and plan['overlapPolicy']==POLICY and plan['sourceCount']==len(case['project']['scenes'])
        expected=np.full((h,w),1 if key=='qa_pixel' else 0,dtype=np.uint16);covered=np.zeros((h,w),dtype=bool)
        assert {originals[pin['jobId']]['itemId'] for pin in spec['sources']}=={s['itemId'] for s in case['project']['scenes']}
        dates=[originals[pin['jobId']]['itemId'].split('_')[3] for pin in spec['sources']];assert dates==sorted(dates)
        assert len(spec.get('coverageSources',[]))==(len(spec['sources']) if key=='qa_radsat' else 0)
        fallback=overlap=0
        for i,pin in enumerate(spec['sources']):
            original=originals[pin['jobId']];assert pin['sha256']==original['sha256'] and original['assetKey']==key
            ds=sources[original['id']]
            fx=(ds.bounds.left-plan['bounds'][0])/30;fy=(plan['bounds'][3]-ds.bounds.top)/30
            ox,oy=round(fx),round(fy);assert abs(fx-ox)<1e-7 and abs(fy-oy)<1e-7
            left,top=max(0,ox),max(0,oy);right,bottom=min(w,ox+ds.width),min(h,oy+ds.height)
            if right<=left or bottom<=top:continue
            window=Window(left-ox,top-oy,right-left,bottom-top);raw=ds.read(1,window=window)
            pixel=raw
            if key=='qa_radsat':
                paired_pin=spec['coverageSources'][i];paired=originals[paired_pin['jobId']];pixel_ds=sources[paired['id']]
                assert paired['assetKey']=='qa_pixel' and paired['itemId']==original['itemId'] and paired_pin['sha256']==paired['sha256']
                assert paired['href'].rsplit('/',1)[0]==original['href'].rsplit('/',1)[0]
                assert pixel_ds.transform==ds.transform and pixel_ds.shape==ds.shape and pixel_ds.crs==ds.crs
                pixel=pixel_ds.read(1,window=window)
            valid=(pixel&1)==0;previous=covered[top:bottom,left:right]
            fallback+=int(np.count_nonzero(previous&~valid));overlap+=int(np.count_nonzero(previous&valid))
            copy=valid|~previous;expected[top:bottom,left:right][copy]=raw[copy];previous[copy]=valid[copy]
        area=polygon_mask(plan,case['project'].get('geometry'));expected[~area]=1 if key=='qa_pixel' else 0;covered&=area
        assert int(np.count_nonzero(~area))==plan['maskedPixels'] and int(np.count_nonzero(covered))==plan['coveredPixels']
        p=managed(root,job['outputPath']);assert sha(p.read_bytes())==job['sha256']
        with tifffile.TiffFile(p) as tf:
            assert json.loads(tf.pages[0].description)==profile and 42113 not in tf.pages[0].tags
            mask_pages=[page for page in tf.pages if page.tags.get(254) and int(page.tags[254].value)==4]
            assert len(mask_pages)==1 and mask_pages[0].bitspersample==1 and mask_pages[0].shape==(h,w)
            assert np.array_equal(mask_pages[0].asarray().astype(bool),covered)
        with rasterio.open(p) as ds:
            assert ds.count==1 and ds.dtypes==('uint16',) and ds.nodata is None and ds.res==(30,30)
            assert list(ds.bounds)==plan['bounds']==meta['bounds'] and ds.crs.to_string()==plan['crs']==meta['crs']
            assert ds.tags()['AREA_OR_POINT']=='Area' and ds.shape==(h,w) and plan['bandCount']==1
            actual=ds.read(1);assert np.array_equal(actual,expected);assert np.array_equal(ds.read_masks(1)>0,covered)
        hist=bins(expected);valid_hist=np.bincount(expected[covered],minlength=65536);codes=np.arange(65536);groups=reference.display(key,codes)
        classes=[int(valid_hist[groups==i].sum()) for i in range(len(reference.COLORS[key]))];assert classes==[c['count'] for c in meta['classes']]
        q=meta['quality'];assert q['sampleCount']==w*h and q['validSampleCount']==int(covered.sum()) and q['pixelInterpretation']=='PixelIsArea'
        flags=q['flags'];assert flags['sourceNoData'] is None and flags['coverage']==COVERAGE and flags['coverageMask']=={'kind':'internal-1bit','coveredPixels':int(covered.sum()),'uncoveredPixels':int((~covered).sum())}
        definitions=reference.PIXEL if key=='qa_pixel' else reference.RADSAT;observations=[]
        for field,(name,start,end,labels) in zip(flags['fields'],definitions,strict=True):
            code=(codes>>start)&((1<<(end-start+1))-1);counts=[int(hist[code==v].sum()) for v in range(1<<(end-start+1))]
            assert field=={'name':name,'startBit':start,'endBit':end,'counts':counts};observations.append(field)
        for pixel in entry['pixels']:
            x,y=pixel['pixel'];raw=int(expected[y,x]);valid=bool(covered[y,x]);assert pixel['value']==raw and pixel['isNoData']==(not valid)
            actual=pixel['quality'];assert actual['covered']==valid and actual['binary']==f'{raw:016b}' and actual['hex']==f'0x{raw:04X}'
            for field,(name,start,end,labels) in zip(actual['fields'],definitions,strict=True):
                code=(raw>>start)&((1<<(end-start+1))-1);label=labels[code] if code<len(labels) else 'Unused bits set';defined=code<len(labels) and label!='Reserved code' and not(name.startswith('Unused') and code!=0)
                assert field=={'name':name,'startBit':start,'endBit':end,'value':code,'label':label,'defined':defined}
        thumb=entry['thumbnail'];preview=png_check(key,expected,covered,meta['previewDataUrl'],meta['previewWidth'],meta['previewHeight']);thumbnail=png_check(key,expected,covered,thumb['dataUrl'],thumb['width'],thumb['height'])
        points=[]
        for label,select in [('valid-zero',covered&(expected==0)),('uncovered-zero',~covered&(expected==0)),('uncovered',~covered),('covered',covered),('highest-bit',covered&((expected&32768)!=0))]:
            found=np.flatnonzero(select)
            if len(found):
                index=int(found[len(found)//2]);row,col=divmod(index,w);points.append({'case':label,'raw':int(expected[row,col]),'covered':bool(covered[row,col]),'pixel':[col,row],'coordinate':[plan['bounds'][0]+(col+.5)*30,plan['bounds'][3]-(row+.5)*30]})
        samples[job['id']]=points
        result={'case':entry['case'],'key':key,'jobId':job['id'],'outputSha256':job['sha256'],'rawPlaneSha256':sha(expected.astype('<u2').tobytes()),'maskPlaneSha256':sha(covered.tobytes()),'samplesCompared':w*h,'coverageBitsCompared':w*h,'coveredPixels':int(covered.sum()),'uncoveredPixels':int((~covered).sum()),'maskedPixels':int((~area).sum()),'fallbackEvents':fallback,'overlapEvents':overlap,'coveredZeroValues':int(np.count_nonzero(covered&(expected==0))),'uncoveredZeroValues':int(np.count_nonzero(~covered&(expected==0))),'classCounts':classes,'flags':observations,'preview':preview,'thumbnail':thumbnail,'rawSamplesCompared':len(entry['pixels'])}
        results.append(result);print(json.dumps({'case':entry['case'],'key':key,'samplesCompared':w*h,'fallbackEvents':fallback}),flush=True)
    for ds in sources.values():ds.close()
    assert any(c['samplesCompared']>8_000_000 for c in results) and any(c['fallbackEvents']>0 for c in results)
    assert any(c['key']=='qa_radsat' and c['coveredZeroValues']>0 and c['uncoveredZeroValues']>0 for c in results)
    report={'schema':'geod-landsat-quality-processing-gdal/v1','status':'passed','nativeReceiptSha256':sha(receipt.read_bytes()),'nativeBinarySha256':native['nativeBinarySha256'],'independentDecoder':f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__} / tifffile {tifffile.__version__}','originals':source_proofs,'cases':results}
    (root/'sample-plan.json').write_text(json.dumps(samples,indent=2),encoding='utf-8');(root/'independent-processing-verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({'status':'passed','samplesCompared':sum(c['samplesCompared'] for c in results),'outputs':len(results)}))
if __name__=='__main__':main()
