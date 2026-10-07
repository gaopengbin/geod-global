import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash,randomUUID } from 'node:crypto';
import { mkdtemp,mkdir,readFile,rm,writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { AgentService } from './service.mjs';
import { readImage,sameImage,sessionImages } from './image-store.mjs';
import { translateResponses,startBridge,BRIDGE_MODEL,LIMITS } from './protocol.mjs';
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jY1kAAAAASUVORK5CYII=','base64');
const image={id:createHash('sha256').update(png).digest('hex'),name:'map.png',mimeType:'image/png',bytes:png.length,width:1,height:1};
async function stored(t) {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-image-'));t.after(()=>rm(home,{recursive:true,force:true}));
  await mkdir(join(home,'images'));await writeFile(join(home,'images',`${image.id}.png`),png);
  await writeFile(join(home,'images',`${image.id}.json`),JSON.stringify({version:1,image}));return home;
}
test('managed image references detect changed files and reject external paths or metadata',async t=>{
  const home=await stored(t);assert.deepEqual((await readImage(home,image.id)).image,image);
  await assert.rejects(readImage(home,'../../outside'));
  await writeFile(join(home,'images',`${image.id}.json`),JSON.stringify({version:1,image:{...image,path:'C:/private'}}));
  await assert.rejects(readImage(home,image.id));
  await writeFile(join(home,'images',`${image.id}.json`),JSON.stringify({version:1,image}));
  await writeFile(join(home,'images',`${image.id}.png`),Buffer.alloc(png.length));await assert.rejects(readImage(home,image.id));
});
test('image identity tolerates native JSON key ordering but rejects changed metadata',()=>{
  const reordered=Object.fromEntries(Object.entries(image).reverse());
  assert(sameImage(image,reordered));
  assert.deepEqual(sessionImages([{type:'user',images:[image]},{type:'user',images:[reordered]}]),[reordered]);
  for(const changed of [{...reordered,name:'different.png'},{...reordered,width:2},{...reordered,bytes:png.length+1},{...reordered,path:'outside'}]){
    assert(!sameImage(image,changed));
    assert.throws(()=>sessionImages([{type:'user',images:[image]},{type:'user',images:[changed]}]));
  }
});
test('image storage protects history from every connection and refuses active responses without model calls',async t=>{
  const home=await stored(t), other={...image,id:'b'.repeat(64)}, selectedId=randomUUID();
  const sessions=[{id:selectedId,connectionId:'old-connection',status:'completed',entries:[{id:randomUUID(),type:'user',status:'completed',images:[image]}]},
    {id:randomUUID(),connectionId:'different-connection',status:'completed',entries:[{id:randomUUID(),type:'user',status:'completed',images:[other]}]}];
  await writeFile(join(home,'sessions.json'),JSON.stringify({version:1,selectedId,sessions}));
  const service=await new AgentService({home,executable:'not-needed',callTool:()=>{throw Error('No tools during storage management');},hostFactory:()=>{throw Error('No model during storage management');}}).open();
  assert.deepEqual(await service.imageReferences(),[image.id,other.id]);
  service.active={};await assert.rejects(service.imageReferences(),/response to finish/);service.active=null;
  service.preparing=true;await assert.rejects(service.imageReferences(),/response to finish/);service.preparing=false;
  await service.close();
  const reopened=await new AgentService({home,executable:'not-needed',callTool:()=>{throw Error('No tools');}}).open();
  assert.deepEqual(await reopened.imageReferences(),[image.id,other.id]);assert.deepEqual(reopened.sessions,sessions);await reopened.close();
});
test('Responses user images become SDK byte parts; remote URLs, forged types, assistant images and text floods fail',()=>{
  const part={type:'input_image',image_url:`data:image/png;base64,${png.toString('base64')}`};
  const translated=translateResponses({input:[{role:'user',content:[{type:'input_text',text:'Describe' },part]}]},[]);
  assert.equal(translated.messages[0].content[1].mediaType,'image/png');assert.deepEqual(translated.messages[0].content[1].data,png);
  for(const changed of [{...part,image_url:'https://example.test/private.png'},{...part,image_url:'file:///C:/private.png'},
    {...part,image_url:'data:image/svg+xml;base64,PHN2Zy8+'},{...part,image_url:'data:image/jpeg;base64,'+png.toString('base64')},
    {type:'file',data:'private'},{type:'input_image',image_url:null}]) assert.throws(()=>translateResponses({input:[{role:'user',content:[changed]}]},[]));
  assert.throws(()=>translateResponses({input:[{role:'assistant',content:[part]}]},[]));
  assert.throws(()=>translateResponses({input:[{role:'user',content:Array(7).fill(part)}]},[]));
  assert.throws(()=>translateResponses({input:[{role:'user',content:'x'.repeat(LIMITS.text+1)}]},[]));
  assert.throws(()=>sessionImages([{type:'assistant',images:[image]}]));
});
test('each bridge step restores selected images lost by mid-turn compaction without changing tool pairs or leaking session scope',async t=>{
  const home=await stored(t),sessionId=randomUUID(),seen=[];
  const definitions=[{name:'geod_projects_list',inputSchema:{type:'object'}}];
  const config={label:'Owned context test',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'controlled-test',apiKey:'private-test-marker'};
  const bridge=await startBridge({config,definitions,token:'owned-local-token',home,sessionId,
    imageContext:id=>id===sessionId?[image]:[],stream:options=>{seen.push(options);return {fullStream:(async function*(){yield {type:'text-delta',text:'Controlled answer'};})(),finishReason:Promise.resolve('stop'),usage:Promise.resolve({})};}});
  t.after(()=>bridge.close());
  const input=[{role:'user',content:'Native compacted summary'},
    {type:'function_call',call_id:'owned-read',name:'geod_projects_list',arguments:'{}'},
    {type:'function_call_output',call_id:'owned-read',output:'{"total":0}'}];
  const request=async input=>{const result=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-local-token'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input,tools:[{type:'function',name:'geod_projects_list'}]})});await result.text();return result.status;};
  assert.equal(await request(input),200);
  assert.equal(seen[0].messages.at(-1).role,'tool');assert.equal(seen[0].messages.at(-1).content[0].toolCallId,'owned-read');
  assert.deepEqual(seen[0].messages[0].content.find(part=>part.type==='file').data,png);assert(!seen[0].tools.geod_projects_list.execute);
  assert.equal(await request([{role:'user',content:[{type:'input_image',image_url:'data:image/png;base64,'+png.toString('base64')}]},...input.slice(1)]),200);
  assert.equal(seen[1].messages[0].content.filter(part=>part.type==='file').length,1);
  await bridge.resetBudget(randomUUID());assert.equal(await request(input),200);assert.equal(seen[2].messages[0].content,'Native compacted summary');
  await bridge.resetBudget(sessionId);await writeFile(join(home,'images',`${image.id}.png`),Buffer.alloc(png.length));
  assert.equal(await request(input),400);assert.equal(seen.length,3);
});
test('image-only send uses owned localImage paths and persists references across service restart',async t=>{
  const home=await stored(t), calls=[],threadId=randomUUID();let event;
  const config={label:'Owned fixture',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'vision-test',apiKey:'synthetic-key'};
  const rig=()=>new AgentService({home,executable:'unit-test-only',callTool:async()=>({}),
    bridgeFactory:async()=>({baseUrl:'http://127.0.0.1:1/v1',resetBudget(){},async close(){}}),
    hostFactory:async options=>{event=options.onEvent;return {async rpc(method,params){calls.push({method,params});return method==='thread/start'?{thread:{id:threadId}}:method==='turn/start'?{turn:{id:randomUUID()}}:{};},async close(){}};}});
  let service=rig();await service.open();await service.configure(config,[{name:'geod_projects_list',inputSchema:{type:'object'}}]);
  await assert.rejects(service.send({text:'',images:['https://example.test/x.png']}));
  await service.send({text:'',images:[image.id]});await service.turnTask;
  assert.deepEqual(calls.find(call=>call.method==='turn/start').params.input,[{type:'localImage',path:join(home,'images',`${image.id}.png`)}]);
  const id=service.selectedId;event({method:'turn/completed',params:{threadId,turn:{status:'completed'}}});await new Promise(r=>setImmediate(r));await service.close();
  const raw=await readFile(join(home,'sessions.json'),'utf8');assert(!raw.includes('data:image'));assert(!raw.includes(home));assert(!raw.includes(config.apiKey));
  const saved=JSON.parse(raw);saved.sessions[0].entries[0].images[0]=Object.fromEntries(Object.entries(image).sort(([a],[b])=>a.localeCompare(b)));
  await writeFile(join(home,'sessions.json'),JSON.stringify(saved));
  service=rig();await service.open();await service.configure(config,[{name:'geod_projects_list',inputSchema:{type:'object'}}]);
  assert.deepEqual(service.snapshot().selected.entries[0].images,[image]);await service.send({sessionId:id,text:'Still see it?'});await service.turnTask;
  assert(calls.some(call=>call.method==='thread/resume' && call.params.threadId===threadId));
  event({method:'turn/completed',params:{threadId,turn:{status:'completed'}}});await new Promise(r=>setImmediate(r));
  await writeFile(join(home,'images',`${image.id}.png`),Buffer.alloc(png.length));
  const count=service.snapshot().selected.entries.length;await assert.rejects(service.send({sessionId:id,text:'Changed image'}));assert.equal(service.snapshot().selected.entries.length,count);await service.close();
});
