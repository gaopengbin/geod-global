// Controlled upstream generation only. Callers still use the actual SDK,
// pinned Codex and native GeoD core; this is not live-model evidence.
export function openaiResponsesReply(response,{model,callId,inputTokens=10,text='Controlled native projects: 0.'}) {
  response.writeHead(200,{'content-type':'text/event-stream'});
  const emit=(type,data)=>response.write(`event: ${type}\ndata: ${JSON.stringify({type,...data})}\n\n`);
  const envelope={id:'resp_controlled',model,created_at:1,status:'in_progress',output:[]};
  emit('response.created',{response:envelope});const output=[];
  if(callId) {
    const item={id:`rs_${callId}`,type:'reasoning',status:'in_progress',summary:[],encrypted_content:`controlled-incomplete-${callId}`};
    emit('response.output_item.added',{output_index:0,item});
    const done={...item,status:'completed',encrypted_content:`controlled-opaque-${callId}`};
    emit('response.output_item.done',{output_index:0,item:done});output.push(done);
    const tool={id:`fc_${callId}`,type:'function_call',status:'in_progress',call_id:callId,name:'geod_projects_list',arguments:''};
    emit('response.output_item.added',{output_index:1,item:tool});
    emit('response.function_call_arguments.delta',{output_index:1,item_id:tool.id,delta:'{}'});
    const finished={...tool,status:'completed',arguments:'{}'};emit('response.output_item.done',{output_index:1,item:finished});output.push(finished);
  }else {
    const item={id:'msg_controlled',type:'message',role:'assistant',status:'in_progress',content:[],phase:'final_answer'};
    emit('response.output_item.added',{output_index:0,item});
    emit('response.output_text.delta',{output_index:0,item_id:item.id,content_index:0,delta:text});
    const done={...item,status:'completed',content:[{type:'output_text',text,annotations:[]}]};emit('response.output_item.done',{output_index:0,item:done});output.push(done);
  }
  emit('response.completed',{response:{...envelope,status:'completed',output,usage:{input_tokens:inputTokens,output_tokens:10,total_tokens:inputTokens+10}}});response.end();
}
