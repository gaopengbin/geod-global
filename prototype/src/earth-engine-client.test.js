import test from 'node:test';
import assert from 'node:assert/strict';
import { earthEngineProject, earthEngineScript, landsatEarthEngineId, earthEngineRequest } from './earth-engine-client.js';
const link='https://earthengine.googleapis.com/v1/projects/example-project/thumbnails/0123456789abcdef-0123456789abcdef:getPixels';
test('GEE links are restricted and never accept external hosts or embedded credentials',()=>{
  assert.equal(earthEngineProject(link),'example-project');
  for(const bad of [link+'?token=secret',link+'#token',link.replace('https:','http:'),link.replace('googleapis.com','googleapis.com.evil.test'),link.replace('earthengine.googleapis.com','user:pass@earthengine.googleapis.com'),link.replace('thumbnails','assets')]) assert.equal(earthEngineProject(bad),null);
});
test('Landsat export script preserves image identity and tags compute without pinning our project',()=>{
  const imageId=landsatEarthEngineId('LC09_L2SP_123032_20260929_20260930_02_T1');
  assert.equal(imageId,'LANDSAT/LC09/C02/T1_L2/LC09_123032_20260929');
  assert.equal(landsatEarthEngineId('S2A_50SMJ_20261007_0_L2A'),'');
  const script=earthEngineScript({imageId,band:'SR_B4',bounds:[116.25,39.8,116.5,40],tag:'geod-export-test'});
  assert.match(script,/setWorkloadTag\("geod-export-test"\)/);
  assert.match(script,/LANDSAT_PRODUCT_ID/);
  assert.doesNotMatch(script,/geod-global-dev|setCloudApiUserProject/);
  assert.throws(()=>earthEngineScript({imageId,band:'SR_B4',bounds:[-180,-80,180,80],tag:'geod-test'}),/too large/);
  assert.throws(()=>earthEngineScript({imageId:'invalid',band:'SR_B4',bounds:[116,39,116.01,39.01],tag:'geod-test'}));
});
test('unknown EECU remains null and invalid zero usage is rejected',async()=>{
  const original=globalThis.fetch;
  const record={id:'01234567-1234-1234-1234-0123456789ab',name:'Test',projectId:'example-project',sha256:'a'.repeat(64),bytes:1024,downloadSeconds:1,createdAt:'2026-10-11T10:00:00Z',eecuSeconds:null,usageStatus:'not_measured'};
  try {
    globalThis.fetch=async()=>({ok:true,json:async()=>[record]});
    assert.equal((await earthEngineRequest('list'))[0].eecuSeconds,null);
    globalThis.fetch=async()=>({ok:true,json:async()=>[{...record,eecuSeconds:0}]});
    await assert.rejects(earthEngineRequest('list'),/Invalid/);
  } finally {globalThis.fetch=original;}
});
