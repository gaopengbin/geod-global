// Fixed public marketing labels. Never serialize location, referrer, IP or form values.
export const CAMPAIGN='landsat_batch_20261023';
const KEY='geod-global-tutorial-v1',UUID=/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/i;
const NAMES=['tutorial_viewed','tutorial_download_clicked'];
export function channelFrom(search){
  const p=new URLSearchParams(search);
  return p.get('utm_campaign')===CAMPAIGN&&['zhihu','bilibili'].includes(p.get('utm_source'))?p.get('utm_source'):null;
}
export function createTracker({source,storage,uuid=()=>crypto.randomUUID(),now=()=>Date.now(),privacy=()=>false,fetcher=(...args)=>fetch(...args),environment='production'}={}){
  let state={enabled:true,id:null,queue:[]},sending=null,generation=0;
  const session=uuid();
  const valid=e=>e&&NAMES.includes(e.name)&&UUID.test(e.id)&&UUID.test(e.installationId)&&UUID.test(e.sessionId)
    &&e.fields?.acquisitionCampaign===CAMPAIGN&&['zhihu','bilibili'].includes(e.fields?.acquisitionSource)&&e.fields?.acquisitionKind==='website'
    &&Date.parse(e.at)>now()-30*86400000&&Date.parse(e.at)<now()+300000
    &&Object.keys(e.fields).length===3&&e.product==='geod-global'&&e.schemaVersion===1&&e.platform==='browser'
    &&['production','development'].includes(e.environment)&&e.version==='0.2.2'
    &&Object.keys(e).length===11&&Object.keys(e).every(k=>['schemaVersion','product','id','installationId','sessionId','version','platform','environment','name','at','fields'].includes(k));
  if(!privacy())try{
    const saved=JSON.parse(storage?.getItem(KEY)||'null');
    if(saved?.enabled===false)state.enabled=false;
    else if(saved){state={enabled:saved.enabled===true,id:UUID.test(saved.id||'')?saved.id:null,queue:Array.isArray(saved.queue)?saved.queue.filter(valid).slice(-50):[]};}
  }catch{state.enabled=false;}
  const enabled=()=>Boolean(source)&&state.enabled&&!privacy();
  const save=()=>{try{storage?.setItem(KEY,JSON.stringify(state));}catch{}};
  const api={
    enabled,
    setEnabled(allow){generation++;sending?.abort();sending=null;state={enabled:Boolean(allow),id:null,queue:[]};save();},
    record(name){
      if(!enabled()||!NAMES.includes(name))return;
      if(!state.id)state.id=uuid();
      const e={schemaVersion:1,product:'geod-global',id:uuid(),installationId:state.id,sessionId:session,version:'0.2.2',platform:'browser',environment,name,at:new Date(now()).toISOString(),
        fields:{acquisitionSource:source,acquisitionCampaign:CAMPAIGN,acquisitionKind:'website'}};
      state.queue=[...state.queue.filter(valid),e].slice(-50);save();
    },
    async flush(){
      if(!enabled()||sending||!state.queue.length)return;
      const controller=new AbortController();sending=controller;const revision=generation,batch=state.queue.filter(valid).slice(0,50);
      if(!batch.length){sending=null;state.queue=[];save();return;}
      const timer=setTimeout(()=>controller.abort(),5000);timer.unref?.();
      try{
        const response=await fetcher('/api/usage/v1/events',{method:'POST',headers:{'Content-Type':'application/json'},credentials:'omit',referrerPolicy:'no-referrer',keepalive:true,signal:controller.signal,
          body:JSON.stringify({schemaVersion:1,product:'geod-global',events:batch})});
        if(!response.ok)throw new Error('Unavailable');
        const reply=await response.json(),ids=new Set(batch.map(e=>e.id));
        if(!Array.isArray(reply.acceptedIds)||new Set(reply.acceptedIds).size!==ids.size||reply.acceptedIds.some(id=>!ids.has(id)))throw new Error('Invalid receipt');
        if(revision===generation&&enabled()){state.queue=state.queue.filter(e=>!ids.has(e.id));save();}
      }catch{/* Download navigation remains available when collection is unavailable. */}
      finally{clearTimeout(timer);if(sending===controller)sending=null;}
    }
  };
  return api;
}

export function wireDownloads(document,source,tracker){
  const primary=document.querySelector('[data-tutorial-download]');
  const url=source&&primary?.getAttribute('data-package-'+source);
  for(const a of document.querySelectorAll('[data-tutorial-download],[data-release-link="setup"],[data-release-link="portable"]')){
    if(url){
      a.href=url;
      if(!a.hasAttribute('data-tutorial-download')){
        // The campaign offers a portable ZIP. Keep every case-page download
        // entry in the same cohort, and prevent the generic release script
        // from restoring an unlabelled installer URL later.
        a.removeAttribute('data-release-link');a.setAttribute('data-tutorial-download','');a.textContent='下载 Windows 便携版';
      }
    }
    a.addEventListener('click',()=>{tracker.record('tutorial_download_clicked');void tracker.flush();});
  }
}

if(typeof document!=='undefined'){
  const source=channelFrom(location.search),privacy=()=>Boolean(navigator.globalPrivacyControl||navigator.doNotTrack==='1'||window.doNotTrack==='1');
  let storage;try{storage=localStorage;}catch{}
  const qa=new URLSearchParams(location.search).get('qa')==='acceptance-20261023';
  const tracker=createTracker({source,storage,privacy,environment:qa||['localhost','127.0.0.1'].includes(location.hostname)?'development':'production'});
  const button=document.querySelector('[data-tutorial-stats]');
  function status(){if(button){button.textContent=privacy()?'统计已按隐私偏好关闭':!source?'此入口未启用渠道统计':tracker.enabled()?'匿名统计已开启 · 关闭':'匿名统计已关闭 · 开启';button.disabled=privacy()||!source;button.setAttribute('aria-pressed',String(tracker.enabled()));}}
  status();button?.addEventListener('click',()=>{tracker.setEnabled(!tracker.enabled());status();});
  wireDownloads(document,source,tracker);
  tracker.record('tutorial_viewed');void tracker.flush();
  const timer=setInterval(()=>void tracker.flush(),15000);
  window.addEventListener('online',()=>void tracker.flush());
  window.addEventListener('pagehide',()=>{clearInterval(timer);void tracker.flush();});
}
