import React,{createContext,useCallback,useContext,useEffect,useRef,useState} from 'react';
import {Activity,Bell,Check,Clock,Map as MapIcon,MessageSquare,Pause,Play,Plus,Radar,RefreshCw,ShieldCheck,Trash2} from 'lucide-react';
import { InfoDetails,Badge,Button,Disclosure,EmptyState,Input,Modal,PageHeader,Select,Spinner,Surface} from './ui/index.jsx';
import {useIdentity} from './identity-ui.jsx';
import {useI18n} from './i18n.jsx';
import {desktopAvailable} from './runtime-client.js';
import {officialRequest,rectangleGeometry} from './official-client.js';
import './official.css';
const AreaPicker=React.lazy(()=>import('./area-picker.jsx').then(module=>({default:module.AreaPicker})));
const Context=createContext(null);
const empty=()=>({capabilities:null,account:null,monitors:[],quotes:[],notifications:[],orders:[]});
const fallback={...empty(),loading:false,error:'',refresh:async()=>{},request:officialRequest};
export const useOfficialServices=()=>useContext(Context)||fallback;
export function OfficialServicesProvider({children}){
 const {snapshot}=useIdentity(),user=snapshot.user?.id;
 const [data,setData]=useState(empty),[loading,setLoading]=useState(false),[error,setError]=useState('');
 const revision=useRef(0),mounted=useRef(false),pending=useRef(null);
 const refresh=useCallback(async()=>{
  if(!desktopAvailable()||pending.current)return;
  const current=revision.current,ticket={};pending.current=ticket;setLoading(true);
  const tasks=[['capabilities','capabilities'],...(user?[['account','account'],['monitors','monitors'],['quotes','quotes'],['notifications','notifications'],['orders','orders']]:[])];
  const results=await Promise.allSettled(tasks.map(async([operation,key])=>{const result=await officialRequest(operation);return [key,operation==='capabilities'?result:result[key]];}));
  if(!mounted.current||current!==revision.current){if(pending.current===ticket)pending.current=null;return;}
  let failure='';
  const next={};results.forEach((result,index)=>{if(result.status==='fulfilled')next[result.value[0]]=result.value[1];else failure ||= result.reason?.message||'Official service is unavailable.';});
  setData(previous=>({...previous,...next}));setError(failure);if(pending.current===ticket)pending.current=null;setLoading(false);
 },[user]);
 // Capabilities is the public response itself, while private responses have named envelopes.
 const request=useCallback(async(operation,payload={})=>{
  const current=revision.current;
  const result=await officialRequest(operation,payload);
  if(current!==revision.current||!mounted.current)throw Error('Your account changed. Retry this request.');
  if(operation!=='quoteMonitor')await refresh();
  return result;
 },[refresh]);
 useEffect(()=>{
  mounted.current=true;revision.current++;pending.current=null;setData(empty());setError('');setLoading(false);
  if(user)void refresh();const timer=setInterval(()=>{if(user&&!document.hidden)void refresh();},30_000);
  const focused=()=>{if(user)void refresh();};window.addEventListener('focus',focused);
  let off,disposed=false;
  if(window.__TAURI__?.event?.listen)window.__TAURI__.event.listen('geod-official-changed',()=>void refresh()).then(remove=>{if(disposed)remove();else off=remove;}).catch(()=>{});
  return()=>{disposed=true;off?.();mounted.current=false;revision.current++;clearInterval(timer);window.removeEventListener('focus',focused);};
 },[refresh]);
 return <Context.Provider value={{...data,loading,error,refresh,request}}>{children}</Context.Provider>;
}

function exportArea(area){
 const url=URL.createObjectURL(new Blob([JSON.stringify({type:'Feature',properties:{name:area.place?.name||'Monitoring area'},geometry:area.geometry||rectangleGeometry(area.bounds)})],{type:'application/geo+json'}));
 const link=document.createElement('a');link.href=url;link.download='geod-monitor-area.geojson';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}

export function MonitorReview({quote,onConfirm,busy}){
  const {t,number,date}=useI18n(),config=quote.config;
  const positions=config.geometry?.coordinates?.flat(config.geometry.type==='MultiPolygon'?2:1)||[];
  const bounds=positions.length?[Math.min(...positions.map(p=>p[0])),Math.min(...positions.map(p=>p[1])),Math.max(...positions.map(p=>p[0])),Math.max(...positions.map(p=>p[1]))]:null;
  const source=config.sourceId==='earth-search'?'Earth Search · Sentinel-2 L2A':'Planetary Computer · Landsat C2 L2';
 return <Surface className="official-review" aria-label={t('Complete monitoring task')}>
  <div className="official-heading"><h3><ShieldCheck size={17}/>{t('Complete monitoring task')}</h3><Badge tone="accent">{t('Awaiting confirmation')}</Badge></div>
   <strong>{config.name}</strong><p>{t('Monitor {source} every {hours} hours. Search the last {days} days with scene clouds at most {cloud}%.',{source,hours:config.intervalHours,days:config.lookbackDays,cloud:config.cloudMax})}</p>
   {bounds&&<InfoDetails closeLabel={t('Close')} summary={t('Monitoring area')}><p className="official-boundary">WGS 84 · {bounds.map(value=>number(value,{maximumFractionDigits:5})).join(', ')}</p><p>{t('Original {kind} boundary · {count} vertices. The bounding box above is a reference; checks use the saved polygon.',{kind:config.geometry.type,count:positions.length})}</p></InfoDetails>}
  <p>{t('Uses the selected polygon. Matching scenes are not proof of full coverage; downloads still require a separate complete task review.')}</p>
  <dl><dt>{t('Per check')}</dt><dd>{number(quote.checkCostMicros/1_000_000)} {t('test credits')}</dd><dt>{t('Daily ceiling')}</dt><dd>{number(config.dailyCreditLimit)} {t('test credits')}</dd><dt>{t('Total ceiling')}</dt><dd>{number(config.totalCreditLimit)} {t('test credits')}</dd><dt>{t('Review expires')}</dt><dd>{date(quote.expiresAt*1000,{hour:'2-digit',minute:'2-digit'})}</dd></dl>
   <p>{t('Confirming authorizes scheduled catalog checks within these limits. It does not authorize imagery purchases or downloads.')}</p>
   <p>{t('The selected boundary and conditions are stored by the official service and sent to the selected catalog provider for checks.')}</p>
  <Button variant="primary" disabled={busy} icon={Check} onClick={()=>onConfirm(quote)}>{t('Confirm monitoring task')}</Button>
 </Surface>;
}

export function ConversationMonitorReviews({conversationId}){
 const service=useOfficialServices(),{t}=useI18n();
 const [busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{setError('');},[conversationId]);
 const quotes=service.quotes.filter(quote=>conversationId&&quote.conversationId===conversationId);
 if(!quotes.length)return null;
 const confirm=async quote=>{if(busy)return;setBusy(true);setError('');try{await service.request('acceptMonitor',{id:quote.id,hash:quote.hash});}catch(cause){setError(cause.message);}finally{setBusy(false);}};
 return <div className="official-conversation-reviews">{quotes.map(quote=><MonitorReview key={quote.id} quote={quote} onConfirm={confirm} busy={busy}/>)}{error&&<p className="official-error" role="alert">{t(error)}</p>}</div>;
}

export function ImageryMonitoringPage({initialArea,onPreviewArea,onOpenAgent,onExplore}){
 const {t,date,number}=useI18n(),{snapshot}=useIdentity(),service=useOfficialServices();
 const [area,setArea]=useState(initialArea||null),[choosing,setChoosing]=useState(false),[creating,setCreating]=useState(false),[working,setWorking]=useState(''),[failure,setFailure]=useState(''),[quote,setQuote]=useState(null),[notice,setNotice]=useState('');
 const [config,setConfig]=useState({name:'',sourceId:'earth-search',cloudMax:10,lookbackDays:30,intervalHours:24,dailyCreditLimit:5,totalCreditLimit:100});
 const opKey=useRef(new Map());
 useEffect(()=>{void service.refresh();},[service.refresh]);
 useEffect(()=>{setQuote(null);setCreating(false);setFailure('');setNotice('');opKey.current.clear();},[snapshot.user?.id]);
 const signed=Boolean(snapshot.user),pro=service.account?.plan==='pro',ready=Boolean(service.capabilities&&service.account&&!service.error);
 const act=async(name,operation,payload)=>{
  if(working)return null;setWorking(name);setFailure('');setNotice('');
  try{return await service.request(operation,payload);}catch(error){setFailure(error.message);return null;}finally{setWorking('');}
 };
 const keyFor=name=>{if(!opKey.current.has(name))opKey.current.set(name,crypto.randomUUID());return opKey.current.get(name);};
 const check=async monitor=>{const result=await act(monitor.id,'runMonitor',{id:monitor.id,idempotencyKey:keyFor('run:'+monitor.id)});if(result)opKey.current.delete('run:'+monitor.id);};
 const confirm=async value=>{if(await act('accept','acceptMonitor',{id:value.id,hash:value.hash})){setQuote(null);setCreating(false);setNotice(t('Monitoring is active. New matches will appear in your notifications.'));}};
 const prepare=async()=>{
  if(!area?.bounds&&!area?.geometry){setFailure(t('Choose a monitoring area first.'));return;}
  const payload={...config,geometry:area.geometry||rectangleGeometry(area.bounds)};
  const result=await act('prepare','quoteMonitor',payload);if(result)setQuote(result.quote);
 };
 return <section className="official-page" aria-label={t('Imagery monitoring')}>
  <PageHeader title={t('Imagery monitoring')} description={t('Get new imagery alerts for your places, preview the coverage and choose what to download.')} actions={<Button size="icon" variant="quiet" icon={RefreshCw} aria-label={t('Refresh monitoring')} disabled={service.loading||Boolean(working)||!desktopAvailable()} onClick={service.refresh}/>}/>
  {!ready&&<Surface className="monitoring-intro"><div className="official-heading"><h2><Radar size={19}/>{t('Follow imagery changes')}</h2><Badge>{t(service.loading?'Loading…':!signed?'Sign in to monitor':!desktopAvailable()?'Desktop feature':'In preparation')}</Badge></div><p>{t('Save an area and your imagery conditions. New matches appear in notifications for preview and download review.')}</p><ol className="monitoring-steps"><li>{t('Choose an area')}</li><li>{t('Set dates and clouds')}</li><li>{t('Receive imagery alerts')}</li></ol><p role="status">{t(service.loading?'Loading monitoring tasks…':!signed?'Sign in to set up imagery monitoring.':!desktopAvailable()?'Use the desktop app to set up monitoring.':'Monitoring is being prepared. You can search and download imagery with Agent now.')}</p><div className="official-actions">{onOpenAgent&&<Button variant="primary" icon={MessageSquare} onClick={onOpenAgent}>{t('Find imagery with Agent')}</Button>}{onExplore&&<Button icon={MapIcon} onClick={onExplore}>{t('Explore imagery')}</Button>}{!signed&&<Button asChild><a href="#SignIn">{t('Sign in')}</a></Button>}</div></Surface>}
  {failure&&<p className="official-error" role="alert">{t(failure)}</p>}{notice&&<p role="status">{notice}</p>}
  {ready&&<div className="official-heading"><h2>{t('Monitored areas')}</h2><Button icon={Plus} variant="primary" disabled={!pro||Boolean(working)} onClick={()=>setCreating(!creating)}>{t('Add monitored area')}</Button></div>}
  {ready&&!pro&&<p className="official-help">{t('Imagery monitoring is a Pro feature in testing.')} <a href="#Account">{t('View membership')}</a></p>}
  {creating&&<Surface className="official-form"><h3>{t('Monitoring conditions')}</h3><div className="official-fields">
   <label>{t('Task name')}<Input value={config.name} maxLength={120} onChange={e=>setConfig({...config,name:e.target.value})}/></label>
   <label>{t('Source')}<Select value={config.sourceId} onChange={e=>setConfig({...config,sourceId:e.target.value})}>{(service.capabilities?.sources||[]).map(source=><option key={source.id} value={source.id}>{source.name}</option>)}</Select></label>
   {[['cloudMax','Maximum scene clouds (%)',0,100],['lookbackDays','Look back (days)',1,90],['intervalHours','Check interval (hours)',1,168],['dailyCreditLimit','Daily test credit ceiling',1,100],['totalCreditLimit','Total test credit ceiling',1,10000]].map(([key,label,min,max])=><label key={key}>{t(label)}<Input type="number" value={config[key]} min={min} max={max} step={1} onChange={e=>setConfig({...config,[key]:Number(e.target.value)})}/></label>)}
  </div><div className="official-area"><MapIcon size={17}/><span>{area?.place?.name||t(area?.geometry||area?.bounds?'Selected monitoring area':'No area selected')}</span><Button size="sm" onClick={()=>setChoosing(true)}>{t('Choose area')}</Button></div><p>{t('The area is sent to the official service for scheduled catalog checks. Imagery and local files are not uploaded.')}</p><Button variant="primary" disabled={Boolean(working)||!config.name.trim()} onClick={prepare}>{t('Prepare monitoring review')}</Button></Surface>}
  {quote&&<MonitorReview quote={quote} busy={Boolean(working)} onConfirm={confirm}/>}
  {service.quotes.filter(item=>item.id!==quote?.id).map(item=><MonitorReview key={item.id} quote={item} busy={Boolean(working)} onConfirm={confirm}/>)}
  <div className="official-list">{service.monitors.map(monitor=><Surface className="official-monitor" key={monitor.id} id={'monitor-'+monitor.id}>
   <div className="official-heading"><h3>{monitor.config.name}</h3><Badge>{t(monitor.state==='active'?'Active':'Paused')}</Badge></div>
   <p>{monitor.config.sourceId} · {t('Every {hours} hours',{hours:monitor.config.intervalHours})} · {t('Scene clouds ≤ {cloud}%',{cloud:monitor.config.cloudMax})}</p>
   <p><Clock size={13}/>{monitor.last_check?t('Last checked: {date}',{date:date(monitor.last_check*1000,{hour:'2-digit',minute:'2-digit'})}):t('Waiting for first check')}</p>
   {monitor.last_error&&<p className="official-error">{t(monitor.last_error==='budget_limit'?'The confirmed monitoring budget has been reached.':monitor.last_error==='pro_required'?'This official service requires Pro. Local tools remain available.':'The last check failed. Unused test credits were released.')}</p>}
   <div className="official-actions"><Button size="sm" icon={RefreshCw} disabled={Boolean(working)||monitor.state!=='active'||!pro} onClick={()=>check(monitor)}>{t('Check now')}</Button><Button size="sm" icon={monitor.state==='active'?Pause:Play} disabled={Boolean(working)||(!pro&&monitor.state!=='active')} onClick={()=>act(monitor.id,'controlMonitor',{id:monitor.id,action:monitor.state==='active'?'pause':'resume'})}>{t(monitor.state==='active'?'Pause':'Resume')}</Button><Button size="sm" icon={MapIcon} onClick={()=>onPreviewArea(monitor)}>{t('Preview area')}</Button><Button size="icon" icon={Trash2} aria-label={t('Remove monitored area')} disabled={Boolean(working)} onClick={()=>act(monitor.id,'controlMonitor',{id:monitor.id,action:'delete'})}/></div>
   <Disclosure summary={t('Recent checks')}><div className="official-scroll">{monitor.runs.map(run=><div className="official-run" key={run.id}><strong>{t({queued:'Queued',running:'Running',completed:'Completed',failed:'Failed',partial:'Partial results'}[run.status]||run.status)}</strong><span>{date(run.created*1000,{hour:'2-digit',minute:'2-digit'})} · {number(run.consumed/1_000_000)} {t('test credits')}</span>{run.result&&<p>{t('{count} matches · {newCount} new',{count:run.result.count,newCount:run.result.newCount})}</p>}{run.result&&!run.result.complete&&<p>{t('Catalog limit reached. These are partial results, not a complete search.')}</p>}</div>)}{!monitor.runs.length&&<p>{t('Waiting for first check')}</p>}</div></Disclosure>
  </Surface>)}{ready&&!service.monitors.length&&<EmptyState title={t('No monitored areas yet')} icon={Radar}>{t('Choose an area, review the conditions and authorize a bounded monitoring task.')}</EmptyState>}</div>
  {(ready||service.notifications.length>0)&&<Surface className="official-notifications"><h2><Bell size={18}/>{t('Imagery updates')}</h2><div className="official-scroll">{service.notifications.map(item=><div key={item.id} className="official-alert"><div><strong>{t('{count} new imagery matches',{count:item.newCount})}</strong><p>{item.sourceId} · {date(item.created*1000,{hour:'2-digit',minute:'2-digit'})}</p></div><Button size="sm" onClick={()=>{const monitor=service.monitors.find(m=>m.id===item.monitorId);if(monitor)onPreviewArea(monitor);}}>{t('Preview area')}</Button>{!item.read&&<Button size="sm" onClick={()=>act(item.id,'readNotifications',{ids:[item.id]})}>{t('Mark read')}</Button>}</div>)}{!service.notifications.length&&<p>{t('New matches appear here after a successful catalog check.')}</p>}</div></Surface>}
  {choosing&&<Modal className="area-dialog" title={t('Choose monitoring area')} onClose={()=>setChoosing(false)}><React.Suspense fallback={<Spinner/>}><AreaPicker initialBbox={area?.bounds} applyLabel="Use this area" onClose={()=>setChoosing(false)} onExport={exportArea} onApply={next=>{setArea(next);setChoosing(false);if(!config.name)setConfig({...config,name:next.place?.name||''});setQuote(null);}}/></React.Suspense></Modal>}
 </section>;
}

// Retain source compatibility for older development fixtures; navigation uses the product feature name.
export const OfficialServicesPage=ImageryMonitoringPage;

export function MembershipPanel(){
 const {t,date,number}=useI18n(),{snapshot}=useIdentity(),service=useOfficialServices();
 const [working,setWorking]=useState(''),[failure,setFailure]=useState(''),[notice,setNotice]=useState(''),[purchase,setPurchase]=useState(null),[refund,setRefund]=useState(null),[refundReason,setRefundReason]=useState('');
 const opKey=useRef(new Map());
 useEffect(()=>{void service.refresh();},[service.refresh]);
 useEffect(()=>{setPurchase(null);setRefund(null);setFailure('');setNotice('');opKey.current.clear();},[snapshot.user?.id]);
 const signed=Boolean(snapshot.user),account=service.account,pro=account?.plan==='pro',credits=account?account.buckets.reduce((sum,b)=>sum+b.availableMicros,0):null;
 const sandbox=Boolean(account?.sandboxCheckoutAllowed&&service.capabilities?.checkout?.available);
 const plans=service.capabilities?.checkout?.products||[];
 const act=async(name,operation,payload)=>{if(working)return null;setWorking(name);setFailure('');setNotice('');try{return await service.request(operation,payload);}catch(error){setFailure(error.message);return null;}finally{setWorking('');}};
 const keyFor=name=>{if(!opKey.current.has(name))opKey.current.set(name,crypto.randomUUID());return opKey.current.get(name);};
 return <section className="official-page account-membership" aria-label={t('Membership and usage')}>
  <Surface className="membership-summary"><div className="official-heading"><h2>{t('Membership')}</h2><Badge>{t(account?(pro?'Pro':'Free'):'Unavailable')}</Badge></div><p>{pro&&account.proExpiresAt?t('Pro ends {date}',{date:date(account.proExpiresAt*1000)}):t('Open data downloads, local processing and your own model connections remain available.')}</p><div className="membership-balance"><span>{t('Available credits')}</span><strong>{credits===null?t(signed?'Not loaded':'Sign in to view'):number(credits/1_000_000)}</strong>{account?.mode==='beta-test-credits'&&<Badge>{t('Test credits')}</Badge>}</div>{service.error&&<p className="official-help" role="status">{t('Membership information could not be loaded. Your local workspace remains available.')}</p>}<div className="official-actions"><Button size="sm" icon={RefreshCw} disabled={service.loading||Boolean(working)||!desktopAvailable()} onClick={service.refresh}>{t('Refresh membership')}</Button>{!sandbox&&<span className="official-help">{t('Pro purchases are not open yet.')}</span>}</div></Surface>
  {failure&&<p className="official-error" role="alert">{t(failure)}</p>}{notice&&<p role="status">{notice}</p>}
  {sandbox&&<Disclosure summary={t('Service credits and sandbox payments')}><p>{t('These are test prices and test credits. No real card charge or live subscription is available. Local tools are not charged.')}</p><div className="official-products">{plans.map(plan=><Surface key={plan.id}><strong>{t({'pro-monthly':'Pro · one month','pro-yearly':'Pro · one year','credits-1000':'1,000 service credits','credits-3000':'3,000 service credits'}[plan.id])}</strong><p>{number(plan.priceMinor/100,{style:'currency',currency:'USD'})} · {t('Sandbox only')}</p><Button disabled={!signed||!plan.purchasable||Boolean(working)} onClick={()=>setPurchase(plan)}>{t('Test checkout')}</Button></Surface>)}</div><Button disabled={Boolean(working)} icon={RefreshCw} onClick={()=>act('restore','restorePurchases',{})}>{t('Restore purchases')}</Button></Disclosure>}
  {account&&<Disclosure summary={t('Usage history')}><div className="official-scroll">{account.ledger.map(entry=><div key={entry.seq} className="official-ledger-row"><span>{t({grant:'Credit grant',reserve:'Reserved',settle:'Consumed',release:'Released','monitor-confirmed':'Monitoring confirmed','payment-fulfilled-test':'Sandbox payment verified','beta-pro-grant':'Beta Pro access granted'}[entry.kind]||entry.kind)}</span><span>{number(entry.amountMicros/1_000_000)} · {date(entry.created*1000)}</span></div>)}{!account.ledger.length&&<p>{t('No usage records yet.')}</p>}</div></Disclosure>}
  <Disclosure summary={t('Orders')}><div className="official-scroll">{service.orders.map(order=><div className="official-order" key={order.id}><strong>{order.sku}</strong><span>{number(order.amount/100,{style:'currency',currency:'USD'})} · {t(order.status==='fulfilled'?'Sandbox payment verified':'Awaiting payment verification')}</span>{order.status==='fulfilled'&&<Button size="sm" onClick={()=>setRefund(order)}>{t('Request refund review')}</Button>}</div>)}{!service.orders.length&&<p>{t(service.error?'Orders could not be loaded.':'No orders yet.')}</p>}</div></Disclosure>
  {purchase&&<Modal title={t('Confirm sandbox checkout')} onClose={()=>setPurchase(null)} footer={<><Button disabled={Boolean(working)} onClick={()=>setPurchase(null)}>{t('Cancel')}</Button><Button variant="primary" disabled={Boolean(working)} onClick={async()=>{const result=await act('checkout','checkout',{sku:purchase.id,idempotencyKey:keyFor('buy:'+purchase.id)});if(result){setPurchase(null);setNotice(t('Complete the test payment in your browser, then restore purchases. Browser success alone does not activate access.'));}}}>{t('Open test checkout')}</Button></>}><p>{purchase.id} · {number(purchase.priceMinor/100,{style:'currency',currency:'USD'})}</p><p>{t('Stripe test mode. Fixed term, no automatic renewal. This does not approve any download or supplier purchase.')}</p></Modal>}
  {refund&&<Modal title={t('Request refund review')} onClose={()=>setRefund(null)} footer={<Button disabled={Boolean(working)||!refundReason.trim()} onClick={async()=>{if(await act('refund','refundRequest',{id:refund.id,reason:refundReason})){setRefund(null);setRefundReason('');setNotice(t('Refund review requested. This is not confirmation that funds have been returned.'));}}}>{t('Submit review request')}</Button>}><Input aria-label={t('Refund reason')} maxLength={500} value={refundReason} onChange={e=>setRefundReason(e.target.value)}/></Modal>}
 </section>;
}
