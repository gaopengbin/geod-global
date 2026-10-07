import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash,randomUUID} from 'node:crypto';
import {mkdtemp,mkdir,readFile,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {readDocument,documentInput,sessionDocuments,restoreDocumentContext} from './document-store.mjs';
import {AgentService} from './service.mjs';
import {startBridge,BRIDGE_MODEL} from './protocol.mjs';
const text='\ufeff名称,经度,纬度\r\n北京,116.4,39.9 🌍\r\n';
const document={id:createHash('sha256').update(text).digest('hex'),name:'坐标.csv',mimeType:'text/plain',bytes:Buffer.byteLength(text),characters:[...text].length};
async function stored(t){
  const home=await mkdtemp(join(tmpdir(),'geod-document-'));t.after(()=>rm(home,{recursive:true,force:true}));
  await mkdir(join(home,'documents'));await writeFile(join(home,'documents',`${document.id}.txt`),text);
  await writeFile(join(home,'documents',`${document.id}.json`),JSON.stringify({version:1,document}));return home;
}
const config={label:'Controlled text fixture',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'controlled-text',apiKey:'private-test-marker'};
const definitions=[{name:'geod_projects_list',inputSchema:{type:'object'}}];
const tick=()=>new Promise(resolve=>setImmediate(resolve));
test('document references preserve UTF-8 and reject paths, forged metadata, assistant attachments and changed content',async t=>{
  const home=await stored(t);assert.deepEqual(await readDocument(home,document.id),{document,text});
  await assert.rejects(readDocument(home,'../outside'));
  assert.throws(()=>sessionDocuments([{type:'assistant',documents:[document]}]));
  assert.throws(()=>sessionDocuments([{type:'user',images:[{},{},{}],documents:[document]}]));
  await writeFile(join(home,'documents',`${document.id}.json`),JSON.stringify({version:1,document,path:'outside'}));await assert.rejects(readDocument(home,document.id));
  await writeFile(join(home,'documents',`${document.id}.json`),JSON.stringify({version:1,document}));
  await writeFile(join(home,'documents',`${document.id}.txt`),Buffer.alloc(document.bytes,120));await assert.rejects(readDocument(home,document.id));
});
test('document-only turns save references, resume the same thread, restore after context organization and preserve history on corruption',async t=>{
  const home=await stored(t),calls=[],threadId=randomUUID();
  const rig=async()=>{
    const service=await new AgentService({home,executable:'unit-test-only',callTool:async()=>({}),
      bridgeFactory:async()=>({baseUrl:'http://127.0.0.1:1/v1',resetBudget(){},async close(){}}),
      hostFactory:async()=>({async rpc(method,params){calls.push({method,params});if(method==='thread/start')return {thread:{id:threadId}};if(method==='turn/start')return {turn:{id:randomUUID()}};return {};},async close(){}})}).open();
    await service.configure(config,definitions);return service;
  };
  let service=await rig();await service.send({text:'',documents:[document.id]});await service.turnTask;
  assert.equal(calls.find(c=>c.method==='turn/start').params.input[0].text,documentInput({document,text}));
  const sessionId=service.selectedId;
  service.event({method:'item/completed',params:{threadId,item:{type:'agentMessage',id:randomUUID(),text:'Controlled text result'}}});
  service.event({method:'turn/completed',params:{threadId,turn:{status:'completed'}}});await tick();await service.close();
  service=await rig();assert.equal(service.snapshot().selected.threadId,threadId);
  service.sessions[0].contextState={status:'completed',count:1,lastCompletedAt:new Date().toISOString(),usedTokens:null,windowTokens:null};
  await service.send({sessionId,text:'Continue after controlled checkpoint'});await service.turnTask;
  assert(calls.some(c=>c.method==='thread/resume' && c.params.threadId===threadId));
  assert(calls.filter(c=>c.method==='turn/start').at(-1).params.input.some(part=>part.text===documentInput({document,text})));
  service.event({method:'item/completed',params:{threadId,item:{type:'agentMessage',id:randomUUID(),text:'Controlled continuation'}}});
  service.event({method:'turn/completed',params:{threadId,turn:{status:'completed'}}});await tick();
  assert.deepEqual((await service.attachmentReferences()).documents,[document.id]);
  const before=structuredClone(service.sessions);await writeFile(join(home,'documents',`${document.id}.txt`),Buffer.alloc(document.bytes,120));
  await assert.rejects(service.send({sessionId,text:'Changed file must not append'}));assert.deepEqual(service.sessions,before);await service.close();
  const saved=JSON.parse(await readFile(join(home,'sessions.json'),'utf8'));assert(!JSON.stringify(saved).includes(text));assert(!JSON.stringify(saved).includes(home));
});
test('mid-turn document recovery keeps tool pairs intact, avoids duplicates and never restores outside the active scope',async t=>{
  const home=await stored(t),sessionId=randomUUID(),seen=[];
  const messages=[{role:'user',content:'A compacted working summary'},
    {role:'assistant',content:[{type:'tool-call',toolCallId:'read',toolName:'geod_projects_list',input:{}}]},
    {role:'tool',content:[{type:'tool-result',toolCallId:'read',toolName:'geod_projects_list',output:{type:'json',value:{total:0}}}]}];
  const recovered=await restoreDocumentContext(messages,[document],home);assert.deepEqual(recovered.slice(1),messages.slice(1));
  assert.equal(recovered[0].content[1].text,documentInput({document,text}));assert.deepEqual(await restoreDocumentContext(recovered,[document],home),recovered);
  const bridge=await startBridge({config,definitions,home,token:'owned-token',sessionId,documentContext:id=>id===sessionId?[document]:[],stream:options=>{
    seen.push(options);return {fullStream:(async function*(){yield {type:'text-delta',text:'Controlled reply'};})(),finishReason:Promise.resolve('stop'),usage:Promise.resolve({})};
  }});t.after(()=>bridge.close());
  const request=async()=>{const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-token'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Summary'}]})});await response.text();return response.status;};
  assert.equal(await request(),200);assert(seen[0].messages[0].content.some(part=>part.text===documentInput({document,text})));
  await bridge.resetBudget(randomUUID());assert.equal(await request(),200);assert.equal(seen[1].messages[0].content,'Summary');
  await bridge.resetBudget(sessionId);await writeFile(join(home,'documents',`${document.id}.txt`),Buffer.alloc(document.bytes,120));assert.equal(await request(),400);assert.equal(seen.length,2);
});
