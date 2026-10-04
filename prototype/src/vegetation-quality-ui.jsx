import React, { useState } from 'react';
import { Download, Layers } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { projectSourceJobs } from './projects-client.js';
import { VI_POLICIES, VI_SELECTION_KEYS, supportsViSelection } from './vegetation-quality.js';
import { Button, Disclosure, Progress, Select, Surface } from './ui/index.jsx';

export function VegetationQualityControls({ project, jobs, health, busy, onAction }) {
  const { t, number } = useI18n();
  const [key,setKey] = useState('ndvi'), [policy,setPolicy] = useState('good');
  if (!supportsViSelection(project)) return null;
  const needed = policy === 'none' ? [key] : VI_SELECTION_KEYS;
  const sources = needed.flatMap(k=>projectSourceJobs(project,jobs,k));
  const done = sources.filter(j=>j.status === 'succeeded').length, total = project.scenes.length*needed.length;
  const complete = done === total, active = sources.some(j=>['queued','running'].includes(j.status));
  const processing = jobs.some(j=>j.mosaic?.projectId === project.id && j.assetKey === key && ['queued','running'].includes(j.status));
  const download = async()=>{
    for (const k of needed) if (projectSourceJobs(project,jobs,k).filter(j=>j.status==='succeeded').length < project.scenes.length) {
      const ok = await onAction(k,'downloadProject'); if (ok === false) break;
    }
  };
  return <Surface variant="inset" className="project-asset modis-science-tools vi-quality-tools" aria-label={t('Vegetation index processing')}>
    <div className="modis-science-toolbar">
      <strong>{t('Vegetation indices')}</strong>
      <Select aria-label={t('Vegetation index')} value={key} onChange={e=>setKey(e.target.value)}><option value="ndvi">NDVI</option><option value="evi">EVI</option></Select>
      <Select aria-label={t('Pixel quality')} value={policy} onChange={e=>setPolicy(e.target.value)}>
        {Object.entries(VI_POLICIES).map(([k,v])=><option key={k} value={k}>{t(v)}</option>)}
        <option value="none">{t('Keep original values')}</option>
      </Select>
      <div className="project-actions">
        <Button size="sm" variant={complete?'secondary':'primary'} disabled={!health||busy||active||complete} onClick={download}><Download size={15}/>{t(policy==='none'?'Download index':'Download indices and QA')}</Button>
        <Button size="sm" variant={complete?'primary':'secondary'} disabled={!health||busy||!complete||processing} onClick={()=>onAction(key,'mosaicProject',policy==='none'?{}:{viQuality:{policy}})}><Layers size={15}/>{t(policy==='none'?'Clip or mosaic index':'Quality-screen and clip')}</Button>
      </div>
      <span>{t('{done} / {total} downloaded',{done:number(done),total:number(total)})}</span>
    </div>
    <Progress value={done} max={total} aria-label={t('Index and quality files ready')}/>
    <p className="project-asset-help">{t(policy==='none'?'Newest non-NoData composites win; no quality screening.':'NDVI and EVI use the same qualified observation. Newer rejected observations fall back together.')}</p>
  </Surface>;
}

export function VegetationSelectionDetails({ job, metadata }) {
  const { t, number, date } = useI18n();
  const result = metadata?.vegetation?.qualitySelection || job?.mosaicOutput?.viQuality;
  if (!result) return null;
  const spec = job?.mosaic?.viSelection;
  return <Disclosure className="wm-legend vi-selection-details" summary={t('Vegetation quality selection')}>
    <dl className="runtime-details">
      <dt>{t('Quality rule')}</dt><dd>{t(VI_POLICIES[result.policy])}</dd>
      <dt>{t('Complete index candidates')}</dt><dd>{number(result.inputCommonValidPixels)}</dd>
      <dt>{t('Removed by quality screening')}</dt><dd>{number(result.removedValidPixels)}</dd>
      <dt>{t('Older qualified observations retained')}</dt><dd>{number(result.fallbackPixels)}</dd>
      <dt>{t('No qualified observation')}</dt><dd>{number(result.rejectedPixels)}</dd>
    </dl>
    <p>{t('Counts cover every output pixel. Missing pairs, outside-area pixels and quality failures are NoData.')}</p>
    <p>{t('Reject missing or undefined QA, low usefulness, high aerosol, adjacent or mixed clouds, snow/ice and shadow. These are GeoD screening choices.')}</p>
    {spec && <Disclosure summary={t('Observation sources and contributions')}>
      <ul className="vi-source-list">{spec.scenes.map((s,i)=><li key={s.itemId}>
        <strong>{t('Composite start')} · {date(s.compositeStart)} · {number(result.sceneValidPixels[i])} {t('pixels')}</strong>
        <code className="runtime-wrap">{s.itemId}</code>
        <div>{s.sources.map((source,k)=><a key={source.jobId} href={source.href} target="_blank" rel="noreferrer" title={`SHA-256 ${source.sha256}`}>{['NDVI','EVI',t('Detailed VI quality'),t('Pixel reliability')][k]}</a>)}</div>
      </li>)}</ul>
    </Disclosure>}
    <a href="https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf" target="_blank" rel="noreferrer">{t('NASA MOD13 quality definitions')}</a>
  </Disclosure>;
}
