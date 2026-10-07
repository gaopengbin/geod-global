import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,readFile,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash,randomUUID} from 'node:crypto';
import {AgentService,validContextState} from './service.mjs';
const config={label:'Controlled context test',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'controlled-test',apiKey:'private-test-marker'};
const definitions=[{name:'geod_download_plan',inputSchema:{type:'object'}}];
const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function rig(t,home=null) {
  if(!home){home=await mkdtemp(join(tmpdir(),'geod-context-'));t.after(()=>rm(home,{recursive:true,force:true}));}
  const calls=[],threadId=randomUUID();let emit;
  const service=new AgentService({home,executable:'unit-test-only',callTool:async()=>({planId:randomUUID(),kind:'download',status:'pending'}),
    bridgeFactory:async()=>({baseUrl:'http://127.0.0.1:1/v1',resetBudget(){},async close(){}}),
    hostFactory:async options=>{emit=options.onEvent;return {async rpc(method,params){calls.push({method,params});
      if(method==='thread/start')return {thread:{id:threadId}};
      if(method==='turn/start')return {turn:{id:randomUUID()}};
      if(method==='turn/interrupt')emit({method:'turn/completed',params:{threadId:service.active.session.threadId,turn:{status:'interrupted'}}});return {};},async close(){}};}});
  await service.open();await service.configure(config,definitions);
  return {service,home,calls,event(method,params={}){emit({method,params:{threadId:service.active?.session.threadId??threadId,...params}});}};
}
async function started(r,text='Read my workspace',images=[]) {await r.service.send({text,images});await r.service.turnTask;}
async function completed(r){r.event('turn/completed',{turn:{status:'completed'}});await tick();}
test('native context checkpoint preserves review entries and restores owned images after restart',async t=>{
  let r=await rig(t);
  const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jY1kAAAAASUVORK5CYII=','base64');
  const image={id:createHash('sha256').update(png).digest('hex'),name:'map.png',mimeType:'image/png',bytes:png.length,width:1,height:1};
  await mkdir(join(r.home,'images'));await writeFile(join(r.home,'images',`${image.id}.png`),png);await writeFile(join(r.home,'images',`${image.id}.json`),JSON.stringify({version:1,image}));
  await started(r,'Prepare a review', [image.id]);const sessionId=r.service.selectedId,threadId=r.service.active.session.threadId;
  await r.service.tool({threadId,turnId:r.service.active.turnId,tool:'geod_download_plan',arguments:{}});await completed(r);
  const before=r.service.snapshot().selected.entries;
  await r.service.compact({sessionId});await r.service.turnTask;
  assert(r.service.snapshot().busy);assert(r.calls.some(call=>call.method==='thread/compact/start' && call.params.threadId===threadId));
  await assert.rejects(r.service.tool({threadId,tool:'geod_download_plan',arguments:{}}));await assert.rejects(r.service.send({sessionId,text:'Concurrent message'}));await assert.rejects(r.service.configure(config,definitions));
  r.event('item/started',{item:{id:'compact-checkpoint',type:'contextCompaction'}});
  r.event('item/agentMessage/delta',{itemId:'summary',delta:'Private working summary should not appear in chat'});
  r.event('item/completed',{item:{id:'compact-checkpoint',type:'contextCompaction'}});r.event('item/completed',{item:{id:'compact-checkpoint',type:'contextCompaction'}});await completed(r);
  assert.deepEqual(r.service.snapshot().selected.entries,before);assert.equal(r.service.snapshot().selected.contextState.count,1);
  assert.equal(r.service.snapshot().selected.contextState.status,'ready');await r.service.close();
  const home=r.home;r=await rig(t,home);assert.equal(r.service.snapshot().selected.contextState.count,1);
  await r.service.send({sessionId,text:'Continue with the same image and pending review'});await r.service.turnTask;
  const input=r.calls.find(call=>call.method==='turn/start').params.input;
  assert(input.some(item=>item.type==='localImage' && item.path===join(home,'images',`${image.id}.png`)));
  assert.equal(r.service.snapshot().selected.imageContextVersion,1);assert(r.calls.some(call=>call.method==='thread/resume' && call.params.threadId===threadId));
  assert(!JSON.stringify(r.service.snapshot()).includes('Private working summary'));assert(!JSON.stringify(r.service.snapshot()).includes(config.apiKey));
  await completed(r);await r.service.close();
});
test('compaction interruption, missing checkpoint and incompatible connection retain original history',async t=>{
  const r=await rig(t);await started(r);await completed(r);const sessionId=r.service.selectedId,before=r.service.snapshot().selected.entries;
  await assert.rejects(r.service.compact({sessionId:randomUUID()}));
  await r.service.compact({sessionId});await r.service.turnTask;await r.service.interrupt();
  r.event('turn/started',{turn:{id:randomUUID()}});
  const deadline=Date.now()+1000;while(r.service.active && Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,5));assert(!r.service.active);
  assert.equal(r.service.snapshot().selected.contextState.status,'interrupted');assert.deepEqual(r.service.snapshot().selected.entries,before);
  await r.service.compact({sessionId});await r.service.turnTask;await completed(r);
  assert.equal(r.service.snapshot().selected.status,'failed');assert.equal(r.service.snapshot().selected.contextState.status,'failed');assert.deepEqual(r.service.snapshot().selected.entries,before);
  await r.service.configure({...config,model:'another-model'},definitions);await assert.rejects(r.service.compact({sessionId}));await r.service.close();
});
test('only bounded native token usage and context state survive persistence',async t=>{
  const r=await rig(t);await started(r);
  r.event('thread/tokenUsage/updated',{tokenUsage:{last:{totalTokens:20000},modelContextWindow:32768}});await tick();
  assert.equal(r.service.snapshot().selected.contextState.usedTokens,20000);
  r.event('thread/tokenUsage/updated',{tokenUsage:{last:{totalTokens:-1},modelContextWindow:32768}});
  assert.equal(r.service.snapshot().selected.contextState.usedTokens,20000);
  for(const value of [{...r.service.snapshot().selected.contextState,apiKey:'private'}, {...r.service.snapshot().selected.contextState,count:-1},
    {...r.service.snapshot().selected.contextState,status:'approved'}, {...r.service.snapshot().selected.contextState,windowTokens:0}])assert(!validContextState(value));
  await completed(r);await r.service.close();
  const path=join(r.home,'sessions.json'),data=JSON.parse(await readFile(path,'utf8'));data.sessions[0].contextState.count=-1;await writeFile(path,JSON.stringify(data));
  await assert.rejects(rig(t,r.home));assert.equal(JSON.parse(await readFile(path,'utf8')).sessions[0].contextState.count,-1);
});
