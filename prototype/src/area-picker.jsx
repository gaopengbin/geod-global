import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Download, Globe2, Maximize, Minus, Plus, Search, SquareDashed } from 'lucide-react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import VectorLayer from 'ol/layer/Vector.js';
import VectorSource from 'ol/source/Vector.js';
import GeoJSON from 'ol/format/GeoJSON.js';
import DragBox from 'ol/interaction/DragBox.js';
import Feature from 'ol/Feature.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import { Fill, Stroke, Style } from 'ol/style.js';
import { asArray } from 'ol/color.js';
import { SAMPLE_BBOX, validateBounds } from './catalog.js';
import { useI18n } from './i18n.jsx';
import { Badge, Button, Input, Spinner } from './ui/index.jsx';
import 'ol/ol.css';
import './area-picker.css';

const directions = ['West', 'South', 'East', 'North'];
const roundCoordinate = value => Math.round(value * 100000) / 100000;

function styles(target) {
  const theme = getComputedStyle(target);
  const color = name => theme.getPropertyValue(name).trim();
  const accent = color('--accent');
  return {
    land: new Style({ fill: new Fill({ color: color('--aoi-land') }), stroke: new Stroke({ color: color('--aoi-shore'), width: 0.8 }) }),
    selection: [new Style({ stroke: new Stroke({ color: color('--ink'), width: 5 }) }),
      new Style({ stroke: new Stroke({ color: accent, width: 2.5 }), fill: new Fill({ color: [...asArray(accent).slice(0, 3), 0.16] }) })],
  };
}

export function AreaPicker({ initialBbox, sample, onApply, onExport, onClose }) {
  const { t, number } = useI18n();
  const [fields, setFields] = useState(() => initialBbox.map(String));
  const [drawing, setDrawing] = useState(false);
  const [mapState, setMapState] = useState('loading');
  const [drawError, setDrawError] = useState(false);
  const target = useRef(null);
  const map = useRef(null);
  const box = useRef(null);
  const selection = useRef(null);
  const mapStyles = useRef(null);
  const parsed = useMemo(() => {
    try { return { bounds: validateBounds(fields), error: '' }; }
    catch (error) { return { bounds: null, error: error.message }; }
  }, [fields]);

  useEffect(() => {
    if (!target.current) return;
    const land = new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false });
    const chosen = new VectorSource({ wrapX: false });
    mapStyles.current = styles(target.current);
    const landLayer = new VectorLayer({ source: land, style: () => mapStyles.current.land });
    const selectionLayer = new VectorLayer({ source: chosen, style: () => mapStyles.current.selection, zIndex: 10 });
    const instance = new OLMap({ target: target.current, controls: [], layers: [landLayer, selectionLayer],
      view: new View({ projection: 'EPSG:4326', center: [-122.435, 37.76], zoom: 5, extent: [-180, -90, 180, 90], enableRotation: false, multiWorld: false }) });
    const dragBox = new DragBox({ className: 'aoi-drag-box', condition: () => true, minArea: 16 });
    dragBox.setActive(false);
    instance.addInteraction(dragBox);
    dragBox.on('boxend', () => {
      const extent = dragBox.getGeometry().getExtent().map(roundCoordinate);
      try { setFields(validateBounds(extent).map(String)); setDrawing(false); setDrawError(false); }
      catch { setDrawError(true); }
    });
    land.on('featuresloadend', () => setMapState('ready'));
    land.on('featuresloaderror', () => setMapState('error'));
    let fitted = false;
    const resize = new ResizeObserver(() => {
      instance.updateSize();
      if (!fitted && instance.getSize()?.every(size => size > 0)) {
        instance.getView().fit(initialBbox, { padding: [56, 56, 56, 56], maxZoom: 8, duration: 0 });
        fitted = true;
      }
    });
    resize.observe(target.current);
    const theme = new MutationObserver(() => {
      mapStyles.current = styles(target.current);
      land.changed(); chosen.changed();
    });
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    map.current = instance; box.current = dragBox; selection.current = chosen;
    return () => {
      resize.disconnect(); theme.disconnect();
      instance.setTarget(undefined); instance.dispose();
      map.current = null; box.current = null; selection.current = null; mapStyles.current = null;
    };
  }, []);

  useEffect(() => { box.current?.setActive(drawing); }, [drawing]);
  useEffect(() => {
    if (!selection.current) return;
    selection.current.clear();
    if (parsed.bounds) selection.current.addFeature(new Feature(fromExtent(parsed.bounds)));
  }, [parsed.bounds]);

  const fit = bounds => {
    if (!map.current || !bounds) return;
    map.current.updateSize();
    map.current.getView().fit(bounds, { padding: [56, 56, 56, 56], maxZoom: 8, duration: 0 });
  };
  const changeField = (index, value) => { setDrawError(false); setFields(current => current.map((item, i) => i === index ? value : item)); };
  const exportArea = () => onExport(parsed.bounds);

  return <div className="aoi-picker">
    <div className="aoi-intro"><Badge tone="blue">{t(sample ? 'SAMPLE AREA' : 'LIVE SEARCH AREA')}</Badge><p>{t('Choose a WGS 84 rectangle on the reference map, or enter exact coordinates. The selected bounds will be sent to Earth Search; scene thumbnails are not selection maps.')}</p></div>
    <div className="aoi-layout">
      <div className="aoi-map-section">
        <div className="aoi-map-toolbar"><Button selected={drawing} onClick={() => setDrawing(value => !value)}><SquareDashed size={16}/>{t(drawing ? 'Cancel drawing' : 'Draw rectangle')}</Button><div><Button size="icon" aria-label={t('Zoom in')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() + 1)}><Plus size={16}/></Button><Button size="icon" aria-label={t('Zoom out')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() - 1)}><Minus size={16}/></Button><Button size="icon" aria-label={t('Fit selected area')} disabled={!parsed.bounds} onClick={() => fit(parsed.bounds)}><Maximize size={16}/></Button><Button size="icon" aria-label={t('World view')} onClick={() => fit([-180, -90, 180, 90])}><Globe2 size={16}/></Button></div></div>
        <div className="aoi-map-wrap"><div ref={target} className={'aoi-map' + (drawing ? ' is-drawing' : '')} tabIndex={0} role="application" aria-label={t('WGS 84 reference map. Pan and zoom, then draw a rectangle or enter exact bounds.')} />{drawing && <p className="aoi-draw-hint" role="status">{t('Drag from one corner to the opposite corner to select an area.')}</p>}</div>
        <div className="aoi-map-caption"><span>{t('Natural Earth · 1:50m land · offline reference map')}</span>{mapState === 'loading' && <span role="status"><Spinner size={14}/>{t('Loading reference map…')}</span>}{mapState === 'error' && <span role="alert">{t('Reference map unavailable; enter coordinates instead.')}</span>}{drawError && <span role="alert">{t('Draw a larger rectangle within WGS 84 limits.')}</span>}</div>
      </div>
      <div className="aoi-form"><h3>{t('Selected bounds')}</h3><p>{t('Longitude and latitude in degrees. West must be less than east; south must be less than north.')}</p><div className="aoi-fields">{directions.map((direction, index) => <label key={direction}>{t(direction)}<Input type="number" step="any" min={index % 2 === 0 ? -180 : -90} max={index % 2 === 0 ? 180 : 90} value={fields[index]} onChange={event => changeField(index, event.target.value)}/></label>)}</div>{parsed.error && <p className="aoi-error" role="alert">{t(parsed.error)}</p>}{parsed.bounds && <output className="aoi-summary mono">[{parsed.bounds.map(value => number(value, { maximumFractionDigits: 5 })).join(', ')}]</output>}<Button size="sm" onClick={() => { setFields(SAMPLE_BBOX.map(String)); fit(SAMPLE_BBOX); }}>{t('Use San Francisco sample bounds')}</Button></div>
    </div>
    <div className="aoi-actions"><Button onClick={onClose}>{t('Cancel')}</Button><Button icon={Download} disabled={!parsed.bounds} onClick={exportArea}>{t('Download area GeoJSON')}</Button><Button primary icon={Search} disabled={!parsed.bounds} onClick={() => onApply(parsed.bounds)}>{t('Search this area')}</Button></div>
  </div>;
}
