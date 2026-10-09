import React,{createContext,useContext,useEffect,useRef,useState} from 'react';
import {ArrowRight,ArrowUpRight,Check,FolderCheck,LogOut,Map,MessageSquare,RefreshCw,ShieldCheck,UserRound,X} from 'lucide-react';
import {Badge,Button,Input,Spinner,Surface} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {desktopAvailable} from './runtime-client.js';
import {identityRequest,signedOutIdentity} from './identity-client.js';
import {SOURCE_DIRECTORY,SOURCE_GROUPS} from './source-directory.js';
import './identity.css';
const IdentityContext=createContext(null);
const messages={
 'service-unavailable':'Account service is unavailable. Try again.',
 'desktop-not-ready':'Desktop sign-in is not available on the account server yet. You can continue locally.',
 expired:'Your sign-in has expired. Sign in again.',
 denied:'Sign-in was cancelled in the browser. You can try again.',
 timeout:'Sign-in timed out. Try again and finish in the browser within five minutes.',
 'browser-failed':'The browser could not be opened. Check your default browser and try again.',
 'callback-failed':'The sign-in callback could not be verified. Start a new sign-in.',
 'storage-unavailable':'Secure account storage is unavailable. Clear the checkbox to sign in for this session.',
 'logout-not-confirmed':'You are signed out on this device. Server revocation could not be confirmed while offline.',
};
export function IdentityProvider({children}){
 const [snapshot,setSnapshot]=useState(signedOutIdentity),[loading,setLoading]=useState(false),[failure,setFailure]=useState('');
 const alive=useRef(true),inFlight=useRef(false),revision=useRef(0);
 const run=async(operation,payload)=>{
  if(inFlight.current&&operation!=='cancel')return;
  const current=++revision.current;
  inFlight.current=true;setLoading(true);setFailure('');
  try{const next=await identityRequest(operation,payload);if(alive.current&&revision.current===current)setSnapshot(next);return next;}
  catch(error){if(alive.current&&revision.current===current)setFailure(error.message);}
  finally{if(revision.current===current){inFlight.current=false;if(alive.current)setLoading(false);}}
 };
 useEffect(()=>{alive.current=true;if(desktopAvailable())run('snapshot');return()=>{alive.current=false;};},[]);
 useEffect(()=>{if(!snapshot.busy)return;const id=setInterval(()=>{if(!inFlight.current)run('snapshot');},700);return()=>clearInterval(id);},[snapshot.busy]);
 return <IdentityContext.Provider value={{snapshot,loading,error:failure||messages[snapshot.error]||'',run}}>{children}</IdentityContext.Provider>;
}
export function useIdentity(){return useContext(IdentityContext)||{snapshot:signedOutIdentity(),loading:false,error:'',run:async()=>{}};}
export function AccountAvatar({user}){
 const [failed,setFailed]=useState(false);useEffect(()=>setFailed(false),[user?.avatar]);
 return <span className="identity-avatar">{user?.avatar&&!failed?<img src={user.avatar} alt="" onError={()=>setFailed(true)}/>:user?<span>{(user.name||user.email||'G').slice(0,1).toUpperCase()}</span>:<UserRound size={18} aria-hidden="true"/>}</span>;
}
export function IdentityEntry({onOpen}){
 const {t}=useI18n(),{snapshot}=useIdentity();
 return <Button variant="quiet" className="identity-entry" onClick={onOpen} aria-label={t(snapshot.user?'Your account':'Sign in')} tooltip={t(snapshot.user?'Your account':'Sign in')}><AccountAvatar user={snapshot.user}/><span className="identity-entry-label">{snapshot.user?.name||t('Sign in')}</span></Button>;
}
export function SignInPage({onContinue}){
 const {t,locale}=useI18n(),{snapshot,loading,error,run}=useIdentity(),[remember,setRemember]=useState(true);
 const previous=useRef(snapshot.status);
 useEffect(()=>{if(previous.current==='waiting'&&snapshot.status==='signed-in')onContinue();previous.current=snapshot.status;},[snapshot.status,onContinue]);
 const native=desktopAvailable(),waiting=snapshot.busy;
 const proceed=()=>{if(waiting||loading)void run('cancel');onContinue();};
 return <section className="identity-page dark" data-theme="dark" aria-label={t('GeoD Global account')}>
  <div className="identity-introduction">
   <div className="identity-wordmark"><img src="/brand/geod-symbol.png" alt="" width="38" height="38"/><span>GeoD <strong>Global</strong></span></div>
   <div className="identity-pitch"><Badge tone="accent">{t('Many sources. One Agent.')}</Badge><h1>{t('Start with a place. Leave with useful data.')}</h1><p>{t('Find imagery, preview your area, review the plan and keep the result locally.')}</p>
    <div className="identity-workflow">{[[MessageSquare,'Describe your goal'],[Map,'Preview and review'],[FolderCheck,'Keep local results']].map(([Icon,label])=><div key={label}><Icon size={19} aria-hidden="true"/><span>{t(label)}</span></div>)}</div>
    <div className="identity-source-summary"><div><strong>{SOURCE_DIRECTORY.length}</strong><span>{t('Directory entries')}</span></div><div><strong>{SOURCE_GROUPS.length}</strong><span>{t('Product groups')}</span></div></div>
    <p className="identity-scope">{t('Imagery · elevation · maps · vectors. Integration status is shown for every entry.')}</p>
   </div><div className="identity-local-note"><ShieldCheck size={16}/><span>{t('Your files and local tasks stay on this device.')}</span></div>
  </div>
  <div className="identity-login"><Surface className="identity-login-card">
   <div className="identity-login-icon">{snapshot.user?<AccountAvatar user={snapshot.user}/>:<UserRound size={24} aria-hidden="true"/>}</div>
   <h2>{t(snapshot.user?'Your GeoD Global account':'Welcome to GeoD Global')}</h2>
   <p>{t(snapshot.user?'Signed in to your independent Global account.':'Sign in with your email, Google or GitHub. Your first sign-in creates an independent Global account.')}</p>
   {snapshot.user?<><div className="identity-user-details"><strong>{snapshot.user.name}</strong>{snapshot.user.email&&<span>{snapshot.user.email}</span>}<span>{({email:'GeoD Global',google:'Google',github:'GitHub'})[snapshot.user.provider]}<Badge tone="accent"><Check size={11}/>{t('Signed in')}</Badge></span></div><Button variant="primary" onClick={onContinue} icon={ArrowRight}>{t('Continue to workspace')}</Button><Button variant="quiet" disabled={loading} onClick={()=>run('logout')} icon={LogOut}>{t(loading?'Signing out…':'Sign out of this device')}</Button></>:
    <>{waiting?<div className="identity-waiting" role="status"><Spinner size={24}/><strong>{t('Finish signing in in your browser')}</strong><p>{t('Authorize GeoD Global, then return here. This page will update automatically.')}</p><Button variant="secondary" icon={X} disabled={loading} onClick={()=>run('cancel')}>{t('Cancel sign-in')}</Button></div>:
     <div className="identity-provider-buttons"><Button variant="primary" className="identity-provider identity-provider-email" disabled={loading||!native||!snapshot.ready||!snapshot.providers.find(p=>p.id==='email')?.available} onClick={()=>run('begin',{provider:'email',locale,remember})}><img src="/brand/geod-symbol.png" width="22" height="22" alt=""/>{t('GeoD Global account sign-in')}<ArrowUpRight size={15} aria-hidden="true"/></Button><p className="identity-email-hint">{t('Email code · automatic registration on first sign-in')}</p>{native&&snapshot.ready&&!snapshot.providers.find(p=>p.id==='email')?.available&&<p className="identity-email-unavailable" role="status">{t('Email sign-in is not available yet. Google and GitHub are still available.')}</p>}{['google','github'].map(provider=><Button key={provider} className={'identity-provider identity-provider-'+provider} disabled={loading||!native||!snapshot.ready||!snapshot.providers.find(p=>p.id===provider)?.available} onClick={()=>run('begin',{provider,locale,remember})}><img src={'/account-marks/'+provider+'.svg'} width="18" height="18" alt=""/>{t(provider==='google'?'Continue with Google':'Continue with GitHub')}<ArrowUpRight size={15} aria-hidden="true"/></Button>)}</div>}
     {!waiting&&<label className="identity-remember"><Input type="checkbox" checked={remember} disabled={loading} onChange={event=>setRemember(event.target.checked)}/><span>{t('Keep me signed in on this device')}</span></label>}
     <div className="identity-browser-note"><ShieldCheck size={15}/><span>{t('Complete email verification or provider authorization in your system browser, then return to the app.')}</span></div>
     {!native&&<p className="identity-note">{t('Open the desktop app to sign in. You can still explore the local interface here.')}</p>}
     {loading&&!waiting&&<p className="identity-note" role="status"><Spinner size={14}/>{t('Checking account service…')}</p>}
    </>}
   {error&&<div className="identity-error" role="alert"><p>{t(error)}</p><Button variant="quiet" size="sm" disabled={loading||waiting} onClick={()=>run('snapshot',{refresh:true})} icon={RefreshCw}>{t('Retry')}</Button></div>}
   {!snapshot.user&&<><div className="identity-divider"><span>{t('or')}</span></div><Button className="identity-guest" variant="secondary" onClick={proceed} icon={ArrowRight}>{t('Continue without an account')}</Button></>}
   <p className="identity-account-scope">{t('Global sign-in is separate from NASA Earthdata and Copernicus data authorization.')}</p>
  </Surface><div className="identity-policy"><Button asChild variant="link" size="sm"><a href="https://geod-global.laogao.xyz/privacy.html" target="_blank" rel="noreferrer">{t('Privacy')}</a></Button></div></div>
 </section>;
}
