import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {failureDiagnostics,diagnosticError,diagnosticStep} from './usage-diagnostics.js';
import {createUsageClient} from './usage-client.js';
import {beginUsage} from './usage-events.js';
import {loadPreviewRange} from './cog-preview-source.js';

test('raw credentials, paths, coordinates and messages never enter diagnostic fields',()=>{
 const raw=new Error('HTTP 403 https://secret.test/?sig=PRIVATE C:\\private\\report.tif 116.341,40.121 prompt=private');
 const fields=failureDiagnostics(diagnosticError(raw,{stage:'authorization',httpStatus:'403',retries:2,diagnosticOrigin:'explicit'}));
 assert.deepEqual(fields,{stage:'authorization',reason:'authorization',httpStatus:'403',retries:'2',diagnosticOrigin:'explicit'});
 assert(!JSON.stringify(fields).includes('PRIVATE'));
 assert.equal(failureDiagnostics(null,'agent_turn').diagnosticOrigin,'unavailable');
});

test('real HTTP preview rejection retains phase, 503 and exhausted retries',async()=>{
 const server=createServer((_req,res)=>{res.writeHead(503);res.end('private upstream message');});
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 try{await assert.rejects(diagnosticStep('preview_metadata',()=>loadPreviewRange(`http://127.0.0.1:${server.address().port}`,{Range:'bytes=0-3'})),error=>{
  assert.deepEqual(failureDiagnostics(error),{stage:'preview_metadata',reason:'upstream_unavailable',httpStatus:'503',retries:'2',diagnosticOrigin:'explicit'});return true;
 });}finally{server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
});

test('search, queue, inspect, save and Agent failure events carry their operation IDs and phase',()=>{
 for(const action of ['search','download_submit','inspect','save_project','agent_send']){
  const client=createUsageClient({storage:null,endpoint:null});client.setPreferences({enabled:true});
  const op=beginUsage(action,{},client);op.finish(null,diagnosticError(new Error('HTTP 429'),{stage:action==='agent_send'?'agent_dispatch':'service_request',httpStatus:'429',diagnosticOrigin:'explicit'}));
  const event=client.report().events.find(e=>e.name.endsWith('_failed'));
  assert(event);assert.equal(event.operationId,op.operationId);assert.equal(event.fields.reason,'rate_limited');assert.equal(event.fields.httpStatus,'429');
 }
});

test('background downloads distinguish checksum validation from transfer failure without uploading raw error text',()=>{
 const client=createUsageClient({storage:null,endpoint:null});client.setPreferences({enabled:true});
 const id=crypto.randomUUID();client.watchJobs([{id,status:'running'}],{source:'earth-search'});
 client.observeJobs([{id,status:'failed',error:'SHA-256 mismatch C:/PRIVATE/file.tif'}]);
 const event=client.report().events.find(e=>e.name==='download_failed');
 assert.equal(event.fields.stage,'artifact_validation');assert.equal(event.fields.reason,'checksum_mismatch');
 assert.equal(event.fields.diagnosticOrigin,'classified');assert(!JSON.stringify(event).includes('PRIVATE'));
});
