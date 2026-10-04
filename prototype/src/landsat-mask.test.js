import test from 'node:test';
import assert from 'node:assert/strict';
import { localRgbGroups, landsatRgbQualityJobs, validRgbQualityMask, rgbQualityPolicyLabel } from './local-rgb.js';
import { LANDSAT_QUALITY_DEFINITION } from './landsat-quality.js';
const item='LC09_L2SP_044034_20250628_02_T1', product='LC09_L2SP_044034_20250628_20250629_02_T1';
const keys=['red','green','blue','qa_pixel','qa_radsat'];
const jobs=keys.map((assetKey,i)=>({id:`00000000-0000-4000-8000-00000000000${i}`,assetKey,kind:'download',status:'succeeded',itemId:item,sha256:String(i+1).repeat(64),bytesDownloaded:1000,
  href:`https://landsateuwest.blob.core.windows.net/landsat-c2/level-2/standard/oli-tirs/2025/044/034/${product}/${product}_${['SR_B4','SR_B3','SR_B2','QA_PIXEL','QA_RADSAT'][i]}.TIF`}));
const group=localRgbGroups(jobs)[0];
const pins=jobs.map(j=>({jobId:j.id,band:j.assetKey,kind:j.kind,itemId:j.itemId,sha256:j.sha256,href:j.href,bytes:j.bytesDownloaded}));
test('Landsat RGB matches both quality layers by exact scene and processing version',()=>{
  assert.deepEqual(landsatRgbQualityJobs(group,jobs),jobs.slice(3));
  const wrong=jobs.map(j=>j.assetKey==='qa_radsat'?{...j,href:j.href.replaceAll('20250629','20250630')}:j);
  assert.deepEqual(landsatRgbQualityJobs(group,wrong),[]);
  assert.deepEqual(landsatRgbQualityJobs(group,jobs.slice(0,4)),[]);
  assert.deepEqual(landsatRgbQualityJobs(group,jobs.map(j=>j.assetKey==='qa_radsat'?{...j,status:'running'}:j)),[]);
});
test('Landsat RGB enables coherent selection only with all matching original scene layers',()=>{
  const make=(parents)=>jobs.map((j,i)=>({...j,id:`clip-${i}`,kind:'raster_mosaic',itemId:'project:clip',mosaic:{projectId:'clip',sources:parents.map(a=>({jobId:a[i].id,sha256:a[i].sha256}))},
    mosaicOutput:{width:10,height:8,crs:'EPSG:32610',bounds:[0,0,300,240],pixelSize:[30,30],bandCount:1,...(i<3?{calibration:{product:'landsat-c2-l2'}}:{landsatQuality:{product:'landsat-c2-l2'}})}}));
  const clip=make([jobs]), all=[...jobs,...clip], derived=localRgbGroups(all).find(g=>g.derived);
  assert.deepEqual(landsatRgbQualityJobs(derived,all),clip.slice(3));
  const older=jobs.map((j,i)=>({...j,id:`old-${i}`,itemId:j.itemId.replace('20250628','20250612'),href:j.href.replaceAll('20250628','20250612').replaceAll('20250629','20250613')}));
  const multi=make([older,jobs]), inputs=[...older,...jobs,...multi], g=localRgbGroups(inputs).find(g=>g.derived);
  assert.deepEqual(landsatRgbQualityJobs(g,inputs),multi.slice(3));
  assert.deepEqual(landsatRgbQualityJobs(g,inputs.filter(j=>j.id!==older[3].id)),[]);
  assert.deepEqual(landsatRgbQualityJobs(g,inputs.map(j=>j.id===older[4].id?{...j,status:'failed'}:j)),[]);
});

test('saved coherent Landsat results reject reordered, mismatched and incomplete scene evidence without parents',()=>{
  const older=pins.map((p,i)=>({...p,jobId:`older-${i}`,itemId:p.itemId.replace('20250628','20250612'),href:p.href.replaceAll('20250628','20250612').replaceAll('20250629','20250613')}));
  const grid={width:10,height:8,crs:'EPSG:32610',bounds:[0,0,300,240],pixelSize:[30,30],pixelInterpretation:'PixelIsArea'};
  const project={id:'project',bounds:[-123,37,-122,38],geometry:null};
  const parents=pins.map((p,c)=>({...p,jobId:`parent-${c}`,kind:'raster_mosaic',itemId:'project:project',provenance:{project,
    sources:[older[c],p].map((s,i)=>({...s,acquiredAt:i?'2025-06-28T00:00:00Z':'2025-06-12T00:00:00Z'}))}}));
  const saved={rgbSpec:{grid,profile:{product:'landsat-c2-l2'},sources:parents.slice(0,3),qualityMask:{schemaVersion:'geod-landsat-rgb-mask/v2',
    definition:LANDSAT_QUALITY_DEFINITION,policy:'cloud_free_conservative',excludeSnow:true,sources:parents.slice(3),
    coupled:{selection:'newest complete qualified RGB scene wins; acquisition date then item ID break ties',geometry:null,scenes:[{sources:older,grid:{...grid,pixelInterpretation:'PixelIsPoint'}},{sources:pins,grid}]}}},
    rgbOutput:{channelValidPixels:[60,60,60],commonValidPixels:60,qualityMask:{examinedPixels:80,rejectedPixels:20,inputCommonValidPixels:70,removedValidPixels:10,coupled:{sceneValidPixels:[20,40],fallbackPixels:15}}}};
  assert.equal(validRgbQualityMask(saved),true);
  for(const change of [
    j=>j.rgbSpec.qualityMask.coupled.scenes.reverse(),
    j=>{j.rgbSpec.qualityMask.coupled.scenes[0].sources[3].sha256='0'.repeat(64);},
    j=>{j.rgbSpec.qualityMask.coupled.scenes[0].sources[2].itemId=item;},
    j=>{j.rgbSpec.qualityMask.coupled.scenes[0].grid.bounds[0]+=15;},
    j=>{j.rgbSpec.qualityMask.sources[1].provenance.sources.pop();},
    j=>{j.rgbSpec.qualityMask.schemaVersion='geod-landsat-rgb-mask/v1';},
    j=>{j.rgbOutput.qualityMask.coupled.sceneValidPixels[0]++;},
    j=>{j.rgbOutput.channelValidPixels[0]++;},
    j=>{delete j.rgbOutput.qualityMask.coupled;},
  ]){const invalid=structuredClone(saved);change(invalid);assert.equal(validRgbQualityMask(invalid),false);}
});
test('saved Landsat masks bind all five pins, chosen rules and counts after parents disappear',()=>{
  const saved={rgbSpec:{grid:{width:10,height:8},profile:{product:'landsat-c2-l2'},sources:pins.slice(0,3),qualityMask:{schemaVersion:'geod-landsat-rgb-mask/v1',definition:LANDSAT_QUALITY_DEFINITION,policy:'cloud_free',excludeSnow:false,sources:pins.slice(3)}},
    rgbOutput:{commonValidPixels:60,qualityMask:{examinedPixels:80,rejectedPixels:18,inputCommonValidPixels:70,removedValidPixels:10}}};
  assert.equal(validRgbQualityMask(saved),true);
  for(const mutation of [
    j=>{j.rgbSpec.qualityMask.policy='clear';},
    j=>{j.rgbSpec.qualityMask.sources[1].href=j.rgbSpec.qualityMask.sources[1].href.replaceAll('20250629','20250630');},
    j=>{j.rgbSpec.qualityMask.sources[0].sha256='bad';},
    j=>{j.rgbSpec.qualityMask.sources[0].jobId=pins[0].jobId;},
    j=>{j.rgbSpec.qualityMask.coupled={scenes:[]};},
    j=>{j.rgbOutput.qualityMask.removedValidPixels=19;},
    j=>{j.rgbOutput.qualityMask.examinedPixels=79;},
  ]) { const bad=structuredClone(saved);mutation(bad);assert.equal(validRgbQualityMask(bad),false); }
  assert.equal(rgbQualityPolicyLabel('cloud_free'),'Exclude cloud, shadow and RGB saturation');
  assert.equal(rgbQualityPolicyLabel('cloud_free_conservative'),'Conservative cloud-free flags');
});
