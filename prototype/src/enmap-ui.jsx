import React from 'react';
import {Select,Table,THead,TBody,TR,TH,TD} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';

export function SpectralBandSelect({data,disabled,onChange}) {
  const {t}=useI18n();if(!data?.spectralEncoding)return null;
  return <label className="stac-field"><span>{t('Display spectral band')}</span><Select aria-label={t('Display spectral band')} value={String(data.displayBand)} disabled={disabled} onChange={event=>onChange(Number(event.target.value))}>
    {data.spectralEncoding.wavelengths.map((w,i)=><option key={i} value={i+1}>{t('Band')} {i+1} · {w} nm</option>)}
  </Select></label>;
}
export function SpectralPixelTable({pixel,encoding}) {
  const {t}=useI18n();
  return <><p className="stac-help">{t('All 224 original samples. Reflectance uses header gain 0.0001; NoData is excluded before scaling. Quality masks remain separate.')}</p><div className="stac-table-wrap" role="region" aria-label={t('Original spectral samples')} tabIndex={0}><Table className="stac-table enmap-spectral-table"><THead><TR><TH>{t('Band')}</TH><TH>nm<span className="enmap-spectral-secondary" title={t('FWHM')}>FWHM · nm</span></TH><TH>{t('Raw sample')}</TH><TH>{t('Reflectance')}</TH></TR></THead><TBody>
    {pixel.values.map((value,i)=><TR key={i}><TD>{i+1}</TD><TD>{encoding.wavelengths[i]}<span className="enmap-spectral-secondary">{encoding.fwhm[i]}</span></TD><TD>{value}</TD><TD>{pixel.noData[i]?t('NoData'):(value*encoding.gain+encoding.offset).toFixed(4)}</TD></TR>)}
  </TBody></Table></div></>;
}
