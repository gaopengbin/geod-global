"""Independently verify actual GeoTIFFs, source pixels, exact mosaic copies and masks."""
import hashlib
import json
from pathlib import Path
import numpy as np
import rasterio
import tifffile
from rasterio.features import geometry_mask
from rasterio.windows import Window

base=Path('.verification/glo90-public')
runtime=json.loads((base/'runtime.json').read_text(encoding='utf-8'))
originals={entry['job']['id']:entry for entry in runtime['originals']}
sources=[]
for entry in originals.values():
    job=entry['job']; metadata=entry['metadata']; path=Path(job['outputPath'])
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    assert digest==job['sha256'] and path.stat().st_size==job['bytesDownloaded']
    with rasterio.open(path) as source:
        assert source.dtypes==('float32',) and source.count==1
        assert source.crs.to_epsg()==4326 and source.tags()['AREA_OR_POINT']=='Point'
        assert (source.width,source.height)==(metadata['width'],metadata['height'])
        assert np.allclose(tuple(source.bounds),metadata['bounds'],rtol=0,atol=1e-10)
        assert np.allclose((source.transform.a,-source.transform.e),metadata['pixelSize'],rtol=0,atol=1e-12)
        data=source.read(1)
        assert np.all(np.isfinite(data)), 'These real acceptance originals have no NoData'
        for pixel in entry['pixels']:
            col,row=pixel['pixel']; value=float(data[row,col])
            assert value==pixel['value'] and not pixel['isNoData']
            assert np.allclose(source.xy(row,col),pixel['center'],rtol=0,atol=1e-10)
        sources.append({'itemId':job['itemId'],'sha256':digest,'bytes':path.stat().st_size,
            'dimensions':[source.width,source.height],'pixelSize':metadata['pixelSize'],
            'crs':'EPSG:4326','dataType':'Float32','pixelInterpretation':'PixelIsPoint',
            'nodata':source.nodata,'comparedNativePixelProbes':len(entry['pixels']),
            'minimum':float(data.min()),'maximum':float(data.max()),'zeroPixels':int((data==0).sum()),
            'negativePixels':int((data<0).sum())})
results=[]
for entry in runtime['results']:
    job=entry['job']; plan=job['mosaicOutput']; path=Path(job['outputPath'])
    assert hashlib.sha256(path.read_bytes()).hexdigest()==job['sha256']
    assert plan['elevation']['product']=='cop-dem-glo-90'
    with rasterio.open(path) as output:
        actual=output.read(1);expected=np.full(actual.shape,np.nan,dtype=np.float32)
        assert output.dtypes==('float32',) and np.isnan(output.nodata)
        assert output.tags()['AREA_OR_POINT']=='Point' and output.crs.to_epsg()==4326
        assert np.allclose(tuple(output.bounds),plan['bounds'],rtol=0,atol=1e-10)
        with tifffile.TiffFile(path) as tiff:
            raw=tiff.pages[0].tags['GeoKeyDirectoryTag'].value
            keys={raw[index]:raw[index+3] for index in range(4,4+raw[3]*4,4)}
            assert keys[1025]==2 and keys[2048]==4326 and keys[4096]==3855 and keys[4099]==9001
        for pin in job['mosaic']['sources']:
            source=originals[pin['jobId']]['job'];assert source['sha256']==pin['sha256']
            with rasterio.open(source['outputPath']) as raster:
                assert abs(raster.transform.a-output.transform.a)<1e-12 and abs(raster.transform.e-output.transform.e)<1e-12
                offx=round((raster.bounds.left-output.bounds.left)/output.transform.a)
                offy=round((output.bounds.top-raster.bounds.top)/-output.transform.e)
                x0=max(0,-offx);y0=max(0,-offy);x1=min(raster.width,output.width-offx);y1=min(raster.height,output.height-offy)
                if x1<=x0 or y1<=y0:continue
                pixels=raster.read(1,window=Window(x0,y0,x1-x0,y1-y0))
                target=expected[y0+offy:y1+offy,x0+offx:x1+offx];valid=np.isfinite(pixels)
                if raster.nodata is not None and np.isfinite(raster.nodata):valid &= pixels!=raster.nodata
                target[valid]=pixels[valid]
        geometry=entry['project'].get('geometry');masked=0
        if geometry:
            inside=geometry_mask([geometry],actual.shape,output.transform,invert=True,all_touched=False)
            masked=int((~inside).sum());expected[~inside]=np.nan
        assert np.array_equal(actual,expected,equal_nan=True)
        finite=np.isfinite(expected);assert np.array_equal(actual[finite].view(np.uint32),expected[finite].view(np.uint32))
        assert int(finite.sum())==plan['coveredPixels'] and masked==plan['maskedPixels']
        results.append({'label':entry['label'],'sha256':job['sha256'],'bytes':path.stat().st_size,
            'dimensions':[output.width,output.height],'comparedPixels':int(actual.size),'validPixels':int(finite.sum()),
            'maskedPixels':masked,'allOriginalFloat32BitsMatch':True,'horizontalCrs':'EPSG:4326',
            'verticalReference':'EPSG:3855','heightUnit':'metre','pixelInterpretation':'PixelIsPoint','nodata':'NaN'})
report={'passed':True,'independentLibraries':['Rasterio/GDAL','NumPy','tifffile'],
    'sourceFiles':sources,'processedOutputs':results,'comparedOutputPixels':sum(result['comparedPixels'] for result in results)}
(base/'independent-reference.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps(report))
