import schema from '../../schemas/usage-events.json' with {type:'json'};
import {failureDiagnostics,actionStage,diagnosticError} from './usage-diagnostics.js';
import {usage,usageFields,usageJobSource,classifyUsageError,countBucket,durationBucket} from './usage-client.js';

const actions={
  distribution:{update_check:'update_check',update_download:'update_download',update_install:'update_install'},
  runtime:{create:'download_submit',downloadProject:'download_submit',createProject:'save_project',addProjectScenes:'save_project',
    cancel:'cancel',retry:'retry',raster:'inspect',composite:'inspect',rgb:'inspect',mosaicProject:'process',prepareProject:'process',runRgb:'process',runRecipe:'process',planRecipe:'process',saveRecipe:'save_project',reveal:'inspect',revealPackage:'inspect',package:'export',connectAccount:'source_connect',verifyAccount:'source_connect'},
  agent:{send:'agent_send',approvePlan:'agent_confirm',revisePlan:'agent_revise',interrupt:'agent_stop',saveModel:'model_connect',testModel:'model_test',attachImage:'import',attachDocument:'import'},
  'public-dataset':{resolve:'search',connect:'source_connect'},
  stac:{connect:'source_connect',search:'search',project:'save_project',downloads:'download_submit',inspect:'inspect'},
  features:{connect:'source_connect',query:'search'},gscloud:{search:'search',download:'download_submit'},
  wcs:{connect:'source_connect',plan:'search',project:'save_project',downloads:'download_submit',inspect:'inspect'},
  official:{search:'search',download:'download_submit',inspect:'inspect'},
  'official-vector':{search:'search',download:'download_submit',inspect:'inspect'},
  wms:{connect:'source_connect',get:'preview',export:'export'},
  tiles:{connect:'source_connect',extract:'export',import:'import'},
  local:{open:'import',inspect:'inspect',export:'export'},overture:{search:'search',download:'download_submit',metadata:'inspect'},
  gebco:{create:'download_submit',inspect:'inspect'},'earth-engine':{download:'download_submit',inspect:'inspect'},
  vector:{import:'import',inspect:'inspect',export:'export'},processing:{run:'process'},
};
const turns=new Map();
export function usagePage(hash=globalThis.location?.hash||'') {
  try{const page=decodeURIComponent(hash.replace(/^#/, '').split('?')[0])||'Home';return schema.fields.page.includes(page)?page:'Other';}catch{return 'Other';}
}
export function usageContext({agent=false,scene,page=usagePage()}={}) {
  const surface=agent?(scene?.kind==='conversation'||page==='Home'?'agent_home':scene?.page==='Explore'||page==='Explore'?'agent_explore':'agent_workspace')
    :({Explore:'manual_explore',Workspace:'workspace','My Data':'library',Tasks:'tasks',Settings:'settings'})[page]||'other';
  return {page,surface,channel:agent?'agent':'manual'};
}
export function usageSource(value,fallback='other') {
  const source=typeof value==='string'?value.replace(/^dataset-/,''):value;
  return schema.fields.source.includes(source)?source:schema.fields.source.includes(fallback)?fallback:'other';
}
export const usageError=classifyUsageError;
const resultCount=value=>Array.isArray(value)?value.length:Array.isArray(value?.scenes)?value.scenes.length:Array.isArray(value?.items)?value.items.length
  :Array.isArray(value?.jobs)?value.jobs.length:Array.isArray(value?.plan?.sources)&&value.plan.sources.length===0?0:Number.isSafeInteger(value?.featureCount)?value.featureCount:null;
const jobsIn=value=>Array.isArray(value?.jobs)?value.jobs:Array.isArray(value)?value:value?.job?[value.job]:value?.id&&value.status?[value]:[];
export function beginUsage(action,fields={},client=usage) {
  const active=client.enabled(),generation=client.generation(),operationId=active?crypto.randomUUID():null,start=Date.now();
  const prefix=['search','search_more','download_submit'].includes(action)?action:'operation';
  const context={...usageContext(),...usageFields(fields),action};
  if(active)client.record(`${prefix}_started`,context,operationId);
  let finished=false;
  return {operationId,fields:context,finish(value,error){
    if(!active||finished||generation!==client.generation())return;
    finished=true;
    const count=resultCount(value),kind=error?(usageError(error)==='cancelled'?'cancelled':'failed'):'completed';
    client.record(`${prefix}_${kind}`,{...context,result:error?kind==='cancelled'?'cancelled':'error':count===0?'empty':'success',
      duration:durationBucket(Date.now()-start),...(count===null?{}:{count:countBucket(count)}),...(error?{error:usageError(error),...failureDiagnostics(error,actionStage(action))}:{})},operationId);
    if(!error&&['download_submit','retry','process'].includes(action))client.watchJobs(jobsIn(value),context,operationId);
    const artifact=value?.record||value;
    if(!error&&action==='download_submit'&&/^[a-f0-9]{64}$/i.test(artifact?.sha256||'')){
      client.record('download_completed',context,operationId);client.record('artifact_validated',context,operationId);
    }
    if(!error&&action==='save_project')client.record('project_saved',context,operationId);
    if(!error&&action==='inspect'&&context.page==='Workspace')client.record('artifact_opened',context,operationId);
  }};
}
export async function usageRequest(scope,operation,payload,run) {
  if(!usage.enabled())return run();
  const generation=usage.generation();
  const action=actions[scope]?.[operation];
  const fields={...usageContext({agent:scope==='agent',scene:payload?.scene}),
    source:usageSource(payload?.provider??payload?.datasetId??payload?.productId,scope==='runtime'?usageJobSource(payload):scope)};
  const observed=action?beginUsage(action,fields):null;
  let value;
  try{value=await run();}catch(error){observed?.finish(null,error);throw error;}
  if(!usage.enabled()||generation!==usage.generation())return value;
  const sourceFailure=scope==='public-dataset'&&operation==='connect'&&value?.complete===false
    ?diagnosticError(new Error(value.failures?.[0]?.error||'A published source could not be checked'),{stage:'source_connect',diagnosticOrigin:'classified'}):null;
  const modelFailure=scope==='agent'&&operation==='testModel'&&value?.status==='failed'
    ?diagnosticError(new Error(value.message||'Model connection test failed'),{stage:'model_test',diagnosticOrigin:'classified'}):null;
  observed?.finish(value,sourceFailure||modelFailure||(action==='import'&&value===null?new DOMException('Import cancelled','AbortError'):null));
  if(scope==='runtime'&&operation==='list')usage.observeJobs(Array.isArray(value)?value:value?.jobs);
  if(scope==='agent'){
    if(operation==='send'&&value?.selected?.id){
      turns.set(value.selected.id,{fields,at:Date.now(),operationId:observed?.operationId,generation});
      if(turns.size>128)turns.delete(turns.keys().next().value);
    }
    if(operation==='approvePlan'){
      const plan=value?.plans?.find(plan=>plan.planId===payload?.planId);
      if(plan?.kind==='download')usage.record('download_confirmed',fields,observed?.operationId);
      usage.watchJobs(plan?.jobs||[],fields,observed?.operationId);
    }
    const session=value?.selected;let turn=session&&turns.get(session.id);
    if(turn&&turn.generation!==generation){turns.delete(session.id);turn=null;}
    if(turn)usage.watchJobs((value?.plans||[]).filter(plan=>plan.kind==='download'&&plan.status==='submitted').flatMap(plan=>plan.jobs||[]),turn.fields,turn.operationId);
    if(turn&&['completed','failed','interrupted'].includes(session.status)){
      usage.record(session.status==='completed'?'agent_turn_completed':session.status==='failed'?'agent_turn_failed':'agent_turn_cancelled',
        {...turn.fields,duration:durationBucket(Date.now()-turn.at),...(session.status==='completed'?{}:{error:session.status==='interrupted'?'cancelled':usageError(session.error),...failureDiagnostics(session.status==='interrupted'?new DOMException('Interrupted','AbortError'):session.error,'agent_turn')})},turn.operationId);turns.delete(session.id);
    }
  }
  return value;
}
