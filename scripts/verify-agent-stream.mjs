// Prepared Node + actual owned Codex + AI SDK + controlled local SSE source.
// Verifies stream delivery, not model intelligence or data processing.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

const root=process.cwd(), directory=path.join(root,'.verification',`agent-stream-${Date.now()}`);
await mkdir(directory,{recursive:true});
const parts=['第一段输出。','\n第二段输出。','\n第三段输出。'];
let providerRequests=0, toolRequests=0;
const source=createServer(async(request,response)=>{
  const input=[];for await(const chunk of request)input.push(chunk);
  const body=JSON.parse(Buffer.concat(input).toString('utf8'));
  if(request.url!=='/v1/chat/completions'||body.stream!==true||++providerRequests>1){response.writeHead(400);response.end();return;}
  response.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache'});
  const chunk=(delta,finish_reason=null)=>response.write(`data: ${JSON.stringify({id:'owned-stream',object:'chat.completion.chunk',created:1,model:'owned-fixture',choices:[{index:0,delta,finish_reason}]})}\n\n`);
  chunk({role:'assistant',content:''});
  for(const text of parts){await new Promise(resolve=>setTimeout(resolve,160));chunk({content:text});}
  chunk({},'stop');response.end('data: [DONE]\n\n');
});
await new Promise(resolve=>source.listen(0,'127.0.0.1',resolve));
const runtime=path.join(root,'.agent-runtime/win32-x64');
const environment={NODE_USE_ENV_PROXY:'1',NO_PROXY:'127.0.0.1,localhost'};
for(const name of ['SystemRoot','SYSTEMROOT','WINDIR','TEMP','TMP'])if(process.env[name])environment[name]=process.env[name];
const child=spawn(path.join(runtime,'node.exe'),[path.join(runtime,'agent.mjs'),path.join(directory,'home'),path.join(runtime,'codex.exe')],{env:environment,cwd:directory,windowsHide:true,stdio:['pipe','pipe','pipe']});
let sequence=0,buffer='',closed=false,capturing=false,dirty=false;
const pending=new Map(),notifications=[],observations=[];
const exited=new Promise(resolve=>child.once('exit',(code)=>{closed=true;resolve(code);}));
child.stderr.resume();
const send=value=>child.stdin.write(JSON.stringify(value)+'\n');
const rpc=(method,params={})=>new Promise((resolve,reject)=>{
  const id=++sequence,timer=setTimeout(()=>{pending.delete(id);reject(Error(`Stream check ${method} timed out.`));},30_000);
  pending.set(id,{resolve,reject,timer});send({id,method,params});
});
async function capture(){
  if(capturing){dirty=true;return;}
  capturing=true;
  try{
    const value=await rpc('snapshot');
    const assistant=value.selected?.entries.filter(entry=>entry.type==='assistant').at(-1);
    observations.push({revision:value.revision,busy:value.busy,text:assistant?.text??'',status:assistant?.status??null});
  }finally{capturing=false;if(dirty&&!closed){dirty=false;void capture();}}
}
child.stdout.setEncoding('utf8');
child.stdout.on('data',text=>{
  buffer+=text;let end;
  while((end=buffer.indexOf('\n'))!==-1){
    const value=JSON.parse(buffer.slice(0,end));buffer=buffer.slice(end+1);
    if(value.method==='geod.changed'){
      assert.deepEqual(Object.keys(value).sort(),['method','params']);assert.deepEqual(Object.keys(value.params),['revision']);
      assert(Number.isSafeInteger(value.params.revision)&&value.params.revision>=0);
      notifications.push(value.params.revision);void capture();
    }else if(value.method==='geod.tool'){
      toolRequests++;send({id:value.id,error:{message:'No business operation is authorized in this stream check.'}});
    }else if(pending.has(value.id)){
      const entry=pending.get(value.id);pending.delete(value.id);clearTimeout(entry.timer);
      value.error?entry.reject(Error(value.error.message)):entry.resolve(value.result);
    }
  }
});
async function until(predicate){
  const deadline=Date.now()+30_000;
  while(!predicate()){if(closed||Date.now()>deadline)throw Error('Owned stream verification did not finish.');await new Promise(resolve=>setTimeout(resolve,20));}
}
const receipt={schema:'geod-agent-stream-acceptance/v1',status:'pending',runtime:'prepared Node and actual owned Codex process',provider:'controlled loopback SSE via the actual AI SDK adapter',paidModelCalls:0,businessToolsRun:0,usedUserDesktop:false};
try{
  await rpc('configure',{config:{label:'Owned stream check',protocol:'openai-compatible',baseUrl:`http://127.0.0.1:${source.address().port}/v1`,model:'owned-fixture',apiKey:'controlled-local-key'},definitions:[{name:'geod_health',description:'Read native workspace health',inputSchema:{type:'object',properties:{},additionalProperties:false}}]});
  await rpc('send',{text:'只回复三句短文本，无需调用工具。'});
  await until(()=>observations.some(value=>value.status==='completed'&&!value.busy));
  const partial=observations.filter(value=>value.busy&&value.status==='running'&&value.text);
  assert(partial.some(value=>value.text===parts[0]));
  assert(partial.some(value=>value.text===parts.slice(0,2).join('')));
  assert.equal(observations.at(-1).text,parts.join(''));
  assert.equal(providerRequests,1);assert.equal(toolRequests,0);
  assert(notifications.every((value,index)=>!index||value>notifications[index-1]));
  Object.assign(receipt,{status:'passed',providerRequests,toolRequests,notifications:notifications.length,partialSnapshots:partial.length,finalText:observations.at(-1).text,observations});
}catch(error){receipt.status='failed';receipt.failure=error.message;throw error;}
finally{
  child.stdin.end();await Promise.race([exited,new Promise(resolve=>setTimeout(()=>{child.kill();resolve();},10_000))]);
  for(const entry of pending.values()){clearTimeout(entry.timer);entry.resolve({});}pending.clear();
  await new Promise(resolve=>source.close(resolve));
  await writeFile(path.join(directory,'verification.json'),JSON.stringify(receipt,null,2));
  console.log(JSON.stringify({status:receipt.status,directory,paidModelCalls:0,notifications:receipt.notifications,partialSnapshots:receipt.partialSnapshots}));
}
