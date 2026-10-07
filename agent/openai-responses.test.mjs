import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {mkdtemp,readFile,readdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {startBridge,BRIDGE_MODEL,translateResponses} from './protocol.mjs';
import {ProviderReplay} from './provider-replay.mjs';

const definitions=[{name:'geod_projects_list',description:'Read saved projects',inputSchema:{type:'object',properties:{},additionalProperties:false}}];
const config={label:'Owned OpenAI wire',protocol:'openai-responses',baseUrl:'https://example.test/v1',model:'gpt-5',apiKey:'owned-synthetic-key'};
const requestBody=input=>({model:BRIDGE_MODEL,stream:true,input,tools:[{type:'function',name:'geod_projects_list'}]});
const post=(bridge,input)=>fetch(bridge.baseUrl+'/responses',{method:'POST',headers:{authorization:'Bearer owned-loopback'},body:JSON.stringify(requestBody(input))});
async function complete(response) {
  const events=(await response.text()).split('\n').filter(line=>line.startsWith('data: ')).map(line=>JSON.parse(line.slice(6)));
  assert.equal(events.at(-1).type,'response.completed');
  return events.at(-1).response.output;
}
function reply(response,{call=false,summary=false,encrypted='opaque-complete-cipher',phase='final_answer',messageId='msg_owned'}={}) {
  response.writeHead(200,{'content-type':'text/event-stream'});
  const emit=(type,data)=>response.write(`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`);
  const envelope={id:'resp_owned',model:config.model,created_at:1,status:'in_progress',output:[]};
  emit('response.created',{response:envelope});
  const output=[];
  if(call) {
    const item={id:'rs_owned',type:'reasoning',status:'in_progress',summary:[],encrypted_content:'incomplete-cipher'};
    emit('response.output_item.added',{output_index:0,item});
    if(summary)emit('response.reasoning_summary_text.delta',{output_index:0,item_id:item.id,summary_index:0,delta:'Controlled summary'});
    const done={...item,status:'completed',summary:summary?[{type:'summary_text',text:'Controlled summary'}]:[],encrypted_content:encrypted};
    emit('response.output_item.done',{output_index:0,item:done});output.push(done);
    const tool={id:'fc_owned',type:'function_call',status:'in_progress',call_id:'call_owned',name:'geod_projects_list',arguments:''};
    emit('response.output_item.added',{output_index:1,item:tool});
    emit('response.function_call_arguments.delta',{output_index:1,item_id:tool.id,delta:'{}'});
    const finished={...tool,status:'completed',arguments:'{}'};emit('response.output_item.done',{output_index:1,item:finished});output.push(finished);
  }else {
    const item={id:messageId,type:'message',role:'assistant',status:'in_progress',content:[],phase};
    emit('response.output_item.added',{output_index:0,item});
    emit('response.output_text.delta',{output_index:0,item_id:item.id,content_index:0,delta:'Controlled native projects read'});
    const done={...item,status:'completed',content:[{type:'output_text',text:'Controlled native projects read',annotations:[]}]};
    emit('response.output_item.done',{output_index:0,item:done});output.push(done);
  }
  emit('response.completed',{response:{...envelope,status:'completed',output,usage:{input_tokens:10,output_tokens:12,total_tokens:22}}});response.end();
}

test('actual OpenAI SDK preserves completed encrypted reasoning, tool results and phase after bridge restart',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-openai-wire-'));t.after(()=>rm(home,{recursive:true,force:true}));
  let count=0,wireFailure;
  const server=createServer(async(request,response)=>{
    try {
      const parts=[];for await(const part of request)parts.push(part);const body=JSON.parse(Buffer.concat(parts));count++;
      assert.equal(request.url,'/v1/responses');assert.equal(request.headers.authorization,'Bearer '+config.apiKey);
      assert.equal(body.model,config.model);assert.equal(body.store,false);assert.equal(body.stream,true);
      assert.equal(body.parallel_tool_calls,false);assert(body.include.includes('reasoning.encrypted_content'));
      assert.equal(body.tools[0].name,'geod_projects_list');assert.equal(body.tools[0].strict,false);
      assert(!body.previous_response_id && !body.conversation);assert(!body.input.some(item=>item.type==='item_reference'));
      if(count===2) {
        assert.deepEqual(body.input.find(item=>item.type==='reasoning'),{type:'reasoning',id:'rs_owned',encrypted_content:'opaque-complete-cipher',summary:[]});
        assert(body.input.some(item=>item.type==='function_call' && item.call_id==='call_owned'));
        assert(body.input.some(item=>item.type==='function_call_output' && item.call_id==='call_owned' && item.output==='{"total":1}'));
      }
      if(count>=3)assert.equal(body.input.find(item=>item.role==='assistant').phase,'final_answer');
      reply(response,{call:count===1,messageId:`msg_owned_${count}`});
    }catch(error){wireFailure=error;response.writeHead(500).end('{"error":{"message":"Owned wire assertion failed"}}');}
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));t.after(()=>new Promise(resolve=>{server.closeAllConnections();server.close(resolve);}));
  const settings={...config,baseUrl:`http://127.0.0.1:${server.address().port}/v1`};
  let bridge=await startBridge({config:settings,home,definitions,identity:'owned-connection',token:'owned-loopback'});
  const user={role:'user',content:'Read saved projects'};
  const output=await complete(await post(bridge,[user]));
  assert.equal(output[0].type,'reasoning');assert.equal(output[0].encrypted_content,'opaque-complete-cipher');assert.equal(output[1].type,'function_call');
  await bridge.close();bridge=await startBridge({config:settings,home,definitions,identity:'owned-connection',token:'owned-loopback'});t.after(()=>bridge.close());
  const history=output.map(({id,...item})=>item);
  const result={type:'function_call_output',call_id:'call_owned',output:'{"total":1}'};
  const answer=await complete(await post(bridge,[user,...history,result]));assert.equal(answer[0].content[0].text,'Controlled native projects read');
  const restoredAnswer=answer.map(({id,...item})=>item);
  const repeated=await complete(await post(bridge,[user,...history,result,...restoredAnswer,{role:'user',content:'Continue'}]));
  await complete(await post(bridge,[user,...history,result,...restoredAnswer,{role:'user',content:'Continue'},...repeated.map(({id,...item})=>item),{role:'user',content:'Read again'}]));
  assert(!wireFailure,wireFailure?.message);assert.equal(count,4);
  const files=await readdir(join(home,'provider-replay'));assert.equal(files.length,1);
  const state=await readFile(join(home,'provider-replay',files[0]),'utf8');assert(state.includes('opaque-complete-cipher'));assert(!state.includes('incomplete-cipher'));
  for(const privateText of [config.apiKey,user.content,'Controlled native projects read','{"total":1}'])assert(!state.includes(privateText));
  const tampered=history.map(item=>item.type==='reasoning'?{...item,encrypted_content:'changed-cipher'}:item);
  const refused=await post(bridge,[user,...tampered,result]);assert.equal(refused.status,400);assert.equal(count,4);assert(!(await refused.text()).includes(config.apiKey));
});

test('encrypted replay binds ciphertext and scope, rejects missing state and keeps protocol metadata closed',async t=>{
  const home=await mkdtemp(join(tmpdir(),'geod-openai-replay-'));t.after(()=>rm(home,{recursive:true,force:true}));
  const replay=await ProviderReplay.open({config,home,identity:'first'});
  const item={id:'rs_first',type:'reasoning',content:[],summary:[],encrypted_content:'cipher-first'};
  const options=replay.options({openai:{itemId:'rs_upstream',reasoningEncryptedContent:'cipher-first'}});
  replay.remember(item,options);await replay.flush();
  const same=await ProviderReplay.open({config,home,identity:'first'});assert.deepEqual(same.restore(item),options);
  assert.throws(()=>same.restore({...item,encrypted_content:'cipher-changed'}));
  const foreign=await ProviderReplay.open({config,home,identity:'second'});assert.throws(()=>foreign.restore(item));
  assert.throws(()=>translateResponses({input:[item]},definitions));
  for(const metadata of [{openai:{reasoningEncryptedContent:config.apiKey}},{openai:{itemId:'../other'}},
    {openai:{itemId:'rs',reasoningEncryptedContent:'x'.repeat(131073)}},{openai:{itemId:'rs',phase:'execute'}},
    {openai:{itemId:'rs',previousResponseId:'foreign'}},{openai:{itemId:'rs'},google:{thoughtSignature:'foreign'}}])assert.throws(()=>replay.options(metadata));
});

test('native Responses refuses reasoning without terminal ciphertext and provider-side custom actions',async t=>{
  for(const part of [
    {type:'reasoning-start',id:'rs:0',providerMetadata:{openai:{itemId:'rs',reasoningEncryptedContent:null}}},
    {type:'custom',kind:'openai.compaction',providerMetadata:{openai:{encryptedContent:'unrequested'}}},
  ]) {
    const bridge=await startBridge({config,definitions,token:'owned-loopback',stream:()=>({fullStream:(async function*(){yield part;yield {type:'text-delta',text:'Do not accept missing state'};})(),finishReason:Promise.resolve('stop'),usage:Promise.resolve({})})});
    const body=await (await post(bridge,[{role:'user',content:'Read'}])).text();assert(body.includes('response.failed'));assert(!body.includes('response.completed'));assert(!body.includes(config.apiKey));await bridge.close();
  }
});
