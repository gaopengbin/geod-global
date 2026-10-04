import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {viirsIdentity,viirsAssetIdentity,verifiedViirsScience,viirsPreparationScience} from './viirs.js';
import {compositePeriodLabel} from './composite-period.js';
import {normalizeScene,searchURL,nextPageURL} from './catalog.js';
import {projectRequest,jobsForProject,projectSourceJobs} from './projects-client.js';
import {verifiedMapMetadata,rasterProjectionDefinition} from './workspace-map-geometry.js';
import {localRgbGroups,verifiedCompositeMetadata} from './local-rgb.js';
import {projectCatalogScenes,projectExploreSearch} from './project-explore.js';
import {downloadableAssets} from './runtime-client.js';
import {isSupportedAsset,providerForAssets} from './providers.js';
const items=JSON.parse(readFileSync(new URL('../qa/viirs-catalog-items.json',import.meta.url),'utf8'));
test('native synthetic prepared bands bind their HDF5, map geometry, RGB and project membership',()=>{
  const fixture=JSON.parse(readFileSync(new URL('../qa/viirs-prepared-fixture.json',import.meta.url),'utf8'));
  assert.match(fixture.provenance,/Synthetic/);
  const {source,jobs,inspections,composite,project}=fixture;
  for(const [index,job] of jobs.entries()) {
    const s=viirsPreparationScience(job);assert.ok(s);
    assert.equal(verifiedMapMetadata(job,inspections[index]),inspections[index]);
    assert.match(rasterProjectionDefinition(inspections[index].crs),/\+proj=sinu/);
    assert.equal(projectSourceJobs(project,[source,...jobs],job.assetKey)[0].id,job.id);
    for(const change of [j=>j.parentId='wrong',j=>j.assetKey='visual',j=>j.viirsPrepare.sourceSha256='0'.repeat(64),j=>j.viirsPrepare.science.bounds[0]+=1,j=>j.viirsPrepare.science.qualityMaskApplied=true]) {
      const altered=structuredClone(job);change(altered);assert.equal(viirsPreparationScience(altered),null);
      assert.throws(()=>verifiedMapMetadata(altered,inspections[index]));
    }
    const shifted=structuredClone(inspections[index]);shifted.bounds[0]+=1;assert.throws(()=>verifiedMapMetadata(job,shifted));
  }
  assert.equal(jobsForProject(project,[source,...jobs]).length,4);
  const [rgb]=localRgbGroups(jobs);assert.equal(rgb.product,'viirs-09a1-v002');assert.equal(verifiedCompositeMetadata(rgb,composite),composite);
  const mixed=structuredClone(jobs);mixed[1].parentId=mixed[1].viirsPrepare.sourceJobId='11111111-1111-4111-8111-111111111111';
  assert.equal(localRgbGroups(mixed).length,0);
  const altered=structuredClone(composite);altered.composite.sources[0].sha256='0'.repeat(64);assert.throws(()=>verifiedCompositeMetadata(rgb,altered));
});
test('native synthetic summaries remain bound to product, checksum, original grid and band values',()=>{
  const evidence=JSON.parse(readFileSync(new URL('../qa/viirs-science-summary-fixtures.json',import.meta.url),'utf8'));
  assert.match(evidence.provenance,/Synthetic/);
  for(const s of evidence.summaries){
    const job={kind:'download',assetKey:'viirs',status:'succeeded',itemId:s.itemId,href:viirsIdentity(s.itemId).href,sha256:s.sourceSha256,viirsScience:s};
    assert.equal(verifiedViirsScience(job),s);
    assert.equal(verifiedViirsScience({...job,sha256:'0'.repeat(64)}),null);
    assert.equal(verifiedViirsScience({...job,status:'failed'}),null);
    for(const change of [x=>x.bands[0].scale=0.001,x=>x.bands.reverse(),x=>x.bounds[0]+=1,x=>x.qualityMaskApplied=true,x=>x.bands[0].sampleCount=10,x=>x.bands[0].minimum=-40000]){
      const altered=structuredClone(s);change(altered);
      assert.equal(verifiedViirsScience({...job,viirsScience:altered}),null);
    }
  }
});
test('real VIIRS catalogue binds each platform, full HDF5 and eight-day period across project restore',()=>{
  for(const item of items){const identity=viirsIdentity(item.id),scene=normalizeScene(item,identity.provider);
    assert.equal(scene.cloud,null);assert.equal(scene.gsd,1000);assert.equal(scene.crs,null);
    assert.equal(scene.endDate,'2025-07-03T23:59:59Z');
    assert.equal(scene.assets.viirs.type,'application/x-hdf5');assert.equal(scene.thumbnail,identity.browse);
    assert.deepEqual(downloadableAssets(scene).map(a=>a.key),['viirs']);
    assert.equal(providerForAssets(scene.assets),identity.provider);
    const project=projectRequest({name:'VIIRS',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]});
    const restored=projectCatalogScenes(project)[0];assert.equal(restored.provider,identity.provider);assert.equal(restored.endDate,scene.endDate);assert.equal(restored.thumbnail,scene.thumbnail);
    assert.equal(projectExploreSearch(project,{}).provider,identity.provider);
    assert.equal(compositePeriodLabel(restored,v=>v.slice(0,10)), '2025-06-26 – 2025-07-03');
    const url=new URL(searchURL({provider:identity.provider,bbox:project.bounds,start:'2025-06-01',end:'2025-06-30',cloud:'invalid',limit:20}));
    assert.equal(url.searchParams.get('query'),null);assert.equal(url.searchParams.get('sortby'),'-properties.datetime');
    assert.throws(()=>nextPageURL({links:[{rel:'next',href:url.href.replace(identity.collection,'HLSL30_2.0')}]},identity.provider));
    for(const key of ['red','green','blue','product','srtm','thumbnail'])assert.equal(isSupportedAsset(identity.href,key),false);
    for(const bad of [identity.href+'?token=secret',identity.href.replace('.002/','.001/'),identity.href.replace('.h5','.tif'),identity.href.replace('nasa.gov','nasa.gov.evil.test')])assert.equal(viirsAssetIdentity(bad),null);
    for(const mutate of [i=>{i.collection='HLSL30_2.0'},i=>{i.properties.end_datetime='2025-07-04T23:59:59Z'},i=>{i.assets[identity.production].href=identity.browse},i=>{i.assets.browse.href=identity.browse.replace('.1.jpg','.2.jpg')}]){const bad=structuredClone(item);mutate(bad);assert.throws(()=>normalizeScene(bad,identity.provider));}
  }
  const id=items[0].id;
  for(const bad of [id.replace('2025177','2025178'),id.replace('h08','h36'),id.replace('v05','v18'),id.replace('.002.','.001.')])assert.equal(viirsIdentity(bad),null);
  assert.equal(viirsIdentity('VNP09A1.A2025361.h08v05.002.2026001224010').endDate,'2025-12-31T23:59:59Z');
});
