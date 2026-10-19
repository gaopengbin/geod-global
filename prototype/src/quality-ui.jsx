import React from 'react';
import { InfoDetails } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { decodeQuality } from './quality.js';

export function QualityPixelDetails({ pixel }) {
  const { t, number } = useI18n();
  if (!pixel?.quality) return null;
  return <InfoDetails closeLabel={t('Close')} summary={t('Decode quality flags')}>
    <p className="mono runtime-wrap">{pixel.quality.hex} · {pixel.quality.binary}</p>
    {pixel.quality.fields.length > 0 && <dl className="runtime-details quality-fields">{pixel.quality.fields.map(field => <React.Fragment key={field.name}>
      <dt>{t(field.name)}</dt><dd className="quality-field-value"><span>{t(field.label)} · {number(field.value)}</span><small className="quality-bit-range">{t('bits {start}–{end}', {start:field.startBit,end:field.endBit})}</small></dd>
    </React.Fragment>)}</dl>}
  </InfoDetails>;
}
export function QualityLegend({ metadata, embedded = false }) {
  const { t, number } = useI18n();
  if (!metadata?.quality) return null;
  return <InfoDetails embedded={embedded} closeLabel={t('Close')} contentClassName="wm-legend" summary={t(metadata.quality.displayField)}>
    <ul>{metadata.classes.map(item => <li key={item.value}><i style={{background:item.color}}/><span>{t(item.label)} · {number(item.count)}</span></li>)}</ul>
    <p>{t('Full raster counts for this file; not a cloud-cover estimate.')} {t('NoData')} · {number(metadata.quality.sampleCount - metadata.quality.validSampleCount)}</p>
    {metadata.quality.flags && <>
      <p>{t(metadata.quality.flags.coverage)}</p>
      <p>{t('Display groups use flag priority; individual flag counts can overlap.')}</p>
      <section className="bui-info-section"><h4>{t('Full-resolution flag counts')}</h4>
        <p>{t(metadata.quality.flags.coverageMask ? 'Counts include every stored pixel, including uncovered cells and reserved codes.' : 'Counts include every original pixel, including fill and reserved codes.')}</p>
        <dl className="runtime-details quality-fields" data-quality-counts>{metadata.quality.flags.fields.map(field => <React.Fragment key={field.name}>
          <dt>{t(field.name)} <small>{t('bits {start}–{end}',{start:field.startBit,end:field.endBit})}</small></dt>
          <dd data-quality-field={field.name}>{field.counts.map((count,value) => count > 0 && <div key={value} data-quality-code={value}>{t(decodeQuality(metadata.quality.band,value*2**field.startBit).fields.find(item=>item.name===field.name).label)} · {number(value)}: {number(count)}</div>)}</dd>
        </React.Fragment>)}</dl>
      </section>
    </>}
    <a href={metadata.quality.definition} target="_blank" rel="noreferrer">{t(metadata.quality.product === 'landsat-c2-l2' ? 'USGS quality flag definitions' : 'NASA quality flag definitions')}</a>
  </InfoDetails>;
}
