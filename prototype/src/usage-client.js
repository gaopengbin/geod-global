import schema from '../../schemas/usage-events.json' with {type:'json'};
import {RELEASE_VERSION} from './release-policy.js';
import {failureDiagnostics,diagnosticError} from './usage-diagnostics.js';
import {acquisitionLabel} from './acquisition.js';

export const USAGE_STORAGE_KEY='geod-global-usage-v1';
export const DEFAULT_USAGE_ENDPOINT='https://geod-global.laogao.xyz/api/usage/v1/events';
const DAY=86400000, MAX_EVENTS=2000, MAX_JOBS=256;
const uuidPattern=/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/i;
const validVersion=value=>typeof value==='string'&&/^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}(?:-[a-zA-Z0-9.-]{1,32})?$/.test(value);
const storageDefault=()=>{try{return globalThis.window?globalThis.window.localStorage:null;}catch{return null;}};
export const usagePrivacyBlocked=()=>Boolean(globalThis.navigator?.globalPrivacyControl||globalThis.navigator?.doNotTrack==='1'||globalThis.window?.doNotTrack==='1');
export function usageFields(fields={}) {
  return Object.fromEntries(Object.entries(schema.fields).flatMap(([key,values])=>values.includes(fields[key])?[[key,fields[key]]]:[]));
}
export function usageJobSource(job) {
  if(job?.wcsSource)return 'wcs';
  if(job?.stacSource)return 'stac';
  try{
    const host=new URL(job?.href).hostname;
    return ({'naipeuwest.blob.core.windows.net':'planetary-naip','landsateuwest.blob.core.windows.net':'planetary-landsat',
      'sentinel2l2a01.blob.core.windows.net':'planetary-computer','sentinel1euwestrtc.blob.core.windows.net':'planetary-radar',
      'sentinel-cogs.s3.us-west-2.amazonaws.com':'earth-search','copernicus-dem-30m.s3.eu-central-1.amazonaws.com':'copernicus-dem',
      'copernicus-dem-90m.s3.eu-central-1.amazonaws.com':'copernicus-dem-90','data.lpdaac.earthdatacloud.nasa.gov':'nasa-earthdata',
      'download.dataspace.copernicus.eu':'copernicus'})[host]||(/(^|\.)gscloud\.(cn|org\.cn)$/.test(host)?'gscloud':'other');
  }catch{return 'other';}
}
export function classifyUsageError(error) {
  const name=error?.name,text=String(error?.message??error??'');
  return name==='AbortError'?'cancelled':name==='TimeoutError'||/timed? ?out|timeout/i.test(text)?'timeout'
    :/401|403|unauthori[sz]ed|forbidden|credential|login|sign.?in/i.test(text)?'authorization'
    :/disk|storage|no space|permission denied/i.test(text)?'storage'
    :/network|fetch|connection|offline|502|503|504/i.test(text)?'network'
    :/invalid|mismatch|unsupported|not match|validation/i.test(text)?'validation':'unknown';
}
export const durationBucket=ms=>ms<1000?'lt_1s':ms<3000?'1_3s':ms<10000?'3_10s':ms<30000?'10_30s':ms<120000?'30_120s':ms<600000?'120_600s':'gte_600s';
export const countBucket=count=>count===0?'0':count===1?'1':count<=5?'2_5':count<=20?'6_20':count<=100?'21_100':count<=1000?'101_1000':'gte_1001';
export const bytesBucket=bytes=>!bytes?'0':bytes<1048576?'lt_1mib':bytes<10485760?'1_10mib':bytes<104857600?'10_100mib':bytes<1073741824?'100_1024mib':bytes<10737418240?'1_10gib':'gte_10gib';
export function usageEndpoint(raw) {
  if(!raw)return null;
  try{const url=new URL(raw);return !url.username&&!url.password&&!url.search&&!url.hash&&['/v1/events','/api/usage/v1/events'].includes(url.pathname)
    &&(url.origin==='https://geod-global.laogao.xyz'||url.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(url.hostname))?url.href:null;}catch{return null;}
}
function validEvent(event) {
  return event&&event.product===schema.product&&event.schemaVersion===1&&schema.events.includes(event.name)
    &&[event.id,event.installationId,event.sessionId].every(value=>uuidPattern.test(value||''))
    &&(!event.operationId||uuidPattern.test(event.operationId))&&validVersion(event.version)
    &&['desktop','browser'].includes(event.platform)&&['development','production'].includes(event.environment)&&Number.isFinite(Date.parse(event.at))
    &&Object.keys(event).every(key=>['schemaVersion','product','id','installationId','sessionId','operationId','version','platform','environment','name','at','fields'].includes(key))
    &&event.fields&&JSON.stringify(usageFields(event.fields))===JSON.stringify(event.fields);
}

export function createUsageClient({storage=storageDefault(),now=()=>Date.now(),uuid=()=>crypto.randomUUID(),
  fetcher=(...args)=>fetch(...args),endpoint=null,version=RELEASE_VERSION,platform='browser',
  environment=import.meta.env?.PROD?'production':'development',privacy=usagePrivacyBlocked}={}) {
  let state={enabled:false,share:false,autoShare:false,sharingTarget:null,installationId:null,events:[],jobs:{}}, revision=0, collectionRevision=0, started=false, timer, retryAt=0, failures=0, inFlight=null;
  const listeners=new Set(), address=usageEndpoint(endpoint), sessionId=uuid();let saving=false;
  const save=()=>{try{storage?.setItem(USAGE_STORAGE_KEY,JSON.stringify(state));}catch{/* Diagnostics must never block product work. */}};
  const saveSoon=()=>{if(saving)return;saving=true;queueMicrotask(()=>{saving=false;save();});};
  const prune=()=>{state.events=state.events.filter(row=>Date.parse(row.event.at)>=now()-30*DAY&&Date.parse(row.event.at)<=now()+300000).slice(-MAX_EVENTS);};
  try{
    const saved=JSON.parse(storage?.getItem(USAGE_STORAGE_KEY)||'null');
    if(saved===null){
      state.enabled=true;state.share=Boolean(address);state.autoShare=true;state.sharingTarget=address;
    }else if(saved&&saved.enabled===true&&uuidPattern.test(saved.installationId||'')){
      const share=Boolean(address)&&(saved.autoShare===true||saved.share===true&&saved.sharingTarget===address);
      state={enabled:true,share,autoShare:saved.autoShare===true,sharingTarget:share?address:null,installationId:saved.installationId,
        events:Array.isArray(saved.events)?saved.events.filter(row=>row&&validEvent(row.event)).map(row=>({event:row.event,sent:row.sent===true})):[],
          jobs:saved.jobs&&typeof saved.jobs==='object'&&!Array.isArray(saved.jobs)?Object.fromEntries(Object.entries(saved.jobs).slice(-MAX_JOBS).filter(([_id,value])=>value&&uuidPattern.test(value.operationId||'')&&Number.isFinite(value.at)&&value.at>=now()-30*DAY).map(([id,value])=>[id,{operationId:value.operationId,at:value.at,fields:usageFields(value.fields)}])):{}};
      prune();
      if(acquisitionLabel(saved.acquisition)&&['new_installation','existing_installation'].includes(saved.acquisitionKind)){
        state.acquisition={schemaVersion:1,source:saved.acquisition.source,campaign:saved.acquisition.campaign};state.acquisitionKind=saved.acquisitionKind;
      }
    }
  }catch{/* An unreadable saved preference must not override a possible opt-out. */}
  const savedAcquisition=acquisitionLabel(state.acquisition);
  if(!savedAcquisition)delete state.acquisition;
  const enabled=()=>state.enabled&&!privacy();
  const snapshot=()=>({enabled:enabled(),preference:state.enabled,blocked:Boolean(privacy()),sharing:enabled()&&state.share&&Boolean(address),
    configured:Boolean(address),recorded:state.events.length,pending:state.events.filter(row=>!row.sent).length,
    status:!enabled()?'off':inFlight?'sending':failures?'waiting':state.share&&address?'sharing':'local'});
  const notify=()=>listeners.forEach(fn=>{try{fn(snapshot());}catch{}});
  const record=(name,fields={},operationId)=>{
    if(!enabled()||!schema.events.includes(name)||operationId&&!uuidPattern.test(operationId)||!validVersion(version))return null;
    if(!state.installationId){state.installationId=uuid();if(name!=='app_first_run')record('app_first_run');}
    const event={schemaVersion:1,product:schema.product,id:uuid(),installationId:state.installationId,sessionId,
      version,platform,environment,name,at:new Date(now()).toISOString(),fields:usageFields({...fields,...(acquisitionLabel(state.acquisition)||{}),...(state.acquisition?{acquisitionKind:state.acquisitionKind}:{})}),...(operationId?{operationId}:{})};
    state.events.push({event,sent:false});prune();saveSoon();notify();return event.id;
  };
  const openSession=()=>{if(enabled()&&!started){started=true;record('app_started');}};
  const stopSending=()=>{revision++;inFlight?.abort();inFlight=null;retryAt=0;failures=0;};
  const api={
    enabled,snapshot,record,generation:()=>collectionRevision,
    setAcquisition(value){
      if(!enabled()||state.acquisition||!acquisitionLabel(value))return false;
      state.acquisition={schemaVersion:1,source:value.source,campaign:value.campaign};
      state.acquisitionKind=state.installationId?'existing_installation':'new_installation';
      if(state.installationId)save();return true;
    },
    subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},
    setPreferences(preferences={}){
      const {enabled:allow=state.enabled,share=state.share}=preferences;
      if(Object.hasOwn(preferences,'share'))state.autoShare=false;
      if(Boolean(allow)!==state.enabled)collectionRevision++;
      stopSending();
      if(!allow){state={enabled:false,share:false,autoShare:false,sharingTarget:null,installationId:null,events:[],jobs:{}};started=false;}
      else{
        const first=!state.installationId;
        state.enabled=true;state.share=Boolean(address)&&(state.autoShare||Boolean(share));
        state.sharingTarget=state.share?address:null;
        if(first&&!privacy()){state.installationId=uuid();record('app_first_run');}
        if(privacy()){state.enabled=false;state.share=false;}
        openSession();
      }
      save();notify();return snapshot();
    },
    start(){openSession();if(!timer){timer=setInterval(()=>{void api.flush();},15000);timer.unref?.();}queueMicrotask(()=>{if(timer)void api.flush();});return()=>{clearInterval(timer);timer=null;stopSending();};},
    async flush(){
      if(!enabled()||!state.share||!address||inFlight||now()<retryAt)return;
      prune();const rows=state.events.filter(row=>!row.sent).slice(0,50);if(!rows.length)return;
      const generation=revision,controller=new AbortController();inFlight=controller;notify();
      const timeout=setTimeout(()=>controller.abort(),5000);timeout.unref?.();
      try{
        const response=await fetcher(address,{method:'POST',headers:{'Content-Type':'application/json'},
          credentials:'omit',referrerPolicy:'no-referrer',keepalive:true,body:JSON.stringify({schemaVersion:1,product:schema.product,events:rows.map(row=>row.event)}),signal:controller.signal});
        if(!response.ok)throw new Error('Usage collection unavailable.');
        const result=await response.json(),ids=new Set(rows.map(row=>row.event.id));
        if(!Array.isArray(result.acceptedIds)||result.acceptedIds.length!==ids.size||new Set(result.acceptedIds).size!==ids.size||result.acceptedIds.some(id=>!ids.has(id)))throw new Error('Invalid usage receipt.');
        if(generation!==revision||!enabled()||!state.share)return;
        state.events.forEach(row=>{if(ids.has(row.event.id))row.sent=true;});failures=0;retryAt=0;save();
      }catch{if(generation===revision&&enabled()&&state.share){failures++;retryAt=now()+Math.min(300000,15000*2**Math.min(failures-1,5));}}
      finally{clearTimeout(timeout);if(inFlight===controller)inFlight=null;notify();}
    },
    clear(){collectionRevision++;stopSending();state.events=[];state.jobs={};state.installationId=enabled()?uuid():null;save();notify();},
    report(){prune();return {schemaVersion:1,product:schema.product,generatedAt:new Date(now()).toISOString(),
      coverage:'enabled-device-observations',counts:Object.fromEntries(schema.events.map(name=>[name,state.events.filter(row=>row.event.name===name).length])),
      events:state.events.map(row=>row.event),pending:snapshot().pending};},
    watchJobs(jobs,fields={},operationId=uuid()){
      if(!enabled())return;
      for(const job of Array.isArray(jobs)?jobs:[]){if(!job?.id||state.jobs[job.id]||!['queued','running'].includes(job.status))continue;
        state.jobs[job.id]={operationId,at:now(),fields:usageFields({...fields,
          source:fields.source&&fields.source!=='other'?fields.source:usageJobSource(job),
          ...(job.kind&&job.kind!=='download'?{action:'process'}:{})})};}
      state.jobs=Object.fromEntries(Object.entries(state.jobs).slice(-MAX_JOBS));save();
    },
    observeJobs(jobs){
      if(!enabled())return;
      for(const job of Array.isArray(jobs)?jobs:[]){const pending=state.jobs[job?.id];if(!pending||!['succeeded','failed','cancelled','interrupted'].includes(job.status))continue;
        const cancelStage=job.error==='Cancelled while waiting for a download slot. Retry restarts the operation.'?'download_queue':job.error==='Cancelled during an active download. Retry restarts the operation.'?'transfer':null;
        const fields={...pending.fields,duration:durationBucket(now()-pending.at),
          ...(Number.isFinite(job.bytesDownloaded)&&job.bytesDownloaded>=0?{bytes:bytesBucket(job.bytesDownloaded)}:{}),
          status:job.status,
          result:job.status==='succeeded'?'success':job.status==='failed'?'error':'cancelled',
          ...(job.status==='failed'?{error:classifyUsageError(new Error(job.error||'')),...failureDiagnostics(diagnosticError(new Error(job.error||''),{retries:job.networkRetries}),pending.fields.action==='process'?'processing':'transfer')}:['cancelled','interrupted'].includes(job.status)?{
            ...failureDiagnostics(diagnosticError(new Error('Transfer cancelled'),{retries:job.networkRetries,...(cancelStage?{stage:cancelStage,diagnosticOrigin:'explicit'}:{})}),pending.fields.action==='process'?'processing':'operation'),
            reason:job.status==='interrupted'?'task_interrupted':'cancelled'}:{})};
        const prefix=pending.fields.action==='process'?'operation':'download';
        record(job.status==='succeeded'?`${prefix}_completed`:job.status==='failed'?`${prefix}_failed`:`${prefix}_cancelled`,fields,pending.operationId);
        if(job.status==='succeeded'&&/^[a-f0-9]{64}$/i.test(job.sha256||''))record('artifact_validated',fields,pending.operationId);
        delete state.jobs[job.id];}
      save();
    },
  };
  return api;
}

export const usage=createUsageClient({endpoint:import.meta.env?.VITE_USAGE_ENDPOINT??(import.meta.env?.MODE&&import.meta.env.MODE!=='test'?DEFAULT_USAGE_ENDPOINT:null),
  platform:globalThis.window?.__TAURI__?.core?.invoke?'desktop':'browser'});
