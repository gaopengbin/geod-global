import test from 'node:test';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {createServer} from 'node:http';
import {mkdtemp,mkdir,readFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {documentId,documentInput,readDocument,restoreDocumentContext} from './document-store.mjs';
import {startBridge,BRIDGE_MODEL} from './protocol.mjs';
import {AgentService} from './service.mjs';
import {openaiResponsesReply} from '../scripts/acceptance/openai-responses-wire.mjs';
const records=JSON.parse(await readFile(new URL('./fixtures/office-records.json',import.meta.url),'utf8'));
const originals=await Promise.all(records.map(async({document})=>({document,bytes:await readFile(new URL('./fixtures/'+document.name,import.meta.url))})));
const definitions=[{name:'geod_projects_list',inputSchema:{type:'object',properties:{},additionalProperties:false}}];
async function store(t){
  const home=await mkdtemp(join(tmpdir(),'geod-office-'));t.after(()=>rm(home,{recursive:true,force:true}));await mkdir(join(home,'documents'));
  for(const {document,bytes} of originals){assert.equal(documentId(bytes,document),document.id);assert.equal(bytes.length,document.bytes);
    const extension=document.name.split('.').at(-1);await writeFile(join(home,'documents',document.id+'.'+extension),bytes);await writeFile(join(home,'documents',document.id+'.json'),JSON.stringify({version:1,document}));}
  return home;
}
test('Office originals reach the actual Responses SDK unchanged after restart and remain isolated from other conversations',async t=>{
  const home=await store(t),sessionId=randomUUID(),seen=[],diagnostics=[];let wireFailure;
  const server=createServer(async(request,response)=>{try{
    const chunks=[];for await(const part of request)chunks.push(part);const body=JSON.parse(Buffer.concat(chunks));seen.push(body);assert.equal(body.store,false);
    const files=body.input.flatMap(item=>Array.isArray(item.content)?item.content:[]).filter(part=>part.type==='input_file');
    assert.equal(files.length,seen.length===3?0:3);
    if(seen.length!==3)for(const {document,bytes} of originals){const part=files.find(file=>file.filename===document.name);assert(part);
      assert.equal(part.file_data,`data:${document.mimeType};base64,`+bytes.toString('base64'));}
    openaiResponsesReply(response,{model:body.model,text:'Controlled Office transport check'});
  }catch(error){wireFailure=error;response.writeHead(500).end('{"error":{"message":"Controlled Office wire failure"}}');}});
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));t.after(()=>new Promise(resolve=>{server.closeAllConnections();server.close(resolve);}));
  const config={label:'Owned Office wire',protocol:'openai-responses',baseUrl:`http://127.0.0.1:${server.address().port}/v1`,model:'controlled-office',apiKey:'owned-office-synthetic-key'};
  const documents=originals.map(item=>item.document);
  const open=()=>startBridge({config,definitions,home,identity:'owned-office',sessionId,token:'owned-loopback',documentContext:id=>id===sessionId?documents:[],onDiagnostic:value=>diagnostics.push(value)});
  let bridge=await open();t.after(()=>bridge.close());
  const post=async()=>{const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{authorization:'Bearer owned-loopback'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Saved working summary'}]})});
    const body=await response.text();if(response.status===200)assert(body.includes('response.completed'),JSON.stringify({diagnostics,requests:seen.length,wireFailure:wireFailure?.message.slice(0,200)}));return response.status;};
  assert.equal(await post(),200);await bridge.close();bridge=await open();assert.equal(await post(),200);
  await bridge.resetBudget(randomUUID());assert.equal(await post(),200);
  await bridge.resetBudget(sessionId);const {document}=originals[0];await writeFile(join(home,'documents',document.id+'.docx'),Buffer.alloc(document.bytes,120));
  assert.equal(await post(),400);assert.equal(seen.length,3);assert(!wireFailure,wireFailure?.message);
});
test('unsupported Office protocols refuse before appending history or starting a model request',async t=>{
  const home=await store(t),config={label:'Owned compatible fixture',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'controlled',apiKey:'owned-synthetic-key'};
  let starts=0;const service=await new AgentService({home,executable:'unit-test-only',callTool:async()=>({}),bridgeFactory:async()=>{starts++;throw Error('Must not start');}}).open();
  t.after(()=>service.close());await service.configure(config,definitions);
  await assert.rejects(service.send({text:'',documents:[originals[0].document.id]}),/Office files require/);
  assert.equal(service.sessions.length,0);assert.equal(starts,0);
  await assert.rejects(restoreDocumentContext([{role:'user',content:'Summary'}],[originals[0].document],home,text=>text,'anthropic'),/Office files require/);
  assert.deepEqual((await readDocument(home,originals[0].document.id)).bytes,originals[0].bytes);
  assert(!documentInput({document:originals[0].document}).includes(originals[0].bytes.toString('base64')));
});
