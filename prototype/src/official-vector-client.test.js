import test from 'node:test';import assert from 'node:assert/strict';
import assets from './official-vector-native.fixture.json' with {type:'json'};
import {validateOfficialVectorPackage,validateOfficialVectorProvenance,officialVectorRequest} from './official-vector-client.js';
import {validateVectorAsset,validateVectorInspection} from './vector-client.js';
import overture from './overture-native.fixture.json' with {type:'json'};
test('actual Microsoft and Monaco native receipts bind the complete original format and publisher identity',()=>{
  for(const asset of assets){assert.equal(validateVectorAsset(asset),asset);assert.equal(validateOfficialVectorProvenance(asset.officialPackage,asset),asset.officialPackage);for(const key of ['url','licenseUrl','release','sourceIndexUrl','attribution']){const changed=structuredClone(asset);changed.officialPackage.package[key]+='changed';assert.throws(()=>validateVectorAsset(changed));}assert.throws(()=>validateVectorAsset({...asset,sourceSha256:'a'.repeat(64)}));assert.throws(()=>validateOfficialVectorPackage({...asset.officialPackage.package,extra:true}));}
});
test('official partition area remains a package selection and cannot become an unrelated query or another source format',()=>{
  const asset=assets[0];assert.throws(()=>validateOfficialVectorProvenance({...asset.officialPackage,requestedBounds:[10,10,11,11]},asset));assert.throws(()=>validateVectorAsset({...asset,format:'geojson'}));assert.throws(()=>validateVectorAsset({...asset,bytes:20*1024*1024+1}));assert.throws(()=>validateVectorAsset({...asset,geoParquet:overture.asset.geoParquet}));
});
test('shared vector validator preserves Overture original bytes separately from its smaller area preview',()=>{
  assert.equal(validateVectorInspection(overture,overture.asset.id),overture);assert.notEqual(overture.asset.bytes,overture.asset.geoParquet.originalBytes);assert.notEqual(overture.asset.sourceSha256,overture.asset.geoParquet.originalSha256);
  const changed=structuredClone(overture);changed.asset.geoParquet.originalBytes=changed.asset.bytes;assert.throws(()=>validateVectorInspection(changed,changed.asset.id));
  const row=structuredClone(overture);row.geojson.features[0].geodOriginalRow=407277;assert.throws(()=>validateVectorInspection(row,row.asset.id));
});
test('native official search honors cancellation and cannot return another area',async()=>{
  const asset=assets[0],request={datasetId:'ms-buildings',bounds:asset.officialPackage.requestedBounds},response={datasetId:request.datasetId,bounds:request.bounds,items:[asset.officialPackage.package],complete:true,reviewedIndexOnly:true,liveAvailabilityChecked:false};
  globalThis.window={__TAURI__:{core:{invoke:async(command,args)=>{assert.equal(command,'search_official_vector_packages');assert.deepEqual(args,{request});return {...response,bounds:[10,10,11,11]};}}}};
  try{await assert.rejects(officialVectorRequest('search',request),/receipt changed/);const abort=new AbortController();abort.abort();await assert.rejects(officialVectorRequest('search',request,abort.signal),{name:'AbortError'});}finally{delete globalThis.window;}
});
