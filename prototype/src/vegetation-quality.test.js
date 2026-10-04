import test from 'node:test';
import assert from 'node:assert/strict';
import { MODIS_SCIENCE } from './modis-science-layers.js';
import { VI_SELECTION_KEYS, VI_SELECTION_SCHEMA, VI_SELECTION_RULE, supportsViSelection, validViSelectionJob, viSelectionMatches } from './vegetation-quality.js';
// Simulated protocol records. Actual TIFF quality masks are checked separately
// against GDAL/NumPy; these records are never scientific/download evidence.
function fixture() {
  const itemId='MOD13Q1.A2025177.h08v05.061.2025195142416';
  const projectId='12345678-1234-4234-8234-123456789abc';
  const sources=VI_SELECTION_KEYS.map((key,k)=>({jobId:`12345678-1234-4234-8234-00000000000${k}`,sha256:'a'.repeat(64),bytes:1000,attribution:'NASA MODIS',href:`https://modiseuwest.blob.core.windows.net/modis-061-cogs/MOD13Q1/08/05/2025177/${itemId}_${MODIS_SCIENCE[key]?.asset||`250m_16_days_${key.toUpperCase()}`}.tif`}));
  const result={schemaVersion:VI_SELECTION_SCHEMA,policy:'good',specSha256:'a'.repeat(64),countsFullResolution:true,inputCommonValidPixels:4,removedValidPixels:1,rejectedPixels:1,fallbackPixels:0,sceneValidPixels:[3],indicesSha256:['b'.repeat(64),'c'.repeat(64)],selectionSha256:'d'.repeat(64)};
  const job={kind:'raster_mosaic',assetKey:'ndvi',itemId:`project:${projectId}`,mosaic:{projectId,assetKey:'ndvi',sources:[{jobId:sources[0].jobId,sha256:sources[0].sha256}],viSelection:{schemaVersion:VI_SELECTION_SCHEMA,product:'modis-13q1-v061',policy:'good',selection:VI_SELECTION_RULE,bounds:[-122.5,37.6,-122.3,37.8],scenes:[{itemId,compositeStart:'2025-06-26T00:00:00Z',sources}]}},mosaicOutput:{width:2,height:2,sourceCount:1,coveredPixels:3,maskedPixels:0,viIndex:'ndvi',overlapPolicy:VI_SELECTION_RULE,viQuality:result}};
  return {job,data:{vegetation:{qualitySelection:structuredClone(result)}}};
}
test('quality processing requires both indices and both exact scene quality assets',()=>{
  const f=fixture(),scene={itemId:f.job.mosaic.viSelection.scenes[0].itemId,assets:Object.fromEntries(VI_SELECTION_KEYS.map(k=>[k,{}]))};
  assert(supportsViSelection({scenes:[scene]}));
  delete scene.assets.vi_reliability;assert.equal(supportsViSelection({scenes:[scene]}),false);
});
test('a paired selection binds source identity, count semantics and displayed result',()=>{
  const {job,data}=fixture();assert(validViSelectionJob(job));assert(viSelectionMatches(job,data));
  const unmasked={kind:'raster_mosaic'};assert(validViSelectionJob(unmasked));assert(viSelectionMatches(unmasked,{vegetation:{}}));
  assert.equal(viSelectionMatches(unmasked,data),false);
});
test('reject forged source groups, adverse contract changes and sampled counts',()=>{
  const changes=[
    j=>j.mosaic.viSelection.scenes[0].sources[2].href=j.mosaic.viSelection.scenes[0].sources[0].href,
    j=>j.mosaic.viSelection.scenes[0].sources[3].jobId=j.mosaic.viSelection.scenes[0].sources[2].jobId,
    j=>j.mosaic.viSelection.scenes[0].sources[1].sha256='bad',
    j=>j.mosaic.viSelection.scenes[0].compositeStart='2025-06-27T00:00:00Z',
    j=>j.mosaic.viSelection.selection='independent-layer-mosaic',
    j=>j.mosaic.viSelection.product='modis-09a1-v061',
    j=>j.mosaic.sources[0].jobId=j.mosaic.viSelection.scenes[0].sources[1].jobId,
    j=>j.mosaicOutput.viIndex='evi',
    j=>j.mosaicOutput.viQuality.countsFullResolution=false,
    j=>j.mosaicOutput.viQuality.removedValidPixels=0,
    j=>j.mosaicOutput.viQuality.sceneValidPixels=[2],
    j=>j.mosaicOutput.viQuality.rejectedPixels=0,
    j=>j.mosaicOutput.viQuality.fallbackPixels=4,
    j=>j.mosaicOutput.viQuality.policy='usable',
  ];
  for(const change of changes) {const {job}=fixture();change(job);assert.equal(validViSelectionJob(job),false);}
});
test('refuse mismatching selection statistics, digests, missing rules and extra fields',()=>{
  const {job,data}=fixture();
  for(const [key,value] of [['fallbackPixels',1],['selectionSha256','e'.repeat(64)],['newField',1]]) {
    const bad=structuredClone(data);bad.vegetation.qualitySelection[key]=value;assert.equal(viSelectionMatches(job,bad),false);
  }
  assert.equal(viSelectionMatches(job,{vegetation:{}}),false);
});
