import test from 'node:test';
import assert from 'node:assert/strict';
import item from './srtm-num.fixture.json' with {type:'json'};
import {normalizeScene} from './catalog.js';
import {numCell,numHref,isSupportedAsset,providerForAssets} from './providers.js';
import {numMatchesJob,validNumPixel} from './srtm-num.js';
import {verifiedMapMetadata,verifyPixelResult,mapClipRecipe} from './workspace-map-geometry.js';
import {projectCatalogScenes} from './project-explore.js';
const clone=x=>structuredClone(x);
test('real SRTM NUM keeps its official original, product identity and observation period',()=>{
  const scene=normalizeScene(clone(item),'srtm-num');
  assert.equal(scene.provider,'srtm-num');assert.equal(scene.crs,'EPSG:4326');
  assert.equal(scene.endDate,'2000-02-21T23:59:59.000Z');assert.equal(scene.assets.srtm_num.href,numHref(item.id));
  assert.equal(scene.assets.srtm_num.type,'application/zip');assert.equal(providerForAssets(scene.assets),'srtm-num');
  assert.deepEqual(numCell(item.id),[116,39]);
  for(const change of [x=>x.collection='SRTMGL1_003',x=>x.assets.num.href=x.assets.num.href.replace('.003/','.002/'),x=>x.id='N39E117.SRTMGL1N.num']) {
    const changed=clone(item);change(changed);assert.throws(()=>normalizeScene(changed,'srtm-num'));
  }
  for(const href of [numHref(item.id)+'?token=PRIVATE',numHref(item.id).replace('https:','http:'),numHref(item.id).replace('nasa.gov','nasa.gov.evil')])assert.equal(isSupportedAsset(href,'srtm_num'),false);
  assert.equal(isSupportedAsset(numHref(item.id),'srtm'),false);
  const restored=projectCatalogScenes({scenes:[{itemId:item.id,date:scene.date,bbox:item.bbox,crs:'EPSG:4326',assets:{srtm_num:{href:numHref(item.id),mediaType:'application/zip'}}}]});
  assert.equal(restored[0].provider,'srtm-num');assert.equal(restored[0].assets.srtm_num.href,numHref(item.id));
});
test('NUM map and pixel receipts preserve byte codes including zero and 255, never elevation or automatic clips',()=>{
  const job={id:'num-job',kind:'download',status:'succeeded',assetKey:'srtm_num',itemId:item.id,href:numHref(item.id),sha256:'a'.repeat(64)};
  const h=1/7200,metadata={width:3601,height:3601,bandCount:1,dataType:'UInt8',crs:'EPSG:4326',nodata:null,bounds:[116-h,39-h,117+h,40+h],pixelSize:[1/3600,1/3600],sha256:job.sha256};
  assert.equal(numMatchesJob(job,metadata),true);assert.equal(verifiedMapMetadata(job,metadata),metadata);
  for(const field of [{dataType:'Int16'},{nodata:0},{elevation:{}},{pixelSize:[1/1200,1/1200]},{bounds:[116,39,117,40]}])assert.equal(numMatchesJob(job,{...metadata,...field}),false);
  for(const value of [0,1,102,206,255]) {
    const pixel={jobId:job.id,sha256:job.sha256,crs:metadata.crs,coordinate:[116,40],pixel:[0,0],value,label:'SRTM NUM source code',color:'#000000',isNoData:false};
    assert.equal(validNumPixel(pixel),true);assert.equal(verifyPixelResult(pixel,job,metadata,[116,40]),pixel);
    assert.equal(validNumPixel({...pixel,isNoData:true}),false);
  }
  assert.throws(()=>mapClipRecipe(job,metadata,metadata.bounds,'NUM clip'));
});
