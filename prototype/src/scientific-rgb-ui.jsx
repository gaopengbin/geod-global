import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Layers } from 'lucide-react';
import { runtimeRequest, formatBytes } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Button, Input, Select, Modal, Spinner, Surface, Disclosure } from './ui/index.jsx';
import { rgbQualityJobs } from './local-rgb.js';
import './scientific-rgb.css';

function waitForPreflightRetry(signal, delay) {
  return new Promise(resolve=>{
    const finish=()=>{clearTimeout(timer);signal.removeEventListener('abort',finish);resolve();};
    const timer=setTimeout(finish,delay);
    signal.addEventListener('abort',finish,{once:true});
    if(signal.aborted)finish();
  });
}

export function ScientificRgbDialog({group,jobs=[],projectId,onClose,onQueued}) {
  const {t,locale,number}=useI18n();
  const [plan,setPlan]=useState(null),[name,setName]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const [waitingForRead,setWaitingForRead]=useState(false);
  const controller=useRef(null);
  const defaultName=useRef('');
  const [policy,setPolicy]=useState('none'),[excludeSnow,setExcludeSnow]=useState(false),[planKey,setPlanKey]=useState('');
  const quality=useMemo(()=>rgbQualityJobs(group,jobs),[group,jobs]);
  const landsat=group.product==='landsat-c2-l2';
  const masked=policy!=='none' && quality.length===2;
  const coupled=masked && group.derived && group.sourceJobs[0].mosaic?.sources?.length>1;
  const request={jobIds:group.sourceJobs.map(job=>job.id),...(projectId?{projectId}:{}),
    ...(masked?{qualityMask:{...(landsat?{qaPixelJobId:quality[0].id,qaRadsatJobId:quality[1].id}:{qcJobId:quality[0].id,stateJobId:quality[1].id}),policy,excludeSnow}}:{})};
  const requestKey=JSON.stringify(request);
  useEffect(()=>{
    const abort=new AbortController();controller.current=abort;setPlan(null);setError('');setWaitingForRead(false);
    const payload=JSON.parse(requestKey);
    const check=async()=>{
      const deadline=Date.now()+60000;
      let attempts=0;
      while(!abort.signal.aborted){
        try {
          const data=await runtimeRequest('planRgb',payload,abort.signal);
          if(abort.signal.aborted)return;
          const expected=payload.qualityMask,actual=data.spec.qualityMask;
          if (expected ? !actual || actual.policy!==expected.policy || actual.excludeSnow!==expected.excludeSnow
            || actual.sources?.[0]?.jobId!==(expected.qcJobId || expected.qaPixelJobId) || actual.sources?.[1]?.jobId!==(expected.stateJobId || expected.qaRadsatJobId) : Boolean(actual))
            throw new Error('The RGB quality preflight does not match the selected rules.');
          if (expected && !coupled && (actual.schemaVersion!==(landsat?'geod-landsat-rgb-mask/v1':'geod-modis-rgb-mask/v1') || actual.coupled))
            throw new Error('The RGB quality preflight does not match the selected rules.');
          if (coupled && (actual?.schemaVersion!==(landsat?'geod-landsat-rgb-mask/v2':'geod-modis-rgb-mask/v2') || actual.coupled?.scenes?.length!==group.sourceJobs[0].mosaic.sources.length))
            throw new Error('The RGB quality preflight does not match the selected rules.');
          const previousDefault=defaultName.current;setPlan(data);setPlanKey(requestKey);setName(previous=>previous && previous!==previousDefault ? previous : data.spec.name);defaultName.current=data.spec.name;
          setWaitingForRead(false);return;
        } catch(e){
          if(abort.signal.aborted)return;
          if(e.message!=='Raster inspection is busy; try again shortly' || Date.now()>=deadline){setWaitingForRead(false);setError(e.message);return;}
          setWaitingForRead(true);
          await waitForPreflightRetry(abort.signal,Math.min(500*++attempts,1500,deadline-Date.now()));
        }
      }
    };
    check();
    return ()=>abort.abort();
  },[requestKey]);
  useEffect(()=>()=>controller.current?.abort(),[]);
  const run=async()=>{
    if (!plan || planKey!==requestKey || busy) return;
    const abort=new AbortController();controller.current=abort;setBusy(true);setError('');
    try {const job=await runtimeRequest('runRgb',{...request,name:name.trim()},abort.signal);if(!abort.signal.aborted)await onQueued(job);}
    catch(e){if(!abort.signal.aborted)setError(e.message);}
    finally{if(!abort.signal.aborted)setBusy(false);}
  };
  return <Modal className="scientific-rgb-dialog" title={t('Create scientific RGB')} description={t('Combine the checked red, green and blue bands into a reusable GeoTIFF.')} onClose={onClose} closeLabel={t('Close')}>
    {['modis-09a1-v061','landsat-c2-l2'].includes(group.product) && <div className="processing-plan">
      <label className="runtime-field">{t('Quality screening')}<Select value={policy} disabled={busy} onChange={e=>setPolicy(e.target.value)}>
        <option value="none">{t('Keep all original values')}</option>
        {landsat ? <><option value="cloud_free" disabled={quality.length!==2}>{t('Exclude cloud, shadow and RGB saturation')}</option><option value="cloud_free_conservative" disabled={quality.length!==2}>{t('Conservative cloud-free flags')}</option></>
          : <><option value="clear" disabled={quality.length!==2}>{t('Exclude cloud and shadow flags')}</option><option value="clear_best" disabled={quality.length!==2}>{t('Clear pixels with best RGB quality')}</option></>}
      </Select></label>
      {masked ? <><label className="runtime-check"><Input type="checkbox" checked={excludeSnow} disabled={busy} onChange={e=>setExcludeSnow(e.target.checked)}/>{t('Also exclude snow and ice')}</label><p>{t(landsat
        ? policy==='cloud_free_conservative' ? 'Require clear and explicit low cloud, shadow and cirrus confidence. Unset confidence and unused saturation bits are excluded; water remains eligible.' : 'Exclude fill, cloud, cirrus, shadow, RGB saturation and terrain occlusion flags. Water and non-RGB saturation alone remain eligible.'
        : coupled ? 'Overlaps use the newest qualified complete RGB scene. An older qualified scene fills flagged or incomplete newer pixels.' : 'Only explicit clear flags are accepted. Unknown flags and quality fill values are excluded.')}</p></>
        : quality.length!==2 && <p>{t(landsat ? 'Download matching Landsat QA_PIXEL and QA_RADSAT files to enable screening.' : 'Download the matching MODIS QC and state files to enable screening.')}</p>}
    </div>}
    {coupled && landsat && <p>{t('Overlaps use the newest qualified complete RGB scene. An older qualified scene fills flagged or incomplete newer pixels.')}</p>}
    {!plan && !error && <p role="status"><Spinner size={16}/> {t(waitingForRead?'Waiting for the current raster read…':'Checking band grids…')}</p>}
    {plan && <><label className="runtime-field">{t('Result name')}<Input value={name} maxLength={160} disabled={busy} onChange={e=>setName(e.target.value)}/></label>
      <div className="processing-plan scientific-rgb-summary"><dl className="runtime-details"><dt>{t('Pixels')}</dt><dd>{number(plan.spec.grid.width)} × {number(plan.spec.grid.height)}</dd><dt>{t('Data type')}</dt><dd>{plan.spec.profile.signed?'Int16':'UInt16'} · RGB · {plan.spec.grid.crs}</dd><dt>{t('Working disk space')}</dt><dd>{formatBytes(plan.requiredDiskBytes,locale)}</dd></dl></div>
      <p>{t(masked ? 'Accepted pixels keep original DN. Rejected pixels become NoData; the rules and source checksums are saved with the result.' : 'Original DN, per-band calibration and NoData are retained. The task continues in the background.')}</p>
      <footer className="runtime-dialog-actions"><Button onClick={onClose}>{t('Cancel')}</Button><Button variant="primary" disabled={busy||!name.trim()||planKey!==requestKey} onClick={run}>{busy?<Spinner size={16}/>:<Layers size={16}/>} {t(busy?'Submitting…':'Create RGB file')}</Button></footer></>}
    {error && <Surface className="runtime-error" role="alert"><p>{t(error==='Raster inspection is busy; try again shortly'?'Raster reading is taking longer than expected. Try again shortly.':'The RGB file could not be created. Check the three source bands and retry.')}</p><Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(error)}</p></Disclosure></Surface>}
  </Modal>;
}
