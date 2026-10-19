const count = value => Number.isSafeInteger(value) && value >= 0 ? value : undefined;
// Do not turn omitted provider fields into zero. DeepSeek uses a separate
// prompt_cache_hit_tokens field which the generic SDK converter misses.
export function compatibleUsage(raw) {
  const input=count(raw?.prompt_tokens),output=count(raw?.completion_tokens);
  const cache=count(raw?.prompt_tokens_details?.cached_tokens ?? raw?.prompt_cache_hit_tokens);
  const reasoning=count(raw?.completion_tokens_details?.reasoning_tokens);
  return {inputTokens:{total:input,noCache:input!==undefined&&cache!==undefined&&cache<=input?input-cache:undefined,cacheRead:cache!==undefined&&input!==undefined&&cache<=input?cache:undefined,cacheWrite:undefined},
    outputTokens:{total:output,text:output!==undefined&&reasoning!==undefined&&reasoning<=output?output-reasoning:undefined,reasoning:reasoning!==undefined&&output!==undefined&&reasoning<=output?reasoning:undefined},raw};
}
export function responseUsage(usage = {}) {
  const input=count(usage.inputTokens),output=count(usage.outputTokens);
  const cache=count(usage.inputTokenDetails?.cacheReadTokens),write=count(usage.inputTokenDetails?.cacheWriteTokens),reasoning=count(usage.outputTokenDetails?.reasoningTokens);
  return {input_tokens:input??null,output_tokens:output??null,total_tokens:input!==undefined&&output!==undefined?count(input+output)??null:null,
    input_tokens_details:{cached_tokens:cache!==undefined&&input!==undefined&&cache<=input?cache:null,cache_write_tokens:write!==undefined&&input!==undefined&&write<=input?write:null},output_tokens_details:{reasoning_tokens:reasoning!==undefined&&output!==undefined&&reasoning<=output?reasoning:null}};
}
export function wireUsage(usage) {
  // The pinned native protocol requires integer counters and rejects null.
  // Unknown values remain null in our ledger; omit optional detail on wire.
  const details=value=>Object.fromEntries(Object.entries(value).filter(([,v])=>v!==null));
  const input=details(usage.input_tokens_details),output=details(usage.output_tokens_details);
  return {input_tokens:usage.input_tokens??0,output_tokens:usage.output_tokens??0,total_tokens:usage.total_tokens??0,
    ...(Object.keys(input).length?{input_tokens_details:input}:{}),...(Object.keys(output).length?{output_tokens_details:output}:{})};
}
export function recordUsage(session, record) {
  const ledger=session.usage??={version:1,requests:0,totals:{input:0,output:0,total:0,cachedInput:0,reasoningOutput:0},records:[]};
  if(ledger.records.some(r=>r.id===record.id))return;
  const values={input:record.usage.input_tokens,output:record.usage.output_tokens,total:record.usage.total_tokens,cachedInput:record.usage.input_tokens_details.cached_tokens,reasoningOutput:record.usage.output_tokens_details.reasoning_tokens};
  for(const [key,value] of Object.entries(values))ledger.totals[key]=ledger.totals[key]===null||value===null?null:ledger.totals[key]+value;
  ledger.requests++;
  ledger.records.push({...record,goalId:session.goal?.id??null});
  if(ledger.records.length>128)ledger.records.splice(0,ledger.records.length-128);
}
export function validUsage(value) {
  // Bounded, numeric-only accounting: never persist provider raw payloads,
  // headers, credentials, prompts or private reasoning in this ledger.
  if(!value || value.version!==1 || !Number.isSafeInteger(value.requests)||value.requests<0||!Array.isArray(value.records)||value.records.length>128)return false;
  const totals=['input','output','total','cachedInput','reasoningOutput'];
  if(!value.totals||Object.keys(value.totals).length!==totals.length||!totals.every(k=>value.totals[k]===null||count(value.totals[k])!==undefined))return false;
  return value.records.every(r=>typeof r.id==='string'&&/^resp_[a-f0-9]{32}$/.test(r.id)&&typeof r.at==='string'&&Number.isFinite(Date.parse(r.at))&&['human','workflow','compaction'].includes(r.origin)
    &&(r.requestId===undefined||r.requestId===null||typeof r.requestId==='string'&&/^[a-f0-9-]{36}$/.test(r.requestId))
    &&(r.state===undefined||['completed','failed'].includes(r.state))
    && typeof r.turnId==='string'&&r.turnId.length<=80 &&(r.goalId===null||typeof r.goalId==='string'&&r.goalId.length<=80)
    && ['input_tokens','output_tokens','total_tokens'].every(k=>r.usage?.[k]===null||count(r.usage?.[k])!==undefined)
    && [r.usage?.input_tokens_details?.cached_tokens,r.usage?.input_tokens_details?.cache_write_tokens,r.usage?.output_tokens_details?.reasoning_tokens].every(n=>n===null||count(n)!==undefined)
    &&r.context&&Object.values(r.context).every(n=>count(n)!==undefined));
}
