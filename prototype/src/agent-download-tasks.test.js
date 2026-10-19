import test from 'node:test';
import assert from 'node:assert/strict';
import {downloadWork,downloadTaskSummary} from './agent-download-tasks.js';

const plan=(id,jobs=[],status='submitted')=>({planId:id,kind:'download',status,files:[{itemId:'scene',assetKey:'visual'}],jobs});
test('one work group collects submitted download references across tool batches and leaves pending approvals and processing in chronology',()=>{
  const entries=[{id:'tool',type:'tool',references:[{id:'a',kind:'plan'}]},
    {id:'review1',type:'reviews',references:[{id:'a',kind:'plan'},{id:'pending',kind:'plan'}]},
    {id:'note',type:'assistant',text:'Continue'},
    {id:'review2',type:'reviews',references:[{id:'a',kind:'plan'},{id:'b',kind:'plan'},{id:'mosaic',kind:'plan'}]}];
  const before=structuredClone(entries),plans=[plan('a'),plan('b'),plan('pending',[],'pending'),{...plan('mosaic'),kind:'mosaic'}];
  const projected=downloadWork(entries,plans);
  assert.deepEqual(projected.plans.map(p=>p.planId),['a','b']);
  assert.deepEqual(projected.entries.map(e=>e.id),['tool','review1','note','review2']);
  assert.deepEqual(projected.entries[1].references,[{id:'pending',kind:'plan'}]);
  assert.deepEqual(projected.entries[3].references,[{id:'mosaic',kind:'plan'}]);
  assert.deepEqual(entries,before);
});
test('shared native job receipts count only once and partial byte totals never masquerade as a full total',()=>{
  const shared={id:'same',status:'running',bytesDownloaded:100,totalBytes:200};
  let summary=downloadTaskSummary([plan('a',[shared]),plan('b',[shared])]);
  assert.equal(summary.total,1);assert.equal(summary.transferred,100);assert.equal(summary.totalBytes,200);
  summary=downloadTaskSummary([plan('a',[shared]),plan('b',[{id:'unknown',status:'queued',bytesDownloaded:0,totalBytes:null}])]);
  assert.equal(summary.total,2);assert.equal(summary.totalBytes,null);assert.equal(summary.progressBytes,null);
});
test('receiving all bytes does not finish a download before native settlement and failed or stopped jobs remain actionable',()=>{
  const job={id:'file',status:'succeeded',settled:false,bytesDownloaded:200,totalBytes:200};
  assert.equal(downloadTaskSummary([plan('a',[job])]).status,'Finalizing output');
  for(const status of ['failed','interrupted']){
    const summary=downloadTaskSummary([plan('a',[{...job,status,settled:true}])]);
    assert.equal(summary.needsAttention,true);assert.equal(summary.complete,false);assert.equal(summary.failed[0].id,'file');
  }
  const cancelled=downloadTaskSummary([plan('a',[{...job,status:'cancelled',settled:true}])]);
  assert.equal(cancelled.needsAttention,false);assert.equal(cancelled.complete,false);assert.equal(cancelled.status,'Tasks stopped');
  const summary=downloadTaskSummary([plan('a',[{...job,settled:true}])]);
  assert.equal(summary.complete,true);assert.equal(summary.ready,1);
});
test('a submitted plan without native job receipts stays incomplete and contributes its expected files',()=>{
  const summary=downloadTaskSummary([plan('a'),plan('b',[{id:'ready',status:'succeeded',settled:true,bytesDownloaded:20,totalBytes:20}])]);
  assert.equal(summary.total,2);assert.equal(summary.ready,1);assert.equal(summary.complete,false);assert.equal(summary.totalBytes,null);
});
