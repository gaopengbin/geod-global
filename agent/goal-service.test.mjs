import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, sep } from 'node:path';
import { randomUUID } from 'node:crypto';
import { AgentService } from './service.mjs';
const config={label:'Controlled goal service',protocol:'openai-compatible',baseUrl:'http://127.0.0.1:1/v1',model:'controlled',apiKey:'test-marker'};
async function rig(t){
  const home=await mkdtemp(join(tmpdir(),'geod-goal-service-')),calls=[],threadId=randomUUID();let engine=null,beforeComplete;
  const plan={planId:randomUUID(),kind:'download',status:'pending',jobs:[{id:randomUUID(),status:'queued',settled:false}]};
  const service=await new AgentService({home,executable:'controlled-test',callTool:async(name,args)=>{
    calls.push({name,args});if(name==='geod_plan_status'||name==='geod_download_plan')return structuredClone(plan);
    if(name==='geod_execution_policy')return {mode:'confirm-each'};
    if(name==='geod_raster_inspect'||name==='geod_gscloud_archive_inspect')return {jobId:args.id,sha256:'a'.repeat(64)};
    return {};
  },bridgeFactory:async options=>{beforeComplete=options.beforeComplete;return {baseUrl:'http://127.0.0.1:1/v1',resetBudget:async()=>{},close:async()=>{}};},
  hostFactory:async()=>({rpc:async(method,args)=>{
    calls.push({method,args});if(method==='thread/start')return {thread:{id:threadId}};if(method==='turn/start')return {turn:{id:randomUUID()}};
    if(method==='thread/goal/set'){engine={threadId,objective:args.objective??engine?.objective,status:args.status,tokenBudget:null,tokensUsed:0,timeUsedSeconds:0,createdAt:1,updatedAt:1};return {goal:structuredClone(engine)};}
    if(method==='thread/goal/get')return {goal:engine};if(method==='thread/goal/clear'){engine=null;return {};}
    return {};
  },close:async()=>{}})}).open();
  t.after(async()=>{await service.close();assert(resolve(home).startsWith(resolve(tmpdir())+sep));assert(home.includes('geod-goal-service-'));await rm(home,{recursive:true,force:true});});
  await service.configure(config,[{name:'geod_download_plan',inputSchema:{type:'object'}},{name:'geod_raster_inspect',inputSchema:{type:'object'}}]);
  const send=async args=>{await service.send({text:'Download the requested imagery.',...args});await service.turnTask;};
  const tool=(name,args)=>service.tool({threadId:service.active.session.threadId,turnId:service.active.turnId,tool:name,arguments:args});
  return {home,service,plan,calls,send,tool,beforeComplete:()=>beforeComplete()};
}
test('reply completion waits for native confirmation, then verifies the final actual plan rather than treating the reply as success',async t=>{
  const r=await rig(t);await r.send();await r.tool('geod_goal_define',{objective:'Deliver the requested imagery.',outputs:[{id:'image',label:'Requested original imagery',kind:'raster'}]});
  await r.tool('geod_download_plan',{});await r.tool('geod_goal_bind',{outputId:'image',planId:r.plan.planId});await r.beforeComplete();
  const activities=[];r.service.onChange=()=>activities.push(r.service.snapshot().activity);
  await r.service.finish('completed');assert.equal(r.service.snapshot().selected.goal.status,'waiting_confirmation');
  assert.equal(activities.at(-1),null,'Native goal verification must notify the UI when the turn actually ends');
  const turns=r.calls.filter(call=>call.method==='turn/start').length;await r.service.workflow.tick();assert.equal(r.calls.filter(call=>call.method==='turn/start').length,turns);
  assert.equal(r.service.snapshot().selected.goal.engine.status,'paused');
  r.plan.status='submitted';r.plan.jobs[0].status='succeeded';r.plan.jobs[0].settled=true;
  await r.send({sessionId:r.service.selectedId,text:'Check the completed delivery.'});await r.tool('geod_goal_check',{});await r.beforeComplete();await r.service.finish('completed');
  assert.equal(r.service.snapshot().selected.goal.status,'complete');assert(r.calls.some(call=>call.name==='geod_raster_inspect'));
  assert(!r.calls.some(call=>call.name==='geod_plan_execute'));
});
test('archive goals wait for every confirmed ZIP and verify originals without a raster decoder',async t=>{
  const r=await rig(t);await r.send();
  await r.tool('geod_goal_define',{objective:'Deliver the selected GSCloud ZIP originals.',outputs:[{id:'zip',label:'Original ZIPs',kind:'archive'}]});
  await r.tool('geod_download_plan',{});await r.tool('geod_goal_bind',{outputId:'zip',planId:r.plan.planId});
  await r.beforeComplete();await r.service.finish('completed');assert.equal(r.service.snapshot().selected.goal.status,'waiting_confirmation');
  r.plan.status='submitted';r.plan.jobs.push({id:randomUUID(),status:'queued',settled:false});r.plan.jobs[0].status='succeeded';r.plan.jobs[0].settled=true;
  await r.send({sessionId:r.service.selectedId,text:'Verify the original ZIP delivery.'});await r.tool('geod_goal_check',{});
  assert.equal(r.service.snapshot().selected.goal.status,'waiting_jobs');
  r.plan.jobs[1].status='succeeded';r.plan.jobs[1].settled=true;
  await r.tool('geod_goal_check',{});
  assert.equal(r.service.snapshot().selected.goal.status,'complete');
  assert.equal(r.service.snapshot().selected.goal.outputs[0].verifiedIds.length,2);
  assert(r.calls.some(call=>call.name==='geod_gscloud_archive_inspect'));
  assert(!r.calls.some(call=>call.name==='geod_raster_inspect'||call.name==='geod_plan_execute'));
});

test('replayed native confirmation adds no workflow, reply or model request after submission',async t=>{
  const r=await rig(t);await r.send();await r.tool('geod_download_plan',{});await r.service.finish('completed');
  r.plan.status='submitted';const binding=randomUUID(),sessionId=r.service.selectedId;
  const control={sessionId,text:'',action:'confirm',plans:[r.plan],executionMode:'confirm-each',executionBinding:binding};
  await r.service.recordControl(control);
  const session=r.service.sessions.find(session=>session.id===sessionId);session.workflow.status='completed';session.workflow.pendingPlans=[];
  const entries=JSON.stringify(session.entries),workflow=JSON.stringify(session.workflow),turns=r.calls.filter(call=>call.method==='turn/start').length;
  await r.service.recordControl(control);await r.service.workflow.tick();
  assert.equal(JSON.stringify(session.entries),entries);assert.equal(JSON.stringify(session.workflow),workflow);
  assert.equal(r.calls.filter(call=>call.method==='turn/start').length,turns);
});

test('persisted goals recover paused with the complete manifest, and native clear never removes data or grants permission',async t=>{
  const r=await rig(t);await r.send();await r.tool('geod_goal_define',{objective:'Get imagery and requested statistics.',outputs:[{id:'image',label:'Imagery',kind:'raster'},{id:'table',label:'CSV statistics',kind:'unavailable'}]});
  await r.beforeComplete();await r.service.finish('completed');assert.equal(r.service.snapshot().selected.goal.status,'needs_attention');
  const restored=await new AgentService({home:r.home,executable:'controlled-test',callTool:async()=>{throw Error('No runtime read during history restore.');}}).open();
  assert.equal(restored.snapshot().selected.goal.status,'paused');assert.equal(restored.snapshot().selected.goal.outputs.length,2);await restored.close();
  await r.service.goalControl({sessionId:r.service.selectedId,action:'clear'});assert.equal(r.service.snapshot().selected.goal,undefined);
  assert(r.calls.some(call=>call.method==='thread/goal/clear'));assert(!r.calls.some(call=>call.name==='geod_plan_execute'||call.name==='geod_job_control'));
});
test('bounded continuation keeps the same goal, and a model reply without progress stops rather than spinning',async t=>{
  const r=await rig(t);await r.send();await r.tool('geod_goal_define',{objective:'Prepare and deliver imagery.',outputs:[{id:'image',label:'Imagery',kind:'raster'}]});
  await r.beforeComplete();await r.service.finish('completed');const goalId=r.service.snapshot().selected.goal.id;
  await r.service.workflow.tick();await r.service.turnTask;assert.equal(r.service.active.origin,'workflow');assert.equal(r.service.snapshot().selected.goal.id,goalId);
  await assert.rejects(r.tool('geod_goal_define',{objective:'Drop the imagery requirement.',outputs:[{id:'less',label:'Just metadata',kind:'metadata'}]}),/human task/);
  await r.beforeComplete();await r.service.finish('completed');assert.equal(r.service.snapshot().selected.goal.status,'needs_attention');
  const count=r.calls.filter(call=>call.method==='turn/start').length;await r.service.workflow.tick();assert.equal(r.calls.filter(call=>call.method==='turn/start').length,count);
});
test('a status question does not restart a paused goal',async t=>{
  const r=await rig(t);await r.send();await r.tool('geod_goal_define',{objective:'Deliver imagery.',outputs:[{id:'image',label:'Imagery',kind:'raster'}]});
  await r.beforeComplete();await r.service.finish('completed');await r.service.goalControl({sessionId:r.service.selectedId,action:'pause'});
  await r.send({sessionId:r.service.selectedId,text:'Why is this goal paused?'});await r.beforeComplete();await r.service.finish('completed');
  assert.equal(r.service.snapshot().selected.goal.status,'paused');const turns=r.calls.filter(call=>call.method==='turn/start').length;
  await r.service.workflow.tick();assert.equal(r.calls.filter(call=>call.method==='turn/start').length,turns);
});
