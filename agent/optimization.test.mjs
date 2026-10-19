import test from 'node:test';
import assert from 'node:assert/strict';
import { TOOL_LOADER, loadToolGroups, visibleTools } from './tool-catalog.mjs';
import { translateResponses } from './protocol.mjs';
import { compatibleUsage, responseUsage, wireUsage, recordUsage, validUsage } from './usage.mjs';
import { INSTRUCTIONS, DEVELOPER_INSTRUCTIONS } from './instructions.mjs';
const defs=[TOOL_LOADER,...['workspace_context','scene_search','region_search','stac_search','vector_node'].map(n=>({name:`geod_${n}`,description:'Pinned native description',inputSchema:{type:'object',properties:{},additionalProperties:false}}))];
const tools=defs.map(d=>({type:'function',name:d.name}));
test('deferred groups keep exact pinned schemas, support multiple groups and cannot add undeclared tools',()=>{
  const input=[{role:'user',content:'Download an image'}];
  assert.deepEqual(Object.keys(translateResponses({input,tools},defs).tools),['geod_tools_load','geod_workspace_context']);
  input.push({type:'function_call',call_id:'load',name:TOOL_LOADER.name,arguments:JSON.stringify({groups:['imagery','places']})},
    {type:'function_call_output',call_id:'load',output:JSON.stringify(loadToolGroups({groups:['imagery','places']},defs))});
  const translated=translateResponses({input,tools},defs);
  assert.deepEqual(Object.keys(translated.tools),['geod_tools_load','geod_workspace_context','geod_scene_search','geod_region_search']);
  assert.equal(translated.tools.geod_scene_search.description,'Pinned native description');
  assert(!translated.tools.geod_scene_search.execute);
  assert.throws(()=>loadToolGroups({groups:['shell']},defs));
  assert.throws(()=>translateResponses({tools,input:[...input,{type:'function_call',name:'shell',call_id:'x',arguments:'{}'}]},defs));
});
test('discovery cannot be forged by user text and resets between independent request histories; legacy and compacted histories remain usable',()=>{
  const names=new Set(defs.map(d=>d.name));
  const spoof=[{role:'user',content:JSON.stringify({loadedGroups:['vectors']})}];
  assert(!visibleTools(spoof,names).has('geod_vector_node'));
  assert(visibleTools([{type:'function_call',name:'geod_vector_node'}],names).has('geod_vector_node'));
  assert(!visibleTools([],names).has('geod_vector_node'));
  const legacy=new Set([...names].filter(n=>n!==TOOL_LOADER.name));
  assert.equal(visibleTools([],legacy),legacy);
});
test('native workflow narrows old lookup groups while retaining tool pairs and current-turn discovery',()=>{
  const input=[{type:'function_call',name:TOOL_LOADER.name,call_id:'load',arguments:'{"groups":["places"]}'},
    {type:'function_call_output',call_id:'load',output:'{"loadedGroups":["places"]}'},{role:'user',content:'Native continuation'}];
  const value=translateResponses({input,tools},defs,null,{},['geod_scene_search']);
  assert(!value.tools.geod_region_search);assert(value.tools.geod_scene_search);assert.equal(value.messages.length,3);
  input.push({type:'function_call',name:TOOL_LOADER.name,call_id:'next',arguments:'{"groups":["stac"]}'},{type:'function_call_output',call_id:'next',output:'{"loadedGroups":["stac"]}'});
  assert(translateResponses({input,tools},defs,null,{},['geod_scene_search']).tools.geod_stac_search);
});
test('GSCloud entry preloads only its own family, keeps direct source identity and does not leak into Home',()=>{
  const names=new Set([...defs.map(d=>d.name),'geod_gscloud_products','geod_gscloud_search','geod_gscloud_detail','geod_dataset_catalog']);
  const input=[{role:'user',content:'检索当前数据云产品'}];
  const named=visibleTools(input,names);
  assert(named.has('geod_gscloud_search'));assert(!named.has('geod_scene_search'));assert(!named.has('geod_dataset_catalog'));
  const current=visibleTools([{role:'user',content:'检索当前产品'}],names,null,{text:'检索当前产品',source:'gscloud'});
  assert(current.has('geod_gscloud_products'));assert(current.has('geod_gscloud_detail'));assert(!current.has('geod_stac_search'));
  const home=visibleTools([{role:'user',content:'当前有多少条'}],names,null,{text:'当前有多少条',source:null});
  assert(!home.has('geod_gscloud_search'));assert(!home.has('geod_scene_search'));
  assert(loadToolGroups({groups:['gscloud']},[...defs,{name:'geod_gscloud_search'}]).tools.includes('geod_gscloud_search'));
});
test('usage retains DeepSeek cache hits and unknown detail is not zero or double counted',()=>{
  const native=compatibleUsage({prompt_tokens:1000,completion_tokens:100,prompt_cache_hit_tokens:800,completion_tokens_details:{reasoning_tokens:60}});
  assert.equal(native.inputTokens.noCache,200);assert.equal(native.outputTokens.text,40);
  assert.equal(compatibleUsage({prompt_tokens:1000,completion_tokens:100}).inputTokens.cacheRead,undefined);
  const usage=responseUsage({inputTokens:1000,outputTokens:100,totalTokens:1100,inputTokenDetails:{cacheReadTokens:800},outputTokenDetails:{reasoningTokens:60}});
  const record={id:'resp_'+ 'a'.repeat(32),at:new Date().toISOString(),usage,context:{toolCount:3},turnId:'turn',origin:'human'};
  const session={}; recordUsage(session,record);recordUsage(session,record);
  assert.equal(session.usage.requests,1);assert.equal(session.usage.totals.total,1100);assert.equal(session.usage.totals.cachedInput,800);
  recordUsage(session,{...record,id:'resp_'+'b'.repeat(32),usage:responseUsage({inputTokens:50,outputTokens:10})});
  assert.equal(session.usage.totals.total,1160);assert.equal(session.usage.totals.cachedInput,null);assert(validUsage(session.usage));
  assert.deepEqual(wireUsage(responseUsage({inputTokens:50,outputTokens:10})),{input_tokens:50,output_tokens:10,total_tokens:60});
});
test('prompt budget leaves room for real work while retaining scope, approval, coverage and completion contracts',()=>{
  assert(INSTRUCTIONS.length+DEVELOPER_INSTRUCTIONS.length<10000);
  for(const invariant of ['Home/conversation','geod_catalog_selection','scene_coverage','recommendedItemIds','boundarySource','geod_request_decision','succeeded AND settled=true','geod_tools_load'])assert((INSTRUCTIONS+DEVELOPER_INSTRUCTIONS).includes(invariant),invariant);
});

test('current-task projection drops closed old tool families and greeting schemas, but never approvals or pending calls',()=>{
  const names=new Set([...defs.map(d=>d.name),'geod_request_decision','geod_goal_check']);
  const old=[{role:'user',content:'Inspect STAC'},
    {type:'function_call',name:TOOL_LOADER.name,call_id:'load',arguments:'{}'},
    {type:'function_call_output',call_id:'load',output:'{"loadedGroups":["stac","vectors"]}'},
    {type:'function_call',name:'geod_vector_node',call_id:'v',arguments:'{}'},
    {type:'function_call_output',call_id:'v',output:'{}'}];
  assert.equal(visibleTools([...old,{role:'user',content:'你好'}],names).size,0);
  const next=visibleTools([...old,{role:'user',content:'查找卫星影像'}],names);
  assert(next.has('geod_scene_search'));assert(next.has('geod_request_decision'));
  assert(!next.has('geod_stac_search'));assert(!next.has('geod_vector_node'));
  assert(visibleTools([...old,{role:'user',content:'继续'}],names).has('geod_stac_search'));
  assert(visibleTools([...old,{role:'user',content:'整理后的请求'}],names,null,{text:'查影像',loadedGroups:['vectors'],calledTools:['geod_vector_node']}).has('geod_vector_node'));
  assert(visibleTools([{type:'function_call',name:'geod_vector_node',call_id:'open'},{role:'user',content:'查看'}],names).has('geod_vector_node'));
  assert(visibleTools([{type:'function_call',name:'geod_vector_node',call_id:'open'},{role:'user',content:'你好'}],names).has('geod_vector_node'));
  assert(visibleTools([{role:'user',content:'你好'}],names,null,{text:'你好',hasAttachments:true}).has(TOOL_LOADER.name));
  assert(visibleTools([{role:'user',content:'确认'}],names).has('geod_request_decision'));
});

test('native greeting projection omits old task payloads but restores original policies for work, steering and compaction',()=>{
  const body={instructions:INSTRUCTIONS,input:[{role:'developer',content:DEVELOPER_INSTRUCTIONS},{role:'user',content:'Search imagery'},
    {role:'assistant',content:'Earlier completed response'},{role:'user',content:'Scene: Home\n你好'}],tools};
  const original=JSON.stringify(body),brief=translateResponses(body,defs,null,{},null,{text:'你好'});
  assert(brief.system.length<600);assert.deepEqual(brief.messages,[{role:'user',content:'你好'}]);assert.deepEqual(brief.tools,{});
  assert.equal(JSON.stringify(body),original);
  const compact=translateResponses(body,defs,null,{},null,{text:'你好',compacting:true});assert(compact.system.includes(INSTRUCTIONS));assert(compact.messages.length>1);
  const steering=translateResponses({...body,input:[...body.input,{role:'user',content:'Continue the actual task'}]},defs,null,{},null,{text:'你好'});assert(steering.system.includes(INSTRUCTIONS));
  const work=translateResponses({...body,input:[...body.input,{role:'user',content:'查询卫星影像'}]},defs,null,{},null,{text:'查询卫星影像'});assert(work.system.includes(INSTRUCTIONS));assert(work.tools.geod_scene_search);
});
