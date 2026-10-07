import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID, createHash } from 'node:crypto';
import { AgentService } from './service.mjs';
import { BRIDGE_MODEL } from './protocol.mjs';
import { TASK_PLANNING_INSTRUCTIONS } from './task-categories.mjs';
import { DECISION_TOOL } from './decisions.mjs';
import { GOAL_TOOLS } from './goals.mjs';
const config = { label:'Test', protocol:'openai-compatible', baseUrl:'https://example.com/v1', model:'test-model', apiKey:'private-test-key' };
const definitions = [{ name:'geod_projects_list', description:'Read projects', inputSchema:{ type:'object', properties:{}, additionalProperties:false } }];
function rig(home, additional = {}) {
  const calls = [], threadId = randomUUID(), turnId = randomUUID(); let event;
  const service = new AgentService({ home, executable:'unit-test-only', callTool:async () => ({ projects:[{ id:randomUUID(), name:'Saved project' }], total:1 }),
    bridgeFactory:async () => ({ baseUrl:'http://127.0.0.1:1/v1', resetBudget(){}, async close(){calls.push('bridge-close');} }),
    hostFactory:async options => {
      event = options.onEvent;
      return { async rpc(method, params) { calls.push([method, params]); if (method === 'thread/start') return { thread:{id:threadId} }; if (method === 'turn/start') return { turn:{id:turnId} }; if (method === 'turn/interrupt') event({method:'turn/completed',params:{threadId:service.active.session.threadId,turn:{status:'interrupted'}}}); return {}; }, async close(){calls.push('host-close');} };
    }, ...additional });
  return { service, calls, threadId, turnId, event(value){event(value);} };
}
const tick = () => new Promise(resolve => setImmediate(resolve));
test('legacy unanswered missing-boundary cards retire on matching native geometry and remain persisted without granting approval',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-legacy-boundary-')),reference={id:randomUUID(),sha256:'a'.repeat(64)},bounds=[-74.258843,40.476578,-73.700169,40.917705];let nativeReads=0;
  const r=rig(home,{callTool:async(name)=>{
    if(name==='geod_boundary_read'){nativeReads++;return {boundary:reference,bounds,geometryType:'MultiPolygon'};}
    if(name==='geod_execution_policy')return {mode:'confirm-each',modelCanChangePermission:false};
    return {planId:randomUUID(),planHash:'b'.repeat(64),kind:'project',status:'pending'};
  }});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,['geod_boundary_read','geod_project_plan','geod_execution_policy'].map(name=>({name,inputSchema:{type:'object'}})));
  await r.service.send({text:'要按纽约市区范围裁剪出来'});await ready(r.service);
  const invoke=(tool,args={})=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  const choice=await invoke('geod_request_decision',{title:'纽约市裁剪范围',questions:[{id:'crop_area',prompt:'没有经核验的行政区多边形，请选择裁剪方式。',options:[
    {id:'rectangle',label:'按外接矩形裁剪',description:'西 -74.258843，南 40.476578，东 -73.700233，北 40.91763。'},
    {id:'boundary',label:'提供/选择多边形边界',description:'使用真实边界。'}]}]});
  await assert.rejects(invoke('geod_project_plan',{boundary:reference}),/pending decision/);
  assert.equal(r.service.active.session.entries.at(-1).failureCode,'pending-decision');
  assert.equal((await invoke('geod_execution_policy')).pendingDecisionCount,1);
  await r.service.finish('completed');await r.service.send({sessionId:r.service.selectedId,text:'再试试'});await ready(r.service);
  const result=await invoke('geod_boundary_read',{provider:'census',candidateId:'city',lookupId:randomUUID()});
  assert.deepEqual(result.resolvedDecisions,[choice.decision.id]);assert.equal((await invoke('geod_execution_policy')).pendingDecisionCount,0);
  await invoke('geod_project_plan',{boundary:reference});
  const decision=r.service.active.session.entries.find(e=>e.decision)?.decision;
  assert.equal(decision.status,'superseded');assert.equal(decision.answers,undefined);assert.equal(r.service.active.session.executionMode,'confirm-each');
  const stored=JSON.parse(await readFile(join(home,'sessions.json'),'utf8'));
  assert.equal(stored.sessions[0].entries.find(e=>e.decision).decision.status,'superseded');assert.equal(nativeReads,1);
});
test('duplicate stable reads reuse original turn receipts without consuming native budget, while permission and changed requests stay fresh',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-turn-reads-'));let nativeReads=0;
  const r=rig(home,{callTool:async()=>({checkedAt:new Date().toISOString(),total:++nativeReads})});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,['geod_workspace_context','geod_execution_policy'].map(name=>({name,inputSchema:{type:'object'}})));
  await r.service.send({text:'Read current scope'});await ready(r.service);
  const invoke=(tool,args={})=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  const first=await invoke('geod_workspace_context');
  for(let i=0;i<24;i++){
    const reused=await invoke('geod_workspace_context');assert.equal(reused.total,first.total);assert(reused.turnReadReuse.entryId);
  }
  assert.equal(nativeReads,1);assert.equal(r.service.active.tools,1);
  await invoke('geod_execution_policy');await invoke('geod_execution_policy');assert.equal(nativeReads,3);
  await r.service.finish('completed');await r.service.send({sessionId:r.service.selectedId,text:'Read again'});await ready(r.service);
  await invoke('geod_workspace_context');assert.equal(nativeReads,4);
  r.service.active.tools=20;await assert.rejects(invoke('geod_execution_policy'),/turn operation budget reached/);
});
test('actual native boundary reads unlock a polygon project review and do not invent an upload choice or execute it',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-source-boundary-'));const native=[];
  const reference={id:randomUUID(),sha256:'a'.repeat(64)};
  const r=rig(home,{callTool:async(name,args)=>{
    native.push({name,args});
    if(name==='geod_boundary_read')return {boundary:reference,name:'Actual source fixture',bounds:[0,0,1,1],provenance:{fixture:true}};
    return {planId:randomUUID(),planHash:'b'.repeat(64),kind:'project',status:'pending'};
  }});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,[{name:'geod_boundary_read',inputSchema:{type:'object'}},{name:'geod_project_plan',inputSchema:{type:'object'}}]);
  await r.service.send({text:'按市行政区范围裁剪卫星影像'});await ready(r.service);
  const invoke=(tool,args)=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  await assert.rejects(invoke('geod_project_plan',{boundary:reference}),/Choose the crop area/);
  assert.equal(native.length,0);
  await invoke('geod_boundary_read',{provider:'natural-earth',candidateId:'source-fixture',countryCode:'DEU',adminLevel:1});
  await invoke('geod_project_plan',{boundary:reference});
  assert.deepEqual(native.map(c=>c.name),['geod_boundary_read','geod_project_plan']);
  assert.deepEqual(native[1].args.boundary,reference);
  assert.ok(!r.service.snapshot().selected.entries.some(e=>e.decision));
  assert.ok(r.service.snapshot().selected.entries.some(e=>e.name==='geod_project_plan'&&e.status==='completed'));
});

test('retry keeps the latest processing requirement while retaining the visible retry message',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-retry-request-'));const r=rig(home);
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,definitions);
  await r.service.send({text:'按纽约市区范围裁剪出来'});await ready(r.service);await r.service.finish('completed');
  await r.service.send({sessionId:r.service.selectedId,text:'重试'});await ready(r.service);
  assert.equal(r.service.active.humanText,'按纽约市区范围裁剪出来');
  assert.equal(r.service.snapshot().selected.entries.at(-1).text,'重试');
});

test('crop accuracy waits for a real option answer before any project preflight and does not approve downloads',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-crop-choice-'));let nativeCalls=0;
  const r=rig(home,{callTool:async()=>{nativeCalls++;return {planId:randomUUID(),kind:'project',status:'pending'};}});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,[{name:'geod_project_plan',inputSchema:{type:'object'}}]);
  await r.service.send({text:'按行政区范围裁剪哨兵影像'});await ready(r.service);
  const invoke=(tool,args)=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  await assert.rejects(invoke('geod_project_plan',{}),/Choose the crop area/);assert.equal(nativeCalls,0);
  assert.equal(r.service.snapshot().selected.entries.at(-1).failureCode,'crop-area');
  const choice=await invoke('geod_request_decision',{title:'选择裁剪范围',questions:[{id:'crop_area',prompt:'采用哪种裁剪范围？',options:[{id:'rectangle',label:'外接矩形',description:'按解析的范围裁剪。'},{id:'boundary',label:'实际行政区边界',description:'需要真实边界多边形。'}]}]});
  await assert.rejects(invoke('geod_project_plan',{}),/pending decision card/);assert.equal(nativeCalls,0);
  await r.service.finish('completed');
  await r.service.send({sessionId:r.service.selectedId,text:'',decisionAnswer:{decisionId:choice.decision.id,answers:[{questionId:'crop_area',optionId:'rectangle'}]}});await ready(r.service);
  const review=await invoke('geod_project_plan',{});assert.equal(review.status,'pending');assert.equal(nativeCalls,1);
  assert(!r.service.snapshot().selected.entries.some(entry=>entry.name==='geod_plan_execute'));
});

test('general task policy reaches fresh, resumed and compacted conversations without changing native permissions or tools', async t => {
  const home = await mkdtemp(join(tmpdir(), 'geod-agent-task-classes-'));
  const r = rig(home);
  t.after(async () => { await r.service.close(); await rm(home, { recursive: true, force: true }); });
  await r.service.open(); await r.service.configure(config, definitions);
  await r.service.send({ text: 'Prepare data for my research.' }); await ready(r.service);
  const started = r.calls.find(call => Array.isArray(call) && call[0] === 'thread/start')[1];
  assert(started.developerInstructions.includes(TASK_PLANNING_INSTRUCTIONS));
  assert.deepEqual(started.dynamicTools, [...definitions, DECISION_TOOL, ...GOAL_TOOLS]);
  assert.equal(started.sandbox, 'read-only');
  await r.service.finish('completed');
  const sessionId = r.service.selectedId;
  await r.service.send({ sessionId, text: 'Keep the same required outputs.' }); await ready(r.service);
  const resumed = r.calls.filter(call => Array.isArray(call) && call[0] === 'thread/resume').at(-1)[1];
  assert(resumed.developerInstructions.includes(TASK_PLANNING_INSTRUCTIONS));
  assert(resumed.developerInstructions.includes('Native conversation permission is authoritative'));
  assert.equal(resumed.threadId, r.threadId);
  await r.service.finish('completed');
  await r.service.compact({ sessionId }); await r.service.turnTask;
  const compacted = r.calls.filter(call => Array.isArray(call) && call[0] === 'thread/resume').at(-1)[1];
  assert(compacted.developerInstructions.includes(TASK_PLANNING_INSTRUCTIONS));
  assert.equal(compacted.sandbox, 'read-only');
  assert(r.calls.some(call => Array.isArray(call) && call[0] === 'thread/compact/start'));
  await r.service.finish('completed');
});
test('a failed city lookup cannot reach the native catalog using state or current map bounds',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-city-scope-'));
  const calls=[],city=[-74.258843,40.476578,-73.700233,40.91763];let cityReady=false;
  const r=rig(home,{callTool:async(name,args)=>{
    calls.push({name,args});
    if(name==='geod_place_search'){
      if(!cityReady)throw Error('Place lookup is temporarily unreachable.');
      return {candidates:[{kind:'city',bounds:city}]};
    }
    return {candidates:[{kind:'region',level:'ADM1',bounds:[-79.763,40.499,-71.857,45.010]}]};
  }});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,['geod_place_search','geod_region_search','geod_scene_search'].map(name=>({name,inputSchema:{type:'object'}})));
  await r.service.send({text:'Latest imagery of New York city'});await ready(r.service);
  const call=(tool,args)=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  await assert.rejects(call('geod_place_search',{query:'New York',kind:'city',countryCode:'US'}),/unreachable/);
  await call('geod_region_search',{query:'New York',countryCode:'US'});
  for(const extent of [[-79.763,40.499,-71.857,45.010],[-122.55,37.68,-122.32,37.84]])await assert.rejects(call('geod_scene_search',{bounds:extent}),/Resolve the requested city/);
  assert.equal(calls.filter(c=>c.name==='geod_scene_search').length,0);
  assert.equal(r.service.snapshot().selected.entries.find(e=>e.name==='geod_place_search').failureCode,'source-network');
  cityReady=true;await call('geod_place_search',{query:'New York',kind:'city',countryCode:'US'});
  await call('geod_scene_search',{bounds:city});
  assert.equal(calls.filter(c=>c.name==='geod_scene_search').length,1);
});
test('native query storage failure persists a safe reason and stops repeated geographic requests',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-storage-scope-'));let count=0;
  const r=rig(home,{callTool:async()=>{count++;throw Error('Agent record directory was redirected.');}});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,['geod_place_search','geod_region_levels','geod_scene_search'].map(name=>({name,inputSchema:{type:'object'}})));
  await r.service.send({text:'Find a city'});await ready(r.service);
  const call=(tool,args={})=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  await assert.rejects(call('geod_place_search',{query:'New York',kind:'city'}),/record directory/);
  for(const name of ['geod_region_levels','geod_place_search','geod_scene_search'])await assert.rejects(call(name),/Stop geographic retries/);
  assert.equal(count,1);await r.service.finish('completed');await r.service.close();
  const saved=JSON.parse(await readFile(join(home,'sessions.json'),'utf8'));
  const entry=saved.sessions[0].entries.find(e=>e.type==='tool');
  assert.equal(entry.status,'failed');assert.equal(entry.failureCode,'record-storage');
  assert(!JSON.stringify(saved).includes('record directory was redirected'));
});
test('explicit resume recovers an interrupted status turn without inventing a new completion, and clearing selection prevents conversation theft',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-explicit-resume-'));const r=rig(home);
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Check my task'});await ready(r.service);
  await assert.rejects(r.service.select(null),/finish/);await r.service.finish('completed');
  const session=r.service.sessions[0],binding=randomUUID();
  session.workflow={version:1,status:'paused',pendingPlans:[],receipts:[],context:null,continuations:1,updatedAt:new Date().toISOString()};
  await r.service.recordControl({sessionId:session.id,text:'继续任务',action:'resume',executionMode:'confirm-each',executionBinding:binding});await ready(r.service);
  assert.equal(r.service.active.session.id,session.id);assert.equal(r.service.active.requestId,null);
  const input=r.calls.filter(call=>Array.isArray(call)&&call[0]==='turn/start').at(-1)[1].input;
  assert(input.some(part=>part.text?.includes('No new completion receipt is available')));
  assert.equal(session.entries.at(-1).type,'system');assert.equal(session.entries.at(-1).text,'Resuming your request.');
  await r.service.finish('completed');await r.service.select(null);assert.equal(r.service.snapshot().selected,null);
});
test('project task summaries retain bounded native page states, never addresses or raw records',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-project-status-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const projectId=randomUUID(),checkedAt='2026-10-05T00:00:00Z';
  const r=rig(home,{callTool:async()=>({project:{id:projectId,name:'Private project'},checkedAt,total:7,
    jobs:[{status:'succeeded',settled:true,href:'https://example.test/private?token=secret'},
      {status:'failed',settled:true,error:'private failure'},{status:'running',settled:false}]})});
  await r.service.open();await r.service.configure(config,[{name:'geod_jobs_list',inputSchema:{type:'object'}}]);
  await r.service.send({text:'Check this project'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_jobs_list',arguments:{projectId}});
  const summary=r.service.snapshot().selected.entries.at(-1).summary;
  assert.deepEqual(summary,{kind:'jobs',count:3,total:7,projectId,checkedAt:'2026-10-05T00:00:00.000Z',
    pageStatuses:{succeeded:1,failed:1,running:1},pageSettledCount:2});
  for(const privateValue of ['example.test','secret','Private project','private failure']) assert(!JSON.stringify(summary).includes(privateValue));
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();await r.service.close();
});
test('orchestrator text route is independent of the selected upstream model and history identity',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-route-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const connection={id:randomUUID(),provider:'openai'},upstream={...config,model:'gpt-5.6-terra',connection};
  const r=rig(home);await r.service.open();await r.service.configure(upstream,definitions);
  await r.service.send({text:'Controlled route isolation test'});await ready(r.service);
  assert.equal(r.calls.find(([method])=>method==='thread/start')[1].model,BRIDGE_MODEL);
  const instructions=r.calls.find(([method])=>method==='thread/start')[1].developerInstructions;
  assert(instructions.includes('provided read-only tools'));
  assert(instructions.includes('geod_jobs_list or geod_job_status'));
  assert(instructions.includes('pass the actual projectId to geod_jobs_list'));
  assert(instructions.includes('Native conversation permission is authoritative'));
  assert(instructions.includes('geod_place_search with kind=city'));
  assert(instructions.includes('geod_workspace_context.searchDefaults'));
  assert(instructions.includes('cloudMax=100'));
  assert(instructions.includes('Before every area review call geod_scene_coverage'));
  assert(instructions.includes('download-review preparation can be retried without an existing queued task'));
  assert(instructions.includes('A retry does not by itself confirm or execute a new plan'));
  assert(instructions.includes('never claim text confirmation is unsupported or require a card click'));
  const base=r.calls.find(([method])=>method==='thread/start')[1].baseInstructions;
  assert(!base.includes('Ask for missing region/time/source/output'));
  assert(base.includes('A place explicitly named by the human overrides an unrelated map area'));
  assert.equal(r.service.snapshot().model.model,upstream.model);
  assert.equal(r.service.snapshot().selected.modelId,upstream.model);
  assert.equal(r.service.snapshot().selected.modelConnectionId,connection.id);
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();await r.service.close();
});
test('vector extraction reviews persist as pending and register typed results only after native confirmation',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-vector-review-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const planId=randomUUID(),vectorId=randomUUID(),hash='a'.repeat(64);
  let submitted=false;
  const tools=[{name:'geod_vector_extract_plan',inputSchema:{type:'object'}},{name:'geod_plan_status',inputSchema:{type:'object'}}];
  const r=rig(home,{callTool:async()=>({planId,kind:'vector',status:submitted?'submitted':'pending',jobs:[],vector:submitted?{id:vectorId,name:'Actual vector',geojsonSha256:hash,verified:true}:null})});
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Prepare extraction'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_vector_extract_plan',arguments:{}});
  assert.deepEqual(r.service.snapshot().selected.entries.at(-1).references,[{kind:'plan',id:planId,label:'Vector extraction'}]);
  submitted=true; // Native callback result fixture, not a model execution tool.
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_plan_status',arguments:{planId}});
  const entry=r.service.snapshot().selected.entries.at(-1);
  assert(entry.references.some(ref=>ref.kind==='vector'&&ref.id===vectorId));
  assert(!entry.references.some(ref=>ref.kind==='job'));
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();await r.service.close();
  const reopened=rig(home);await reopened.service.open();
  assert.deepEqual(reopened.service.snapshot().selected.entries.at(-1).references,entry.references);await reopened.service.close();
});
test('vector reads persist typed file links and verification summaries without raster job claims',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-vector-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const id=randomUUID(),hash='a'.repeat(64);const tools=[{name:'geod_vector_inspect',inputSchema:{type:'object'}}];
  const r=rig(home,{callTool:async()=>({verified:true,asset:{id,name:'Actual vector',geojsonSha256:hash,featureCount:25}})});
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Inspect vector'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_vector_inspect',arguments:{id}});
  const entry=r.service.snapshot().selected.entries.at(-1);
  assert.deepEqual(entry.references,[{kind:'vector',id,label:'Actual vector'}]);
  assert.deepEqual(entry.summary,{kind:'vector',verified:true,count:25,sha256:hash});
  assert(!JSON.stringify(entry).includes('succeeded'));
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();await r.service.close();
  const reopened=rig(home);await reopened.service.open();
  assert.deepEqual(reopened.service.snapshot().selected.entries.at(-1).references,entry.references);await reopened.service.close();
});
test('human revisions bind to a completed selected native reference, persist once and never become model tools',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-human-review-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const old=randomUUID(),next=randomUUID();const tools=[{name:'geod_download_plan',inputSchema:{type:'object'}}];
  const r=rig(home,{callTool:async()=>({planId:old,kind:'download',status:'pending'})});
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Prepare'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_download_plan',arguments:{}});
  const sessionId=r.service.selectedId,revision={sessionId,originalPlanId:old,planId:next,kind:'download'};
  await assert.rejects(r.service.recordPlanRevision(revision));
  await assert.rejects(r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'recordPlanRevision',arguments:revision}));
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();
  for(const invalid of [{...revision,sessionId:randomUUID()},{...revision,originalPlanId:randomUUID()},{...revision,planId:old},{...revision,kind:'execute'}])await assert.rejects(r.service.recordPlanRevision(invalid));
  await r.service.recordPlanRevision(revision);await r.service.recordPlanRevision(revision);
  const entries=r.service.snapshot().selected.entries;assert.equal(entries.filter(entry=>entry.summary?.revisionOf===old).length,1);
  assert(entries.some(entry=>entry.references?.some(ref=>ref.id===old)));assert(entries.some(entry=>entry.references?.some(ref=>ref.id===next)));
  await r.service.send({sessionId,text:'Check the corrected plan'});await ready(r.service);
  const input=r.calls.filter(call=>Array.isArray(call)&&call[0]==='turn/start').at(-1)[1].input;
  assert(input[0].text.includes(old)&&input[0].text.includes(next));assert(input[0].text.includes('separate native confirmation'));
  await r.service.close();const reopened=rig(home);await reopened.service.open();
  assert.equal(reopened.service.snapshot().selected.entries.filter(entry=>entry.summary?.revisionOf===old).length,1);await reopened.service.close();
});
test('registry connections isolate identical endpoint/model identities and recover their own history', async t => {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-registry-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const first={id:randomUUID(),provider:'custom'}, second={id:randomUUID(),provider:'custom'};
  const r=rig(home);await r.service.open();await r.service.configure({...config,connection:first},definitions);
  await r.service.send({text:'First connection'});await ready(r.service);const sessionId=r.service.selectedId;
  for(const connection of [first,second])await assert.rejects(r.service.configure({...config,connection},definitions));
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();
  await r.service.configure({...config,connection:second},definitions);
  assert.equal(r.service.snapshot().sessions[0].compatible,false);
  await assert.rejects(r.service.send({sessionId,text:'Wrong account'}));
  await r.service.configure({...config,connection:first,label:'Renamed'},definitions);
  assert.equal(r.service.snapshot().sessions[0].compatible,true);
  await r.service.send({sessionId,text:'Resume first'});await ready(r.service);
  assert(r.calls.some(call=>Array.isArray(call)&&call[0]==='thread/resume'));
  await r.service.close();
  const saved=JSON.parse(await readFile(join(home,'sessions.json'),'utf8'));
  assert.equal(saved.sessions[0].modelConnectionId,first.id);assert.equal(saved.sessions[0].modelProvider,'custom');
  assert(!JSON.stringify(saved).includes(config.apiKey));
});

test('tool upgrades preserve the app conversation, native references and permissions, restoring context only once', async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-tools-upgrade-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const connection={id:randomUUID(),provider:'custom'},binding=randomUUID(),planId=randomUUID();
  const oldTools=[{name:'geod_health',inputSchema:{type:'object'}}];
  const newTools=[...oldTools,{name:'geod_place_search',inputSchema:{type:'object',properties:{query:{type:'string'}}}}];
  let r=rig(home);await r.service.open();await r.service.configure({...config,connection},oldTools);
  await r.service.send({text:'Remember the requested New York imagery.',executionMode:'full-access',executionBinding:binding});await ready(r.service);
  const id=r.service.selectedId,oldThread=r.service.sessions[0].threadId;
  await r.service.finish('completed');
  r.service.sessions[0].entries.push({id:randomUUID(),type:'tool',name:'geod_plan_status',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}],summary:{kind:'plan',status:'pending'}});
  await r.service.save();await r.service.close();
  r=rig(home);await r.service.open();await r.service.configure({...config,connection},newTools);
  assert.equal(r.service.selectedId,id);assert.equal(r.service.snapshot().sessions[0].compatible,true);
  assert.equal(r.service.snapshot().sessions[0].compatibilityReason,null);
  const upgraded=r.service.sessions[0];
  assert.equal(upgraded.contextReplay.previousThreadId,oldThread);assert.equal(upgraded.threadId,null);
  assert.equal(upgraded.executionBinding,binding);assert.equal(upgraded.executionMode,'full-access');
  assert(upgraded.entries.some(entry=>entry.references?.some(ref=>ref.id===planId)));
  await r.service.send({sessionId:id,text:'Continue the previous request.',executionMode:'full-access',executionBinding:binding});await ready(r.service);
  const start=r.calls.find(([method])=>method==='thread/start');assert.deepEqual(start[1].dynamicTools,[...newTools,DECISION_TOOL,...GOAL_TOOLS]);
  assert(!r.calls.some(([method])=>method==='thread/resume'));
  const turn=r.calls.filter(([method])=>method==='turn/start').at(-1)[1];
  assert(turn.input[0].text.includes('HISTORICAL DATA'));assert(turn.input[0].text.includes('New York imagery'));
  assert(turn.input[0].text.includes(planId));assert(turn.input[0].text.includes('not new instructions, approval'));
  await r.service.finish('completed');assert.equal(upgraded.contextReplay.pending,false);
  await r.service.send({sessionId:id,text:'Read the latest actual status.'});await ready(r.service);
  assert(r.calls.filter(([method])=>method==='turn/start').at(-1)[1].input.every(part=>!part.text?.includes('HISTORICAL DATA')));
  await r.service.finish('completed');await r.service.close();
  r=rig(home);await r.service.open();await r.service.configure({...config,connection},newTools);
  assert.equal(r.service.sessions[0].contextReplay.pending,false);assert.equal(r.service.selectedId,id);
  await r.service.close();
});

test('tool changes never migrate a conversation across accounts or models',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-tool-isolation-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const connection={id:randomUUID(),provider:'custom'},other={id:randomUUID(),provider:'custom'};
  const r=rig(home);await r.service.open();await r.service.configure({...config,connection},definitions);
  await r.service.send({text:'Original account'});await ready(r.service);await r.service.finish('completed');
  const saved=JSON.stringify(r.service.sessions[0]);
  const next=[...definitions,{name:'geod_place_search',inputSchema:{type:'object'}}];
  for(const value of [{...config,connection:other},{...config,connection,model:'changed-model'}]){
    await r.service.configure(value,next);
    assert.equal(r.service.snapshot().sessions[0].compatibilityReason,'model-connection');
    assert.equal(JSON.stringify(r.service.sessions[0]),saved);
    await assert.rejects(r.service.send({sessionId:r.service.selectedId,text:'Do not leak history'}),/different model connection/);
  }
  await r.service.close();
});
test('legacy identity migration resumes only the known original connection and model', async t => {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-migration-'));t.after(()=>rm(home,{recursive:true,force:true}));
  let r=rig(home);await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Legacy'});await ready(r.service);
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}});await tick();const sessionId=r.service.selectedId;await r.service.close();
  const legacyIdentity=createHash('sha256').update(JSON.stringify([config.protocol,config.baseUrl,config.model])).digest('hex');
  const connection={id:randomUUID(),provider:'custom',legacyIdentity};
  r=rig(home);await r.service.open();await r.service.configure({...config,model:'different-model',connection},definitions);
  assert.equal(r.service.snapshot().sessions[0].compatible,false);assert.equal(r.service.snapshot().selected.modelConnectionId,undefined);
  await r.service.configure({...config,connection},definitions);assert.equal(r.service.snapshot().sessions[0].compatible,true);
  assert.equal(r.service.snapshot().selected.modelConnectionId,connection.id);
  await r.service.send({sessionId,text:'Resume migrated'});await ready(r.service);
  assert(r.calls.some(call=>Array.isArray(call)&&call[0]==='thread/resume'));await r.service.close();
  await r.service.configure({...config,baseUrl:'https://changed.example/v1',connection},definitions);
  await assert.rejects(r.service.send({sessionId,text:'Wrong endpoint'}));await r.service.close();
});
test('scientific inspection exposes the verified artifact ID without pretending a plan is completed',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const id=randomUUID(),sha='a'.repeat(64);const r=rig(home,{callTool:async()=>({artifact:{jobId:id,sha256:sha},width:3,height:2})});
  const tools=[{name:'geod_rgb_inspect',inputSchema:{type:'object'}}];
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Inspect the actual RGB'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_rgb_inspect',arguments:{id}});
  const entry=r.service.snapshot().selected.entries.at(-1);
  assert.deepEqual(entry.references,[{kind:'job',id,label:'Scientific RGB'}]);assert.deepEqual(entry.summary,{kind:'file',sha256:sha});
  await r.service.close();
});
async function ready(service) { await service.turnTask; assert(service.active?.turnId); }
test('human choices wait, block dependent planning, persist real answers and remain separate from execution approval', async t => {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-decisions-')), planId=randomUUID(), native=[];
  const options={title:'Choose boundary',questions:[{id:'area',prompt:'Which area definition?',recommendedOptionId:'polygon',options:[{id:'polygon',label:'Actual boundary',description:'Use the saved polygon.'},{id:'bbox',label:'Rectangle',description:'Use its bounding box.'}]}]};
  let r=rig(home,{callTool:async(name,args)=>{native.push({name,args});return {planId,kind:'download',status:'pending'};}});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  const tools=[{name:'geod_download_plan',inputSchema:{type:'object'}}];
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Download data for my requested area.'});await ready(r.service);
  const call=(tool,args)=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:args});
  const requested=await call('geod_request_decision',options);assert.equal(requested.requiresAnswer,true);assert.equal(requested.decision.answers,undefined);
  await assert.rejects(call('geod_download_plan',{}),/pending decision/);assert.equal(native.length,0);
  await r.service.finish('completed');const sessionId=r.service.selectedId;
  await assert.rejects(r.service.recordPlanRevision({sessionId,originalPlanId:planId,planId:randomUUID(),kind:'download'}),/pending decision/);
  await assert.rejects(r.service.send({sessionId,text:'',decisionAnswer:{decisionId:requested.decision.id,answers:[{questionId:'area',optionId:'invented'}]}}),/actual options/);
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.decision).decision.status,'pending');
  await r.service.send({sessionId,text:'',decisionAnswer:{decisionId:requested.decision.id,answers:[{questionId:'area',optionId:'polygon'}]}});await ready(r.service);
  const session=r.service.snapshot().selected;
  assert.equal(session.entries.find(entry=>entry.decision).decision.status,'answered');assert.equal(session.executionMode,'confirm-each');
  assert.match(session.entries.at(-1).text,/not execution approval/);assert.match(session.entries.at(-1).text,/Actual boundary/);
  await call('geod_download_plan',{});const review=r.service.snapshot().selected.entries.at(-1).taskContext;
  assert.equal(review.requestText,'Download data for my requested area.');assert.equal(review.choices[0].answer,'Actual boundary');
  assert.equal(native.length,1);await r.service.finish('completed');
  await r.service.recordControl({sessionId,text:'确认执行',action:'confirm',plans:[{planId,kind:'download',status:'submitted',jobs:[]}],executionMode:'confirm-each',executionBinding:randomUUID()});
  assert.deepEqual(r.service.snapshot().selected.entries.findLast(entry=>entry.name==='geod_plan_execute').taskContext,review);
  await r.service.close();
  r=rig(home);await r.service.open();await r.service.configure(config,tools);
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.decision).decision.status,'answered');
  assert.deepEqual(r.service.snapshot().selected.entries.findLast(entry=>entry.name==='geod_plan_execute').taskContext,review);
  await assert.rejects(r.service.send({sessionId,text:'',decisionAnswer:{decisionId:requested.decision.id,answers:[{questionId:'area',optionId:'bbox'}]}}),/actual options/);
});
test('an oversized task summary cannot corrupt history or expose an incomplete confirmation card',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-review-limit-'));let r=rig(home,{callTool:async()=>({planId:randomUUID(),kind:'download',status:'pending'})});
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  await r.service.open();await r.service.configure(config,[{name:'geod_download_plan',inputSchema:{type:'object'}}]);
  await r.service.send({text:'Prepare my requested data.'});await ready(r.service);
  for(let i=0;i<31;i++)r.service.active.session.entries.push({id:randomUUID(),type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:randomUUID(),title:'Prior recorded preference',status:'answered',questions:[{id:'quality',prompt:'Which quality?',options:[{id:'good',label:'Good',description:'Strict quality.'},{id:'all',label:'All',description:'Keep all observations.'}]}],answers:[{questionId:'quality',optionId:'good'}]}});
  await assert.rejects(r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_download_plan',arguments:{}}),/too many decision answers/);
  const entry=r.service.snapshot().selected.entries.at(-1);assert.equal(entry.status,'failed');assert.equal(entry.taskContext,undefined);assert.deepEqual(entry.references,[]);
  await r.service.finish('completed');await r.service.close();r=rig(home);await r.service.open();
  assert.equal(r.service.snapshot().selected.entries.at(-1).status,'failed');
});
test('explicit native automatic execution skips choices while saving failure does not accept a human answer', async t => {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-decision-mode-'));const r=rig(home);
  t.after(async()=>{await r.service.close();await rm(home,{recursive:true,force:true});});
  const args={title:'Choose quality',questions:[{id:'quality',prompt:'Which policy?',options:[{id:'good',label:'Good',description:'Strict screening.'},{id:'usable',label:'Usable',description:'Keep more observations.'}]}]};
  await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Prepare data'});await ready(r.service);
  const call=()=>r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_request_decision',arguments:args});
  const value=await call();await r.service.finish('completed');
  const save=r.service.save.bind(r.service);r.service.save=async()=>{throw Error('Storage unavailable');};
  await assert.rejects(r.service.send({sessionId:r.service.selectedId,text:'',decisionAnswer:{decisionId:value.decision.id,answers:[{questionId:'quality',optionId:'good'}]}}),/Storage unavailable/);
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.decision).decision.status,'pending');r.service.save=save;
  await r.service.recordControl({sessionId:r.service.selectedId,text:'不用问我',action:'mode',executionMode:'full-access',executionBinding:randomUUID()});
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.decision).decision.status,'skipped');
  await r.service.send({sessionId:r.service.selectedId,text:'Continue',executionMode:'full-access'});await ready(r.service);
  assert.equal((await call()).requiresAnswer,false);assert.equal(r.service.snapshot().selected.entries.filter(entry=>entry.decision).length,1);
  await r.service.finish('completed');
});
test('source summaries retain native status dates without credentials, URLs or entitlement claims', async t => {
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const value={sources:Array.from({length:9},(_,i)=>({id:String(i)})),checkedAt:'2026-10-05T09:00:00+08:00',accountSources:[
    {id:'nasa-earthdata',authorization:{status:'saved',expiresAt:'2026-10-06T00:00:00Z',verifiedAt:'2026-10-05T00:00:00Z',token:'synthetic-hidden-source-token'},username:'private-user',href:'https://not-a-ui-link.test/'},
    {id:'copernicus',authorization:{status:'expired',expiresAt:'2026-10-01T00:00:00Z',verifiedAt:null},password:'synthetic-hidden-password'},
  ]};
  const r=rig(home,{callTool:async()=>value});await r.service.open();await r.service.configure(config,[{name:'geod_sources_list',inputSchema:{type:'object'}}]);
  await r.service.send({text:'Check data accounts'});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_sources_list',arguments:{}});
  const entry=r.service.snapshot().selected.entries.at(-1);
  assert.deepEqual(entry.summary,{kind:'sources',count:9,checkedAt:'2026-10-05T01:00:00.000Z',accounts:[
    {provider:'nasa-earthdata',status:'saved',expiresAt:'2026-10-06T00:00:00.000Z',verifiedAt:'2026-10-05T00:00:00.000Z'},
    {provider:'copernicus',status:'expired',expiresAt:'2026-10-01T00:00:00.000Z',verifiedAt:null},
  ]});
  assert.deepEqual(entry.references,[]);await r.service.close();
  const saved=await readFile(join(home,'sessions.json'),'utf8');
  for(const text of ['synthetic-hidden','private-user','not-a-ui-link','entitlement']) assert(!saved.includes(text));
});
test('real persisted sessions retain tool references and resume the same Codex thread', async t => {
  const home = await mkdtemp(join(tmpdir(),'geod-agent-')); t.after(()=>rm(home,{recursive:true,force:true}));
  let r = rig(home); await r.service.open(); await r.service.configure(config,definitions);
  const accepted = await r.service.send({text:'Read projects'}); assert(accepted.busy); await ready(r.service);
  const value = await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_projects_list',arguments:{}});
  r.event({method:'item/completed',params:{threadId:r.threadId,item:{id:'answer',type:'agentMessage',text:'Saved project'}}});
  r.event({method:'turn/completed',params:{threadId:r.threadId,turn:{status:'completed'}}}); await tick();
  assert.equal(r.service.snapshot().selected.entries[1].references[0].id,value.projects[0].id);
  const id = r.service.selectedId; await r.service.close();
  assert(!(await readFile(join(home,'sessions.json'),'utf8')).includes(config.apiKey));
  r = rig(home); await r.service.open(); await r.service.configure(config,definitions); await r.service.send({sessionId:id,text:'Read again'}); await ready(r.service);
  assert(r.calls.some(call=>Array.isArray(call)&&call[0]==='thread/resume'&&call[1].threadId!==null));
  assert(r.calls.find(call=>Array.isArray(call)&&call[0]==='thread/resume')[1].developerInstructions.includes('Only total=0 establishes that no task exists'));
  assert(!r.calls.some(call=>Array.isArray(call)&&call[0]==='thread/start'));
  await r.service.close();
});
test('stop freezes streamed text, denies late tools and changes only the Agent response',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const revisions=[];
  const r=rig(home,{onChange:revision=>revisions.push(revision)});await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Read'});await ready(r.service);
  const before=r.service.revision;
  r.event({method:'item/agentMessage/delta',params:{threadId:r.threadId,itemId:'partial',delta:'Before stop'}});
  assert.equal(revisions.at(-1),before+1);
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.id==='partial').status,'running');
  assert(!JSON.parse(await readFile(join(home,'sessions.json'),'utf8')).sessions[0].entries.some(entry=>entry.id==='partial'),'token notifications do not write a transcript for every delta');
  await r.service.interrupt();
  const stopped=revisions.length;
  r.event({method:'item/agentMessage/delta',params:{threadId:r.threadId,itemId:'partial',delta:'After stop'}});
  assert.equal(revisions.length,stopped,'stopped output does not notify the renderer');
  assert(revisions.every((revision,index)=>Number.isSafeInteger(revision) && (!index || revision>revisions[index-1])));
  assert.equal(r.service.snapshot().selected.status,'interrupted');
  assert.equal(r.service.snapshot().selected.entries.find(entry=>entry.id==='partial').text,'Before stop');
  await assert.rejects(r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_projects_list',arguments:{}}));
  assert(!r.calls.some(call=>Array.isArray(call)&&call[0]==='geod_job_cancel'));await r.service.close();
});
test('unknown/write tools, concurrent sends and model changes are refused',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  let count=0;const r=rig(home,{callTool:async()=>{count++;return {};}});await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Read'});await ready(r.service);
  await assert.rejects(r.service.send({text:'Second'}));await assert.rejects(r.service.configure(config,definitions));
  for(const tool of ['geod_download','shell_command','geod_job_cancel']) await assert.rejects(r.service.tool({threadId:r.threadId,turnId:r.turnId,tool,arguments:{}}));
  assert.equal(count,0);await r.service.close();
});
test('interrupted persisted turns recover; corrupt history is retained',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const id=randomUUID();await writeFile(join(home,'sessions.json'),JSON.stringify({version:1,selectedId:id,sessions:[{id,title:'Stopped',status:'running',entries:[{id:'tool',type:'tool',status:'running'}]}]}));
  const r=rig(home);await r.service.open();assert.equal(r.service.snapshot().selected.status,'interrupted');assert.equal(r.service.snapshot().selected.entries[0].status,'interrupted');await r.service.close();
  await writeFile(join(home,'sessions.json'),'broken');await assert.rejects(rig(home).service.open());assert.equal(await readFile(join(home,'sessions.json'),'utf8'),'broken');
});
test('closing during initialization closes the late-created runtime too',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  let release,closed=false;const gate=new Promise(resolve=>{release=resolve;});
  const r=rig(home,{hostFactory:async()=>{await gate;return {rpc:async()=>({}),close:async()=>{closed=true;}};}});
  await r.service.open();await r.service.configure(config,definitions);await r.service.send({text:'Start'});await tick();
  const shutdown=r.service.close();release();await shutdown;assert(closed);assert.equal(r.service.snapshot().selected.status,'interrupted');
});
test('native plan references are persisted and tool scope cannot be supplied by the model',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-agent-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const planId=randomUUID(),context={provider:'earth-search',bounds:[-122,37,-121,38]};let nativeScope;
  const r=rig(home,{callTool:async(name,args,scope)=>{nativeScope=scope;return {planId,kind:'download',status:'pending',jobs:[]};}});
  const tools=[...definitions,{name:'geod_download_plan',inputSchema:{type:'object'}}];
  await r.service.open();await r.service.configure(config,tools);await r.service.send({text:'Prepare a plan',context});await ready(r.service);
  await r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_download_plan',arguments:{sessionId:'forged'}});
  assert.equal(nativeScope.sessionId,r.service.selectedId);assert.deepEqual(nativeScope.context,context);
  assert.deepEqual(r.service.snapshot().selected.entries.at(-1).references,[{kind:'plan',id:planId,label:'Download plan'}]);
  await assert.rejects(r.service.tool({threadId:r.threadId,turnId:r.turnId,tool:'geod_approve_plan',arguments:{planId}}));await r.service.close();
  const reopened=rig(home);await reopened.service.open();assert.equal(reopened.service.snapshot().selected.entries.at(-1).references[0].id,planId);await reopened.service.close();
});
