import React, { useId, useRef, useState } from 'react';
import { ChevronDown, Database, Info, KeyRound, Layers, Plus } from 'lucide-react';
import { Badge, Button, Popover, Select } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { canDisplayImagery } from './providers.js';
import { catalogPreviewKind } from './catalog-preview.js';
import { originalsReleased } from './release-policy.js';
import { SOURCE_DIRECTORY } from './source-directory.js';
import { channelLabel, productForSource } from './product-catalog.js';

// Compact presentation only. Release eligibility stays in the shared policy.
export const sourcePresentation = {
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
  'srtm-num': ['SRTM NUM · Source codes', '1 arc-second Point grid · raw UInt8 source codes'],
  'nasa-viirs-noaa21': ['VIIRS · NOAA-21', '1 km · 8-day reflectance · NASA Earthdata'],
  'nasa-viirs-noaa20': ['VIIRS · NOAA-20', '1 km · 8-day reflectance · NASA Earthdata'],
  'nasa-viirs-npp': ['VIIRS · Suomi-NPP', '1 km · 8-day reflectance · NASA Earthdata'],
  copernicus: ['Sentinel-2 SAFE · Copernicus', 'Sentinel-2 L2A original product · Copernicus Data Space'],
};
const publicProducts = SOURCE_DIRECTORY.filter(source => source.status === 'ready' && source.action?.kind === 'dataset');

export function CatalogSourcePanel({ provider, datasetId, dataset, entry, disabled = false, onOpenPicker, onChooseChannel, pickerOpen = false, pickerTriggerRef, onOpenStac, onOpenWcs, onOpenEarthEngine }) {
  const { t } = useI18n();
  const id = useId(), addTrigger = useRef(null), pendingService = useRef(null);
  const [servicesOpen, setServicesOpen] = useState(false);
  const productSelected = Boolean(datasetId);
  const directoryProduct=!entry&&!productSelected?productForSource(provider.id):null;
  const productSource = publicProducts.find(source => source.action.datasetId === datasetId);
  const product = dataset?.id === datasetId ? dataset : null;
  const released = entry ? false : productSelected ? Boolean(productSource) : originalsReleased(provider);
  const previewLabel = source => catalogPreviewKind(source) ? 'Online preview' : canDisplayImagery(source) ? 'Load to preview' : 'Footprints only';
  const [name, detail] = entry ? [entry.name,entry.detail] : productSelected
    ? [product?.title || productSource?.name || 'Choose a data product', product?.nominalResolution || productSource?.detail || 'This product connector is not available in the running app.']
    : sourcePresentation[provider.id] || [provider.name, provider.dataset];
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
  return <section className="catalog-source-panel" aria-label={t('Data source')} data-downloads={entry ? 'service-specific' : released ? 'available' : 'deferred'} data-preview={entry ? 'service-specific' : productSelected ? 'manual-file' : catalogPreviewKind(provider) ? 'automatic' : canDisplayImagery(provider) ? 'manual' : 'footprints'}>
    <div className="catalog-source-heading">
      <label htmlFor={id}>{t('Data product')}</label>
      <Badge tone={released ? 'accent' : 'neutral'}>{t(entry ? 'Service source' : productSelected ? 'Published data product' : released ? 'Downloadable' : 'Catalog only')}</Badge>
      <Popover className="catalog-description-popover" aria-label={t('Source details')} trigger={<Button variant="quiet" size="icon" tooltip={t('Source details')} aria-label={t('Source details')}><Info size={15} aria-hidden="true"/></Button>}><p className="catalog-source-detail" id={`${id}-detail`}><span className="catalog-source-preview-hint">{t(entry ? 'Source layers and results' : productSelected ? 'Original files downloadable' : previewLabel(provider))}</span> · {t(detail)}</p></Popover>
      <Popover open={servicesOpen} onOpenChange={setServicesOpen} onCloseAutoFocus={afterServiceClose} className="catalog-connections-popover" aria-label={t('Add data source')}
        trigger={<Button ref={addTrigger} disabled={disabled} variant="quiet" size="icon" tooltip={t('Add data source')} aria-label={t('Add data source')}><Plus size={17} aria-hidden="true"/></Button>}>
        <p className="catalog-connections-title">{t('Add data source')}</p>
        {onOpenEarthEngine && <Button variant="quiet" size="row" className="catalog-connection-option" onClick={() => openService(onOpenEarthEngine)}><Layers size={18} aria-hidden="true"/><span><strong>{t('Google Earth Engine exports')}</strong><small>{t('Download GeoTIFF links from your own GEE project')}</small></span></Button>}
        <Button variant="quiet" size="row" className="catalog-connection-option" onClick={() => openService(onOpenStac)}><Database size={18} aria-hidden="true"/><span><strong>{t('Raster catalog (STAC)')}</strong><small>{t('Connect a custom raster catalog')}</small></span></Button>
        <Button variant="quiet" size="row" className="catalog-connection-option" onClick={() => openService(onOpenWcs)}><Layers size={18} aria-hidden="true"/><span><strong>{t('Coverage service (WCS)')}</strong><small>{t('Request a raster subset from a coverage service')}</small></span></Button>
      </Popover>
    </div>
    <Button ref={pickerTriggerRef} id={id} disabled={disabled} variant="secondary" className="catalog-source-trigger" aria-label={t('Choose a data source')} aria-haspopup="dialog" aria-expanded={pickerOpen} aria-describedby={`${id}-availability`} onClick={onOpenPicker}>
      <Database size={16} aria-hidden="true"/><span title={t(name)}>{t(directoryProduct?.channels.length>1?directoryProduct.name:name)}</span><ChevronDown size={16} aria-hidden="true"/>
    </Button>
    {directoryProduct?.channels.length>1&&onChooseChannel&&<label className="catalog-channel"><span>{t('Data channel')}</span><Select aria-label={t('Data channel')} disabled={disabled} value={provider.id} onChange={event=>onChooseChannel(event.target.value)}>{directoryProduct.channels.map(source=><option value={source.id} key={source.id}>{channelLabel(source)}</option>)}</Select></label>}
    {entry ? <span className="sr-only" id={`${id}-availability`}>{t('Preview and delivery depend on the selected source type.')}</span> : productSelected ? <span className="sr-only" id={`${id}-availability`}>{t('Published file downloads are available. Check the selected files before downloading.')}</span>
      : released ? <span className="sr-only" id={`${id}-availability`}>{t(provider.account ? 'Downloads require your own connected source account.' : 'Original downloads available without an account.')}</span>
      : <div className="catalog-source-availability" id={`${id}-availability`}><p>{t('Catalog search is available. Original downloads await real-account verification.')}</p><Button asChild variant="quiet" size="icon" tooltip={t('Manage authorization')}><a href={`#Settings?account=${provider.account}`} aria-label={t('Manage authorization')}><KeyRound size={16} aria-hidden="true"/></a></Button></div>}
    {!entry && !productSelected && provider.id === 'nasa-viirs-npp' && <p className="catalog-source-detail">{t('New Suomi-NPP delivery stops Nov 1, 2026. Prefer NOAA-21 / NOAA-20.')} <a href="https://cmr.earthdata.nasa.gov/stac/LPCLOUD/collections/VNP09A1_002" target="_blank" rel="noreferrer">{t('Official source')}</a></p>}
  </section>;
}
