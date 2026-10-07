import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { startBridge, BRIDGE_MODEL, validateModelConfig, translateResponses } from './protocol.mjs';
import { ProviderReplay } from './provider-replay.mjs';
import { validProviderProtocol } from './providers.mjs';

const definitions=[{name:'geod_projects_list',description:'Read saved project metadata',inputSchema:{type:'object',properties:{},additionalProperties:false}}];
const config={label:'Controlled native protocol',protocol:'anthropic-messages',baseUrl:'https://example.test/v1',model:'claude-haiku-4-5',apiKey:'synthetic-native-key'};
const post=(bridge,input)=>fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{Authorization:'Bearer local-test'},body:JSON.stringify({model:BRIDGE_MODEL,stream:true,input,tools:[{type:'function',name:'geod_projects_list'}]})});
const completed=async response=>{
  const events=(await response.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
  assert(!events.some(event=>event.type==='response.failed'));return events.find(event=>event.type==='response.completed').response.output;
};
async function upstream(t,handler) {
  const server=createServer(async(request,response)=>{
    const chunks=[];for await(const chunk of request) chunks.push(chunk);
    try { await handler(request,JSON.parse(Buffer.concat(chunks)),response); } catch(error) { response.writeHead(500).end(JSON.stringify({error:{message:'Controlled test assertion failed'}})); throw error; }
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  t.after(()=>new Promise(resolve=>{server.closeAllConnections();server.close(resolve);}));
  return `http://127.0.0.1:${server.address().port}`;
}
const anthropicEvent=(response,type,data)=>response.write(`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`);
function anthropicReply(response,tool) {
  response.writeHead(200,{'content-type':'text/event-stream'});
  const emit=(type,data)=>anthropicEvent(response,type,data);
  emit('message_start',{message:{id:'msg_controlled',type:'message',role:'assistant',model:config.model,content:[],stop_reason:null,stop_sequence:null,usage:{input_tokens:10,output_tokens:0}}});
  if(tool) {
    emit('content_block_start',{index:0,content_block:{type:'thinking',thinking:''}});
    emit('content_block_delta',{index:0,delta:{type:'thinking_delta',thinking:'Controlled reasoning'}});
    emit('content_block_delta',{index:0,delta:{type:'signature_delta',signature:'signed-native-'}});
    emit('content_block_delta',{index:0,delta:{type:'signature_delta',signature:'reasoning'}});
    emit('content_block_stop',{index:0});
    emit('content_block_start',{index:1,content_block:{type:'tool_use',id:'call_controlled',name:'geod_projects_list',input:{}}});
    emit('content_block_delta',{index:1,delta:{type:'input_json_delta',partial_json:'{}'}});
    emit('content_block_stop',{index:1});
  } else {
    emit('content_block_start',{index:0,content_block:{type:'text',text:''}});
    emit('content_block_delta',{index:0,delta:{type:'text_delta',text:'Controlled projects read'}});emit('content_block_stop',{index:0});
  }
  emit('message_delta',{delta:{stop_reason:tool?'tool_use':'end_turn',stop_sequence:null},usage:{output_tokens:12}});emit('message_stop',{});response.end();
}
test('native settings preserve explicit protocols and prevent Google model path redirection',()=>{
  for(const protocol of ['openai-compatible','anthropic-messages','google-generative-ai']) assert.equal(validateModelConfig({...config,protocol}).protocol,protocol);
  for(const model of ['models/gemini-test','../messages','test/../../messages','test:generateContent','test@other']) assert.throws(()=>validateModelConfig({...config,protocol:'google-generative-ai',model}));
  for(const [provider,protocol] of [['anthropic','openai-compatible'],['google','anthropic-messages'],['unknown','openai-compatible']]) assert(!validProviderProtocol(provider,protocol));
});
test('real Anthropic SDK wire preserves signed reasoning and tool results after bridge restart',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-native-anthropic-'));t.after(()=>rm(home,{recursive:true,force:true}));let count=0;
  const endpoint=await upstream(t,(request,body,response)=>{
    assert.equal(request.url,'/v1/messages');assert.equal(request.headers['x-api-key'],config.apiKey);assert(!request.headers.authorization);
    assert.equal(body.model,config.model);assert.equal(body.tools[0].name,'geod_projects_list');assert.equal(body.max_tokens,4096);
    if(++count===2) {
      const content=body.messages.find(message=>message.role==='assistant').content;
      assert.deepEqual(content.filter(part=>part.type==='thinking'),[{type:'thinking',thinking:'Controlled reasoning',signature:'signed-native-reasoning'}]);
      assert(content.some(part=>part.type==='tool_use'&&part.id==='call_controlled'));
      assert(body.messages.some(message=>message.content.some(part=>part.type==='tool_result'&&part.tool_use_id==='call_controlled')));
    }
    anthropicReply(response,count===1);
  });
  const settings={...config,baseUrl:endpoint+'/v1'};
  let bridge=await startBridge({config:settings,home,definitions,token:'local-test'});
  const output=await completed(await post(bridge,[{role:'user',content:'Read saved projects'}]));
  assert.equal(output[0].type,'reasoning');assert.equal(output[1].type,'function_call');
  assert(!JSON.stringify(output).includes('signed-native'));assert(!JSON.stringify(output).includes(config.apiKey));
  await bridge.close();bridge=await startBridge({config:settings,home,definitions,token:'local-test'});t.after(()=>bridge.close());
  // Codex is allowed to omit output item IDs on its next request.
  const history=output.map(({id,...item})=>item);
  const answer=await completed(await post(bridge,[{role:'user',content:'Read saved projects'},...history,{type:'function_call_output',call_id:'call_controlled',output:'{"total":1}'}]));
  assert.equal(answer[0].content[0].text,'Controlled projects read');assert.equal(count,2);
});
test('real Google SDK native wire preserves tool thought signature, result and model path',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-native-google-'));t.after(()=>rm(home,{recursive:true,force:true}));let count=0;
  const endpoint=await upstream(t,(request,body,response)=>{
    assert.equal(request.url,'/v1beta/models/gemini-3-flash-preview:streamGenerateContent?alt=sse');
    assert.equal(request.headers['x-goog-api-key'],config.apiKey);assert(!request.headers.authorization);assert.equal(body.generationConfig.maxOutputTokens,4096);
    if(++count===2) {
      const call=body.contents.find(message=>message.role==='model').parts.find(part=>part.functionCall);
      assert.equal(call.thoughtSignature,'signed-google-call');assert.equal(call.functionCall.name,'geod_projects_list');
      assert(body.contents.some(message=>message.parts.some(part=>part.functionResponse?.name==='geod_projects_list')));
      assert(!JSON.stringify(body).includes('skip_thought_signature_validator'));
    }
    response.writeHead(200,{'content-type':'text/event-stream'});
    const parts=count===1 ? [{functionCall:{name:'geod_projects_list',args:{}},thoughtSignature:'signed-google-call'}] : [{text:'Controlled Google projects read'}];
    response.end(`data: ${JSON.stringify({candidates:[{content:{role:'model',parts},finishReason:'STOP',index:0}],usageMetadata:{promptTokenCount:10,candidatesTokenCount:12,totalTokenCount:22}})}\n\n`);
  });
  const settings={...config,protocol:'google-generative-ai',model:'gemini-3-flash-preview',baseUrl:endpoint+'/v1beta'};
  let bridge=await startBridge({config:settings,home,definitions,token:'local-test'});
  const output=await completed(await post(bridge,[{role:'user',content:'Read saved projects'}]));assert.equal(output[0].type,'function_call');await bridge.close();
  bridge=await startBridge({config:settings,home,definitions,token:'local-test'});t.after(()=>bridge.close());
  const answer=await completed(await post(bridge,[{role:'user',content:'Read saved projects'},...output,{type:'function_call_output',call_id:output[0].call_id,output:'{"total":1}'}]));
  assert.equal(answer[0].content[0].text,'Controlled Google projects read');assert.equal(count,2);
});
test('native replay refuses changed calls, ambiguous signatures, forged metadata and missing state',async t=>{
  const replay=await ProviderReplay.open({config});
  const item={id:'rs_first',type:'reasoning',content:[{type:'reasoning_text',text:'Same reasoning'}]};
  replay.remember(item,replay.options({anthropic:{signature:'one'}}));
  replay.remember({...item,id:'rs_second'},replay.options({anthropic:{signature:'two'}}));
  assert.equal(replay.restore(item).anthropic.signature,'one');assert.throws(()=>replay.restore({...item,id:undefined}));
  assert.throws(()=>replay.restore({...item,content:[{type:'reasoning_text',text:'Changed'}]}));
  assert.throws(()=>replay.options({anthropic:{signature:'signed',type:'compaction'}}));
  assert.throws(()=>replay.options({anthropic:{signature:'signed'},google:{thoughtSignature:'foreign'}}));
  assert.throws(()=>replay.options({anthropic:{signature:config.apiKey}}));
  assert.throws(()=>translateResponses({input:[{type:'function_call',name:'geod_projects_list',call_id:'forged',arguments:'{}'}]},definitions,replay));
});
test('unsigned Gemini 3 tool history fails before the SDK can send its bypass sentinel',async t=>{
  let requests=0;
  const endpoint=await upstream(t,(_request,_body,response)=>{
    requests++;response.writeHead(200,{'content-type':'text/event-stream'});
    response.end(`data: ${JSON.stringify({candidates:[{content:{role:'model',parts:[{functionCall:{name:'geod_projects_list',args:{}}}]},finishReason:'STOP',index:0}],usageMetadata:{promptTokenCount:1,candidatesTokenCount:1,totalTokenCount:2}})}\n\n`);
  });
  const settings={...config,protocol:'google-generative-ai',model:'gemini-3-flash-preview',baseUrl:endpoint+'/v1beta'};
  const bridge=await startBridge({config:settings,definitions,token:'local-test'});t.after(()=>bridge.close());
  const output=await completed(await post(bridge,[{role:'user',content:'Read projects'}]));
  const response=await post(bridge,[{role:'user',content:'Read projects'},...output,{type:'function_call_output',call_id:output[0].call_id,output:'{"total":0}'}]);
  const body=await response.text();assert(body.includes('response.failed'));assert(!body.includes('response.completed'));assert.equal(requests,1);
});
