// Real pinned Codex process, private loopback model stub, no upstream calls.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { launchCodex } from '../agent/codex-host.mjs';
import { startBridge, BRIDGE_MODEL } from '../agent/protocol.mjs';
import { validEngineGoal } from '../agent/goals.mjs';
const root=resolve('.verification',`codex-goals-${Date.now()}`),token=randomUUID(),events=[];
await mkdir(root,{recursive:true});let requests=0,host,threadId,failResponse=false;
const bridge=await startBridge({home:root,token,definitions:[],config:{label:'Controlled goal acceptance',protocol:'openai-compatible',baseUrl:'http://127.0.0.1:1',model:'controlled',apiKey:'test-only'},
  beforeComplete:async()=>{if(host&&threadId)await host.rpc('thread/goal/set',{threadId,status:'paused'});},
  onFailure:async()=>{if(host&&threadId)await host.rpc('thread/goal/set',{threadId,status:'paused'});},
  stream:()=>{requests++;return {fullStream:(async function*(){if(failResponse)throw Error('Controlled unavailable response');yield {type:'text-delta',text:'Controlled reply.'};yield {type:'text-end'};})(),finishReason:Promise.resolve('stop'),usage:Promise.resolve({inputTokens:10,outputTokens:5,totalTokens:15})};}});
const launch=()=>launchCodex({executable:resolve('.agent-runtime/win32-x64/codex.exe'),home:join(root,'codex'),cwd:join(root,'sandbox'),bridgeUrl:bridge.baseUrl,token,model:BRIDGE_MODEL,onEvent:event=>events.push(event),callTool:async()=>{throw Error('No data tools in this check.');}});
try{
  host=await launch();const {thread}=await host.rpc('thread/start',{model:BRIDGE_MODEL,modelProvider:'geod',cwd:join(root,'sandbox'),approvalPolicy:'never',sandbox:'read-only',dynamicTools:[]});
  threadId=thread.id;
  const goal=(await host.rpc('thread/goal/set',{threadId:thread.id,objective:'Verify native persisted goal lifecycle with a controlled reply.',status:'active'})).goal;
  assert(validEngineGoal(goal,thread.id));assert.equal(goal.status,'active');
  await host.rpc('turn/start',{threadId:thread.id,input:[{type:'text',text:'Reply with Controlled reply. Do not call tools.'}],approvalPolicy:'never',sandboxPolicy:{type:'readOnly',networkAccess:false}});
  const deadline=Date.now()+30000;
  while(!events.some(event=>event.method==='turn/completed')&&Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,100));
  assert(events.some(event=>event.method==='turn/completed'&&event.params.turn.status==='completed'));assert.equal(requests,1);
  const paused=(await host.rpc('thread/goal/set',{threadId:thread.id,status:'paused'})).goal;
  assert.equal(paused.objective,goal.objective);assert.equal(paused.status,'paused');
  await host.close();host=await launch();await host.rpc('thread/resume',{threadId:thread.id,model:BRIDGE_MODEL,modelProvider:'geod',approvalPolicy:'never',sandbox:'read-only'});
  const restored=(await host.rpc('thread/goal/get',{threadId:thread.id})).goal;
  assert(validEngineGoal(restored,thread.id));assert.equal(restored.status,'paused');assert.equal(restored.objective,goal.objective);
  failResponse=true;const eventStart=events.length;await host.rpc('thread/goal/set',{threadId:thread.id,status:'active'});
  await host.rpc('turn/start',{threadId:thread.id,input:[{type:'text',text:'Controlled failure check.'}],approvalPolicy:'never',sandboxPolicy:{type:'readOnly',networkAccess:false}});
  const failureDeadline=Date.now()+30000;
  while(!events.slice(eventStart).some(event=>event.method==='turn/completed')&&Date.now()<failureDeadline)await new Promise(resolve=>setTimeout(resolve,100));
  assert(events.slice(eventStart).some(event=>event.method==='turn/completed'&&event.params.turn.status==='failed'));assert.equal(requests,2);
  assert.equal((await host.rpc('thread/goal/get',{threadId:thread.id})).goal.status,'paused');
  const completed=(await host.rpc('thread/goal/set',{threadId:thread.id,status:'complete'})).goal;assert.equal(completed.status,'complete');
  await host.rpc('thread/goal/clear',{threadId:thread.id});assert.equal((await host.rpc('thread/goal/get',{threadId:thread.id})).goal,null);
  const report={scope:'Real pinned Codex goal RPC/persistence; loopback model is controlled. No actual geographic task or external model.',status:'passed',requests,successRequests:1,failureRequests:1,failurePausesNativeGoal:true,restored,completed};
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,requests,output:join(root,'result.json')}));
}finally{await host?.close();await bridge.close();}
