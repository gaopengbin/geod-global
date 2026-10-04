import test from 'node:test';
import assert from 'node:assert/strict';
import { validateRasterInspection } from './runtime-client.js';
import { verifiedMapMetadata, verifyPixelResult, mapClipRecipe } from './workspace-map-geometry.js';

function fixture(signed) {
  const itemId=signed?'HLS.L30.T10SEG.2025179T184546.v2.0':'LC09_L2SP_044034_20250628_02_T1';
  const product='LC09_L2SP_044034_20250628_20250629_02_T1';
  const href=signed?`https://data.lpdaac.earthdatacloud.nasa.gov/lp-prod-protected/HLSL30.020/${itemId}/${itemId}.B04.tif`:
    `https://landsateuwest.blob.core.windows.net/landsat-c2/level-2/standard/oli-tirs/2025/044/034/${product}/${product}_SR_B4.TIF`;
  const job={id:'933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b',kind:'download',status:'succeeded',assetKey:'red',itemId,href,sha256:'a'.repeat(64)};
  const data={width:3,height:2,bandCount:1,dataType:signed?'Int16':'UInt16',crs:'EPSG:32610',bounds:[500000,4199940,500090,4200000],
    pixelSize:[30,30],nodata:signed?-9999:0,previewWidth:3,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',sha256:job.sha256,classes:[],
    reflectance:{product:signed?'hls-l30-v2':'landsat-c2-l2',band:'red',scale:signed?0.0001:0.0000275,offset:signed?0:-0.2,pixelInterpretation:'PixelIsArea',displayRange:[100,10000],sampleCount:6,validSampleCount:5}};
  return {job,data};
}
for(const signed of [false,true]) {
  test(`${signed?'HLS signed':'Landsat unsigned'} contracts pin calibration, datatype and product channel`,()=>{
    const {job,data}=fixture(signed);
    assert.equal(validateRasterInspection(data),data);
    assert.equal(verifiedMapMetadata(job,data),data);
    for(const changes of [{dataType:'UInt8'}, {nodata:null}, {bandCount:3}, {pixelSize:[20,20]},
      {reflectance:{...data.reflectance,scale:0.1}}, {reflectance:{...data.reflectance,sampleCount:10}},
      {reflectance:{...data.reflectance,displayRange:[10000,100]}}, {classes:[{value:1,label:'fake',color:'#000000',count:6}]}]) {
      assert.throws(()=>validateRasterInspection({...data,...changes}));
    }
    for(const changes of [{assetKey:'green'}, {itemId:'other'}, {href:job.href+'?sig=secret'}]) {
      assert.throws(()=>verifiedMapMetadata({...job,...changes},data));
    }
    assert.throws(()=>mapClipRecipe(job,data,data.bounds,'raw band'));
  });
  test(`${signed?'HLS signed':'Landsat unsigned'} pixel contracts preserve source DN and unbounded reflectance`,()=>{
    const {job,data}=fixture(signed);
    const coordinate=[500045,4199985];
    const value=signed?-100:65535;
    const pixel={jobId:job.id,sha256:job.sha256,crs:data.crs,coordinate,pixel:[1,0],value,
      reflectance:value*data.reflectance.scale+data.reflectance.offset,label:'red reflectance band',color:'#808080',isNoData:false};
    assert.equal(verifyPixelResult(pixel,job,data,coordinate),pixel);
    assert.equal(verifyPixelResult({...pixel,value:data.nodata,reflectance:undefined,isNoData:true},job,data,coordinate).isNoData,true);
    for(const changes of [{value:signed?32768:-1},{reflectance:0}, {isNoData:true}, {values:[1,2,3]}])
      assert.throws(()=>verifyPixelResult({...pixel,...changes},job,data,coordinate));
    assert.throws(()=>verifyPixelResult({...pixel,value:data.nodata,isNoData:true},job,data,coordinate));
  });
  test(`${signed?'HLS signed':'Landsat unsigned'} derived map metadata pins output geometry and calibration`,()=>{
    const {job,data}=fixture(signed);
    const projectId='686cba2c-18d2-48ed-96b1-4eec078ad9e3';
    const processed={...job,kind:'raster_mosaic',itemId:`project:${projectId}`,
      mosaic:{projectId,assetKey:'red',sources:[{jobId:job.id,sha256:job.sha256}]},
      mosaicOutput:{width:data.width,height:data.height,bandCount:1,crs:data.crs,bounds:data.bounds,pixelSize:data.pixelSize,
        calibration:{product:data.reflectance.product,signed,scale:data.reflectance.scale,offset:data.reflectance.offset,nodata:data.nodata}}};
    assert.equal(verifiedMapMetadata(processed,data),data);
    for(const changes of [{itemId:'other'}, {mosaic:{...processed.mosaic,assetKey:'green'}},
      {mosaicOutput:{...processed.mosaicOutput,width:4}},
      {mosaicOutput:{...processed.mosaicOutput,calibration:{...processed.mosaicOutput.calibration,offset:0.25}}}]) {
      assert.throws(()=>verifiedMapMetadata({...processed,...changes},data));
    }
  });
}
