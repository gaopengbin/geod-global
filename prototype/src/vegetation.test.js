import test from 'node:test';
import assert from 'node:assert/strict';
import {vegetationIdentity,vegetationAssetIdentity,vegetationMatchesJob,validVegetationPixel,VEGETATION_PIXEL} from './vegetation.js';
import {normalizeScene,searchURL} from './catalog.js';
import {projectRequest} from './projects-client.js';
import {projectCatalogScenes,projectExploreSearch} from './project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from './workspace-map-geometry.js';
import {compositePeriodLabel} from './composite-period.js';
import {isSupportedAsset,providerForAssets} from './providers.js';
import {validateRasterInspection} from './runtime-client.js';
const terra='MOD13Q1.A2025177.h08v05.061.2025195142416',aqua='MYD13Q1.A2025169.h08v05.061.2025189102903';
const href=(id,key)=>`https://modiseuwest.blob.core.windows.net/modis-061-cogs/${id.split('.')[0]}/08/05/${id.split('.')[1].slice(1)}/${id}_250m_16_days_${key.toUpperCase()}.tif`;
// Protocol fixtures copied from reviewed product fields, not a download/processing receipt.
function item(id=terra){const p=vegetationIdentity(id);return {id,collection:'modis-13Q1-061',bbox:[-130.540729,30,-103.923048,40],properties:{start_datetime:p.date,end_datetime:p.endDate,'modis:horizontal-tile':8,'modis:vertical-tile':5},assets:Object.fromEntries(['ndvi','evi'].map(key=>[`250m_16_days_${key.toUpperCase()}`,{href:href(id,key),type:'image/tiff; application=geotiff','raster:bands':[{unit:key.toUpperCase(),data_type:'int16',scale:0.0001,spatial_resolution:250}]}]))};}
test('Terra/Aqua shifted 16-day intervals and shortened year-end intervals remain distinct',()=>{
  assert.equal(vegetationIdentity(terra).date.slice(0,10),'2025-06-26');assert.equal(vegetationIdentity(terra).endDate,'2025-07-11T23:59:59Z');
  assert.equal(vegetationIdentity(aqua).date.slice(0,10),'2025-06-18');assert.equal(vegetationIdentity(aqua).endDate,'2025-07-03T23:59:59Z');
  for(const id of [terra.replace('MOD13Q1','MYD13Q1'),aqua.replace('MYD13Q1','MOD13Q1'),terra.replace('2025177','2025178'),terra.replace('.061.','.060.'),terra.replace('h08v05','h36v05'),terra.replace('142416','242416')])assert.equal(vegetationIdentity(id),null);
  assert.equal(vegetationIdentity('MOD13Q1.A2024353.h08v05.061.2025001142416').endDate,'2024-12-31T23:59:59Z');
  assert.equal(vegetationIdentity('MYD13Q1.A2025361.h08v05.061.2026001142416').endDate,'2025-12-31T23:59:59Z');
});
test('Indexes bind exact paths, unsigned URLs, independent product metadata and channel',()=>{
  assert.equal(vegetationAssetIdentity(href(terra,'ndvi'),'ndvi').id,terra);assert(isSupportedAsset(href(terra,'evi'),'evi'));
  for(const bad of [href(terra,'ndvi')+'?sig=secret',href(terra,'ndvi').replace('/08/','/09/'),href(terra,'ndvi').replace('https:','http:'),href(terra,'ndvi').replace('NDVI','EVI')])assert.equal(vegetationAssetIdentity(bad,'ndvi'),null);
  const scene=normalizeScene(item(),'planetary-vegetation');assert.equal(scene.cloud,null);assert.equal(scene.gsd,250);
  for(const mutate of [d=>d.properties.end_datetime='2025-07-03T23:59:59Z',d=>d.properties.platform='aqua',d=>d.collection='modis-09A1-061',d=>d.properties['modis:horizontal-tile']=9,d=>d.assets['250m_16_days_NDVI']['raster:bands'][0].scale=10000,d=>d.assets['250m_16_days_EVI']['raster:bands'][0].unit='NDVI',d=>d.assets['250m_16_days_EVI']['raster:bands'][0].nodata=-28672]){const d=item();mutate(d);assert.throws(()=>normalizeScene(d,'planetary-vegetation'));}
  const url=new URL(searchURL({provider:'planetary-vegetation',bbox:scene.bbox,start:'2025-06-01',end:'2025-06-30',limit:20,cloud:'invalid'}));assert.equal(url.searchParams.get('query'),null);assert.equal(url.searchParams.get('sortby'),'-properties.start_datetime');
});
test('Both indexes survive project persistence, return-to-explore and source calibration pins',()=>{
  const scene=normalizeScene(item(),'planetary-vegetation'),request=projectRequest({name:'indices',bounds:[-122.5,37.7,-122.4,37.8],scenes:[scene]});
  assert.equal(request.scenes[0].assets.ndvi.rasterBand.nodata,-3000);assert.equal(Object.keys(request.scenes[0].assets).join(','),'ndvi,evi');
  assert.equal(providerForAssets(request.scenes[0].assets),'planetary-vegetation');assert.equal(projectExploreSearch(request,{}).provider,'planetary-vegetation');
  const restored=projectCatalogScenes(request)[0];assert.equal(restored.assets.evi.href,scene.assets.evi.href);assert.equal(restored.gsd,250);assert.equal(compositePeriodLabel(restored,d=>d.slice(0,10)),'2025-06-26 – 2025-07-11');
});
test('Index inspection preserves signed extremes separately from NoData and display range',()=>{
  const job={id:'index',kind:'download',itemId:terra,href:href(terra,'ndvi'),assetKey:'ndvi',status:'succeeded',sha256:'a'.repeat(64)};
  const size=VEGETATION_PIXEL*4800,metadata={sha256:job.sha256,width:4800,height:4800,bandCount:1,dataType:'Int16',crs:'MODIS:Sinusoidal',nodata:-3000,bounds:[-10*size,3*size,-9*size,4*size],pixelSize:[VEGETATION_PIXEL,VEGETATION_PIXEL],classes:[],previewWidth:160,previewHeight:160,previewDataUrl:'data:image/png;base64,AAAA',vegetation:{product:'modis-13q1-v061',index:'ndvi',scale:0.0001,offset:0,validRange:[-2000,10000],displayRange:[-2000,10000],palette:'modis-vi-v1',sampleCount:25600,validSampleCount:20000,outOfRangeSampleCount:10,pixelInterpretation:'PixelIsArea'}};
  assert.equal(validateRasterInspection(metadata),metadata);
  for(const mutate of [d=>d.dataType='UInt16',d=>d.vegetation.scale=10000,d=>d.vegetation.palette='unknown',d=>d.vegetation.index='scl',d=>d.reflectance={},d=>d.composite={},d=>d.vegetation.validSampleCount=25601]){const d=structuredClone(metadata);mutate(d);assert.throws(()=>validateRasterInspection(d),/invalid inspection data/);}
  assert.equal(verifiedMapMetadata(job,metadata),metadata);
  for(const value of [-32768,-3000,-2000,-1,0,10000,32767]){const p={value,isNoData:value===-3000,...(value===-3000?{}:{indexValue:value*0.0001})};assert(validVegetationPixel(p,metadata));assert(!validVegetationPixel({...p,reflectance:0.3},metadata));}
  for(const mutate of [d=>d.vegetation.index='evi',d=>d.vegetation.scale=10000,d=>d.width=2400,d=>d.nodata=-28672,d=>d.reflectance={},d=>d.bounds[0]+=500,d=>d.vegetation.outOfRangeSampleCount=20001]){const d=structuredClone(metadata);mutate(d);assert(!vegetationMatchesJob(job,d));}
  const coordinate=[metadata.bounds[0]+.5*VEGETATION_PIXEL,metadata.bounds[3]-.5*VEGETATION_PIXEL];const pixel={jobId:job.id,sha256:job.sha256,crs:metadata.crs,pixel:[0,0],coordinate,center:coordinate,value:32767,indexValue:3.2767,isNoData:false,label:'NDVI vegetation index',color:'#185633'};
  assert.equal(verifyPixelResult(pixel,job,metadata,coordinate),pixel);assert.throws(()=>verifyPixelResult({...pixel,indexValue:1},job,metadata,coordinate));
});
