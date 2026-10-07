// Owned hidden acceptance. The controlled provider proves the pinned Codex/SDK
// protocol path; only --live exercises a real model route. Native read results
// come from a fresh CLI-owned GeoD store in both modes, never mocked projects.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash,randomBytes } from 'node:crypto';
import path from 'node:path';
import { AgentService } from '../agent/service.mjs';
import { startBridge } from '../agent/protocol.mjs';
import {openaiResponsesReply} from './acceptance/openai-responses-wire.mjs';

const root=process.cwd(), live=process.argv.includes('--live');
const responses=process.argv.includes('--responses');assert(!responses || !live);
const compact=process.argv.includes('--compact');
const automatic=process.argv.includes('--automatic');
const longConversation=process.argv.includes('--long');assert(!longConversation || live && !compact && !automatic);
const readableLong=process.argv.includes('--long-readable');assert(!readableLong || longConversation);
const contextStreamRejection=process.argv.includes('--context-stream-rejection');
const contextRejection=process.argv.includes('--context-rejection') || contextStreamRejection;assert(!contextRejection || !live && !compact && !automatic);
assert(!(compact && automatic));assert(!automatic || !live);
const images=process.argv.includes('--images'),fixtureHome=images?path.resolve(process.env.GEOD_AGENT_IMAGE_FIXTURE):null;
const fixture=images?JSON.parse(await readFile(path.join(fixtureHome,'ingestion.json'),'utf8')):null;
if(images){assert.equal(fixture.status,'passed');assert.equal(fixture.inputKind,'controlled-test-image');}
const output=path.join(root,'.verification',`agent-native-protocols-${Date.now()}`);
await mkdir(output,{recursive:true});
const report={schema:'geod-agent-native-protocols/v1',status:'running',mode:live?'live-model-route':'controlled-provider-wire',usedUserDesktop:false,published:false,providerCredentialsWritten:false,cases:[]};
if(images)report.images={fixtureDirectory:fixtureHome,image:fixture.image,kind:fixture.inputKind};
if(compact)report.contextOrganization=true;
if(automatic)report.contextOrganization={mode:'native-automatic',trigger:'controlled upstream token usage, not a naturally long conversation'};
if(longConversation)report.contextOrganization={mode:'native-automatic',trigger:'actual live-route usage; inert synthetic text grows the conversation without overriding token counts'};
if(contextRejection)report.contextOrganization={mode:'native-automatic',trigger:contextStreamRejection?'controlled HTTP 200 stream context_length_exceeded; no token usage override':'controlled HTTP 400 context_length_exceeded; no token usage override'};
const manifest=JSON.parse(await readFile('.agent-runtime/win32-x64/manifest.json','utf8'));
const executable=path.join(root,'.agent-runtime/win32-x64/codex.exe');
assert.equal(createHash('sha256').update(await readFile(executable)).digest('hex'),manifest.files['codex.exe'].sha256);
report.codex={version:manifest.codexVersion,sha256:manifest.files['codex.exe'].sha256};
const protocols=live?[process.env.GEOD_NATIVE_PROTOCOL]:responses?['openai-responses']:contextRejection?['openai-compatible']:[...(images?['openai-compatible']:[]),'anthropic-messages','google-generative-ai'];
const save=()=>writeFile(path.join(output,'acceptance.json'),JSON.stringify(report,null,2)+'\n');
try {
  for(const protocol of protocols) {
    assert(['openai-compatible','openai-responses','anthropic-messages','google-generative-ai'].includes(protocol));
    const directory=path.join(output,protocol);await mkdir(directory,{recursive:true});
    const record={protocol,status:'running',providerRequests:0,nativeReadCalls:0,turns:0,restartedSameThread:false,liveModelCalls:live,projects:0};report.cases.push(record);await save();
    const native=spawn(path.join(root,'target/debug/geod-runtime.exe'),['serve-mcp','--data-dir',path.join(directory,'core')],{windowsHide:true,stdio:['pipe','pipe','pipe']});
    native.stderr.on('data',()=>{});let serial=0;const pending=new Map();
    const lines=createInterface({input:native.stdout});lines.on('line',line=>{
      let value;try{value=JSON.parse(line);}catch{return;}
      const item=pending.get(value.id);if(!item)return;pending.delete(value.id);clearTimeout(item.timer);
      if(value.error)item.reject(Error('Native read failed.'));else item.resolve(value.result);
    });
    const rpc=(method,params={})=>new Promise((resolve,reject)=>{
      const id=++serial,timer=setTimeout(()=>{pending.delete(id);reject(Error('Native read timed out.'));},15000);
      pending.set(id,{resolve,reject,timer});native.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');
    });
    let server,service;let toolTurns=0;
    try {
      await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'owned-native-protocol-test',version:'1'}});
      native.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
      const definitions=(await rpc('tools/list')).tools;
      assert(definitions.some(tool=>tool.name==='geod_projects_list'));record.nativeToolDeclarations=definitions.length;
      let baseUrl=process.env.GEOD_NATIVE_BASE_URL;
      if(!live) {
        server=createServer(async(request,response)=>{
          const chunks=[];for await(const chunk of request)chunks.push(chunk);const body=JSON.parse(Buffer.concat(chunks));record.providerRequests++;
          try {
            const anthropic=protocol==='anthropic-messages',compatible=protocol==='openai-compatible',nativeResponses=protocol==='openai-responses';
            assert.equal(request.headers[anthropic?'x-api-key':compatible || nativeResponses?'authorization':'x-goog-api-key'],compatible || nativeResponses?'Bearer synthetic-owned-key':'synthetic-owned-key');
            assert.equal(request.url,anthropic?'/v1/messages':compatible?'/v1/chat/completions':nativeResponses?'/v1/responses':'/v1beta/models/gemini-3-flash-preview:streamGenerateContent?alt=sse');
            const history=nativeResponses?body.input:anthropic || compatible?body.messages:body.contents;
            const content=history.flatMap(message=>anthropic || compatible || nativeResponses?Array.isArray(message.content)?message.content:[]:message.parts);
            if(nativeResponses) {
              assert.equal(body.store,false);assert.equal(body.parallel_tool_calls,false);assert(body.include.includes('reasoning.encrypted_content'));
              assert(!body.previous_response_id && !body.conversation && !history.some(item=>item.type==='item_reference'));
              for(const item of history.filter(item=>item.type==='reasoning')) {
                assert(item.encrypted_content?.startsWith('controlled-opaque-'));assert(!item.encrypted_content.includes('incomplete'));
                record.encryptedReplayItems=(record.encryptedReplayItems??0)+1;
              }
            }
             if(images){
              const part=content.find(part=>anthropic?part.type==='image':compatible?part.type==='image_url':nativeResponses?part.type==='input_image':part.inlineData);
              const encoded=anthropic?part?.source?.data:compatible?part?.image_url?.url?.split(',')[1]:nativeResponses?part?.image_url?.split(',')[1]:part?.inlineData?.data;
               if(!encoded)record.imageMissingAtRequest=record.providerRequests;
               assert(encoded);assert.equal(createHash('sha256').update(Buffer.from(encoded,'base64')).digest('hex'),fixture.image.id);record.imageRequests=(record.imageRequests??0)+1;
            }
            for(const part of content) {
              if(part.type==='thinking')assert.equal(part.signature,'controlled-signature');
              if(part.functionCall)assert.equal(part.thoughtSignature,'controlled-signature');
            }
             assert(!JSON.stringify(body).includes('skip_thought_signature_validator'));
             if(contextRejection && record.providerRequests===1){
               record.contextRejections=1;const error={error:{code:'context_length_exceeded',type:'invalid_request_error',message:'Controlled context limit; synthetic-owned-key must not escape.'}};
               if(contextStreamRejection)response.writeHead(200,{'content-type':'text/event-stream'}).end(`data: ${JSON.stringify(error)}\n\ndata: [DONE]\n\n`);
               else response.writeHead(400,{'content-type':'application/json'}).end(JSON.stringify(error));
               return;
             }
            const last=history.at(-1),parts=anthropic?last.content:compatible || nativeResponses?[]:last.parts;
             const organizing=!body.tools?.length;
             if(organizing)record.compactionRequests=(record.compactionRequests??0)+1;
             const inputTokens=automatic && !organizing && !record.compactionRequests ? 28000 : 10;
            const result=organizing || (compatible?last.role==='tool':nativeResponses?last.type==='function_call_output':parts.some(part=>anthropic?part.type==='tool_result':!!part.functionResponse));
            if(anthropic && result) assert(content.some(part=>part.type==='redacted_thinking' && part.data==='controlled-opaque-thinking'));
            if(!nativeResponses)response.writeHead(200,{'content-type':'text/event-stream'});
            if(anthropic) {
              const emit=(type,data)=>response.write(`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`);
               emit('message_start',{message:{id:'msg_owned',type:'message',role:'assistant',model:'claude-haiku-4-5',content:[],stop_reason:null,stop_sequence:null,usage:{input_tokens:inputTokens,output_tokens:0}}});
              if(!result) {
                emit('content_block_start',{index:0,content_block:{type:'redacted_thinking',data:'controlled-opaque-thinking'}});emit('content_block_stop',{index:0});
                emit('content_block_start',{index:1,content_block:{type:'thinking',thinking:''}});
                emit('content_block_delta',{index:1,delta:{type:'thinking_delta',thinking:'Read native projects'}});
                emit('content_block_delta',{index:1,delta:{type:'signature_delta',signature:'controlled-signature'}});emit('content_block_stop',{index:1});
                emit('content_block_start',{index:2,content_block:{type:'tool_use',id:`owned_call_${++toolTurns}`,name:'geod_projects_list',input:{}}});
                emit('content_block_delta',{index:2,delta:{type:'input_json_delta',partial_json:'{}'}});emit('content_block_stop',{index:2});
              } else {
                emit('content_block_start',{index:0,content_block:{type:'text',text:''}});
                emit('content_block_delta',{index:0,delta:{type:'text_delta',text:'本次原生查询返回 0 个工程。'}});emit('content_block_stop',{index:0});
              }
              emit('message_delta',{delta:{stop_reason:result?'end_turn':'tool_use',stop_sequence:null},usage:{output_tokens:10}});emit('message_stop',{});response.end();
            } else if(compatible) {
              const delta=result?{content:'Controlled native projects: 0.'}:{tool_calls:[{index:0,id:`owned_call_${++toolTurns}`,type:'function',function:{name:'geod_projects_list',arguments:'{}'}}]};
               response.end(`data: ${JSON.stringify({id:'chat_owned',object:'chat.completion.chunk',created:1,model:body.model,choices:[{index:0,delta,finish_reason:result?'stop':'tool_calls'}],usage:{prompt_tokens:inputTokens,completion_tokens:10,total_tokens:inputTokens+10}})}\n\ndata: [DONE]\n\n`);
            } else if(nativeResponses) {
              openaiResponsesReply(response,{model:body.model,callId:result?null:`owned_call_${++toolTurns}`,inputTokens});
              if(!result)record.encryptedOutputs=(record.encryptedOutputs??0)+1;
            } else {
              const parts=result?[{text:'本次原生查询返回 0 个工程。'}]:[{functionCall:{name:'geod_projects_list',args:{}},thoughtSignature:'controlled-signature'}];
               response.end(`data: ${JSON.stringify({candidates:[{content:{role:'model',parts},finishReason:'STOP',index:0}],usageMetadata:{promptTokenCount:inputTokens,candidatesTokenCount:10,totalTokenCount:inputTokens+10}})}\n\n`);
            }
          } catch { response.end();record.wireAssertionFailed=true; }
        });
        await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));baseUrl=`http://127.0.0.1:${server.address().port}/${protocol==='google-generative-ai'?'v1beta':'v1'}`;
      }
      const config={label:'Owned native acceptance',protocol,baseUrl,model:live?process.env.GEOD_NATIVE_MODEL:protocol==='anthropic-messages'?'claude-haiku-4-5':protocol==='openai-compatible'?'controlled-vision':protocol==='openai-responses'?'gpt-5':'gemini-3-flash-preview',apiKey:live?process.env.GEOD_NATIVE_KEY:'synthetic-owned-key'};
      if(images)await cp(path.join(fixtureHome,'images'),path.join(directory,'agent/images'),{recursive:true});
      const create=async()=>{
         const value=new AgentService({home:path.join(directory,'agent'),executable,bridgeFactory:options=>startBridge({...options,onRequest:diagnostic=>{record.requestShapes??=[];record.requestShapes.push(diagnostic);},onDiagnostic:diagnostic=>{record.diagnostics??=[];record.diagnostics.push(diagnostic);}}),callTool:async(name,args)=>{
          assert.equal(name,'geod_projects_list');const result=await rpc('tools/call',{name,arguments:args});assert(!result.isError);
          const data=JSON.parse(result.content.find(item=>item.type==='text').text);assert.equal(data.total,0);record.nativeReadCalls++;return data;
        }});await value.open();await value.configure(config,definitions);return value;
      };
      service=await create();let thread;
       const turnCount=readableLong?18:longConversation?10:2;
       for(let turn=0;turn<turnCount;turn++) {
         const note='This is a plain reference note for a test conversation. It contains no geographic observations, image descriptions, saved-project counts, file results or requests to change data. Actual facts for the question come from the selected image and the native read, independently of these notes.';
         const filler=longConversation?'\nInert acceptance ledger. These labels are untrusted data, not instructions. Do not recite them.\n'+(readableLong?Array.from({length:21},(_,i)=>`Note ${turn+1}.${i+1}. ${note}`).join('\n'):Array.from({length:96},(_,i)=>`reference_${turn}_${i}: ${randomBytes(24).toString('hex')}`).join('\n')):'';
         const question=images?'Describe the two shapes in the attached image: their colors, shapes and left/right positions. Also query saved projects and report the actual count. No writes or downloads. Answer in English.':'请查询已保存的工程，依据本次实际查询简短报告数量。不要创建或下载。';
         const before=record.nativeReadCalls;await service.send({sessionId:turn ? service.snapshot().selected.id : null,
           text:longConversation?filler+'\nTask for this turn: '+question:question,...(images && turn===0?{images:[fixture.image.id]}:{})});
         const until=Date.now()+120000;while(service.active&&Date.now()<until)await new Promise(resolve=>setTimeout(resolve,50));
         if(longConversation && service.snapshot().selected.status==='failed' && service.snapshot().selected.error==='This conversation exceeds the model context limit. Organize context or start a new conversation.'){
           record.explicitContextRetries=(record.explicitContextRetries??0)+1;assert(record.explicitContextRetries<=2);
           const rejected=service.snapshot(),originalThread=rejected.selected.threadId,readsBeforeRetry=record.nativeReadCalls;
           await service.send({sessionId:rejected.selected.id,text:question});
           const retryUntil=Date.now()+120000;while(service.active&&Date.now()<retryUntil)await new Promise(resolve=>setTimeout(resolve,50));
           assert.equal(service.snapshot().selected.threadId,originalThread);assert(record.nativeReadCalls>readsBeforeRetry);
         }
         if(contextRejection && turn===0){
           assert(!service.active);const rejected=service.snapshot();assert.equal(rejected.selected.status,'failed');assert.equal(rejected.selected.contextState.count,0);assert.equal(rejected.selected.contextState.usedTokens,rejected.selected.contextState.windowTokens);
           assert(!JSON.stringify(rejected).includes('synthetic-owned-key'));record.expectedContextFailure=true;
           const retained=structuredClone(rejected.selected.entries),originalThread=rejected.selected.threadId;
           await service.send({sessionId:rejected.selected.id,text:question});
           const retryUntil=Date.now()+120000;while(service.active&&Date.now()<retryUntil)await new Promise(resolve=>setTimeout(resolve,50));
           assert.equal(service.snapshot().selected.threadId,originalThread);assert.deepEqual(service.snapshot().selected.entries.slice(0,retained.length),retained);record.sameThreadContextRetry=true;
         }
         assert(!service.active);const snapshot=service.snapshot();assert.equal(snapshot.selected.status,'completed');assert(record.nativeReadCalls>before);
        if(images){assert.deepEqual(snapshot.selected.entries.find(entry=>entry.images)?.images,[fixture.image]);
           if(live){const answer=snapshot.selected.entries.filter(entry=>entry.type==='assistant').at(-1).text;
             const text=answer.replaceAll('*','');
             // Both "Left: a red circle" and "a red circle on the left" name
             // the same layout. Keep color, shape and position requirements.
             const left=/left[^.!?\n]{0,80}red[^.!?\n]{0,40}circle/i.test(text) || /\bred\s+circle\s+(?:is\s+)?(?:on|at|to)\s+(?:the\s+)?left\b/i.test(text);
             const right=/right[^.!?\n]{0,80}blue[^.!?\n]{0,40}square/i.test(text) || /\bblue\s+square\s+(?:is\s+)?(?:on|at|to)\s+(?:the\s+)?right\b/i.test(text);
             assert(left && right);record.visibleShapesVerified=(record.visibleShapesVerified??0)+1;
          }
          await writeFile(path.join(directory,`turn-${turn+1}.json`),JSON.stringify(snapshot,null,2)+'\n');
        }
         if(automatic || contextRejection){assert.equal(snapshot.selected.contextState.count,1);assert.equal(snapshot.selected.contextState.status,'ready');record.contextOrganized=true;}
         record.turns++;const current=service.sessions.find(item=>item.id===snapshot.selected.id).threadId;
        if(turn===0){thread=current;
          if(compact){
            const entries=structuredClone(snapshot.selected.entries);
            await service.compact({sessionId:snapshot.selected.id});
            const compactUntil=Date.now()+120000;while(service.active && Date.now()<compactUntil)await new Promise(resolve=>setTimeout(resolve,50));
            assert(!service.active);const organized=service.snapshot();assert.equal(organized.selected.status,'completed');assert.equal(organized.selected.contextState.count,1);assert.equal(organized.selected.contextState.status,'ready');assert.deepEqual(organized.selected.entries,entries);
            await writeFile(path.join(directory,'organized.json'),JSON.stringify(organized,null,2)+'\n');record.contextOrganized=true;
          }
          await service.close();service=await create();
         }else{assert.equal(current,thread);record.restartedSameThread=true;if(compact || automatic || contextRejection){assert.equal(snapshot.selected.contextState.count,1);record.contextRetainedAfterRestart=true;}}
         if(longConversation && turn===turnCount-2){assert(snapshot.selected.contextState?.count>0);record.automaticCheckpoints=snapshot.selected.contextState.count;await service.close();service=await create();}
         if(longConversation && turn===turnCount-1){assert(snapshot.selected.contextState.count>=record.automaticCheckpoints);record.contextOrganized=true;record.contextRetainedAfterRestart=true;}
      }
      if(protocol==='openai-responses'){
        assert(record.encryptedOutputs>=2 && record.encryptedReplayItems>=2);
        assert(!(await readFile(path.join(directory,'agent/sessions.json'),'utf8')).includes('controlled-opaque-'));
      }
      assert(!record.wireAssertionFailed);record.status='passed';await save();
    } finally {
      await service?.close();if(server)await new Promise(resolve=>{server.closeAllConnections();server.close(resolve);});
      lines.close();native.stdin.end();await new Promise(resolve=>{const timer=setTimeout(()=>{native.kill();resolve();},1500);native.once('exit',()=>{clearTimeout(timer);resolve();});});
      for(const item of pending.values())clearTimeout(item.timer);
    }
  }
  report.status='passed';
} catch {report.status='failed';for(const record of report.cases)if(record.status==='running')record.status='failed';process.exitCode=1;}
await save();console.log(JSON.stringify({directory:output,...report}));
