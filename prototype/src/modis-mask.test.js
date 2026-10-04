import test from 'node:test';
import assert from 'node:assert/strict';
import { localRgbGroups, modisRgbQualityJobs, validRgbQualityMask } from './local-rgb.js';
import { MODIS_ASSET_NAMES, MODIS_CRS, MODIS_PIXEL } from './modis.js';
import { QUALITY_DEFINITION } from './quality.js';

export function maskFixture() {
  const item='MYD09A1.A2025177.h08v05.061.2025189031924';
  const jobs=['red','green','blue','modis_qc','modis_state'].map((assetKey,i)=>({
    id:`00000000-0000-4000-8000-00000000000${i}`,kind:'download',status:'succeeded',assetKey,itemId:item,
    href:`https://modiseuwest.blob.core.windows.net/modis-061-cogs/MYD09A1/08/05/2025177/${item}_${MODIS_ASSET_NAMES[assetKey]}.tif`,sha256:String(i+1).repeat(64),bytesDownloaded:1000,
  }));
  return {jobs,group:localRgbGroups(jobs)[0]};
}
test('quality choices require both completed flags from the exact RGB directory and version',()=>{
  const {jobs,group}=maskFixture();assert.deepEqual(modisRgbQualityJobs(group,jobs).map(j=>j.assetKey),['modis_qc','modis_state']);
  assert.deepEqual(modisRgbQualityJobs(group,jobs.slice(0,4)),[]);
  for (const patch of [{status:'running'},{sha256:'bad'},{href:jobs[4].href.replaceAll('2025177','2025185'),itemId:jobs[4].itemId.replace('2025177','2025185')},{href:jobs[4].href+'?token=bad'}])
    assert.deepEqual(modisRgbQualityJobs(group,jobs.map((j,i)=>i===4?{...j,...patch}:j)),[]);
});
test('single-scene crop quality matches the grid and parent pins; duplicate or mismatched selections are excluded',()=>{
  const {jobs}=maskFixture();
  const outputs=jobs.map((j,i)=>({...j,id:`derived-${i}`,kind:'raster_mosaic',itemId:'project:p',mosaic:{projectId:'p',assetKey:j.assetKey,sources:[{jobId:j.id,sha256:j.sha256}]},
    mosaicOutput:{width:8,height:5,crs:MODIS_CRS,bounds:[0,0,8*MODIS_PIXEL,5*MODIS_PIXEL],pixelSize:[MODIS_PIXEL,MODIS_PIXEL],bandCount:1,
      ...(i<3?{calibration:{product:'modis-09a1-v061'}}:{quality:{product:'modis-09a1-v061'}})}}));
  const all=[...jobs,...outputs], group=localRgbGroups(all).find(g=>g.derived);
  assert.equal(modisRgbQualityJobs(group,all).length,2);
  for(const changed of [outputs.map((j,i)=>i===4?{...j,mosaicOutput:{...j.mosaicOutput,width:9}}:j),outputs.map(j=>({...j,mosaic:{...j.mosaic,sources:[...j.mosaic.sources,...j.mosaic.sources]}}))])
    assert.deepEqual(modisRgbQualityJobs({...group,sourceJobs:changed.slice(0,3)},[...jobs,...changed]),[]);
});
test('matched multi-scene quality requires every original layer and preserves ordered same-scene pins',()=>{
  const {jobs}=maskFixture(), oldItem='MYD09A1.A2025169.h08v05.061.2025178155736';
  const older=jobs.map((j,i)=>({...j,id:'older-'+i,itemId:oldItem,href:j.href.replace('2025177/','2025169/').replace(j.itemId,oldItem)}));
  const outputs=jobs.map((j,i)=>({...j,id:'multi-'+i,kind:'raster_mosaic',itemId:'project:p',mosaic:{projectId:'p',assetKey:j.assetKey,sources:[older[i],j].map(p=>({jobId:p.id,sha256:p.sha256}))},
    mosaicOutput:{width:8,height:5,crs:MODIS_CRS,bounds:[0,0,8*MODIS_PIXEL,5*MODIS_PIXEL],pixelSize:[MODIS_PIXEL,MODIS_PIXEL],bandCount:1,...(i<3?{calibration:{product:'modis-09a1-v061'}}:{quality:{product:'modis-09a1-v061'}})}}));
  const all=[...older,...jobs,...outputs],group=localRgbGroups(all).find(g=>g.derived);
  assert.deepEqual(modisRgbQualityJobs(group,all).map(j=>j.id),['multi-3','multi-4']);
  assert.deepEqual(modisRgbQualityJobs(group,all.filter(j=>j.id!==older[3].id)),[]);
  const source=j=>({jobId:j.id,band:j.assetKey,kind:j.kind,sha256:j.sha256,itemId:j.itemId,href:j.href});
  const parents=outputs.map((j,i)=>({...source(j),provenance:{project:{id:'p',geometry:null},sources:[older[i],jobs[i]].map(source)}}));
  const mask={schemaVersion:'geod-modis-rgb-mask/v2',policy:'clear_best',excludeSnow:false,definition:QUALITY_DEFINITION,sources:parents.slice(3),
    coupled:{selection:'newest complete qualified RGB scene wins; composite start then item ID break ties',geometry:null,scenes:[older,jobs].map(scene=>({sources:scene.map(source),grid:{width:2400,height:2400,crs:MODIS_CRS}}))}};
  const saved={rgbSpec:{grid:{width:8,height:5},profile:{product:'modis-09a1-v061'},sources:parents.slice(0,3),qualityMask:mask},
    rgbOutput:{channelValidPixels:[35,35,35],commonValidPixels:35,qualityMask:{examinedPixels:40,rejectedPixels:5,inputCommonValidPixels:38,removedValidPixels:3,coupled:{sceneValidPixels:[10,25],fallbackPixels:8}}}};
  assert.equal(validRgbQualityMask(saved),true);
  for(const mutate of [j=>{j.rgbSpec.qualityMask.coupled.scenes.reverse();},j=>{j.rgbSpec.qualityMask.coupled.scenes[0].sources[4].sha256='bad';},j=>{j.rgbOutput.qualityMask.coupled.sceneValidPixels=[11,25];},j=>{j.rgbOutput.qualityMask.coupled.fallbackPixels=36;}]){
    const changed=structuredClone(saved);mutate(changed);assert.equal(validRgbQualityMask(changed),false);
  }
});
test('saved mask counts and policy stay verifiable after all parents are removed',()=>{
  const {jobs}=maskFixture();
  const mask={schemaVersion:'geod-modis-rgb-mask/v1',policy:'clear_best',excludeSnow:true,definition:QUALITY_DEFINITION,sources:jobs.slice(3).map(j=>({jobId:j.id,band:j.assetKey,sha256:j.sha256,href:j.href}))};
  const job={rgbSpec:{grid:{width:8,height:5},profile:{product:'modis-09a1-v061'},sources:jobs.slice(0,3).map(j=>({jobId:j.id})),qualityMask:mask},
    rgbOutput:{commonValidPixels:25,qualityMask:{examinedPixels:40,rejectedPixels:12,inputCommonValidPixels:35,removedValidPixels:10}}};
  assert.equal(validRgbQualityMask(job),true);
  for(const patch of [{examinedPixels:39},{removedValidPixels:13},{rejectedPixels:-1},{inputCommonValidPixels:41}])
    assert.equal(validRgbQualityMask({...job,rgbOutput:{...job.rgbOutput,qualityMask:{...job.rgbOutput.qualityMask,...patch}}}),false);
  for(const patch of [{definition:'https://example.org'},{policy:'any'},{excludeSnow:'yes'}])
    assert.equal(validRgbQualityMask({...job,rgbSpec:{...job.rgbSpec,qualityMask:{...mask,...patch}}}),false);
  assert.equal(validRgbQualityMask({...job,rgbOutput:{commonValidPixels:25}}),false);
});
