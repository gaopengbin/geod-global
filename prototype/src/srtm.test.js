import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {normalizeScene,searchURL,nextPageURL} from './catalog.js';
import {isSupportedAsset,providerForAssets,providerForSearch,srtmCell} from './providers.js';
import {downloadableAssets,validateRasterInspection} from './runtime-client.js';
import {projectRequest} from './projects-client.js';
import {projectCatalogScenes,projectExploreSearch} from './project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from './workspace-map-geometry.js';
const item=JSON.parse(readFileSync('prototype/public/samples/srtm-response.json','utf8')).features[0];
const scene=normalizeScene(item,'nasa-srtm');
const step=1/3600;
const job={id:'srtm-job',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'srtm',mediaType:'application/zip',href:scene.assets.srtm.href,sha256:'a'.repeat(64)};
const data={width:3601,height:3601,bandCount:1,dataType:'Int16',crs:'EPSG:4326',bounds:[-123-step/2,37-step/2,-122+step/2,38+step/2],pixelSize:[step,step],nodata:-32768,previewWidth:2,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:job.sha256,elevation:{product:'srtmgl1-v003',heightUnit:'metre',coordinateUnit:'degree',verticalReference:'EPSG:5773',pixelInterpretation:'PixelIsPoint',nodataIsNan:false,sampleCount:4,validSampleCount:3,displayRange:[-27,1000]}};
test('SRTM uses official public catalog without date/cloud filters and accepts CMR indexed pagination',()=>{
  const url=searchURL({provider:'nasa-srtm',bbox:[-123,37,-122,38],start:'invalid',cloud:'invalid',limit:20});
  assert.equal(providerForSearch(url).id,'nasa-srtm');
  for(const key of ['datetime','query','sortby'])assert.equal(new URL(url).searchParams.has(key),false);
  const indexed=new URL(url);indexed.searchParams.delete('collections');indexed.searchParams.set('collections[0]','SRTMGL1_003');
  assert.equal(nextPageURL({links:[{rel:'next',href:indexed.href}]},'nasa-srtm'),indexed.href);
  indexed.searchParams.set('collections[1]','HLSL30_2.0');assert.throws(()=>providerForSearch(indexed.href));
  assert.throws(()=>nextPageURL({links:[{rel:'next',href:url.replace('SRTMGL1_003','HLSL30_2.0')}]},'nasa-srtm'));
});
test('SRTM preserves original HGT ZIP and source identity across project restoration',()=>{
  assert.deepEqual(downloadableAssets(scene).map(asset=>[asset.key,asset.type]),[['srtm','application/zip']]);
  assert.equal(scene.crs,'EPSG:4326');assert.equal(scene.cloud,null);assert.equal(providerForAssets(scene.assets),'nasa-srtm');
  const project=projectRequest({scenes:[scene],bounds:[-122.6,37.5,-122.3,37.8],name:'SRTM'});
  assert.equal(projectCatalogScenes(project)[0].provider,'nasa-srtm');
  assert.equal(projectExploreSearch(project,{}).provider,'nasa-srtm');
  const wrong=structuredClone(item);wrong.assets.hgt.href=wrong.assets.hgt.href.replaceAll('N37W123','N38W123');assert.throws(()=>normalizeScene(wrong,'nasa-srtm'));
  assert.equal(isSupportedAsset(job.href,'elevation'),false);assert.equal(isSupportedAsset(job.href+'?token=PRIVATE','srtm'),false);
  for(const id of ['S00E001.SRTMGL1.hgt','N60E000.SRTMGL1.hgt','S57E001.SRTMGL1.hgt','N00W000.SRTMGL1.hgt','N00E180.SRTMGL1.hgt'])assert.equal(srtmCell(id),null);
});
test('signed SRTM profile retains posted boundary samples and rejects Copernicus DEM metadata',()=>{
  assert.equal(validateRasterInspection(data),data);assert.equal(verifiedMapMetadata(job,data),data);
  for(const [key,v] of [['dataType','Float32'],['width',0],['nodata',0]]) assert.throws(()=>validateRasterInspection({...data,[key]:v}));
  assert.throws(()=>verifiedMapMetadata(job,{...data,width:3600}));
  for(const [key,v] of [['product','cop-dem-glo-30-public'],['verticalReference','EPSG:3855'],['pixelInterpretation','PixelIsArea']])assert.throws(()=>validateRasterInspection({...data,elevation:{...data.elevation,[key]:v}}));
  assert.throws(()=>verifiedMapMetadata(job,{...data,bounds:[-123,37,-122,38]}));
});
test('derived SRTM pins cropped Int16 Point geometry and EGM96 without accepting another DEM product',()=>{
  const metadata={...data,width:3,height:2,bounds:[-123-step/2,38-1.5*step,-123+2.5*step,38+step/2]};
  const profile=Object.fromEntries(['product','heightUnit','coordinateUnit','verticalReference','pixelInterpretation'].map(key=>[key,data.elevation[key]]));
  const result={...job,kind:'raster_mosaic',mediaType:'image/tiff',itemId:'project:srtm-project',mosaic:{projectId:'srtm-project',assetKey:'srtm',sources:[{jobId:job.id,sha256:job.sha256}]},
    mosaicOutput:{width:3,height:2,bandCount:1,crs:data.crs,bounds:metadata.bounds,pixelSize:data.pixelSize,elevation:profile}};
  assert.equal(verifiedMapMetadata(result,metadata),metadata);
  for(const plan of [{...result.mosaicOutput,elevation:{...profile,verticalReference:'EPSG:3855'}},{...result.mosaicOutput,bounds:data.bounds},{...result.mosaicOutput,aerial:{coverage:'mask'}}])
    assert.throws(()=>verifiedMapMetadata({...result,mosaicOutput:plan},metadata));
  for(const change of [{assetKey:'elevation'},{mediaType:'application/zip'},{itemId:'project:other'}]) assert.throws(()=>verifiedMapMetadata({...result,...change},metadata));
});
test('SRTM original negative, zero and NoData heights remain distinct in pixel responses',()=>{
  const coordinate=[-123,38];
  const pixel={jobId:job.id,sha256:job.sha256,crs:'EPSG:4326',coordinate,pixel:[0,0],center:coordinate,label:'Elevation',color:'#808080'};
  for(const value of [-32768,-27,0,32767])assert.equal(verifyPixelResult({...pixel,value,isNoData:value===-32768},job,data,coordinate).value,value);
  for(const value of [null,-32769,32768,1.5])assert.throws(()=>verifyPixelResult({...pixel,value,isNoData:false},job,data,coordinate));
  assert.throws(()=>verifyPixelResult({...pixel,value:0,isNoData:true},job,data,coordinate));
});
