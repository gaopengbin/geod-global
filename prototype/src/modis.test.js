import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { modisIdentity, modisAssetIdentity, modisPeriodLabel, MODIS_CRS, MODIS_PIXEL } from './modis.js';
import { normalizeScene, searchURL } from './catalog.js';
import { projectRequest } from './projects-client.js';
import { projectCatalogScenes, projectExploreSearch } from './project-explore.js';
import { localRgbGroups, verifiedCompositeMetadata, verifyCompositePixel } from './local-rgb.js';
import { reflectanceMatchesJob } from './reflectance.js';
import { rasterProjectionDefinition, verifiedMapMetadata } from './workspace-map-geometry.js';
const item = JSON.parse(readFileSync(new URL('../qa/modis-catalog-item.json',import.meta.url),'utf8'));
const scene = normalizeScene(item,'planetary-modis');
const jobs = ['red','green','blue'].map((key,i)=>({id:String(i),kind:'download',status:'succeeded',itemId:scene.id,assetKey:key,href:scene.assets[key].href,sha256:String(i+1).repeat(64)}));

test('MODIS processed bands use the recorded sinusoidal output grid rather than original tile dimensions',()=>{
  const s=MODIS_PIXEL,width=20,height=12,bounds=[-10*s*2400+100*s,4*s*2400-212*s,-10*s*2400+120*s,4*s*2400-200*s];
  const metadata={width,height,bounds,pixelSize:[s,s],crs:MODIS_CRS,sha256:'a'.repeat(64),dataType:'Int16',bandCount:1,nodata:-28672,classes:[],previewWidth:1,previewHeight:1,reflectance:{product:'modis-09a1-v061',band:'red',scale:.0001,offset:0,pixelInterpretation:'PixelIsArea',displayRange:[-100,16000],sampleCount:1,validSampleCount:1}};
  const job={...jobs[0],kind:'raster_mosaic',itemId:'project:modis',sha256:metadata.sha256,mosaic:{projectId:'modis',assetKey:'red',sources:[{jobId:jobs[0].id,sha256:jobs[0].sha256}]},mosaicOutput:{width,height,bounds,pixelSize:[s,s],crs:MODIS_CRS,bandCount:1,calibration:{product:'modis-09a1-v061',signed:true,scale:.0001,offset:0,nodata:-28672}}};
  verifiedMapMetadata(job,metadata);
  assert.equal(reflectanceMatchesJob(jobs[0],metadata),false);
  for(const data of [{...metadata,width:21},{...metadata,nodata:-9999},{...metadata,pixelSize:[500,500]},{...metadata,crs:'EPSG:32610'},{...metadata,bounds:bounds.map(v=>v+1)}]) assert.throws(()=>verifiedMapMetadata(job,data));
  assert.equal(reflectanceMatchesJob({...job,href:job.href.replace('modiseuwest','landsateuwest')},metadata),false);
});
test('MODIS interval, band path, platform and collection cannot be confused with HLS or a scene acquisition',()=>{
  assert.equal(scene.date,'2025-06-26T00:00:00.000Z');assert.equal(scene.endDate,'2025-07-03T23:59:59Z');assert.equal(scene.cloud,null);
  assert.equal(scene.gsd,500);assert.equal(scene.crs,MODIS_CRS);
  assert.equal(modisPeriodLabel(jobs[0],v=>v.slice(0,10)), '2025-06-26 – 2025-07-03');
  const url = new URL(searchURL({provider:'planetary-modis',bbox:scene.bbox,start:'2025-06-01',end:'2025-06-30',limit:20,cloud:'invalid'}));
  assert.equal(url.searchParams.get('query'),null);assert.equal(url.searchParams.get('sortby'),'-properties.start_datetime');
  for(const [key,asset] of Object.entries(scene.assets).filter(([k])=>['red','green','blue'].includes(k))) {
    assert.equal(modisAssetIdentity(asset.href,key).id,scene.id);
    for(const bad of [asset.href+'?sig=secret',asset.href.replace('/08/','/09/'),asset.href.replace('_b0','_b9'),asset.href.replace('.061.','.060.')]) assert.equal(modisAssetIdentity(bad,key),null);
  }
  assert.equal(modisIdentity(scene.id.replace('2025177','2025178')),null);
  const last=modisIdentity(scene.id.replace('2025177','2025361').replace('2025189031924','2026001031924'));
  assert.equal(last.endDate,'2025-12-31T23:59:59Z');
  for(const mutate of [d=>d.properties.end_datetime='2025-07-04T23:59:59Z',d=>d.properties.platform='terra',d=>d.assets.sur_refl_b01.href=d.assets.sur_refl_b03.href]) {
    const changed=structuredClone(item);mutate(changed);assert.throws(()=>normalizeScene(changed,'planetary-modis'));
  }
});
test('MODIS projects restore exact periods, calibration and unsigned source pins',()=>{
  const project=projectRequest({name:'MODIS',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]});
  assert.deepEqual(project.scenes[0].assets.red.rasterBand,{dataType:'int16',scale:.0001,offset:0,nodata:-28672,spatialResolution:500});
  const restored=projectCatalogScenes(project)[0];assert.equal(restored.endDate,scene.endDate);assert.equal(restored.assets.green.href,scene.assets.green.href);
  assert.equal(projectExploreSearch(project,{}).provider,'planetary-modis');
});
test('MODIS local RGB validates its distinct NoData, sinusoidal grid and exact original DN',()=>{
  const group=localRgbGroups(jobs)[0];assert.equal(group.product,'modis-09a1-v061');
  const s=MODIS_PIXEL,metadata={width:2400,height:2400,bandCount:3,dataType:'Int16',crs:MODIS_CRS,bounds:[-10*s*2400,3*s*2400,-9*s*2400,4*s*2400],pixelSize:[s,s],pixelInterpretation:'PixelIsArea',nodata:-28672,previewWidth:1,previewHeight:1,previewDataUrl:'data:image/png;base64,AAAA',composite:{product:group.product,sources:jobs.map(j=>({jobId:j.id,sha256:j.sha256,band:j.assetKey})),scale:.0001,offset:0,displayRanges:[[-100,16000],[-100,16000],[-100,16000]],sampleCount:1,validSampleCount:1}};
  verifiedCompositeMetadata(group,metadata);
  const coordinate=[metadata.bounds[0]+s/2,metadata.bounds[3]-s/2];
  const pixel={sources:metadata.composite.sources,crs:MODIS_CRS,coordinate,pixel:[0,0],center:coordinate,values:[-100,0,16000],reflectances:[-.01,0,1.6],channelNoData:[false,false,false],isNoData:false};
  verifyCompositePixel(pixel,group,metadata,coordinate);
  for(const wrong of [{...metadata,nodata:-9999},{...metadata,crs:'EPSG:32610'},{...metadata,pixelSize:[500,500]}]) assert.throws(()=>verifiedCompositeMetadata(group,wrong));
  const band={...metadata,bandCount:1,classes:[],reflectance:{...metadata.composite,band:'red',pixelInterpretation:'PixelIsArea',displayRange:[-100,16000]}};
  assert.equal(reflectanceMatchesJob(jobs[0],band),true);assert.match(rasterProjectionDefinition(MODIS_CRS),/6371007.181/);
});
