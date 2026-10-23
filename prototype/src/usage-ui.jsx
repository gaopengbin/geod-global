import React,{useEffect,useState} from 'react';
import {usage} from './usage-client.js';
import {usageContext,usagePage} from './usage-events.js';
import {useI18n} from './i18n.jsx';
import {Badge,Button,InfoDetails,Surface,Switch} from './ui/index.jsx';

export function UsageLifecycle({children}) {
  useEffect(()=>{
    const stop=usage.start();let previous;
    const page=()=>{const next=usagePage();if(next!==previous&&usage.enabled()){previous=next;usage.record('page_view',usageContext({page:next}));}};
    page();const unsubscribe=usage.subscribe(()=>{if(!usage.enabled())previous=undefined;else page();});
    const flush=()=>{void usage.flush();};
    const hidden=()=>{if(document.visibilityState==='hidden')flush();};
    window.addEventListener('hashchange',page);window.addEventListener('online',flush);
    window.addEventListener('pagehide',flush);document.addEventListener('visibilitychange',hidden);
    return()=>{unsubscribe();stop();window.removeEventListener('hashchange',page);window.removeEventListener('online',flush);window.removeEventListener('pagehide',flush);document.removeEventListener('visibilitychange',hidden);};
  },[]);
  return children;
}

export function UsageSettings({client=usage}) {
  const {t}=useI18n(),[state,setState]=useState(()=>client.snapshot());
  useEffect(()=>{setState(client.snapshot());return client.subscribe(setState);},[client]);
  const download=()=>{
    const url=URL.createObjectURL(new Blob([JSON.stringify(client.report(),null,2)],{type:'application/json'}));
    const link=document.createElement('a');link.href=url;link.download='geod-global-usage.json';link.click();
    setTimeout(()=>URL.revokeObjectURL(url),1000);
  };
  return <Surface className="settings-panel">
    <div className="bui-info-row"><h2>{t('Usage statistics')}</h2><Badge>{t(state.blocked?'Blocked by privacy preference':state.status==='off'?'Off':state.sharing?'Anonymous sharing':'Local only')}</Badge>
      <InfoDetails closeLabel={t('Close')} summary={t('About usage statistics')}>
        <p>{t('Anonymous usage statistics are on by default for new installations. You can turn them off here at any time. Statistics record feature categories, source identifiers, outcomes and coarse duration and size ranges to help improve searches, downloads and workflows.')}</p>
        <p>{t('Messages, model keys, account details, precise locations, scene identifiers, project names, file paths and source URLs are excluded. Random installation identifiers are independent of your account.')}</p>
        <p>{t('Tutorial distribution packages may include a public channel and campaign label. When statistics are enabled, the first valid label is retained with anonymous feature events. Website visits and installations use separate identifiers; a download click is not an installation.')}</p>
        <p>{t('Records stay on this device for up to 30 days. Anonymous sharing automatically sends unsent records while enabled; disabling collection removes local records and stops pending uploads. Previously saved off preferences remain off. Already received anonymous records expire on the collection service within 90 days.')}</p>
        <p>{t(state.configured?'An anonymous collection service is configured. Sharing is optional and works without signing in.':'No collection service is configured in this build. Records and exports stay on this device.')}</p>
      </InfoDetails>
    </div>
    <div className="settings-fields">
      <span>{t('On by default · turn off at any time')}</span>
      <label><strong>{t('Record usage statistics')}</strong><Switch aria-label={t('Record usage statistics')} checked={state.enabled} disabled={state.blocked} onCheckedChange={enabled=>client.setPreferences({enabled})}/></label>
      {state.configured&&<label><strong>{t('Share anonymous usage')}</strong><Switch aria-label={t('Share anonymous usage')} checked={state.sharing} disabled={!state.enabled} onCheckedChange={share=>client.setPreferences({share})}/></label>}
      {state.enabled&&<div className="bui-info-row"><span>{t('{count} local records',{count:state.recorded})}</span><Button variant="secondary" size="sm" onClick={download}>{t('Export usage report')}</Button><Button variant="quiet" size="sm" onClick={()=>client.clear()}>{t('Clear usage data')}</Button></div>}
    </div>
  </Surface>;
}
