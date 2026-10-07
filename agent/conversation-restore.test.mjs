import test from 'node:test';
import assert from 'node:assert/strict';
import {restoreHistory,validReplay} from './conversation-restore.mjs';
test('restored context is bounded while retaining the original request and latest native references',()=>{
  const entries=[{type:'user',text:'Original New York request',status:'completed'},
    ...Array.from({length:100},(_,i)=>({type:'assistant',text:`Old record ${i} `+'x'.repeat(8000),status:'completed'})),
    {type:'tool',name:'geod_plan_status',references:[{kind:'plan',id:'01a0cce2-809d-7143-bd50-6301a6684723'}],summary:{kind:'plan',status:'pending'},status:'completed'}];
  const original=JSON.stringify(entries),context=restoreHistory(entries);
  assert(Buffer.byteLength(context)<50000);assert(context.includes('Original New York request'));
  assert(context.includes('01a0cce2-809d-7143-bd50-6301a6684723'));assert(context.includes('do not repeat past work'));
  assert.equal(JSON.stringify(entries),original);
});
test('replay metadata cannot introduce a file path, execution switch or malformed thread reference',()=>{
  const valid={pending:true,previousThreadId:null,fromToolSetId:'a'.repeat(64),updatedAt:'2026-10-06T00:00:00Z'};
  assert(validReplay(valid));
  for(const value of [{...valid,path:'external'},{...valid,executionMode:'full-access'},
    {...valid,previousThreadId:'../other-thread'},{...valid,updatedAt:'unknown'}])assert(!validReplay(value));
});
