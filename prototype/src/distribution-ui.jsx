import React,{createContext,useCallback,useContext,useEffect,useRef,useState} from 'react';
import {Bell,CheckCheck,Download,RefreshCw,ShieldCheck,ArrowUpCircle,X,Clock,Inbox} from 'lucide-react';
import {Badge,Button,Disclosure,EmptyState,Modal,Progress,SegmentedControl,Spinner,Surface,Switch,Toast} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {desktopAvailable} from './runtime-client.js';
import {distributionRequest,emptyDistribution} from './distribution-client.js';
import './distribution.css';

const Context=createContext(null);
const fallback={data:emptyDistribution(),loading:false,error:'',refresh:async()=>{},request:distributionRequest};
const useDistribution=()=>useContext(Context)||fallback;
export function DistributionProvider({children}) {
  const [data,setData]=useState(emptyDistribution),[loading,setLoading]=useState(false),[error,setError]=useState(''),[toast,setToast]=useState(null);
  const mounted=useRef(false),pending=useRef(false),urgentPending=useRef(false),current=useRef(data);current.current=data;
  const refresh=useCallback(async()=>{
    if(pending.current)return;pending.current=true;setLoading(true);
    try{const next=await distributionRequest('distribution_snapshot');if(mounted.current){setData(next);setError('');}}
    catch(cause){if(mounted.current)setError(String(cause?.message||cause));}
    finally{pending.current=false;if(mounted.current)setLoading(false);}
  },[]);
  const request=useCallback(async(command,args={})=>{
    setError('');
    try{const result=await distributionRequest(command,args);if(mounted.current&&['distribution_preferences','notifications_refresh','notifications_read'].includes(command))setData(result);else await refresh();return result;}
    catch(cause){if(mounted.current)setError(String(cause?.message||cause));await refresh();throw cause;}
  },[refresh]);
  useEffect(()=>{
    mounted.current=true;void refresh();let unlisten;let disposed=false;
    if(desktopAvailable()&&window.__TAURI__?.event?.listen)window.__TAURI__.event.listen('geod-distribution-changed',refresh).then(off=>{if(disposed)off();else unlisten=off;}).catch(()=>{});
    const timer=setInterval(()=>{if(current.current.busy)void refresh();},750);
    return()=>{disposed=true;mounted.current=false;clearInterval(timer);unlisten?.();};
  },[refresh]);
  useEffect(()=>{
    if(!desktopAvailable())return;
    let stopped=false,lastCheck=0,lastMessages=0;
    const poll=async()=>{
      if(stopped||document.hidden||current.current.busy)return;
      const now=Date.now(),state=current.current;
      if(state.updateConfigured&&state.automaticChecks&&now-Math.max(lastCheck,Date.parse(state.lastUpdateCheck)||0)>=6*60*60*1000){lastCheck=now;try{await request('update_check');}catch{/* Manual retry is available. */}}
      if(state.messagesConfigured&&state.notificationsEnabled&&now-Math.max(lastMessages,Date.parse(state.lastMessagesCheck)||0)>=5*60*1000){lastMessages=now;try{await request('notifications_refresh');}catch{/* Retain the cached inbox. */}}
    };
    // Give the first native snapshot time to arrive before checking channels.
    const first=setTimeout(poll,2500),timer=setInterval(poll,60_000);
    window.addEventListener('focus',poll);document.addEventListener('visibilitychange',poll);
    return()=>{stopped=true;clearTimeout(first);clearInterval(timer);window.removeEventListener('focus',poll);document.removeEventListener('visibilitychange',poll);};
  },[request]);
  useEffect(()=>{
    if(!data.notificationsEnabled||toast||urgentPending.current)return;
    const urgent=data.items.find(item=>item.priority==='important'&&!item.read&&!item.seen);
    if(!urgent)return;
    urgentPending.current=true;
    void request('notifications_read',{ids:[urgent.id],seenOnly:true}).then(()=>setToast(urgent)).catch(()=>{}).finally(()=>{urgentPending.current=false;});
  },[data.items,data.notificationsEnabled,request,toast]);
  const {locale}=useI18n();
  return <Context.Provider value={{data,loading,error,refresh,request}}>{children}{toast&&<Toast message={toast.title[locale]||toast.title.en} onDismiss={()=>setToast(null)} duration={10000}/>}</Context.Provider>;
}

export function UpdatesPanel() {
  const {t,date,number}=useI18n();const {data,loading,error,request}=useDistribution();
  const [working,setWorking]=useState(''),[failure,setFailure]=useState(''),[confirm,setConfirm]=useState(false);
  const update=data.update,phase=update.state;
  const act=async(command,args={})=>{if(working)return false;setWorking(command);setFailure('');try{await request(command,args);return true;}catch(cause){setFailure(String(cause?.message||cause));return false;}finally{setWorking('');}};
  const download=()=>act('update_download',{version:update.version});
  const states={idle:'Check for a newer version.',unconfigured:'The update channel will open with the first signed release.',checking:'Checking for updates…',upToDate:'You are up to date.',available:'A new version is available.',downloading:'Downloading update…',verifying:'Verifying update signature…',ready:'Update verified and ready to install.',installing:'Installing update…',error:'Could not check for updates. Check your network and retry.'};
  const transferring=['downloading','verifying'].includes(phase);
  const progress=Number.isFinite(update.total)&&update.total>0?Math.min(100,(update.downloaded||0)/update.total*100):undefined;
  return <Surface className="settings-panel updates-panel" aria-label={t('Software updates')}>
    <div className="distribution-heading"><div><h2>{t('Software updates')}</h2><p>GeoD Global <span>{data.version}</span></p></div><Badge>{t(data.development?'Development build':'Installed version')}</Badge></div>
    <div className="update-status" role="status" aria-live="polite">{loading||transferring||phase==='checking'?<Spinner/>:phase==='ready'?<ShieldCheck size={18}/>:<ArrowUpCircle size={18}/>}<div><strong>{t(states[phase]||states.idle)}</strong>{update.version&&<span>{t('New version: {version}',{version:update.version})}</span>}</div></div>
    {!desktopAvailable()?<p className="distribution-help">{t('Updates and notifications are available in the desktop app.')}</p>:!data.updateConfigured?<p className="distribution-help">{t('No signed version has been published to this channel yet. This is not an up-to-date result.')}</p>:null}
    {transferring&&<div className="update-progress"><Progress value={progress}/><small>{progress===undefined?t('Receiving update files…'):t('{percent}% downloaded',{percent:number(Math.floor(progress))})}</small></div>}
    {update.notes&&<Disclosure summary={t('Release notes')} defaultOpen><p className="release-notes">{update.notes}</p></Disclosure>}
    <div className="update-actions">
      <Button icon={RefreshCw} disabled={!desktopAvailable()||data.busy||Boolean(working)} onClick={()=>act('update_check')}>{t('Check for updates')}</Button>
      {phase==='available'&&<Button variant="primary" icon={Download} disabled={data.busy||Boolean(working)} onClick={download}>{t('Download update')}</Button>}
      {phase==='downloading'&&<Button icon={X} onClick={()=>distributionRequest('update_cancel').catch(()=>{})}>{t('Cancel download')}</Button>}
      {phase==='ready'&&<Button variant="primary" icon={ArrowUpCircle} disabled={data.development||data.busy||Boolean(working)} onClick={()=>setConfirm(true)}>{t('Install and restart')}</Button>}
    </div>
    {phase==='ready'&&data.development&&<p className="distribution-help">{t('Development builds do not install updates.')}</p>}
    {failure||error?<p className="distribution-error" role="alert">{t(failure||error)}</p>:null}
    {data.recoveredState&&<p className="distribution-help" role="status">{t('Notification preferences were reset. The damaged file was preserved for recovery.')}</p>}
    <div className="distribution-preferences">
      <label><span><strong>{t('Automatically check for updates')}</strong><small>{t('Checks only. Download and installation require your action.')}</small></span><Switch aria-label={t('Automatically check for updates')} checked={data.automaticChecks} disabled={!desktopAvailable()||Boolean(working)} onCheckedChange={checked=>act('distribution_preferences',{automaticChecks:checked,notificationsEnabled:data.notificationsEnabled})}/></label>
      <label><span><strong>{t('In-app notifications')}</strong><small>{t('Show important announcements once. Your read history stays on this device.')}</small></span><Switch aria-label={t('In-app notifications')} checked={data.notificationsEnabled} disabled={!desktopAvailable()||Boolean(working)} onCheckedChange={checked=>act('distribution_preferences',{automaticChecks:data.automaticChecks,notificationsEnabled:checked})}/></label>
    </div>
    {data.lastUpdateCheck&&<p className="distribution-help">{t('Last checked: {date}',{date:date(data.lastUpdateCheck,{hour:'2-digit',minute:'2-digit'})})}</p>}
    {confirm&&<Modal title={t('Install this update?')} closeLabel={t('Close')} closeDisabled={Boolean(working)} onClose={()=>setConfirm(false)} description={t('The app will close and restart. Downloads, processing and Agent replies must finish first. Your projects and files are retained.')} footer={<><Button disabled={Boolean(working)} onClick={()=>setConfirm(false)}>{t('Later')}</Button><Button variant="primary" disabled={Boolean(working)} onClick={()=>act('update_install',{version:update.version}).then(success=>{if(success)setConfirm(false);})}>{working?<Spinner/>:<ArrowUpCircle size={16}/>} {t('Install and restart')}</Button></>}>{failure&&<p className="distribution-error" role="alert">{t(failure)}</p>}</Modal>}</Surface>;
}

export function NotificationCenter() {
  const {t,locale,date,number}=useI18n();const {data,loading,error,request}=useDistribution();
  const [open,setOpen]=useState(false),[filter,setFilter]=useState('all'),[working,setWorking]=useState(false),[failure,setFailure]=useState('');
  const updateAvailable=['available','downloading','verifying','ready'].includes(data.update.state);
  const count=data.unreadCount+(updateAvailable&&!data.updateRead?1:0),items=data.items.filter(item=>filter!=='unread'||!item.read);
  const act=async(command,args={})=>{setWorking(true);setFailure('');try{await request(command,args);}catch(cause){setFailure(String(cause?.message||cause));}finally{setWorking(false);}};
  const navigate=page=>{setOpen(false);window.location.hash=page;};
  return <><Button className="notification-trigger" size="icon" variant="quiet" icon={Bell} aria-label={t('Notifications · {count} unread',{count:number(count)})} tooltip={t('Notifications')} onClick={()=>setOpen(true)}>{count>0&&<span className="notification-dot" aria-hidden="true"/>}</Button>
    {open&&<Modal className="notifications-dialog" title={t('Notifications')} closeLabel={t('Close')} onClose={()=>setOpen(false)}>
      <div className="notification-toolbar"><SegmentedControl value={filter} onValueChange={setFilter} items={[{value:'all',label:t('All notifications')},{value:'unread',label:t('Unread')}]}/><div><Button size="icon" variant="quiet" icon={RefreshCw} aria-label={t('Refresh notifications')} disabled={working||loading||!desktopAvailable()} onClick={()=>act('notifications_refresh')}/><Button size="sm" icon={CheckCheck} disabled={working||count===0} onClick={()=>act('notifications_read',{ids:data.items.filter(i=>!i.read).map(i=>i.id),seenOnly:false,...(updateAvailable?{updateVersion:data.update.version}:{})})}>{t('Mark all as read')}</Button></div></div>
      {(failure||error)&&<p className="distribution-error" role="alert">{t(failure||error)}</p>}
      <div className="notification-list">
        {updateAvailable&&(filter==='all'||!data.updateRead)&&<Surface as="article" variant="inset" className="notification-card update-notification"><ArrowUpCircle size={20}/><div><strong>{t('GeoD Global {version} is available',{version:data.update.version})}</strong><p>{t(data.update.state==='ready'?'Update verified and ready to install.':'View the release notes and download the update in Settings.')}</p><Button size="sm" onClick={()=>{void act('notifications_read',{ids:[],seenOnly:false,updateVersion:data.update.version});navigate('Settings');}}>{t('Open updates')}</Button></div></Surface>}
        {items.map(item=><Surface as="article" variant="inset" className="notification-card" data-unread={!item.read} key={`${item.id}:${item.revision}`}><div className="notification-card-title"><h3>{item.title[locale]||item.title.en}</h3>{item.priority==='important'&&<Badge>{t('Important')}</Badge>}{!item.read&&<span className="notification-unread"/>}</div><time dateTime={item.publishedAt}>{date(item.publishedAt)}</time><p>{item.body[locale]||item.body.en}</p><div className="notification-card-actions">{item.action&&<Button size="sm" onClick={()=>{void act('notifications_read',{ids:[item.id],seenOnly:false});navigate(item.action==='updates'?'Settings':'Home');}}>{t(item.action==='updates'?'Open updates':'Browse data sources')}</Button>}{!item.read&&<Button size="sm" variant="quiet" disabled={working} onClick={()=>act('notifications_read',{ids:[item.id],seenOnly:false})}>{t('Mark as read')}</Button>}</div></Surface>)}
        {!items.length&&(!updateAvailable||filter==='unread'&&data.updateRead)&&<EmptyState icon={Inbox} title={t(filter==='unread'?'No unread notifications':'No notifications yet')} description={t(data.messagesConfigured?'Product announcements and update reminders appear here.':'The announcement channel will open with the signed release channel.')}/>}
      </div>
      <p className="notification-footer"><Clock size={13}/>{data.lastMessagesCheck?t('Last refreshed: {date}',{date:date(data.lastMessagesCheck,{hour:'2-digit',minute:'2-digit'})}):t('Read status is saved locally. No login is required.')}</p>
    </Modal>}</>;
}
