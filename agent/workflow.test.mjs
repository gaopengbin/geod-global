import test from 'node:test';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {WorkflowMonitor, validWorkflow, publicWorkflow, WORKFLOW_LIMITS} from './workflow.mjs';

function rig() {
  const session={id:randomUUID()},plan={planId:randomUUID(),kind:'download',status:'submitted',jobs:[{id:randomUUID(),status:'queued',settled:false,bytesDownloaded:0}]};
  const resumed=[],notices=[],reads=[];
  const service={config:{},sessions:[session],selectedId:session.id,compatible:()=>true,save:async()=>{},desktopNotice:async(_,text)=>notices.push(text),
    callTool:async(name)=>{reads.push(name);return name==='geod_execution_policy'?{mode:'full-access'}:structuredClone(plan);},
    continueWorkflow:async(...args)=>{resumed.push(args);service.active={};}};
  const monitor=new WorkflowMonitor(service,{intervalMs:60_000});
  return {session,plan,service,monitor,resumed,notices,reads};
}
test('native observer waits for settled tasks, resumes the same conversation once, and does not poll the model',async t=>{
  const r=rig();t.after(()=>r.monitor.close());await r.monitor.register(r.session,r.plan,null);
  await r.monitor.tick();await r.monitor.tick();assert.equal(r.resumed.length,0);
  assert.deepEqual(r.reads,['geod_plan_status','geod_plan_status']);
  r.plan.jobs[0].status='succeeded';await r.monitor.tick();assert.equal(r.resumed.length,0);
  r.plan.jobs[0].settled=true;await r.monitor.tick();assert.equal(r.resumed.length,1);
  assert.equal(r.resumed[0][0],r.session);assert.equal(r.resumed[0][2],'full-access');
  assert.equal(r.resumed[0][1][0].jobs[0].settled,true);assert(validWorkflow(r.session.workflow));
  await r.monitor.tick();assert.equal(r.resumed.length,1);
  r.service.active=null;await r.monitor.turnFinished(r.session,'completed');assert.equal(r.session.workflow.status,'completed');
  assert.deepEqual(Object.keys(publicWorkflow(r.session.workflow)),['status','waitingPlans','continuations','updatedAt']);
});
test('pause, connection change, selection change, and a failed continuation preserve native receipts',async t=>{
  const r=rig();t.after(()=>r.monitor.close());await r.monitor.register(r.session,r.plan,null);
  r.plan.jobs[0]={...r.plan.jobs[0],status:'succeeded',settled:true};
  await r.monitor.pause(r.session);await r.monitor.tick();assert.equal(r.reads.length,0);
  await r.monitor.resume(r.session);r.service.compatible=()=>false;await r.monitor.tick();assert.equal(r.reads.length,0);
  r.service.compatible=()=>true;r.service.selectedId=randomUUID();await r.monitor.tick();assert.equal(r.resumed.length,0);assert.equal(r.session.workflow.receipts.length,1);
  r.service.selectedId=r.session.id;r.service.continueWorkflow=async()=>{throw Error('Owned host unavailable');};await r.monitor.tick();
  assert.equal(r.session.workflow.status,'paused');assert.equal(r.session.workflow.receipts.length,1);assert.equal(r.session.workflow.continuations,0);assert(validWorkflow(r.session.workflow));
  await r.monitor.tick();assert.equal(r.session.workflow.receipts.length,1);
});
test('failures wake a status turn with failure receipts; explicit resume renews the finite allowance',async t=>{
  const r=rig();t.after(()=>r.monitor.close());await r.monitor.register(r.session,r.plan,null);
  r.plan.jobs[0]={...r.plan.jobs[0],status:'failed',settled:true};r.session.workflow.continuations=WORKFLOW_LIMITS.continuations;
  await r.monitor.tick();assert.equal(r.resumed.length,0);assert.equal(r.session.workflow.status,'paused');
  await r.monitor.resume(r.session);await r.monitor.tick();assert.equal(r.resumed.length,1);
  assert.equal(r.resumed[0][1][0].hasFailure,true);assert.equal(r.session.workflow.status,'failed');
  await r.monitor.turnFinished(r.session,'completed');assert.equal(r.session.workflow.status,'failed');
});
test('history validation rejects forged receipts, unbounded plans and private metadata',async t=>{
  const r=rig();t.after(()=>r.monitor.close());await r.monitor.register(r.session,r.plan,null);const good=structuredClone(r.session.workflow);
  for(const change of [{pendingPlans:[...good.pendingPlans,...good.pendingPlans]},{continuations:9},{receipts:[{planId:r.plan.planId,status:'settled',jobs:[]}]},{context:{password:'private'}},{secret:'private'}])assert(!validWorkflow({...good,...change}));
  assert(validWorkflow(good));
});
