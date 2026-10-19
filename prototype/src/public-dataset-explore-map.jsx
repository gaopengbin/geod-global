import React, { useCallback, useMemo, useRef, useState } from 'react';
import { ArrowUpToLine, Layers, MapPin, Maximize, Minus, PanelLeftOpen, Plus, RefreshCw, Search, Trash2 } from 'lucide-react';
import { InfoDetails, Button, Disclosure, EmptyState, Input, Spinner, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { validQueryBounds } from './features-client.js';
import { ExploreMap } from './explore-map.jsx';
import './public-dataset-explore-map.css';

const EMPTY = Object.freeze([]);
// View framing only: this is never an item footprint or a search request.
const OVERVIEW_BOUNDS = Object.freeze([-180, -80, 180, 80]);
const hasFootprint = item => validQueryBounds(item.bbox)
  || ['Polygon', 'MultiPolygon'].includes(item.geometry?.type) && Array.isArray(item.geometry.coordinates);

export function PublicDatasetExploreMap({ items = EMPTY, areaBounds, areaGeometry, selectedIds = EMPTY,
  focusedId, onFocusItem, onRequestArea, onShowList, listHidden = false, areaDisabled = false,
  rasterLayers = EMPTY, onRasterChange, onRemoveRaster, onClearRasters, onRaiseRaster }) {
  const { t } = useI18n();
  const map = useRef(null);
  const [rasterStatus, setRasterStatus] = useState({});
  const hasArea = validQueryBounds(areaBounds);
  // Keep publisher geometry and time declarations intact. Do not adapt these
  // scientific products into optical scenes or invent a date/coverage extent.
  const footprints = useMemo(() => items.map(item => ({ id: item.id,
    geometry: item.geometry ?? null, bbox: item.bbox ?? null,
    datetime: item.datetime ?? null, startDatetime: item.startDatetime ?? null,
    endDatetime: item.endDatetime ?? null })), [items]);
  const itemIds = useMemo(() => new Set(footprints.map(item => item.id)), [footprints]);
  const selected = useMemo(() => selectedIds.filter(id => itemIds.has(id)), [selectedIds, itemIds]);
  const focus = itemIds.has(focusedId) ? focusedId : undefined;
  const validRasters = useMemo(() => rasterLayers.filter(entry=>entry.id===JSON.stringify([entry.snapshotId,entry.assetKey])
    && items.some(item=>item.id===entry.snapshotId && item.assets?.some(asset=>asset.key===entry.assetKey && asset.eligible))),[rasterLayers,items]);
  const currentRasters = useRef(validRasters);
  currentRasters.current = validRasters;
  const reportRasterStatus = useCallback((id,status) => {
    const current = currentRasters.current.find(entry=>entry.id===id);
    if (!current || status.generation!==current.generation || status.retry!==current.retry) return;
    setRasterStatus(previous=>({...previous,[id]:status}));
  },[]);
  const statusFor = entry => {
    const status = rasterStatus[entry.id];
    return status?.generation===entry.generation && status.retry===entry.retry ? status : {status:'loading'};
  };
  const focusItem = ids => {
    const id = ids.find(candidate => itemIds.has(candidate));
    if (id != null) onFocusItem?.(id);
  };
  return <section className="public-dataset-map" aria-label={t('Product footprint map')} data-footprint-only={!validRasters.length?'true':'false'}>
    <ExploreMap ref={map} scenes={footprints} loadedScenes={EMPTY} previewScenes={EMPTY}
      selectedIds={selected} focusedIds={focus == null ? EMPTY : [focus]} activeSceneId={focus}
      area={hasArea ? areaBounds : OVERVIEW_BOUNDS} areaGeometry={hasArea ? areaGeometry : undefined}
      showArea={hasArea} onFootprintsPick={focusItem} productRasterLayers={validRasters} onProductRasterStatus={reportRasterStatus}/>
    <div className="public-dataset-map-actions">
      {listHidden && onShowList && <Button size="sm" icon={PanelLeftOpen} onClick={onShowList}>{t('Show product list')}</Button>}
      {onRequestArea && <Button size="sm" icon={MapPin} disabled={areaDisabled} onClick={onRequestArea}>{t('Choose a search area')}</Button>}
    </div>
    {!hasArea && !footprints.some(hasFootprint) && !validRasters.length && <div className="explore-map-empty"><EmptyState icon={Search} title={t('Choose your next observation')}>{t('Choose an area to search published products.')}</EmptyState></div>}
    {validRasters.length>0 && <Surface className="public-dataset-map-layers" aria-label={t('Product preview layers')}>
      <header><Layers size={16}/><strong>{t('Map previews · {count}',{count:validRasters.length})}</strong><Button variant="quiet" size="icon" icon={Trash2} aria-label={t('Clear map previews')} tooltip={t('Clear map previews')} onClick={onClearRasters}/></header>
      <div className="public-dataset-map-layer-list">{validRasters.slice().reverse().map(entry=>{
        const status=statusFor(entry);
        return <div className="public-dataset-map-layer" key={entry.id} data-status={status.status}>
          <label className="public-dataset-map-layer-title"><Input type="checkbox" checked={entry.visible!==false} aria-label={t('Show product preview {name}',{name:entry.title})} onChange={event=>onRasterChange?.(entry.id,{visible:event.target.checked})}/><span title={entry.title}>{entry.title}</span></label>
          <div className="public-dataset-map-layer-status"><span role="status">{status.status==='loading' && entry.visible!==false && <Spinner size={12}/>} {t(status.status==='error' ? 'Preview failed' : entry.visible===false ? 'Preview hidden' : status.status==='ready' ? 'Preview visible' : 'Loading preview…')}</span>
            <Button variant="quiet" size="icon" icon={ArrowUpToLine} aria-label={t('Bring product preview to front {name}',{name:entry.title})} tooltip={t('Bring to front')} onClick={()=>onRaiseRaster?.(entry.id)}/>
            <Button variant="quiet" size="icon" icon={Trash2} aria-label={t('Remove product preview {name}',{name:entry.title})} tooltip={t('Remove preview')} onClick={()=>onRemoveRaster?.(entry.id)}/>
          </div>
          {status.status==='error' && <div className="public-dataset-map-layer-error"><p role="alert">{t(status.error || 'This original cannot be previewed online. Download it to inspect locally.')}</p><Button size="xs" icon={RefreshCw} onClick={()=>onRasterChange?.(entry.id,{retry:true})}>{t('Retry preview')}</Button></div>}
          <label className="public-dataset-map-opacity"><span>{t('Opacity')}</span><Input type="range" min="0" max="100" step="10" value={Math.round((entry.opacity??1)*100)} aria-label={t('Product preview opacity {name}',{name:entry.title})} onChange={event=>onRasterChange?.(entry.id,{opacity:Number(event.target.value)/100})}/><span>{Math.round((entry.opacity??1)*100)}%</span></label>
        </div>;
      })}</div>
      <InfoDetails closeLabel={t('Close')} summary={t('Preview display details')}>
        <p>{t('First-band raw values are stretched to grayscale for display. This is not RGB, a physical-unit conversion, or a classification legend. Embedded TIFF NoData and masks are used; no additional quality mask is applied.')}</p>
        {validRasters.filter(entry=>entry.displayRange).map(entry=><p key={entry.id}>{t('Original-value grayscale range: {min}–{max}. Values outside it are clipped for display only.',{min:entry.displayRange[0],max:entry.displayRange[1]})}</p>)}
        {validRasters.some(entry=>entry.overviewPolicy==='srtm15plus-02-10-v255') && <p>{t('This reviewed SRTM15+ file has empty coarse overviews. The preview reads its verified finer level; the published file and original samples remain unchanged.')}</p>}
      </InfoDetails>
    </Surface>}
    <div className="public-dataset-map-controls" role="group" aria-label={t('Map controls')}>
      <Button variant="quiet" size="icon" icon={Plus} aria-label={t('Zoom in')} tooltip={t('Zoom in')} onClick={() => map.current?.zoomIn()}/>
      <Button variant="quiet" size="icon" icon={Minus} aria-label={t('Zoom out')} tooltip={t('Zoom out')} onClick={() => map.current?.zoomOut()}/>
      <Button variant="quiet" size="icon" icon={Maximize} aria-label={t('Fit published footprints')} tooltip={t('Fit published footprints')} disabled={!footprints.some(hasFootprint)} onClick={() => map.current?.fit()}/>
      <Button variant="quiet" size="icon" icon={MapPin} aria-label={t('Fit search area')} tooltip={t('Fit search area')} disabled={!hasArea} onClick={() => map.current?.fitArea()}/>
    </div>
    <footer className="public-dataset-map-attribution">
      <span>{t('Natural Earth overview · Published file boundaries')}</span>
      <span>{t('Catalog boundaries do not prove valid-pixel coverage.')}</span>
    </footer>
  </section>;
}

export default PublicDatasetExploreMap;
