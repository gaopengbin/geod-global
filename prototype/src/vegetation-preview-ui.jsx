import React from 'react';
import { SegmentedControl, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { compositePeriodLabel } from './composite-period.js';
import { VEGETATION_PREVIEW_COLORS } from './vegetation-preview.js';

export function VegetationPreviewControls({ value, onValueChange }) {
  const { t } = useI18n();
  return <SegmentedControl className="vegetation-preview-controls" aria-label={t('Vegetation index')} value={value}
    onValueChange={onValueChange} items={[
      { value: 'ndvi', label: 'NDVI', title: t('Normalized difference vegetation index') },
      { value: 'evi', label: 'EVI', title: t('Enhanced vegetation index') },
    ]} />;
}

export function VegetationPreviewLegend({ scene, index }) {
  const { t, date, number } = useI18n();
  return <Surface className="vegetation-preview-legend" role="region" aria-label={t('Index preview legend')}>
    <div className="vegetation-preview-caption"><strong>{index.toUpperCase()}</strong><span>{compositePeriodLabel(scene, value => date(value, { year: undefined, month: '2-digit', day: '2-digit' }))}</span></div>
    <div className="vegetation-preview-ramp" aria-hidden="true" style={{ background: `linear-gradient(90deg, ${VEGETATION_PREVIEW_COLORS.join(', ')})` }} />
    <div className="vegetation-preview-ticks">{[-0.2, 0.4, 1].map(value => <span key={value}>{number(value, { minimumFractionDigits: 1, maximumFractionDigits: 1 })}</span>)}</div>
    <p>{t('Online preview · no quality mask applied')}</p>
  </Surface>;
}
