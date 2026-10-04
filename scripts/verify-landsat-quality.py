"""Independent full-resolution QA counts, flag decoding and nearest previews.
No native reader or renderer logic is imported. Definitions follow USGS C2 8/9.
"""
import argparse,base64,hashlib,io,json
from pathlib import Path
import numpy as np
import rasterio
from PIL import Image

PIXEL=[('Fill',0,0,['Image data','Fill data']),('Dilated cloud',1,1,['Not dilated or no cloud','Dilated cloud']),
 ('High-confidence cirrus',2,2,['Cirrus confidence not high','High-confidence cirrus']),('High-confidence cloud',3,3,['Cloud confidence not high','High-confidence cloud']),
 ('High-confidence cloud shadow',4,4,['Shadow confidence not high','High-confidence cloud shadow']),('High-confidence snow / ice',5,5,['Snow confidence not high','High-confidence snow / ice']),
 ('Clear cloud flag',6,6,['Clear flag unset','Cloud and dilated-cloud flags unset']),('Water',7,7,['Land or cloud','Water']),
 ('Cloud confidence',8,9,['Confidence unset','Low','Medium','High']),('Cloud shadow confidence',10,11,['Confidence unset','Low','Reserved code','High']),
 ('Snow / ice confidence',12,13,['Confidence unset','Low','Reserved code','High']),('Cirrus confidence',14,15,['Confidence unset','Low','Reserved code','High'])]
RADSAT=[(f'Band {b} saturation',b-1,b-1,['Not saturated','Saturated']) for b in range(1,8)]+[
 ('Unused bit 7',7,7,['Unset','Unused bit set']),('Band 9 saturation',8,8,['Not saturated','Saturated']),('Unused bit 9',9,9,['Unset','Unused bit set']),
 ('Unused bit 10',10,10,['Unset','Unused bit set']),('Terrain occlusion',11,11,['Not terrain-occluded','Terrain occlusion']),('Unused bits 12–15',12,15,['Unset'])]
COLORS={'qa_pixel':[[107,114,128],[37,99,235],[234,179,8],[226,232,240],[125,211,252],[100,50,0],[255,150,255],[0,160,190]],
 'qa_radsat':[[37,99,235],[239,68,68],[234,179,8],[100,50,0],[107,114,128]]}

def display(key,values):
    result=np.zeros(values.shape,dtype=np.uint8)
    if key=='qa_pixel':
        for bit,index in [(6,1),(7,7),(5,6),(4,5),(2,4),(1,2),(3,3)]:result[(values&(1<<bit))!=0]=index
    else:
        result[(values&0x0171)!=0]=2;result[(values&14)!=0]=1;result[(values&2048)!=0]=3;result[(values&0xf680)!=0]=4
    return result
def png_check(key,data,raw,url,width,height):
    values=raw[(np.arange(height)*raw.shape[0]//height)[:,None],(np.arange(width)*raw.shape[1]//width)[None,:]]
    valid=(values&1)==0 if key=='qa_pixel' else np.ones(values.shape,dtype=bool)
    rgba=np.zeros((height,width,4),dtype=np.uint8);rgba[:,:,:3]=np.asarray(COLORS[key],dtype=np.uint8)[display(key,values)];rgba[:,:,3]=255;rgba[~valid]=0
    png=base64.b64decode(url.split(',',1)[1]);assert np.array_equal(np.asarray(Image.open(io.BytesIO(png)).convert('RGBA')),rgba)
    return {'rgbaPixelsCompared':width*height,'pngSha256':hashlib.sha256(png).hexdigest(),'rgbaSha256':hashlib.sha256(rgba.tobytes()).hexdigest()}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root');p.add_argument('--plan-samples',action='store_true');a=p.parse_args()
    root=Path(a.root).resolve();assert root.parent==Path('.verification').resolve() and root.name.startswith('landsat-quality-')
    report=json.loads((root/'native-verification.json').read_text(encoding='utf-8'));plans={};results=[]
    for entry in report['cases']:
        key,job,meta=entry['key'],entry['job'],entry['metadata'];value=job['outputPath'];source=Path(value[4:] if value.startswith(chr(92)*2+'?'+chr(92)) else value);assert source.resolve().is_relative_to(root)
        assert hashlib.sha256(source.read_bytes()).hexdigest()==job['sha256']
        with rasterio.open(source) as ds:
            assert ds.count==1 and ds.dtypes==('uint16',) and ds.crs.to_string()==meta['crs'];assert ds.res==(30,30)
            assert ds.tags()['AREA_OR_POINT']==meta['quality']['pixelInterpretation'].replace('PixelIs','') and list(ds.bounds)==meta['bounds']
            assert ds.nodata==meta['quality']['flags']['sourceNoData'] and meta['nodata'] is None
            raw=ds.read(1);assert raw.shape==(meta['height'],meta['width'])
            counts=np.zeros(65536,dtype=np.int64)
            for y in range(0,ds.height,64):counts+=np.bincount(raw[y:y+64].ravel(),minlength=65536)
            codes=np.arange(65536);valid=(codes&1)==0 if key=='qa_pixel' else np.ones(65536,dtype=bool)
            assert int(counts[valid].sum())==meta['quality']['validSampleCount'];assert int(counts.sum())==meta['quality']['sampleCount']
            groups=display(key,codes);class_counts=[int(counts[valid&(groups==i)].sum()) for i in range(len(COLORS[key]))]
            assert class_counts==[c['count'] for c in meta['classes']]
            definitions=PIXEL if key=='qa_pixel' else RADSAT;wanted={0};flag_observations=[]
            for actual,(name,first,last,labels) in zip(meta['quality']['flags']['fields'],definitions,strict=True):
                field=(codes>>first)&((1<<(last-first+1))-1)
                bins=[int(counts[field==v].sum()) for v in range(1<<(last-first+1))]
                assert actual=={'name':name,'startBit':first,'endBit':last,'counts':bins}
                for v,count in enumerate(bins):
                    if count:wanted.add(int(codes[(counts>0)&(field==v)][0]))
                flag_observations.append({'name':name,'counts':bins})
            locations={}
            for y in range(0,ds.height,64):
                block=raw[y:y+64]
                for value in wanted-locations.keys():
                    found=np.flatnonzero(block.ravel()==value)
                    if len(found):locations[value]=[int(found[0]%ds.width),y+int(found[0]//ds.width)]
                if wanted<=locations.keys():break
            points=[{'raw':v,'coordinate':list(ds.xy(row,col)),'pixel':[col,row]} for v,(col,row) in sorted(locations.items())]
            for col,row in [(0,0),(ds.width-1,ds.height-1),(ds.width//2,ds.height//2)]:points.append({'raw':int(raw[row,col]),'coordinate':list(ds.xy(row,col)),'pixel':[col,row]})
            plans[key]=points
            preview=png_check(key,meta,raw,meta['previewDataUrl'],meta['previewWidth'],meta['previewHeight'])
            thumb=entry['thumbnail'];thumbnail=png_check(key,meta,raw,thumb['dataUrl'],thumb['width'],thumb['height'])
            reserved=0
            for sampled in entry['pixels']:
                col,row=sampled['pixel'];value=int(raw[row,col]);assert sampled['value']==value
                assert sampled['isNoData']==(key=='qa_pixel' and bool(value&1))
                q=sampled['quality'];assert q['hex']==f'0x{value:04X}' and q['binary']==f'{value:016b}'
                for f,(name,first,last,labels) in zip(q['fields'],definitions,strict=True):
                    v=(value>>first)&((1<<(last-first+1))-1);label=labels[v] if v<len(labels) else 'Unused bits set'
                    defined=v<len(labels) and label!='Reserved code' and not(name.startswith('Unused') and v!=0)
                    assert f=={'name':name,'startBit':first,'endBit':last,'value':v,'label':label,'defined':defined};reserved+=int(not defined)
            results.append({'key':key,'jobId':job['id'],'sourceSha256':job['sha256'],'bytes':job['bytesDownloaded'],'samplesCounted':raw.size,
                'validSamples':int(counts[valid].sum()),'classCounts':class_counts,'flags':flag_observations,'distinctUnsignedValues':int(np.count_nonzero(counts)),
                'rawSamplesCompared':len(entry['pixels']),'reservedOrUnusedFieldsObserved':reserved,'preview':preview,'thumbnail':thumbnail})
    if a.plan_samples:(root/'sample-plan.json').write_text(json.dumps(plans,indent=2)+'\n',encoding='utf-8')
    else:
        assert report['status']=='passed' and all(r['rawSamplesCompared']>=3 for r in results)
        result={'schema':'geod-landsat-quality-gdal/v1','status':'passed','nativeReceiptSha256':hashlib.sha256((root/'native-verification.json').read_bytes()).hexdigest(),
            'independentDecoder':f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}','cases':results}
        (root/'independent-verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'stage':'samples-planned' if a.plan_samples else 'passed','pixelsCounted':sum(r['samplesCounted'] for r in results),'rawSamplesCompared':sum(r['rawSamplesCompared'] for r in results)}))
if __name__=='__main__':main()
