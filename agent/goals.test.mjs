import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { GoalCoordinator } from './goals.mjs';
import { validGoal, validGoalInput } from './goal-contract.mjs';
function rig(){
  const session={id:randomUUID(),threadId:randomUUID(),entries:[]},calls=[],plans=new Map(),files=new Map();let engine=null;
  const service={redact:text=>text.replaceAll('test-secret','[redacted]'),save:async()=>{},host:{rpc:async(method,args)=>{
    calls.push({method,args});
    if(method==='thread/goal/set'){engine={threadId:session.threadId,objective:args.objective??engine?.objective,status:args.status,tokenBudget:null,tokensUsed:0,timeUsedSeconds:0,createdAt:1,updatedAt:1};return {goal:structuredClone(engine)};}
    if(method==='thread/goal/get')return {goal:engine};
  }},callTool:async(name,args,scope)=>{calls.push({name,args,scope});if(name==='geod_plan_status')return structuredClone(plans.get(args.planId));return structuredClone(files.get(args.id));}};
  const goals=new GoalCoordinator(service),active={session,origin:'human',humanText:'Download and crop imagery for the requested area.'};
  const addPlan=(kind='download',count=1)=>{
    const plan={planId:randomUUID(),kind,status:'pending',jobs:Array.from({length:count},()=>({id:randomUUID(),status:'queued',settled:false})),project:{id:randomUUID(),saved:false}};
    plans.set(plan.planId,plan);session.entries.push({id:randomUUID(),type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:plan.planId}]});return plan;
  };
  const succeed=plan=>{plan.status='submitted';for(const job of plan.jobs){job.status='succeeded';job.settled=true;files.set(job.id,{jobId:job.id,sha256:'a'.repeat(64)});}};
  return {session,service,goals,active,calls,plans,files,addPlan,succeed};
}
const define=(r,outputs=[{id:'delivery',label:'Cropped imagery',kind:'raster'}])=>r.goals.define(r.active,{objective:'Deliver requested imagery and required processing.',outputs});
test('rechecking a completed delivery exposes valid snapshots throughout a slow native audit and publishes failures atomically',async()=>{
  const r=rig();await define(r);const plan=r.addPlan();r.succeed(plan);await r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId});await r.goals.check(r.session);
  assert.equal(r.session.goal.status,'complete');
  const prior=structuredClone(r.session.goal),native=r.service.callTool;let entered,release;
  const waiting=new Promise(resolve=>entered=resolve);
  r.service.callTool=async(...args)=>{if(args[0]==='geod_raster_inspect'){entered();await new Promise(resolve=>release=resolve);}return native(...args);};
  const recheck=r.goals.check(r.session);await waiting;
  assert(validGoal(r.session.goal),'the renderer must not observe a verified output with its IDs cleared');
  assert.deepEqual(r.session.goal,prior);
  r.files.set(plan.jobs[0].id,{sha256:'invalid'});release();await recheck;
  assert.equal(r.session.goal.status,'needs_attention');assert.equal(r.session.goal.outputs[0].state,'failed');assert(validGoal(r.session.goal));
});
test('an audit cannot restore a cleared goal or overwrite a newer native output binding',async()=>{
  for(const action of ['clear','rebind']){
    const r=rig();await define(r);const plan=r.addPlan();r.succeed(plan);await r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId});
    const native=r.service.callTool;let entered,release;const waiting=new Promise(resolve=>entered=resolve);
    r.service.callTool=async(...args)=>{if(args[0]==='geod_raster_inspect'){entered();await new Promise(resolve=>release=resolve);}return native(...args);};
    const audit=r.goals.check(r.session);await waiting;
    let replacement;
    if(action==='clear')r.session.goal=null;
    else{replacement=r.addPlan();await r.goals.bind(r.active,{outputId:'delivery',planId:replacement.planId});}
    release();await audit;
    if(action==='clear')assert.equal(r.session.goal,null);
    else{assert.equal(r.session.goal.outputs[0].planId,replacement.planId);assert.equal(r.session.goal.outputs[0].state,'missing');assert(validGoal(r.session.goal));}
  }
});
test('whole output manifest uses real native goal RPC and cannot be replaced by a continuation',async()=>{
  const r=rig();await define(r,[{id:'imagery',label:'Imagery test-secret',kind:'raster'},{id:'table',label:'Regional CSV statistics',kind:'unavailable'}]);
  assert(validGoal(r.session.goal));assert.equal(r.calls[0].method,'thread/goal/set');assert.equal(r.session.goal.outputs[0].label,'Imagery [redacted]');
  await assert.rejects(define(r),/once/);r.active.goalDefined=false;r.active.origin='workflow';await assert.rejects(define(r),/human task/);
  await r.goals.check(r.session);assert.equal(r.session.goal.status,'needs_attention');assert.equal(r.session.goal.outputs.length,2);
  assert(!r.calls.some(call=>call.args?.status==='complete'));
});
test('all required plans and all child files must settle and verify, not merely the first successful output',async()=>{
  const r=rig();await define(r,[{id:'first',label:'First requested delivery',kind:'raster'},{id:'second',label:'Second requested delivery',kind:'raster'}]);
  const first=r.addPlan('download',2);await r.goals.bind(r.active,{outputId:'first',planId:first.planId});r.succeed(first);
  await assert.rejects(r.goals.bind(r.active,{outputId:'second',planId:first.planId}),/same result twice/);
  await r.goals.check(r.session);assert.equal(r.session.goal.status,'active');assert.deepEqual(r.session.goal.outputs.map(output=>output.state),['verified','missing']);
  const second=r.addPlan();await r.goals.bind(r.active,{outputId:'second',planId:second.planId});r.succeed(second);r.files.set(second.jobs[0].id,{sha256:'invalid'});
  await r.goals.check(r.session);assert.equal(r.session.goal.status,'needs_attention');assert.equal(r.session.goal.outputs[1].state,'failed');
  r.files.set(second.jobs[0].id,{jobId:second.jobs[0].id,sha256:'b'.repeat(64)});await r.goals.check(r.session);
  assert.equal(r.session.goal.status,'complete');assert.equal(r.session.goal.engine.status,'complete');assert(validGoal(r.session.goal));
  assert(r.calls.filter(call=>call.name).every(call=>call.scope.readOnly===true||call.name==='geod_plan_status'));
});
test('reviews, intermediate plans, unfinished settlement and failed tasks preserve honest waiting states',async()=>{
  const r=rig();await define(r);const intermediate=r.addPlan('project');r.session.goal.planIds.push(intermediate.planId);
  await r.goals.check(r.session);assert.equal(r.session.goal.status,'waiting_confirmation');
  intermediate.status='submitted';intermediate.jobs=[];
  const plan=r.addPlan();await r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId});
  await r.goals.check(r.session);assert.equal(r.session.goal.status,'waiting_confirmation');
  r.succeed(plan);plan.jobs[0].settled=false;await r.goals.check(r.session);assert.equal(r.session.goal.status,'waiting_jobs');
  plan.jobs[0].settled=true;plan.jobs[0].status='failed';await r.goals.check(r.session);assert.equal(r.session.goal.status,'needs_attention');
  assert(!r.calls.some(call=>call.name==='geod_plan_execute'||call.name==='geod_job_control'));
});
test('unknown, superseded and mismatched plans cannot be bound as successful deliveries',async()=>{
  const r=rig();await define(r);await assert.rejects(r.goals.bind(r.active,{outputId:'delivery',planId:randomUUID()}),/recorded/);
  const plan=r.addPlan('project');await assert.rejects(r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId}),/matching/);
  plan.kind='download';plan.replacedBy=randomUUID();await assert.rejects(r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId}),/superseded/);
});
test('incomplete or unverified area reviews need a replacement instead of prompting confirmation',async()=>{
  for(const status of ['partial','unknown']) {
    const r=rig();await define(r);const plan=r.addPlan();plan.areaCoverage={status};
    r.session.goal.planIds.push(plan.planId);await r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId});
    await r.goals.check(r.session);assert.equal(r.session.goal.status,'needs_attention');
    assert.equal(r.session.goal.outputs[0].state,'failed');assert.match(r.session.goal.reason,/complete area coverage/);
    assert(validGoal(r.session.goal));
  }
});
test('vector, project and exhausted metadata outputs use their actual native contracts',async()=>{
  const r=rig();await define(r,[{id:'vector',label:'Requested vector',kind:'vector'},{id:'project',label:'Saved project',kind:'project'},{id:'catalog',label:'Complete requested metadata',kind:'metadata'}]);
  const vector=r.addPlan('vector');vector.status='submitted';vector.jobs=[];vector.vector={id:randomUUID(),verified:true};r.files.set(vector.vector.id,{verified:true,asset:{id:vector.vector.id,geojsonSha256:'c'.repeat(64)}});
  const project=r.addPlan('project');project.status='submitted';project.jobs=[];project.project.saved=true;r.files.set(project.project.id,{id:project.project.id});
  await r.goals.bind(r.active,{outputId:'vector',planId:vector.planId});await r.goals.bind(r.active,{outputId:'project',planId:project.planId});
  const entry={id:randomUUID(),type:'tool',status:'completed',metadataComplete:false};r.session.entries.push(entry);
  await assert.rejects(r.goals.bind(r.active,{outputId:'catalog',entryId:entry.id}),/exhausted/);entry.metadataComplete=true;
  await r.goals.bind(r.active,{outputId:'catalog',entryId:entry.id});await r.goals.check(r.session);assert.equal(r.session.goal.status,'complete');
  assert(!r.calls.some(call=>call.name==='geod_raster_inspect'));
});
test('stopping, no progress and persisted validation cannot manufacture completion',async()=>{
  const r=rig();await define(r);r.active.goalProgress=false;await r.goals.afterTurn(r.active,'completed');assert.equal(r.session.goal.status,'needs_attention');
  r.session.goal.status='active';await r.goals.afterTurn(r.active,'interrupted');assert.equal(r.session.goal.status,'paused');assert.equal(r.session.goal.engine.status,'paused');
  assert(!validGoal({...r.session.goal,status:'complete'}));assert(!validGoal({...r.session.goal,continuations:9}));
  assert(!validGoalInput({objective:'Test',outputs:[{id:'same',label:'A',kind:'raster'},{id:'same',label:'B',kind:'raster'}]}));
});
test('a pause during an in-flight file audit remains paused and cannot be turned into completion by the late result',async()=>{
  const r=rig();await define(r);const plan=r.addPlan();r.succeed(plan);await r.goals.bind(r.active,{outputId:'delivery',planId:plan.planId});
  const native=r.service.callTool;let release,entered;
  const waiting=new Promise(resolve=>{entered=resolve;});r.service.callTool=async(...args)=>{if(args[0]==='geod_raster_inspect'){entered();await new Promise(resolve=>{release=resolve;});}return native(...args);};
  const check=r.goals.check(r.session);await waiting;await r.goals.pause(r.session);release();await check;
  assert.equal(r.session.goal.status,'paused');assert.equal(r.session.goal.engine.status,'paused');assert(validGoal(r.session.goal));
});
