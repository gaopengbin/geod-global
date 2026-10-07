import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Maximize, Minus, Plus, MapPin, RefreshCw, Map as MapIcon, X } from 'lucide-react';
import { Button, Input, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { agentRequest } from './agent-client.js';
import { loadPlanPreviewScenes } from './agent-map-preview.js';
import { ExploreMap } from './explore-map.jsx';
import { imageryHrefs } from './explore-imagery.js';
import { catalogPreviewChannel } from './catalog-preview.js';
import { prepareAssetAccess } from './providers.js';

export default function AgentPlanMapPreview({plan,sessionId,onClose}) {
  const {t,date,number}=useI18n(),map=useRef(null);
  const returnFocus=useRef(document.activeElement);
  const close=()=>{onClose();requestAnimationFrame(()=>{if(returnFocus.current?.isConnected)returnFocus.current.focus();});};
  const [preview,setPreview]=useState(null),[scenes,setScenes]=useState([]),[activeId,setActiveId]=useState(null),[visibleIds,setVisibleIds]=useState([]),[accessReady,setAccessReady]=useState(false);
  const [loading,setLoading]=useState(true),[error,setError]=useState(''),[failed,setFailed]=useState(0),[retry,setRetry]=useState(0);
  useEffect(()=>{
    const controller=new AbortController();setLoading(true);setError('');setFailed(0);setPreview(null);setScenes([]);setVisibleIds([]);setAccessReady(false);
    (async()=>{
      try {
        const value=await agentRequest('planMapPreview',{sessionId,planId:plan.planId,planHash:plan.planHash});
        if(controller.signal.aborted)return;
        if(value.planId!==plan.planId || value.planHash!==plan.planHash)throw Error('The imagery metadata differs from this task.');
        setPreview(value);
        const result=await loadPlanPreviewScenes(value,{signal:controller.signal});
        if(controller.signal.aborted)return;
        try {await prepareAssetAccess(result.scenes.flatMap(imageryHrefs),{signal:controller.signal});if(!controller.signal.aborted)setAccessReady(true);}
        catch(error){if(!controller.signal.aborted)setError(String(error.message||error));}
        if(controller.signal.aborted)return;
        setScenes(result.scenes);setVisibleIds(result.scenes.map(s=>s.id));setActiveId(result.scenes[0]?.id??null);setFailed(result.failed);
      } catch(error) {if(!controller.signal.aborted)setError(String(error.message||error));}
      finally {if(!controller.signal.aborted)setLoading(false);}
    })();
    return ()=>controller.abort();
  },[sessionId,plan.planId,plan.planHash,retry]);
  const visible=useMemo(()=>scenes.filter(s=>visibleIds.includes(s.id)),[scenes,visibleIds]);
  const scene=visible.find(s=>s.id===activeId)||visible[0]||scenes[0];
  const loaded=useMemo(()=>accessReady?visible.filter(s=>imageryHrefs(s).length):[],[visible,accessReady]);
  const previewScenes=useMemo(()=>visible.filter(s=>catalogPreviewChannel(s,plan.files[0]?.assetKey)),[visible,plan.files]);
  const channel=visible.length?catalogPreviewChannel(scene,plan.files[0]?.assetKey):undefined;
  const toggle=(id,checked)=>setVisibleIds(current=>checked?[...new Set([...current,id])]:current.filter(value=>value!==id));
  return <section className="agent-map-preview-panel" role="region" aria-label={t('Task map preview')}>
    <header className="agent-map-preview-header"><span><MapIcon size={17}/><strong>{t('Map preview')}</strong></span><Button size="icon" variant="quiet" icon={X} aria-label={t('Close map preview')} tooltip={t('Close map preview')} onClick={close}/></header>
    <div className="agent-map-preview-layout">
      <div className="agent-map-preview-canvas">
        {preview ? <ExploreMap ref={map} scene={scene} scenes={scenes} loadedScenes={loaded} previewScenes={previewScenes} selectedIds={visibleIds} activeSceneId={scene?.id} area={preview.bounds} areaGeometry={preview.geometry} showArea previewChannel={channel} onFootprintsPick={ids=>{const id=ids.find(id=>visibleIds.includes(id));if(id)setActiveId(id);}}/> : <div className="agent-map-preview-empty">{loading?<><Spinner size={20}/>{t('Reading task area…')}</>:<MapPin size={24}/>}</div>}
        {preview&&<div className="agent-map-preview-controls"><Button size="icon" icon={Plus} aria-label={t('Zoom in')} tooltip={t('Zoom in')} onClick={()=>map.current?.zoomIn()}/><Button size="icon" icon={Minus} aria-label={t('Zoom out')} tooltip={t('Zoom out')} onClick={()=>map.current?.zoomOut()}/><Button size="icon" icon={Maximize} aria-label={t('Fit selected imagery')} tooltip={t('Fit selected imagery')} onClick={()=>map.current?.fit()}/><Button size="icon" icon={MapPin} aria-label={t('Fit search area')} tooltip={t('Fit search area')} onClick={()=>map.current?.fitArea()}/></div>}
      </div>
      <aside className="agent-map-preview-list">
        <div className="agent-map-preview-area"><strong>{t('Search area')}</strong><span>WGS 84 · {(preview?.bounds||plan.bounds).map(v=>number(v,{maximumFractionDigits:4})).join(', ')}</span></div>
        {loading&&<p role="status"><Spinner size={14}/>{t('Loading selected imagery…')}</p>}
        {(error||failed>0)&&<div role="alert" className="agent-map-preview-warning"><p>{t(error||'Some imagery could not be previewed. The task selection is unchanged.')}</p><Button size="sm" icon={RefreshCw} onClick={()=>setRetry(n=>n+1)}>{t('Retry preview')}</Button></div>}
        {!loading&&scenes.length>0&&<div className="agent-map-preview-selection"><span>{t('Visible imagery · {count}/{total}',{count:visible.length,total:scenes.length})}</span><Button size="xs" variant="quiet" onClick={()=>setVisibleIds(scenes.map(s=>s.id))} disabled={visible.length===scenes.length}>{t('Show all')}</Button><Button size="xs" variant="quiet" onClick={()=>setVisibleIds([])} disabled={!visible.length}>{t('Hide all')}</Button></div>}
        <div className="agent-map-preview-scenes" role="region" tabIndex={0} aria-label={t('Selected imagery')}>{scenes.map(item=><label key={item.id} className="agent-map-preview-scene-row" data-selected={visibleIds.includes(item.id)||undefined}><Input type="checkbox" checked={visibleIds.includes(item.id)} aria-label={t('Show scene {id}',{id:item.id})} onChange={event=>toggle(item.id,event.target.checked)}/><span><strong>{date(item.date)}</strong><small>{item.id}</small><small>{item.cloud==null?t('Cloud cover unavailable'):t('Cloud cover {cloud}',{cloud:number(item.cloud/100,{style:'percent',maximumFractionDigits:1})})}</small></span></label>)}</div>
        {!loading&&scenes.length>0&&!loaded.length&&!channel&&<p className="agent-map-preview-note">{t('This source shows scene footprints here; online imagery preview is unavailable.')}</p>}
      </aside>
    </div>
    <footer className="agent-map-preview-footer"><span>{t('Preview reads online imagery only. No download task is started.')}</span></footer>
  </section>;
}
