// GeoD Agent 0.2.5 turn-token-usage.ts semantics, adapted to Global's local ledger.
// No billing request, model output or prompt is needed to display these receipts.
const count=value=>Number.isSafeInteger(value)&&value>=0;
const optional=value=>value===null||count(value);
export function validUsageLedger(value){
  if(!value||value.version!==1||!count(value.requests)||!Array.isArray(value.records)||value.records.length>128||value.records.length>value.requests)return false;
  if(!value.totals||!['input','output','total','cachedInput','reasoningOutput'].every(key=>optional(value.totals[key])))return false;
  return value.records.every(row=>/^resp_[a-f0-9]{32}$/.test(row.id)&&typeof row.turnId==='string'&&row.turnId.length<=80
    &&(row.requestId==null||typeof row.requestId==='string'&&/^[a-f0-9-]{36}$/.test(row.requestId))
    &&(row.state==null||['completed','failed'].includes(row.state))&&['human','workflow','compaction'].includes(row.origin)
    &&['input_tokens','output_tokens','total_tokens'].every(key=>optional(row.usage?.[key]))
    &&[row.usage?.input_tokens_details?.cached_tokens,row.usage?.input_tokens_details?.cache_write_tokens,row.usage?.output_tokens_details?.reasoning_tokens].every(optional)
    &&row.context&&Object.values(row.context).every(count));
}

export function requestTokenUsage(ledger,requestId,turnIds=[],ambiguousTurns=new Set(),running=false){
  const records=ledger?.records??[];
  const matched=records.filter(row=>row.requestId?row.requestId===requestId:turnIds.includes(row.turnId)&&!ambiguousTurns.has(row.turnId));
  const rows=[...new Map(matched.map(row=>[row.id,row])).values()];
  const actual=rows.filter(row=>count(row.usage?.input_tokens)&&count(row.usage?.output_tokens));
  const input=actual.reduce((n,row)=>n+row.usage.input_tokens,0),output=actual.reduce((n,row)=>n+row.usage.output_tokens,0);
  const detail=(group,key,parent)=>actual.length&&actual.every(row=>count(row.usage[group]?.[key])&&row.usage[group][key]<=row.usage[parent])?actual.reduce((n,row)=>n+row.usage[group][key],0):null;
  // Old records cannot prove failed requests were accounted for. A truncated
  // round also stays partial; later rounds with intact receipts remain complete.
  const truncated=Boolean(ledger&&ledger.requests>records.length&&matched.includes(records[0]));
  const partial=running||truncated||rows.some(row=>!row.requestId)||actual.length<rows.length;
  return {status:!rows.length?running?'pending':'unavailable':partial?'partial':'complete',input,output,total:input+output,
    cachedInput:detail('input_tokens_details','cached_tokens','input_tokens'),reasoningOutput:detail('output_tokens_details','reasoning_tokens','output_tokens'),
    requests:rows.length,knownRequests:actual.length,failedRequests:rows.filter(row=>row.state==='failed').length,
    workflowRequests:rows.filter(row=>row.origin==='workflow').length,compactionRequests:rows.filter(row=>row.origin==='compaction').length,
    maxTools:rows.length&&rows.every(row=>count(row.context?.toolCount))?Math.max(...rows.map(row=>row.context.toolCount)):null,
    truncated,running};
}

export function transcriptTokenUsage(rows,ledger,activeRequestId){
  const requests=[],owners=new Map();let request;
  for(const row of rows){
    if(row.type==='user'){request={id:row.id,turnIds:new Set(),target:null};requests.push(request);}
    if(!request)continue;
    if(row.type==='work'){request.target=row.id;for(const id of row.turnIds??[])request.turnIds.add(id);}
    if(row.type==='assistant'){request.target=row.id;if(row.turnId)request.turnIds.add(row.turnId);}
  }
  for(const request of requests)for(const id of request.turnIds){const set=owners.get(id)??new Set();set.add(request.id);owners.set(id,set);}
  const ambiguous=new Set([...owners].filter(([,ids])=>ids.size>1).map(([id])=>id));
  return new Map(requests.filter(request=>request.target).map(request=>[request.target,requestTokenUsage(ledger,request.id,[...request.turnIds],ambiguous,request.id===activeRequestId)]));
}
