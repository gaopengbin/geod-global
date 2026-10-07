import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { testModelConnection } from './connection-test.mjs';

async function endpoint(t, reply) {
  const requests=[];
  const server=createServer(async(req,res)=>{
    let bytes='';for await(const chunk of req)bytes+=chunk;
    const body=JSON.parse(bytes);requests.push({url:req.url,authorization:req.headers.authorization,body});
    reply(req,res,body,requests.length);
  });
  server.listen(0,'127.0.0.1');await once(server,'listening');
  t.after(()=>{server.closeAllConnections();server.close();});
  return {requests,config:{label:'Owned connection test',protocol:'openai-compatible',baseUrl:`http://127.0.0.1:${server.address().port}/v1`,model:'controlled-model',apiKey:'synthetic-test-secret'}};
}
function completion(res,message,finish='stop') {
  res.writeHead(200,{'content-type':'application/json'}).end(JSON.stringify({id:'owned-completion',object:'chat.completion',created:1,model:'controlled-model',choices:[{index:0,finish_reason:finish,message}],usage:{prompt_tokens:20,completion_tokens:10,total_tokens:30}}));
}
test('connection probe uses the actual adapter for text and one inert tool call, without retries or workspace data',async t=>{
  const {config,requests}=await endpoint(t,(_req,res,body,index)=>{
    if(index===1)return completion(res,{role:'assistant',content:'GeoD connection ready.'});
    const nonce=body.messages.at(-1).content.match(/nonce ([a-f0-9-]{36})/)[1];
    completion(res,{role:'assistant',content:null,tool_calls:[{id:'owned-probe',type:'function',function:{name:'geod_connection_probe',arguments:JSON.stringify({nonce})}}]},'tool_calls');
  });
  const home=await mkdtemp(join(tmpdir(),'geod-connection-probe-'));
  const child=spawn(process.execPath,['agent/stdio.mjs',home,'unused-diagnostic-only'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
  const pending=new Map();let sequence=0,diagnostics='';
  child.stderr.on('data',chunk=>diagnostics+=chunk);
  const lines=createInterface({input:child.stdout});
  lines.on('line',line=>{const value=JSON.parse(line),entry=pending.get(value.id);if(entry){pending.delete(value.id);value.error?entry.reject(Error(value.error.message)):entry.resolve(value.result);}});
  const rpc=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});child.stdin.write(JSON.stringify({id,method,params})+'\n');});
  t.after(async()=>{child.stdin.end();if(child.exitCode===null)await once(child,'exit');lines.close();await rm(home,{recursive:true,force:true});});
  const before=await rpc('snapshot'),stored=await readFile(join(home,'sessions.json'),'utf8');
  const result=await rpc('testModel',{config});
  const after=await rpc('snapshot');assert.deepEqual(after,before);
  assert.equal(await readFile(join(home,'sessions.json'),'utf8'),stored);
  assert.deepEqual(await readdir(home),['sessions.json']);assert.equal(diagnostics,'');
  assert.equal(result.status,'passed');assert.equal(result.text,true);assert.equal(result.functionCalls,true);
  assert.equal(requests.length,2);assert(requests.every(request=>request.url==='/v1/chat/completions' && request.authorization==='Bearer '+config.apiKey));
  assert.equal(requests[0].body.tools,undefined);assert.equal(requests[1].body.tools.length,1);
  assert.equal(requests[1].body.tool_choice.function.name,'geod_connection_probe');
  assert.deepEqual(Object.keys(result).sort(),['version','status','text','functionCalls','latencyMs','checkedAt'].sort());
  assert(!JSON.stringify(result).includes(config.apiKey));
});
test('authorization failure is sanitized and does not retry or test tools',async t=>{
  const {config,requests}=await endpoint(t,(_req,res)=>res.writeHead(401,{'content-type':'application/json'}).end(JSON.stringify({error:{message:'invalid key synthetic-test-secret'}})));
  const result=await testModelConnection(config);
  assert.equal(result.status,'failed');assert.equal(result.message,'Agent model authorization was rejected.');
  assert.equal(result.text,false);assert.equal(result.functionCalls,false);assert.equal(requests.length,1);
  assert(!JSON.stringify(result).includes(config.apiKey));
});
test('a text-only response does not claim tool support; a stalled request is aborted',async t=>{
  const first=await endpoint(t,(_req,res)=>completion(res,{role:'assistant',content:'Plain text only.'}));
  const result=await testModelConnection(first.config);
  assert.equal(result.status,'failed');assert.equal(result.text,true);assert.equal(result.functionCalls,false);assert.equal(first.requests.length,2);
  const stalled=await endpoint(t,()=>{});
  const timedOut=await testModelConnection(stalled.config,{timeoutMs:75});
  assert.equal(timedOut.status,'failed');assert.equal(timedOut.message,'Connection test timed out. Check the endpoint and try again.');
  assert.equal(stalled.requests.length,1);assert(timedOut.latencyMs<2000);
});
