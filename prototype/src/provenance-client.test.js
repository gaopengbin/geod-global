import test from 'node:test';
import assert from 'node:assert/strict';
import {validateProvenanceFile,prepareRasterProvenance,exportRasterProvenance} from './provenance-client.js';
const id='00000000-0000-4000-8000-000000000001';
const file={jobId:id,filename:`${id}.tif.provenance.json`,bytes:1200,sha256:'a'.repeat(64),sourceCount:2,capturedStacItems:1,missingStacItems:1};
test('provenance metadata binds its job, size, checksum and complete/partial source counts',()=>{
  assert.equal(validateProvenanceFile(file,id),file);
  for(const changed of [{jobId:'other'},{filename:'../../outside.json'},{bytes:20*1024*1024},{sha256:'wrong'},{capturedStacItems:2},{sourceCount:-1}])assert.throws(()=>validateProvenanceFile({...file,...changed},id));
});
test('native prepare uses only the selected job and a cancelled dialog never reports a saved pair',async()=>{
  const previous=globalThis.window,calls=[];
  globalThis.window={__TAURI__:{core:{invoke:async(command,args)=>{calls.push({command,args});return command==='prepare_raster_provenance'?file:false;}}}};
  try{assert.equal(await prepareRasterProvenance(id),file);assert.equal(await exportRasterProvenance(id),false);assert.deepEqual(calls,[{command:'prepare_raster_provenance',args:{id}},{command:'export_raster_provenance',args:{id}}]);}
  finally{if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
test('browser prepare uses the protected local metadata endpoint; native file save has no HTTP fallback',async()=>{
  const beforeWindow=globalThis.window,beforeFetch=globalThis.fetch;globalThis.window={};let calls=0;
  globalThis.fetch=async(url,options)=>{calls++;assert.equal(url,`http://127.0.0.1:4318/jobs/${id}/provenance`);assert.equal(options.method,'POST');assert.equal(options.headers['X-GeoD-Client'],'geod-global');return {ok:true,json:async()=>file};};
  try{assert.equal(await prepareRasterProvenance(id),file);await assert.rejects(exportRasterProvenance(id),/desktop/);assert.equal(calls,1);await assert.rejects(prepareRasterProvenance('../outside'),/identifier/);assert.equal(calls,1);}
  finally{globalThis.fetch=beforeFetch;if(beforeWindow===undefined)delete globalThis.window;else globalThis.window=beforeWindow;}
});
