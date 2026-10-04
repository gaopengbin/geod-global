import React, { useState } from 'react';
import { Search, SquareDashed } from 'lucide-react';
import { Button, DatePicker, Input, Modal, Select } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { validateSearch } from './catalog.js';
import { providerById } from './providers.js';

export function CatalogFilters({ initialValues, onApply, onArea, onClose }) {
  const { t, number, locale } = useI18n();
  const [draft, setDraft] = useState(initialValues);
  const [error, setError] = useState('');
  const elevation = providerById(draft.provider).domain === 'elevation';
  const composite = providerById(draft.provider).domain === 'composite';
  const radar = providerById(draft.provider).domain === 'radar';
  const aerial = providerById(draft.provider).domain === 'aerial';
  const update = event => {
    const { name, value } = event.currentTarget;
    setDraft(current => ({ ...current, [name]: value }));
    setError('');
  };
  const submit = event => {
    event.preventDefault();
    const values = { ...draft, ...Object.fromEntries(new FormData(event.currentTarget)) };
    try { validateSearch({ ...values, limit: 100 }); } catch (cause) { setError(cause.message); return; }
    onApply(values);
  };
  return <Modal id="scene-filters" className="catalog-filter-dialog" title={t('Filters')}
    closeLabel={t('Close dialog')} onClose={onClose}>
    <form className="catalog-form" onSubmit={submit}>
      <fieldset className="catalog-filter-section">
        <legend>{t('Search area')}</legend>
        <label>{t('WGS 84 bounds · west, south, east, north')}
          <Input name="bbox" aria-label={t('Search bounding box')} value={draft.bbox} onChange={update} />
        </label>
        <Button type="button" icon={SquareDashed} onClick={() => onArea(draft)}>{t('Draw area on map')}</Button>
      </fieldset>
      {!elevation && <fieldset className="catalog-filter-section">
        <legend>{t('Date range')}</legend>
        <div className="catalog-dates">
          <label>{t('From (UTC)')}<DatePicker name="start" aria-label={t('Search start date')} value={draft.start} onChange={update} locale={locale} /></label>
          <label>{t('Through (UTC)')}<DatePicker name="end" aria-label={t('Search end date')} value={draft.end} onChange={update} locale={locale} /></label>
        </div>
      </fieldset>}
      {!elevation && !aerial && !composite && !radar && <fieldset className="catalog-filter-section">
        <legend>{t('Cloud cover range')}</legend>
        <div className="catalog-cloud-range">
          <label className="range-label"><span>{t('Scene cloud cover ≥ {percent}', { percent: number(Number(draft.cloudMin) / 100, { style: 'percent' }) })}</span><Input name="cloudMin" type="range" aria-label={t('Minimum cloud cover')} min="0" max="100" value={draft.cloudMin} onChange={update} /></label>
          <label className="range-label"><span>{t('Scene cloud cover ≤ {percent}', { percent: number(Number(draft.cloud) / 100, { style: 'percent' }) })}</span><Input name="cloud" type="range" aria-label={t('Live maximum cloud cover')} min="0" max="100" value={draft.cloud} onChange={update} /></label>
        </div>
      </fieldset>}
      {radar && <fieldset className="catalog-filter-section"><legend>{t('Radar acquisition')}</legend><div className="catalog-dates"><label>{t('Orbit direction')}<Select name="orbit" value={draft.orbit || 'all'} onChange={update}><option value="all">{t('All orbits')}</option><option value="ascending">{t('Ascending')}</option><option value="descending">{t('Descending')}</option></Select></label><label>{t('Polarization')}<Select name="polarization" value={draft.polarization || 'all'} onChange={update}>{['all','vv','vh','hh','hv'].map(key => <option key={key} value={key}>{key === 'all' ? t('All polarizations') : key.toUpperCase()}</option>)}</Select></label></div></fieldset>}
      {error && <p className="catalog-error" role="alert">{t(error)}</p>}
      <div className="catalog-filter-footer">
        <Button onClick={onClose}>{t('Cancel')}</Button>
        <Button primary icon={Search} type="submit">{t('Search catalog')}</Button>
      </div>
    </form>
  </Modal>;
}
