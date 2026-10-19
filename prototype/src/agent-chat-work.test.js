import test from 'node:test';
import assert from 'node:assert/strict';
import {transcriptRows,activeWorkId,workEntryNeedsAttention,projectWorkActivities,workRecordState} from './agent-chat-work.js';
test('upstream activity projection coalesces adjacent reads without swallowing commentary, reviews or decisions',()=>{
  const entries=[{id:'a',type:'tool',name:'geod_sources_list',status:'running'},
    {id:'b',type:'tool',name:'geod_scene_search',status:'completed'},
    {id:'note',type:'assistant',text:'The next step needs confirmation.'},
    {id:'plan',type:'tool',name:'geod_download_plan'},
    {id:'review',type:'reviews'}, {id:'choice',type:'tool',decision:{status:'pending'}},
    {id:'unknown',type:'tool',name:'unknown_read_tool'},
    {id:'a',type:'tool',name:'geod_sources_list',status:'completed'}];
  const activities=projectWorkActivities(entries);
  assert.deepEqual(activities.map(a=>a.kind),['exploration','note','tools','attachment','attachment','tools']);
  assert.deepEqual(activities[0].entries.map(e=>e.id),['a','b']);
  assert.equal(activities[0].entries[0].status,'completed');
  assert.equal(workRecordState({status:'running'},false),'attention');
  assert.equal(workRecordState({status:'running'},true),'running');
});
test('a completed unphased message cannot end the authoritative active turn, including compaction and an explicit final item',()=>{
  const activity={kind:'turn',turnId:'current'};
  const messages=[{id:'u',type:'user'},{id:'tool',type:'tool',turnId:'current',status:'completed'},
    {id:'note',type:'assistant',turnId:'current',status:'completed',text:'Now searching'}];
  let rows=transcriptRows(messages,activity);
  assert.deepEqual(rows.map(row=>row.type),['user','work']);assert.equal(activeWorkId(rows,true,activity),rows[1].id);
  messages.push({id:'final',type:'assistant',turnId:'current',phase:'final',status:'completed'});
  rows=transcriptRows(messages,activity);assert.equal(rows.at(-1).type,'assistant');assert.equal(activeWorkId(rows,true,activity),rows[1].id);
  assert.equal(activeWorkId(rows,false,null),undefined);
  assert.equal(activeWorkId(rows,true,{kind:'compaction',turnId:null}),undefined);
  assert.equal(activeWorkId(rows,true,{kind:'turn',turnId:'next'}),undefined);
});
test('turn activity retains the exact chronology of native choices, reviews and replayed updates',()=>{
  const input=[{id:'u',type:'user'}, {id:'p',type:'assistant',phase:'progress',turnId:'turn-1',text:'Resolve area'},
    {id:'t',type:'tool',turnId:'turn-1',references:[{kind:'plan',id:'plan'}],status:'running'},
    {id:'decision',type:'tool',turnId:'turn-1',decision:{status:'pending'}},
    {id:'p2',type:'assistant',turnId:'turn-1',phase:'progress'},
    {id:'t2',type:'tool',turnId:'turn-1',references:[{kind:'plan',id:'plan'}]},
    {id:'t',type:'tool',turnId:'turn-1',references:[],status:'completed'},
    {id:'final',type:'assistant',turnId:'turn-1',phase:'final',status:'completed'}];
  const rows=transcriptRows(input);
  assert.deepEqual(rows.map(row=>row.type),['user','work','assistant']);
  assert.deepEqual(rows[1].entries.map(row=>row.id),['p','t','decision','p2','t2','reviews-t2']);assert.equal(rows[1].entries[1].status,'completed');
  assert.equal(rows[1].entries[2].decision.status,'pending');assert.equal(rows[1].entries[5].references[0].id,'plan');
  assert.equal(activeWorkId(rows,true),undefined);
  assert.equal(activeWorkId(rows.slice(0,-1),true),rows[1].id);
});
test('greetings, every user request and each final response remain outside independent activity groups',()=>{
  const rows=transcriptRows([
    {id:'hello',type:'user'}, {id:'greeting',type:'assistant',turnId:'greeting',phase:'final'},
    {id:'request',type:'user'}, {id:'search',type:'tool',turnId:'first'},
    {id:'first-final',type:'assistant',turnId:'first',phase:'final'},
    {id:'next',type:'user'}, {id:'second-tool',type:'tool',turnId:'second'},
    {id:'second-final',type:'assistant',turnId:'second',phase:'final'}
  ]);
  assert.deepEqual(rows.map(row=>row.type),['user','assistant','user','work','assistant','user','work','assistant']);
  assert.deepEqual(rows.filter(row=>row.type==='work').map(row=>row.entries.map(entry=>entry.id)),[['search'],['second-tool']]);
});
test('a reused native turn ID cannot cross a user request, while automatic follow-on work stays in that request',()=>{
  const rows=transcriptRows([{id:'u',type:'user'},{id:'t1',type:'tool',turnId:'reused'},
    {id:'steer',type:'user'},{id:'t2',type:'tool',turnId:'reused'},
    {id:'final',type:'assistant',phase:'final',turnId:'reused'}, {id:'t3',type:'tool',turnId:'reused'}]);
  assert.deepEqual(rows.map(row=>row.type),['user','work','user','work']);
  assert.equal(new Set(rows.filter(row=>row.type==='work').map(row=>row.id)).size,2);
  assert.deepEqual(rows.at(-1).entries.map(entry=>entry.id),['t2','final','t3']);
  assert.equal(activeWorkId(rows,true,{kind:'turn',turnId:'reused'}),rows.at(-1).id);
});
test('collapsed activity exposes approvals and unfinished tasks while completed result tasks can be revealed separately',()=>{
  const entry={type:'reviews',references:[{kind:'plan',id:'p'}]};
  assert.equal(workEntryNeedsAttention({decision:{status:'pending'}}),true);
  assert.equal(workEntryNeedsAttention({type:'tool',status:'completed'}),false);
  assert.equal(workEntryNeedsAttention(entry,[{planId:'p',status:'pending'}]),true);
  assert.equal(workEntryNeedsAttention(entry,[{planId:'p',status:'submitted',jobs:[{status:'running'}]}]),true);
  assert.equal(workEntryNeedsAttention(entry,[{planId:'p',status:'submitted',jobs:[{status:'succeeded',settled:true}]}]),false);
});
test('legacy conversations never fold user requests or final replies into a work timeline',()=>{
  const rows=transcriptRows([{id:'u',type:'user'},{id:'p',type:'assistant'},{id:'t',type:'tool',references:[]}, {id:'final',type:'assistant',phase:'final'}, {id:'next',type:'user'}, {id:'t2',type:'tool',references:[]}]);
  assert.deepEqual(rows.map(row=>row.type),['user','work','assistant','user','work']);assert.notEqual(rows[1].id,rows[4].id);
  const review=transcriptRows([{id:'reply',type:'assistant',text:'Please confirm this review'}, {id:'approval',type:'tool',origin:'desktop',name:'geod_plan_execute',references:[]}]);
  assert.equal(review[0].type,'work');assert.deepEqual(review[0].entries.map(entry=>entry.id),['reply','approval']);
});
const workflowRequest=()=>[
  {id:'hello',type:'user',text:'Hello'}, {id:'greeting',type:'assistant',turnId:'hello',phase:'final',status:'completed'},
  {id:'request',type:'user',text:'Download and crop Shanghai'},
  {id:'download',type:'tool',turnId:'download-turn',status:'completed'},
  {id:'submitted',type:'assistant',turnId:'download-turn',phase:'final',status:'completed',text:'Download submitted'},
  {id:'wake-download',type:'system',origin:'desktop',text:'Background tasks settled. Checking results and continuing your request.'},
  {id:'mosaic',type:'tool',turnId:'mosaic-turn',status:'completed'},
  {id:'mosaic-submitted',type:'assistant',turnId:'mosaic-turn',phase:'final',status:'completed',text:'Mosaic submitted'},
  {id:'wake-mosaic',type:'system',origin:'desktop',text:'Background tasks settled. Checking results and continuing your request.'},
  {id:'verify',type:'tool',turnId:'verification-turn',status:'completed'},
  {id:'delivered',type:'assistant',turnId:'verification-turn',phase:'final',status:'completed',text:'Verified delivery'}];
test('a real download/mosaic/verification workflow has one delivery reply and one activity group, retaining the greeting',()=>{
  const messages=workflowRequest(),rows=transcriptRows(messages,null,{status:'completed',workflow:{status:'completed'},goal:{status:'complete',outputs:[{state:'verified'}]}});
  assert.deepEqual(rows.map(row=>row.type),['user','assistant','user','work','assistant']);
  assert.deepEqual(rows.filter(row=>row.type==='assistant').map(row=>row.id),['greeting','delivered']);
  assert.deepEqual(rows[3].turnIds,['download-turn','mosaic-turn','verification-turn']);
  assert.deepEqual(rows.flatMap(row=>row.type==='work'?row.entries.map(entry=>entry.id):row.id),messages.map(entry=>entry.id));
});
test('native subturn completion does not publish a final reply or fold the request while background jobs are pending',()=>{
  const messages=workflowRequest().slice(0,5),state={status:'completed',workflow:{status:'waiting'},goal:{status:'waiting_jobs',requestText:'Download and crop Shanghai'}};
  let rows=transcriptRows(messages,null,state),work=rows.at(-1);
  assert.equal(work.type,'work');assert.equal(work.backgroundRunning,true);assert.equal(activeWorkId(rows,false,null),work.id);
  rows=transcriptRows(workflowRequest().slice(0,8),null,{...state,workflow:{status:'ready'}});
  assert.equal(rows.at(-1).id,work.id);assert.equal(rows.at(-1).backgroundRunning,true);
  rows=transcriptRows(messages,null,{...state,workflow:{status:'paused'},goal:{status:'paused'}});
  assert.equal(rows.at(-1).id,'submitted');assert.equal(activeWorkId(rows,false,null),undefined);
});
test('an unrelated question does not inherit the previous task\'s background lifecycle',()=>{
  const messages=[...workflowRequest(),{id:'question',type:'user',text:'What resolution is this?'},{id:'resolution',type:'assistant',phase:'final',text:'10 metres'}];
  const rows=transcriptRows(messages,null,{status:'completed',workflow:{status:'waiting'},goal:{status:'waiting_jobs',requestText:'Download and crop Shanghai'}});
  assert.equal(rows.at(-1).id,'resolution');assert.equal(rows.filter(row=>row.type==='work').length,1);
});
test('a waiting choice stays addressable with one current reply rather than being mistaken for delivery',()=>{
  const messages=workflowRequest().slice(0,5);messages.splice(4,0,{id:'choice',type:'tool',decision:{status:'pending'},turnId:'download-turn'});
  const rows=transcriptRows(messages,null,{workflow:{status:'waiting'},goal:{status:'active'}});
  assert.equal(rows.at(-1).id,'submitted');assert.equal(rows[3].backgroundRunning,false);
  assert.equal(workEntryNeedsAttention(rows[3].entries.find(entry=>entry.id==='choice')),true);
});
test('a review referenced by the current reply remains reachable, including a reply without previous tool entries',()=>{
  const plans=[{planId:'review',status:'pending'}];
  const rows=transcriptRows([{id:'request',type:'user',text:'Download this area'},
    {id:'reply',type:'assistant',phase:'final',references:[{kind:'plan',id:'review'}],text:'Confirm the download review'}],null,{plans});
  assert.deepEqual(rows.map(row=>row.type),['user','work','assistant']);
  assert.equal(rows[1].entries[0].id,'reviews-reply');
  assert.equal(workEntryNeedsAttention(rows[1].entries[0],plans),true);
  assert.equal(rows[2].id,'reply');
});
test('a previously verified goal cannot finish an unrelated continuing workflow',()=>{
  const messages=[...workflowRequest(),{id:'new-request',type:'user',text:'Download Berlin'},
    {id:'new-job',type:'tool',turnId:'berlin',references:[{kind:'plan',id:'berlin-plan'}]},
    {id:'new-stage',type:'assistant',phase:'final',text:'Download submitted'}];
  const rows=transcriptRows(messages,null,{status:'completed',workflow:{status:'ready'},
    goal:{status:'complete',requestText:'Download and crop Shanghai',outputs:[{state:'verified'}]},
    plans:[{planId:'berlin-plan',status:'submitted',jobs:[{status:'succeeded',settled:true}]}]});
  assert.equal(rows.at(-1).type,'work');assert.equal(rows.at(-1).backgroundRunning,true);
  assert.equal(activeWorkId(rows,false,null),rows.at(-1).id);
});
