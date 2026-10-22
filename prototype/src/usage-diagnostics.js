// Only fixed diagnostic codes cross the telemetry boundary. Error messages,
// URLs, model responses, task names and native stack traces never do.
const details=new WeakMap();
export function diagnosticError(error,fields={}) {
  const value=error instanceof Error?error:new Error(typeof error==='string'?error:'Operation failed');
  details.set(value,{...fields,...details.get(value)});
  return value;
}
export async function diagnosticStep(stage,run) {
  try{return await run();}catch(error){throw diagnosticError(error,{stage,diagnosticOrigin:'explicit'});}
}
export function actionStage(action) {
  return ({search:'catalog_request',search_more:'catalog_page',download_submit:'download_queue',save_project:'project_save',
    inspect:'artifact_inspect',process:'processing',export:'export',import:'import',source_connect:'source_connect',
    model_connect:'model_connect',model_test:'model_test',agent_send:'agent_dispatch',agent_confirm:'agent_confirmation',
    agent_revise:'agent_confirmation',agent_stop:'agent_dispatch',retry:'task_retry',cancel:'task_cancel',
    update_check:'update_check',update_download:'update_download',update_install:'update_install'})[action]||'operation';
}
export function failureDiagnostics(error,stage='operation') {
  const text=String(error?.message??error??''),explicit=details.get(error)||{};
  const status=explicit.httpStatus??error?.status??text.match(/\bHTTP(?: error)?[ :]+([45]\d\d)\b/i)?.[1];
  let reason='unknown',inferredStage=stage;
  if(error?.name==='AbortError')reason='cancelled';
  else if(error?.name==='TimeoutError'||/timed? ?out|timeout|deadline/i.test(text))reason='timeout';
  else if(/(?:sha.?256|checksum|digest).*mismatch/i.test(text)){reason='checksum_mismatch';inferredStage='artifact_validation';}
  else if(/incomplete|truncated|byte.*(?:mismatch|differ)|size.*(?:mismatch|differ)/i.test(text)){reason='incomplete_data';}
  else if(/no space|disk full/i.test(text))reason='disk_full';
  else if(/permission denied|access denied/i.test(text))reason='permission_denied';
  else if(String(status)==='429')reason='rate_limited';
  else if(['401','403'].includes(String(status))||/unauthori[sz]ed|forbidden|credential|login|sign.?in/i.test(text))reason='authorization';
  else if(String(status)==='404'||/not found|missing file|unknown (?:job|project|asset)/i.test(text))reason='not_found';
  else if(['400','422'].includes(String(status)))reason='validation';
  else if(Number(status)>=500)reason='upstream_unavailable';
  else if(error instanceof SyntaxError)reason='invalid_response';
  else if(/invalid|mismatch|unsupported|not match|validation|different.*range/i.test(text))reason='validation';
  else if(/network|fetch|connection|offline|transport/i.test(text))reason='network';
  else if(/storage|disk|write|read file|open file/i.test(text))reason='storage_io';
  return {stage:explicit.stage||inferredStage,reason:explicit.reason||reason,
    httpStatus:/^[1-5]\d\d$/.test(String(status))?String(status):'unknown',
    retries:/^[0-3]$/.test(String(explicit.retries))?String(explicit.retries):'unknown',
    diagnosticOrigin:explicit.diagnosticOrigin||(text?'classified':'unavailable')};
}
