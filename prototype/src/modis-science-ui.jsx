import React, { useState } from 'react';
import { Download, Layers } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { MODIS_SCIENCE, MODIS_SCIENCE_KEYS } from './modis-science-layers.js';
import { projectSourceJobs } from './projects-client.js';
import { QualityPixelDetails } from './quality-ui.jsx';
import { Button, Disclosure, Progress, Select, Surface } from './ui/index.jsx';

export function ModisScienceControls({ project, jobs, health, busy, onAction }) {
  const { t, number } = useI18n();
  const available = MODIS_SCIENCE_KEYS.filter(key=>project.scenes.some(scene=>scene.assets?.[key]));
  const [selected,setSelected] = useState('vi_quality');
  if (!available.length) return null;
  const key = available.includes(selected) ? selected : available[0], layer = MODIS_SCIENCE[key];
  const sources = projectSourceJobs(project,jobs,key), done = sources.filter(j=>j.status === 'succeeded').length;
  const active = sources.some(j=>['queued','running'].includes(j.status));
  const complete = project.scenes.every(scene=>scene.assets?.[key]) && done === project.scenes.length;
  const crossYear = key === 'vi_doy' && new Set(project.scenes.map(s=>new Date(s.date).getUTCFullYear())).size > 1;
  const processing = jobs.some(j=>j.mosaic?.projectId === project.id && j.assetKey === key && ['queued','running'].includes(j.status));
  return <Surface variant="inset" className="project-asset modis-science-tools" aria-label={t('MODIS scientific layers')}>
    <div className="modis-science-toolbar">
    <strong>{t('MODIS scientific layers')}</strong>
    <Select aria-label={t('Scientific layer')} value={key} onChange={event=>setSelected(event.target.value)}>
      {available.map(key=><option key={key} value={key}>{t(MODIS_SCIENCE[key].label)}</option>)}
    </Select>
    <span>{t('{done} / {total} downloaded',{done:number(done),total:number(project.scenes.length)})}</span>
    <div className="project-actions">
      <Button size="sm" variant={complete ? 'secondary' : 'primary'} disabled={!health || busy || active || complete} onClick={()=>onAction(key,'downloadProject')}><Download size={15}/>{t('Download layer')}</Button>
      <Button size="sm" variant={complete ? 'primary' : 'secondary'} disabled={!health || busy || !complete || crossYear || processing} onClick={()=>onAction(key,'mosaicProject')}><Layers size={15}/>{t(project.scenes.length === 1 ? 'Clip layer' : 'Mosaic and clip layer')}</Button>
    </div>
    </div>
    <Progress value={done} max={project.scenes.length} aria-label={`${t(layer.label)} · ${t('Files ready')}`}/>
    {crossYear && <p className="project-asset-help">{t('Process observation-day layers separately for each calendar year.')}</p>}
  </Surface>;
}

export function ScienceDetails({ metadata }) {
  const { t, number } = useI18n(), info = metadata?.science;
  if (!info) return null;
  return <Disclosure className="wm-legend" summary={t(MODIS_SCIENCE[info.band].label)}>
    <dl className="runtime-details"><dt>{t('Unit')}</dt><dd>{t(info.unit)}</dd><dt>{t('Original value conversion')}</dt><dd>DN × {info.scale}</dd>
      <dt>{t('Product valid range')}</dt><dd>{info.validRange.map(v=>number(v*info.scale,{maximumFractionDigits:4})).join(' – ')}</dd>
      {info.calendarYear && <><dt>{t('Calendar year')}</dt><dd>{info.calendarYear}</dd></>}
      <dt>{t('Preview samples')}</dt><dd>{number(info.validSampleCount)} / {number(info.sampleCount)}</dd>
      <dt>{t('Outside product valid range')}</dt><dd>{number(info.outOfRangeSampleCount)}</dd></dl>
    {metadata.classes.length > 0 && <ul>{metadata.classes.map(item=><li key={item.value}><i style={{background:item.color}}/><span>{t(item.label)} · {number(item.count)}</span></li>)}</ul>}
    <p>{t('Counts describe sampled preview pixels. Colors do not change original values or apply a quality mask.')}</p>
    <a href={info.definition} target="_blank" rel="noreferrer">{t('NASA science layer definitions')}</a>
  </Disclosure>;
}

export function SciencePixelReadout({ pixel, metadata }) {
  const { t, number, date } = useI18n(), info = metadata.science;
  const value = pixel.science;
  const label = pixel.isNoData ? t('NoData') : info.kind === 'date' ? value.date ? date(value.date) : t('Invalid observation day')
    : info.kind === 'angle' ? `${number(value.convertedValue,{maximumFractionDigits:2})}°`
    : info.kind === 'reflectance' ? number(value.convertedValue,{maximumFractionDigits:4})
    : info.kind === 'rank' ? t(pixel.label) : `QA ${number(pixel.value)}`;
  return <div data-science-pixel className="wm-pixel-value" role="status"><span className="wm-swatch" style={{background:pixel.isNoData ? 'transparent' : pixel.color}}/>
    <div><strong>{label}</strong><p>{t(MODIS_SCIENCE[info.band].label)} · DN {number(pixel.value)}</p>
      {!pixel.isNoData && !value.withinRange && <p>{t('Outside product valid range')}</p>}
      <p>{t('Column {column}, row {row}',{column:number(pixel.pixel[0]),row:number(pixel.pixel[1])})} · {t('zero-based')}</p>
      {value.flags && <QualityPixelDetails pixel={{quality:value.flags}}/>}
    </div>
  </div>;
}
