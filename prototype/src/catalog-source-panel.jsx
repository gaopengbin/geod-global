import React, { useId, useRef, useState } from 'react';
import { Database, KeyRound, Layers, Plus } from 'lucide-react';
import { Badge, Button, Popover, Select } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { PROVIDERS, canDisplayImagery } from './providers.js';
import { catalogPreviewKind } from './catalog-preview.js';
import { originalsReleased } from './release-policy.js';

// Compact presentation only. Release eligibility stays in the shared policy.
const presentation = {
  'earth-search': ['Sentinel-2 · Earth Search', '10 m true color · 20 m SCL · Earth Search'],
  'planetary-computer': ['Sentinel-2 · Planetary Computer', '10 m true color · 20 m SCL · Planetary Computer'],
  'planetary-landsat': ['Landsat 8 / 9', '30 m reflectance + quality layers · Planetary Computer'],
  'planetary-modis': ['MODIS · 8-day reflectance', '500 m · MOD09A1 / MYD09A1 · Planetary Computer'],
  'planetary-vegetation': ['MODIS NDVI / EVI', '250 m · 16-day indices + science layers · Planetary Computer'],
  'planetary-radar': ['Sentinel-1 RTC', '10 m radar · VV / VH / HH / HV · Planetary Computer'],
  'planetary-naip': ['NAIP aerial imagery', 'US aerial imagery · RGB + NIR · Planetary Computer'],
  'copernicus-dem': ['Copernicus DEM · GLO-30', '30 m surface elevation · Public AWS tiles'],
  'copernicus-dem-90': ['Copernicus DEM · GLO-90', '90 m surface elevation · Public AWS tiles'],
  'nasa-earthdata': ['NASA HLS · Landsat L30', '30 m reflectance · NASA Earthdata'],
  'nasa-srtm': ['NASA SRTM · SRTMGL1', '30 m surface elevation · NASA Earthdata'],
  'nasa-viirs-noaa21': ['VIIRS · NOAA-21', '1 km · 8-day reflectance · NASA Earthdata'],
  'nasa-viirs-noaa20': ['VIIRS · NOAA-20', '1 km · 8-day reflectance · NASA Earthdata'],
  'nasa-viirs-npp': ['VIIRS · Suomi-NPP', '1 km · 8-day reflectance · NASA Earthdata'],
  copernicus: ['Sentinel-2 SAFE · Copernicus', 'Sentinel-2 L2A original product · Copernicus Data Space'],
};
const available = PROVIDERS.filter(originalsReleased);
const deferred = PROVIDERS.filter(provider => !originalsReleased(provider));

export function CatalogSourcePanel({ provider, onChange, onOpenStac, onOpenWcs }) {
  const { t } = useI18n();
  const id = useId(), addTrigger = useRef(null), pendingService = useRef(null);
  const [servicesOpen, setServicesOpen] = useState(false);
  const released = originalsReleased(provider);
  const previewLabel = source => catalogPreviewKind(source) ? 'Online preview' : canDisplayImagery(source) ? 'Load to preview' : 'Footprints only';
  const [name, detail] = presentation[provider.id] || [provider.name, provider.dataset];
  const openService = callback => {
    pendingService.current = callback;
    setServicesOpen(false);
  };
  const afterServiceClose = event => {
    if (!pendingService.current) return;
    event.preventDefault();
    const callback = pendingService.current;
    pendingService.current = null;
    // Wait for the closing animation and return focus before the next dialog
    // captures it, so closing that dialog returns to the persistent add button.
    addTrigger.current?.focus();
    callback();
  };
  return <section className="catalog-source-panel" aria-label={t('Data source')} data-downloads={released ? 'available' : 'deferred'} data-preview={catalogPreviewKind(provider) ? 'automatic' : canDisplayImagery(provider) ? 'manual' : 'footprints'}>
    <div className="catalog-source-heading">
      <label htmlFor={id}>{t('Data source')}</label>
      <Badge tone={released ? 'accent' : 'neutral'}>{t(released ? 'Downloadable' : 'Catalog only')}</Badge>
      <Popover open={servicesOpen} onOpenChange={setServicesOpen} onCloseAutoFocus={afterServiceClose} className="catalog-connections-popover" aria-label={t('Add data source')}
        trigger={<Button ref={addTrigger} variant="quiet" size="icon" tooltip={t('Add data source')} aria-label={t('Add data source')}><Plus size={17} aria-hidden="true"/></Button>}>
        <p className="catalog-connections-title">{t('Add data source')}</p>
        <Button variant="quiet" size="row" className="catalog-connection-option" onClick={() => openService(onOpenStac)}><Database size={18} aria-hidden="true"/><span><strong>{t('Raster catalog (STAC)')}</strong><small>{t('Connect a custom raster catalog')}</small></span></Button>
        <Button variant="quiet" size="row" className="catalog-connection-option" onClick={() => openService(onOpenWcs)}><Layers size={18} aria-hidden="true"/><span><strong>{t('Coverage service (WCS)')}</strong><small>{t('Request a raster subset from a coverage service')}</small></span></Button>
      </Popover>
    </div>
    <Select id={id} aria-label={t('Data source')} aria-describedby={`${id}-detail ${id}-availability`} value={provider.id} displayValue={t(name)} contentClassName="catalog-provider-menu" onChange={event => onChange(event.target.value)}>
      <optgroup label={t('Downloads available · no account needed · {count}', { count: available.length })}>{available.map(source => <option key={source.id} value={source.id} data-description={t(previewLabel(source))}>{source.name}</option>)}</optgroup>
      <optgroup label={t('Catalog only · original downloads pending verification · {count}', { count: deferred.length })}>{deferred.map(source => <option key={source.id} value={source.id} data-description={t(previewLabel(source))}>{source.name}</option>)}</optgroup>
    </Select>
    <p className="catalog-source-detail" id={`${id}-detail`}><span className="catalog-source-preview-hint">{t(previewLabel(provider))}</span> · {t(detail)}</p>
    {released ? <span className="sr-only" id={`${id}-availability`}>{t('Original downloads available without an account.')}</span>
      : <div className="catalog-source-availability" id={`${id}-availability`}><p>{t('Catalog search is available. Original downloads await real-account verification.')}</p><Button asChild variant="quiet" size="icon" tooltip={t('Manage authorization')}><a href={`#Settings?account=${provider.account}`} aria-label={t('Manage authorization')}><KeyRound size={16} aria-hidden="true"/></a></Button></div>}
    {provider.id === 'nasa-viirs-npp' && <p className="catalog-source-detail">{t('New Suomi-NPP delivery stops Nov 1, 2026. Prefer NOAA-21 / NOAA-20.')} <a href="https://cmr.earthdata.nasa.gov/stac/LPCLOUD/collections/VNP09A1_002" target="_blank" rel="noreferrer">{t('Official source')}</a></p>}
  </section>;
}
