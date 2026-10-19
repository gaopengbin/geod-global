import test from 'node:test';
import assert from 'node:assert/strict';
import {requestTokenUsage,transcriptTokenUsage,validUsageLedger} from './agent-usage.js';
const requestId='a1234567-1234-1234-1234-123456789abc',other='b1234567-1234-1234-1234-123456789abc';
const record=(id,request=requestId,extra={})=>({id:'resp_'+id.repeat(32),requestId:request,turnId:'turn',origin:'human',state:'completed',
  usage:{input_tokens:100,output_tokens:20,total_tokens:120,input_tokens_details:{cached_tokens:80,cache_write_tokens:null},output_tokens_details:{reasoning_tokens:10}},context:{toolCount:3},...extra});
const ledger=records=>({version:1,requests:records.length,records,totals:{input:0,output:0,total:0,cachedInput:0,reasoningOutput:0}});
test('request accounting sums continuations and compaction once, without adding subset tokens or later requests',()=>{
  const a=record('a'),data=ledger([a,a,record('b',requestId,{origin:'workflow'}),record('c',requestId,{origin:'compaction'}),record('d',other)]);
  const usage=requestTokenUsage(data,requestId);
  assert.equal(usage.requests,3);assert.equal(usage.total,360);assert.equal(usage.cachedInput,240);assert.equal(usage.reasoningOutput,30);assert.equal(usage.status,'complete');
  assert.equal(usage.workflowRequests,1);assert.equal(usage.compactionRequests,1);
});
test('failed, missing, invalid and truncated receipts remain honest partial values; later intact rounds remain complete',()=>{
  const unknown=record('b',requestId,{state:'failed',usage:{input_tokens:null,output_tokens:null,total_tokens:null,input_tokens_details:{cached_tokens:null,cache_write_tokens:null},output_tokens_details:{reasoning_tokens:null}}});
  const usage=requestTokenUsage(ledger([record('a'),unknown]),requestId);
  assert.equal(usage.total,120);assert.equal(usage.status,'partial');assert.equal(usage.failedRequests,1);
  assert.equal(requestTokenUsage(ledger([]),requestId).status,'unavailable');
  assert.equal(requestTokenUsage(ledger([]),requestId,[],new Set(),true).status,'pending');
  const truncated={...ledger([record('a'),record('c',other)]),requests:200};
  assert.equal(requestTokenUsage(truncated,requestId).status,'partial');assert.equal(requestTokenUsage(truncated,other).status,'complete');
  assert(validUsageLedger(ledger([record('a'),unknown])));assert(!validUsageLedger(ledger([record('a',requestId,{context:{toolCount:-1}})])));
  const invalid=record('e');invalid.usage.input_tokens_details.cached_tokens=101;
  assert.equal(requestTokenUsage(ledger([invalid]),requestId).cachedInput,null);
});
test('one footer per human request; reused legacy turn ids cannot leak another request usage',()=>{
  const rows=[{id:requestId,type:'user'},{id:'w1',type:'work',turnIds:['turn','continuation']},{id:'final',type:'assistant',turnId:'continuation'},
    {id:other,type:'user'},{id:'w2',type:'work',turnIds:['turn']}];
  const result=transcriptTokenUsage(rows,ledger([record('a'),record('b',other)]),other);
  assert.equal(result.size,2);assert(!result.has('w1'));assert.equal(result.get('final').total,120);assert.equal(result.get('w2').status,'partial');
  assert.equal(transcriptTokenUsage(rows,ledger([record('c',null)]),'none').get('final').status,'unavailable');
});
