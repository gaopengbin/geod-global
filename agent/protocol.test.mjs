import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { StreamProviderError } from 'ai';
import { translateResponses, startBridge, safeError, contextLimitExceeded, validateModelConfig, BRIDGE_MODEL } from './protocol.mjs';
const definitions=[{name:'geod_projects_list',description:'Read',inputSchema:{type:'object',properties:{},additionalProperties:false}}];
const config={label:'Test',protocol:'openai-compatible',baseUrl:'https://example.com/v1',model:'test-model',apiKey:'private-test-key'};
test('translation preserves tool pairs, merges adjacent assistant messages and removes built-ins',()=>{
  const value=translateResponses({input:[{role:'user',content:'Read projects'},{type:'function_call',call_id:'call',name:'geod_projects_list',arguments:'{}'},{type:'function_call_output',call_id:'call',output:'{"total":1}'},{role:'assistant',content:[{type:'output_text',text:'One project'}]},{role:'assistant',content:'Ready'}],tools:[{type:'function',name:'geod_projects_list'},{type:'function',name:'shell'}]},definitions);
  assert.equal(value.messages.length,4);assert.equal(value.messages[3].content.length,2);assert.deepEqual(Object.keys(value.tools),['geod_projects_list']);assert(!value.tools.geod_projects_list.execute);
  for(const input of [[{type:'reasoning',encrypted_content:'unsupported'}],[{role:'user',content:[{type:'input_image',image_url:'x'}]}],[{type:'function_call',call_id:'bad',name:'geod_download',arguments:'{}'}]]) assert.throws(()=>translateResponses({input},definitions));
});
test('model config rejects plaintext remote endpoints and errors never expose secrets',()=>{
  assert.equal(validateModelConfig(config).model,'test-model');
  for(const baseUrl of ['http://example.com/v1','https://key:secret@example.com/v1','https://example.com/v1?key=secret']) assert.throws(()=>validateModelConfig({...config,baseUrl}));
  assert(!safeError(new Error('Request failed private-test-key headers Authorization'),[config.apiKey]).includes(config.apiKey));
});
test('Responses Lite carries native tools in additional_tools while preserving instructions and tool history',()=>{
  const input=[{type:'additional_tools',id:'at_test',role:'developer',tools:[{type:'namespace',name:'functions',tools:[
    {type:'function',name:'geod_projects_list',description:'Forged description',parameters:{type:'string'}},
    {type:'function',name:'shell'},null]},
    {type:'namespace',name:'shell',tools:[{type:'function',name:'geod_projects_list'}]}]},
    {type:'message',role:'developer',content:[{type:'input_text',text:'Pinned application instructions'}]},
    {type:'message',role:'user',content:[{type:'input_text',text:'Read projects'}]},
    {type:'function_call',namespace:'functions',call_id:'call',name:'geod_projects_list',arguments:'{}'},
    {type:'function_call_output',namespace:'functions',call_id:'call',output:'{"total":1}'}];
  const value=translateResponses({instructions:'',tools:null,input},definitions);
  assert.equal(value.system,'Pinned application instructions');
  assert.deepEqual(Object.keys(value.tools),['geod_projects_list']);
  assert.equal(value.tools.geod_projects_list.description,'Read');assert(!value.tools.geod_projects_list.execute);
  assert.equal(value.messages.length,3);assert.equal(value.messages[1].content[0].toolName,'geod_projects_list');
  assert.equal(value.messages[2].content[0].toolCallId,'call');
  for(const input of [[{type:'additional_tools',role:'user',tools:[]}],
    [{type:'additional_tools',role:'developer',tools:'invalid'}],
    [{type:'additional_tools',role:'developer',tools:[{type:'namespace',name:'functions',tools:'invalid'}]}],
    [{type:'function_call',namespace:'other',call_id:'bad',name:'geod_projects_list',arguments:'{}'}],
    [{type:'function_call',call_id:'bad',name:'geod_projects_list',arguments:'{}',encrypted_function_args:['opaque']}],
    [{type:'unknown',role:'developer',content:'Do not silently discard a typed item'}]]) {
    assert.throws(()=>translateResponses({input},definitions));
  }
  assert.throws(()=>translateResponses({input:[],tools:'invalid'},definitions));
  assert.throws(()=>translateResponses({input:[...input.slice(0,-1),{...input.at(-1),namespace:'other'}]},definitions));
});
test('provider service failures are actionable without exposing upstream diagnostics',()=>{
  for(const statusCode of [500,502,503,504]) {
    const error=Object.assign(new Error(`Gateway error Authorization: ${config.apiKey}`),{statusCode});
    assert.equal(safeError(error,[config.apiKey]),'Agent model service is temporarily unavailable. Try again later.');
  }
  assert.equal(safeError({status:503,message:'Private provider diagnostic'}),'Agent model service is temporarily unavailable. Try again later.');
  assert.equal(safeError(new Error('Unknown private provider diagnostic')),'Agent model request failed. Check the connection and model settings.');
});
test('private text route pins the upstream model and refuses caller-selected model redirection',async t=>{
  let calls=0;
  const stream=options=>{calls++;assert.equal(options.model.modelId,config.model);return {
    fullStream:(async function*(){yield {type:'text-delta',text:'Controlled provider result'};})(),
    finishReason:Promise.resolve('stop'),usage:Promise.resolve({})};};
  const bridge=await startBridge({config,definitions,token:'test',stream});t.after(()=>bridge.close());
  const request=model=>fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer test'},
    body:JSON.stringify({model,stream:true,input:[{role:'user',content:'Read'}]})});
  for(const model of [config.model,'another-remote-model']) assert.equal((await request(model)).status,400);
  assert.equal(calls,0);
  const response=await request(BRIDGE_MODEL);assert.equal(response.status,200);
  const events=(await response.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
  assert.equal(events.find(e=>e.type==='response.completed').response.model,BRIDGE_MODEL);assert.equal(calls,1);
});
test('bridge enforces local authentication and SSE item order without an SDK execution loop',async t=>{
  let seen;
  const stream=options=>{seen=options;return {fullStream:(async function*(){yield {type:'tool-input-start',id:'call',toolName:'geod_projects_list'};yield {type:'tool-input-delta',id:'call',delta:'{}'};yield {type:'tool-call',toolCallId:'call',toolName:'geod_projects_list',input:{}};})(),finishReason:Promise.resolve('tool-calls'),usage:Promise.resolve({inputTokens:1,outputTokens:1,totalTokens:2})};};
  const bridge=await startBridge({config,definitions,token:'local-only',stream});t.after(()=>bridge.close());
  assert.equal((await fetch(bridge.baseUrl+'/responses',{method:'POST',body:'{}'})).status,404);
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer local-only'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}],tools:[{type:'function',name:'geod_projects_list'}]})});
  const body=await response.text();assert.equal(response.status,200);
  assert(body.indexOf('response.function_call_arguments.done')<body.indexOf('response.output_item.done'));
  assert(body.indexOf('response.output_item.done')<body.indexOf('response.completed'));assert(!seen.tools.geod_projects_list.execute);
});
test('a token cap produces a failed stream, never completion',async t=>{
  for(const reasoning of [false,true]){
    const stream=()=>({fullStream:(async function*(){yield reasoning?{type:'reasoning-delta',text:'unsupported'}:{type:'text-delta',text:'incomplete'};})(),finishReason:Promise.resolve('length'),usage:Promise.resolve({})});
    const bridge=await startBridge({config,definitions,token:'test',stream});
    const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer test'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}]})});
    const body=await response.text();assert(body.includes('response.failed'));assert(!body.includes('response.completed'));await bridge.close();
  }
});
test('blank provider answers fail with a bounded public message instead of reporting completion',async t=>{
  let failure;
  const stream=()=>({fullStream:(async function*(){yield {type:'text-delta',text:' \n\t'};})(),finishReason:Promise.resolve('stop'),usage:Promise.resolve({})});
  const bridge=await startBridge({config,definitions,token:'owned-test',stream,onFailure:value=>{failure=value;}});t.after(()=>bridge.close());
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-test'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}]})});
  const body=await response.text();assert(body.includes('response.failed'));assert(!body.includes('response.completed'));
  assert.equal(failure,'The model returned an empty response. Please retry.');assert(!body.includes(config.apiKey));
});
test('reasoning-only token exhaustion is reported as a length limit, never an empty answer or completion',async t=>{
  let failure,diagnosis;
  const stream=()=>({fullStream:(async function*(){yield {type:'reasoning-delta',text:'Controlled reasoning without a final answer'};})(),finishReason:Promise.resolve('length'),usage:Promise.resolve({})});
  const bridge=await startBridge({config,definitions,token:'owned-length',stream,onFailure:value=>{failure=value;},onDiagnostic:value=>{diagnosis=value;}});t.after(()=>bridge.close());
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-length'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Prepare a revised review'}]})});
  const body=await response.text();assert(body.includes('response.failed'));assert(!body.includes('response.completed'));
  assert.equal(diagnosis.category,'incomplete-answer');
  assert.equal(failure,'The model reached its output limit before completing the response. Please retry.');
});

test('actual compatible SDK reserves DeepSeek thinking budget and preserves long reasoning across tool calls',async t=>{
  const reasoning='Controlled reasoning. '.repeat(1800);
  let requests=0;
  const source=createServer(async(request,response)=>{
    const chunks=[];for await(const chunk of request)chunks.push(chunk);
    const body=JSON.parse(Buffer.concat(chunks));requests++;
    assert.equal(body.model,'deepseek-flash');assert.equal(body.max_tokens,16384);
    assert.equal(body.reasoning_effort,'low');
    if(requests===2){
      const assistant=body.messages.find(message=>message.role==='assistant');
      assert.equal(assistant.reasoning_content,reasoning);
      assert.equal(assistant.tool_calls[0].function.name,'geod_projects_list');
      assert(body.messages.some(message=>message.role==='tool'&&message.tool_call_id==='controlled-call'));
    }
    response.writeHead(200,{'content-type':'text/event-stream'});
    const emit=(delta,finish_reason=null)=>response.write(`data: ${JSON.stringify({id:'owned-budget',object:'chat.completion.chunk',created:1,model:'deepseek-flash',choices:[{index:0,delta,finish_reason}]})}\n\n`);
    if(requests===1){emit({reasoning_content:reasoning});emit({tool_calls:[{index:0,id:'controlled-call',type:'function',function:{name:'geod_projects_list',arguments:'{}'}}]});emit({},'tool_calls');}
    else {emit({content:'已准备新的任务选项，等待确认。'});emit({},'stop');}
    response.end('data: [DONE]\n\n');
  });
  await new Promise(resolve=>source.listen(0,'127.0.0.1',resolve));
  t.after(()=>new Promise(resolve=>{source.closeAllConnections();source.close(resolve);}));
  const bridge=await startBridge({config:{...config,model:'deepseek-flash',baseUrl:`http://127.0.0.1:${source.address().port}/v1`},definitions,token:'owned-budget'});t.after(()=>bridge.close());
  const post=async input=>{
    const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-budget'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input,tools:[{type:'function',name:'geod_projects_list'}]})});
    const events=(await response.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
    assert(!events.some(event=>event.type==='response.failed'));return events.find(event=>event.type==='response.completed').response.output;
  };
  const history=await post([{role:'user',content:'Read actual saved projects'}]);
  assert.equal(history[0].content[0].text,reasoning);assert.equal(history[1].name,'geod_projects_list');
  const result=await post([{role:'user',content:'Read actual saved projects'},...history,{type:'function_call_output',call_id:'controlled-call',output:'{"projects":[],"total":0}'}]);
  assert.equal(result[0].content[0].text,'已准备新的任务选项，等待确认。');assert.equal(requests,2);
});

test('structured context limits reach Codex as a typed failure without disclosing provider bodies',async t=>{
  const error={statusCode:400,message:'Private provider header '+config.apiKey,responseBody:JSON.stringify({error:{code:'context_length_exceeded',message:config.apiKey}})};
  assert(contextLimitExceeded(error));assert(contextLimitExceeded({statusCode:400,data:{error:{code:'context_length_exceeded'}}}));
  for(const changed of [{...error,statusCode:401},{...error,responseBody:'not JSON'},{statusCode:400,message:'context_length_exceeded'},
    {statusCode:400,responseBody:JSON.stringify({error:{code:'invalid_request_error',message:'context_length_exceeded'}})},
    {...error,responseBody:' '.repeat(65537)+error.responseBody}])assert(!contextLimitExceeded(changed));
  const bridge=await startBridge({config,definitions,token:'owned-context-error',stream:()=>({fullStream:(async function*(){yield {type:'error',error};})()})});t.after(()=>bridge.close());
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-context-error'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}]})});
  const body=await response.text();assert(body.includes('"code":"context_length_exceeded"'));assert(body.includes('response.failed'));assert(!body.includes('response.completed'));assert(!body.includes(config.apiKey));assert(!body.includes('Private provider header'));
});
test('typed in-stream context errors reach Codex without raw errors or false HTTP status inference',async t=>{
  const privateMessage='Authorization: '+config.apiKey;
  for(const statusCode of [undefined,200,400,413])assert(contextLimitExceeded(new StreamProviderError({message:privateMessage,code:'context_length_exceeded',statusCode})));
  for(const error of [{code:'context_length_exceeded',message:privateMessage},
    new StreamProviderError({message:'context_length_exceeded',code:'invalid_request_error'}),
    new StreamProviderError({message:privateMessage,code:'context_length_exceeded',statusCode:401})])assert(!contextLimitExceeded(error));
  const error=new StreamProviderError({message:privateMessage,code:'context_length_exceeded',data:{headers:{authorization:config.apiKey}}});
  let diagnosis;
  const bridge=await startBridge({config,definitions,token:'owned-stream-error',onDiagnostic:value=>{diagnosis=value;},stream:()=>({fullStream:(async function*(){yield {type:'error',error};})()})});t.after(()=>bridge.close());
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer owned-stream-error'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}]})});
  const body=await response.text();assert.equal(response.status,200);assert(body.includes('"code":"context_length_exceeded"'));assert(body.includes('response.failed'));assert(!body.includes('response.completed'));assert(!body.includes(config.apiKey));assert(!body.includes('Authorization'));
  assert.equal(diagnosis.errorClass,'AI_StreamProviderError');assert.equal(diagnosis.providerCode,'context_length_exceeded');assert.equal(diagnosis.category,'context-limit');assert.equal(diagnosis.status,null);
});
test('provider reasoning survives SSE output and subsequent tool history without becoming answer text',async t=>{
  const stream=()=>({fullStream:(async function*(){yield {type:'reasoning-delta',text:'provider reasoning'};yield {type:'reasoning-end'};yield {type:'tool-call',toolCallId:'call',toolName:'geod_projects_list',input:{}};})(),finishReason:Promise.resolve('tool-calls'),usage:Promise.resolve({})});
  const bridge=await startBridge({config,definitions,token:'test',stream});t.after(()=>bridge.close());
  const response=await fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer test'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input:[{role:'user',content:'Read'}],tools:[{type:'function',name:'geod_projects_list'}]})});
  const events=(await response.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
  const completed=events.find(event=>event.type==='response.completed');assert(completed);
  const reasoning=completed.response.output.find(item=>item.type==='reasoning');assert.equal(reasoning.content[0].text,'provider reasoning');
  assert(!events.some(event=>event.type==='response.output_text.delta'));
  const translated=translateResponses({input:[reasoning,...completed.response.output.filter(item=>item.type==='function_call'),{type:'function_call_output',call_id:'call',output:'{"total":1}'}]},definitions);
  assert.equal(translated.messages[0].content[0].type,'reasoning');assert.equal(translated.messages[0].content[1].type,'tool-call');
  assert.equal(translated.messages[1].role,'tool');
});
