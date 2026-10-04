import React from 'react';
import { Layers } from 'lucide-react';
import { SegmentedControl, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { compositePeriodLabel } from './composite-period.js';
import { RADAR_KEYS } from './radar.js';
import { catalogPreviewKind } from './catalog-preview.js';
import { ELEVATION_PREVIEW_RANGE } from './elevation-preview.js';
import { VegetationPreviewControls, VegetationPreviewLegend } from './vegetation-preview-ui.jsx';

export function CatalogPreviewControls({scene,kind,value,onValueChange}) {
  const { t } = useI18n();
  if (kind === 'vegetation') return <VegetationPreviewControls value={value} onValueChange={onValueChange}/>;
  const items = kind === 'radar'
    ? RADAR_KEYS.map(channel => ({value:channel,label:channel.toUpperCase(),disabled:!scene?.assets?.[channel],title:t('Radar polarization · {channel}',{channel:channel.toUpperCase()})}))
    : [{value:kind === 'elevation' ? 'height' : 'rgb',label:t(kind === 'elevation' ? 'Height' : 'RGB preview'),icon:Layers}];
  return <SegmentedControl className="catalog-preview-controls" aria-label={t(kind === 'radar' ? 'Radar polarization' : 'Preview')}
    value={value} onValueChange={onValueChange} items={items}/>;
}

export function CatalogPreviewLegend({scene,channel}) {
  const { t,date,number } = useI18n();
  const kind = catalogPreviewKind(scene.provider);
  if (kind === 'vegetation') return <VegetationPreviewLegend scene={scene} index={channel}/>;
  const title = kind === 'radar' ? `${channel.toUpperCase()} · γ⁰` : t(kind === 'elevation' ? 'Surface height' : 'Surface reflectance · RGB');
  const ticks = kind === 'radar' ? [-30,-15,0].map(value=>`${number(value)} dB`) : kind === 'elevation'
    ? [ELEVATION_PREVIEW_RANGE[0],500,ELEVATION_PREVIEW_RANGE[1]].map(value=>`${number(value)} m`) : [0,0.15,0.3].map(value=>number(value,{maximumFractionDigits:2}));
  return <Surface className="vegetation-preview-legend catalog-preview-legend" role="region" aria-label={t('Map preview legend')}>
    <div className="vegetation-preview-caption"><strong>{title}</strong>{kind !== 'elevation' && <span>{compositePeriodLabel(scene,value=>date(value,{year:undefined,month:'2-digit',day:'2-digit'}))}</span>}</div>
    {kind !== 'reflectance' && <div className="vegetation-preview-ramp" aria-hidden="true" style={{background:'linear-gradient(90deg, #000, #fff)'}}/>}
    <div className="vegetation-preview-ticks">{ticks.map(value=><span key={value}>{value}</span>)}</div>
    <p>{t(kind === 'radar' ? 'Online preview · no speckle filtering' : kind === 'elevation' ? 'EGM2008 · display stretch only' : 'Gamma 2.2 · no quality mask applied')}</p>
  </Surface>;
}
