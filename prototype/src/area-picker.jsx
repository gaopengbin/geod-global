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
import { administrativePlace, searchAdministrativePlaces } from './admin-areas.js';
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
    countries: new Style({ stroke: new Stroke({ color: color('--aoi-country'), width: 1.2 }), fill: new Fill({ color: 'rgba(0,0,0,0)' }) }),
    provinces: new Style({ stroke: new Stroke({ color: color('--aoi-province'), width: 0.8 }), fill: new Fill({ color: 'rgba(0,0,0,0)' }) }),
    focused: new Style({ stroke: new Stroke({ color: accent, width: 2.5 }), fill: new Fill({ color: [...asArray(accent).slice(0, 3), 0.1] }) }),
    selection: [new Style({ stroke: new Stroke({ color: color('--ink'), width: 5 }) }),
      new Style({ stroke: new Stroke({ color: accent, width: 2.5 }), fill: new Fill({ color: [...asArray(accent).slice(0, 3), 0.16] }) })],
  };
}

export function AreaPicker({ initialBbox, sample, onApply, onExport, onClose }) {
  const { t, number, locale } = useI18n();
  const [fields, setFields] = useState(() => initialBbox.map(String));
  const [drawing, setDrawing] = useState(false);
  const [mapState, setMapState] = useState('loading');
  const [drawError, setDrawError] = useState(false);
  const [countries, setCountries] = useState([]);
  const [provinces, setProvinces] = useState([]);
  const [countryState, setCountryState] = useState('loading');
  const [provinceState, setProvinceState] = useState('loading');
  const [placeQuery, setPlaceQuery] = useState('');
  const [focusedPlace, setFocusedPlace] = useState(null);
  const target = useRef(null);
  const map = useRef(null);
  const box = useRef(null);
  const drawingMode = useRef(false);
  const selection = useRef(null);
  const focused = useRef(null);
  const focusPlaceRef = useRef(null);
  const mapStyles = useRef(null);
  const parsed = useMemo(() => {
    try { return { bounds: validateBounds(fields), error: '' }; }
    catch (error) { return { bounds: null, error: error.message }; }
  }, [fields]);
  const places = useMemo(() => [...countries, ...provinces], [countries, provinces]);
  const placeResults = useMemo(() => searchAdministrativePlaces(places, placeQuery), [places, placeQuery]);
  const countryNames = useMemo(() => new Map(countries.map(place => [place.code, locale === 'zh-CN' && place.nameZh ? place.nameZh : place.nameEn])), [countries, locale]);
  const focusedBoundsApplied = focusedPlace && parsed.bounds?.every((value, index) => value === roundCoordinate(focusedPlace.bounds[index]));
  const nameOf = place => locale === 'zh-CN' && place.nameZh ? place.nameZh : place.nameEn;

  const focusPlace = place => {
    if (!place) return;
    focused.current?.clear();
    focused.current?.addFeature(new Feature(place.feature.getGeometry().clone()));
    setFocusedPlace(place);
    setPlaceQuery('');
    map.current?.getView().fit(place.bounds, { padding: [45, 45, 45, 45], maxZoom: place.kind === 'country' ? 6 : 8, duration: 0 });
  };
  focusPlaceRef.current = focusPlace;

  useEffect(() => {
    if (!target.current) return;
    const land = new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false });
    const countrySource = new VectorSource({ url: './basemaps/natural-earth-50m-admin-0-countries.geojson', format: new GeoJSON(), wrapX: false });
    const provinceSource = new VectorSource({ url: './basemaps/natural-earth-50m-admin-1-states-provinces.geojson', format: new GeoJSON(), wrapX: false });
    const chosen = new VectorSource({ wrapX: false });
    const highlighted = new VectorSource({ wrapX: false });
    mapStyles.current = styles(target.current);
    const landLayer = new VectorLayer({ source: land, style: () => mapStyles.current.land });
    const countryLayer = new VectorLayer({ source: countrySource, style: () => mapStyles.current.countries, zIndex: 2 });
    const provinceLayer = new VectorLayer({ source: provinceSource, style: () => mapStyles.current.provinces, minZoom: 4, zIndex: 3 });
    const focusLayer = new VectorLayer({ source: highlighted, style: () => mapStyles.current.focused, zIndex: 8 });
    const selectionLayer = new VectorLayer({ source: chosen, style: () => mapStyles.current.selection, zIndex: 10 });
    const instance = new OLMap({ target: target.current, controls: [], layers: [landLayer, countryLayer, provinceLayer, focusLayer, selectionLayer],
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
    countrySource.on('featuresloadend', () => { setCountries(countrySource.getFeatures().map(feature => administrativePlace(feature, 'country'))); setCountryState('ready'); });
    countrySource.on('featuresloaderror', () => setCountryState('error'));
    provinceSource.on('featuresloadend', () => { setProvinces(provinceSource.getFeatures().map(feature => administrativePlace(feature, 'province'))); setProvinceState('ready'); });
    provinceSource.on('featuresloaderror', () => setProvinceState('error'));
    instance.on('singleclick', event => {
      if (drawingMode.current) return;
      const picked = instance.forEachFeatureAtPixel(event.pixel, (feature, layer) => {
        if (layer === provinceLayer) return administrativePlace(feature, 'province');
        if (layer === countryLayer) return administrativePlace(feature, 'country');
        return undefined;
      }, { hitTolerance: 3 });
      if (picked) focusPlaceRef.current?.(picked);
    });
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
      land.changed(); countrySource.changed(); provinceSource.changed(); highlighted.changed(); chosen.changed();
    });
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    map.current = instance; box.current = dragBox; selection.current = chosen; focused.current = highlighted;
    return () => {
      resize.disconnect(); theme.disconnect();
      instance.setTarget(undefined); instance.dispose();
      map.current = null; box.current = null; selection.current = null; focused.current = null; mapStyles.current = null;
    };
  }, []);

  useEffect(() => { drawingMode.current = drawing; box.current?.setActive(drawing); }, [drawing]);
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
  const useFocusedBounds = () => {
    if (!focusedPlace) return;
    try { setFields(validateBounds(focusedPlace.bounds.map(roundCoordinate)).map(String)); setDrawError(false); }
    catch { setDrawError(true); }
  };

  return <div className="aoi-picker">
    <div className="aoi-intro"><Badge tone="blue">{t(sample ? 'SAMPLE AREA' : 'LIVE SEARCH AREA')}</Badge><p>{t('Find a country or province, then draw a WGS 84 rectangle or use its bounding box. Earth Search receives the rectangle, not the administrative polygon.')}</p></div>
    <div className="aoi-layout">
      <div className="aoi-map-section">
        <div className="aoi-place-find">
          <label>{t('Find country or province')}<Input value={placeQuery} onChange={event => setPlaceQuery(event.target.value)} placeholder={t('Search by English or local name')} /></label>
          {placeQuery && <div className="aoi-place-results">{placeResults.map(place => <Button key={`${place.kind}-${place.code}`} size="row" onClick={() => focusPlace(place)}><span><strong>{nameOf(place)}</strong><small>{place.kind === 'province' ? countryNames.get(place.parentCode) || place.parentCode : t('Country')}</small></span></Button>)}{!placeResults.length && <p role="status">{countryState === 'loading' || provinceState === 'loading' ? t('Loading administrative areas…') : t('No matching administrative area.')}</p>}</div>}
          <p>{t('Countries are worldwide. Province borders cover nine large countries in this 1:50m dataset. Boundaries are cartographic references, not legal definitions.')}</p>
        </div>
        <div className="aoi-map-toolbar"><Button selected={drawing} onClick={() => setDrawing(value => !value)}><SquareDashed size={16}/>{t(drawing ? 'Cancel drawing' : 'Draw rectangle')}</Button><div><Button size="icon" aria-label={t('Zoom in')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() + 1)}><Plus size={16}/></Button><Button size="icon" aria-label={t('Zoom out')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() - 1)}><Minus size={16}/></Button><Button size="icon" aria-label={t('Fit selected area')} disabled={!parsed.bounds} onClick={() => fit(parsed.bounds)}><Maximize size={16}/></Button><Button size="icon" aria-label={t('World view')} onClick={() => fit([-180, -90, 180, 90])}><Globe2 size={16}/></Button></div></div>
        <div className="aoi-map-wrap"><div ref={target} className={'aoi-map' + (drawing ? ' is-drawing' : '')} tabIndex={0} role="application" aria-label={t('WGS 84 reference map. Pan and zoom, then draw a rectangle or enter exact bounds.')} />{drawing && <p className="aoi-draw-hint" role="status">{t('Drag from one corner to the opposite corner to select an area.')}</p>}</div>
        <div className="aoi-map-caption"><span>{t('Natural Earth · 1:50m administrative reference')}</span>{mapState === 'loading' && <span role="status"><Spinner size={14}/>{t('Loading reference map…')}</span>}{mapState === 'error' && <span role="alert">{t('Reference map unavailable; enter coordinates instead.')}</span>}{countryState === 'error' && <span role="alert">{t('Country boundaries unavailable; draw or enter coordinates instead.')}</span>}{provinceState === 'error' && <span role="alert">{t('Province boundaries unavailable; country search still works.')}</span>}{drawError && <span role="alert">{t('Draw a larger rectangle within WGS 84 limits.')}</span>}</div>
      </div>
      <div className="aoi-form">
        {focusedPlace && <div className="aoi-place-focus"><Badge tone="blue">{t(focusedPlace.kind === 'country' ? 'Country' : 'Province / state')}</Badge><strong>{nameOf(focusedPlace)}</strong><p>{t(focusedBoundsApplied ? 'The region bounding rectangle is selected for search.' : 'The region is highlighted. Your search rectangle is unchanged until you choose its bounds or draw a new one.')}</p><Button size="sm" onClick={useFocusedBounds}>{t('Use region bounding rectangle')}</Button></div>}
        <h3>{t('Selected bounds')}</h3>
        <p>{t('Longitude and latitude in degrees. West must be less than east; south must be less than north.')}</p>
        <div className="aoi-fields">{directions.map((direction, index) => <label key={direction}>{t(direction)}<Input type="number" step="any" min={index % 2 === 0 ? -180 : -90} max={index % 2 === 0 ? 180 : 90} value={fields[index]} onChange={event => changeField(index, event.target.value)}/></label>)}</div>
        {parsed.error && <p className="aoi-error" role="alert">{t(parsed.error)}</p>}
        {parsed.bounds && <output className="aoi-summary mono">[{parsed.bounds.map(value => number(value, { maximumFractionDigits: 5 })).join(', ')}]</output>}
        <Button size="sm" onClick={() => { setFields(SAMPLE_BBOX.map(String)); fit(SAMPLE_BBOX); }}>{t('Use San Francisco sample bounds')}</Button>
      </div>
    </div>
    <div className="aoi-actions"><Button onClick={onClose}>{t('Cancel')}</Button><Button icon={Download} disabled={!parsed.bounds} onClick={exportArea}>{t('Download area GeoJSON')}</Button><Button primary icon={Search} disabled={!parsed.bounds} onClick={() => onApply(parsed.bounds)}>{t('Search this area')}</Button></div>
  </div>;
}
