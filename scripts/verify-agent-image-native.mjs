// Actual desktop Rust -> owned Node IPC -> pinned Codex -> SDK, without a window.
// Only upstream generation and credential storage are controlled fixtures.
import {createServer} from 'node:http';
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import {openaiResponsesReply} from './acceptance/openai-responses-wire.mjs';
const root=process.cwd(),fixture=path.resolve(process.env.GEOD_AGENT_IMAGE_FIXTURE),record=JSON.parse(await readFile(path.join(fixture,'ingestion.json'),'utf8'));
assert.equal(record.status,'passed');
const output=path.join(root,'.verification',`agent-image-native-${Date.now()}`);let calls=0,failed=false;
const compact=process.argv.includes('--compact');
const responses=process.argv.includes('--responses');let encryptedReplayItems=0;
const automatic=process.argv.includes('--automatic');assert(!automatic || compact);
if(compact){const prepared=spawnSync('rtk',['proxy','python','-X','utf8','.verification/prepare-context-source.py',output],{cwd:root,windowsHide:true,stdio:'pipe'});assert.equal(prepared.status,0);}
const server=createServer(async(request,response)=>{
  try {
    assert.equal(request.url,responses?'/v1/responses':'/v1/chat/completions');assert.equal(request.headers.authorization,'Bearer synthetic-owned-key');
    const chunks=[];for await(const chunk of request)chunks.push(chunk);const body=JSON.parse(Buffer.concat(chunks));assert.equal(body.model,responses?'gpt-5':'controlled-vision');
    const history=responses?body.input:body.messages;
    assert([0,48].includes(body.tools?.length??0));const part=history.flatMap(message=>Array.isArray(message.content)?message.content:[]).find(part=>part.type===(responses?'input_image':'image_url'));
    const encoded=(responses?part.image_url:part.image_url.url).split(',')[1];
    assert.equal(createHash('sha256').update(Buffer.from(encoded,'base64')).digest('hex'),record.image.id);calls++;
    if(responses){assert.equal(body.store,false);assert.equal(body.parallel_tool_calls,false);assert(body.include.includes('reasoning.encrypted_content'));for(const item of history.filter(item=>item.type==='reasoning')){assert(item.encrypted_content?.startsWith('controlled-opaque-'));encryptedReplayItems++;}}
    const result=!body.tools?.length || (responses?history.at(-1).type==='function_call_output':history.at(-1).role==='tool');
    const delta=result?{content:'Controlled native project query returned 0.'}:{tool_calls:[{index:0,id:`native_image_${calls}`,type:'function',function:{name:'geod_projects_list',arguments:'{}'}}]};
    const inputTokens=automatic && [1,6].includes(calls)?28000:10;
    if(responses)openaiResponsesReply(response,{model:body.model,callId:result?null:`native_image_${calls}`,inputTokens});
    else response.writeHead(200,{'content-type':'text/event-stream'}).end(`data: ${JSON.stringify({id:'owned_wire',object:'chat.completion.chunk',created:1,model:body.model,choices:[{index:0,delta,finish_reason:result?'stop':'tool_calls'}],usage:{prompt_tokens:inputTokens,completion_tokens:10,total_tokens:inputTokens+10}})}\n\ndata: [DONE]\n\n`);
  }catch {failed=true;response.writeHead(500).end('{"error":{"message":"Controlled image wire assertion failed"}}');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let child,diagnostics='';
try {
  child=spawn('rtk',['proxy','cargo','test','--offline','--locked','-p','geod-global-desktop','--features','custom-protocol','desktop_images_cross_private_ipc_and_resume_same_thread','--','--ignored','--exact','agent::image_acceptance::desktop_images_cross_private_ipc_and_resume_same_thread'],{cwd:root,windowsHide:true,stdio:['ignore','pipe','pipe'],env:{...process.env,GEOD_AGENT_RESPONSES_TEST:responses?'1':'0',GEOD_AGENT_CONTEXT_TEST:compact?'1':'0',GEOD_AGENT_AUTOMATIC_CONTEXT_TEST:automatic?'1':'0',GEOD_AGENT_IMAGE_FIXTURE:fixture,GEOD_AGENT_IMAGE_NATIVE_OUTPUT:output,GEOD_AGENT_IMAGE_BASE_URL:`http://127.0.0.1:${server.address().port}/v1`}});
  const remember=chunk=>{diagnostics=(diagnostics+chunk.toString()).slice(-16000);};
  child.stdout.on('data',remember);child.stderr.on('data',remember);
  const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.once('exit',resolve);});
  assert.equal(code,0);assert(!failed);assert.equal(calls,automatic?9:compact?5:4);
  const receipt=JSON.parse(await readFile(path.join(output,'native-acceptance.json'),'utf8'));assert.equal(receipt.status,'passed');
  if(responses)assert(encryptedReplayItems>=2);
  receipt.providerRequests=calls;receipt.encryptedReplayItems=encryptedReplayItems;await writeFile(path.join(output,'native-acceptance.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({output,status:'passed',providerRequests:calls,nativeReadCalls:receipt.nativeReadCalls}));
} catch {await mkdir(output,{recursive:true});await writeFile(path.join(output,'driver-failed.json'),JSON.stringify({schema:'geod-image-native-driver/v1',status:'failed',providerRequests:calls,wireAssertionFailed:failed,reason:'Compile or native acceptance case did not pass.',nativeExitCode:child?.exitCode??null,diagnosticLocation:diagnostics.match(/panicked at [^\r\n]*/)?.[0]??null,assertion:diagnostics.match(/assertion[^\r\n]*failed[^\r\n]*/)?.[0]??null,nativeError:diagnostics.match(/called `Result::unwrap\(\)` on an `Err` value: [^\r\n]*/)?.[0]?.slice(0,600)??null},null,2)+'\n');process.exitCode=1;console.log(JSON.stringify({output,status:'failed',providerRequests:calls}));}
finally {if(child && child.exitCode===null)child.kill();await new Promise(resolve=>{server.closeAllConnections();server.close(resolve);});}
