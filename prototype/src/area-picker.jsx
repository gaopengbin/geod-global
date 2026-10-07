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
import { administrativePlace, administrativeSelection, indexedAdministrativePlace, searchAdministrativePlaces } from './admin-areas.js';
import { validateBounds } from './catalog.js';
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

export function AreaPicker({ initialBbox, onApply, onExport, onClose }) {
  const { t, number, locale } = useI18n();
  const [fields, setFields] = useState(() => initialBbox?.map(String) || ['', '', '', '']);
  const [drawing, setDrawing] = useState(false);
  const [mapState, setMapState] = useState('loading');
  const [drawError, setDrawError] = useState(false);
  const [countries, setCountries] = useState([]);
  const [provinces, setProvinces] = useState([]);
  const [countryState, setCountryState] = useState('loading');
  const [provinceState, setProvinceState] = useState('loading');
  const [activeCountry, setActiveCountry] = useState('');
  const [countryGeometryState, setCountryGeometryState] = useState('idle');
  const [countryGeometryError, setCountryGeometryError] = useState('');
  const [placeQuery, setPlaceQuery] = useState('');
  const [focusedPlace, setFocusedPlace] = useState(null);
  const [selectedPolygon, setSelectedPolygon] = useState(null);
  const target = useRef(null);
  const map = useRef(null);
  const box = useRef(null);
  const drawingMode = useRef(false);
  const selection = useRef(null);
  const focused = useRef(null);
  const provinceSourceRef = useRef(null);
  const pendingProvinceRef = useRef(null);
  const focusedProvinceRef = useRef(null);
  const provinceCodesRef = useRef(new Set());
  const focusPlaceRef = useRef(null);
  const mapStyles = useRef(null);
  const parsed = useMemo(() => {
    try { return { bounds: validateBounds(fields), error: '' }; }
    catch (error) { return { bounds: null, error: error.message }; }
  }, [fields]);
  const places = useMemo(() => [...countries, ...provinces], [countries, provinces]);
  const placeResults = useMemo(() => searchAdministrativePlaces(places, placeQuery), [places, placeQuery]);
  const countryNames = useMemo(() => new Map(countries.map(place => [place.code, locale === 'zh-CN' && place.nameZh ? place.nameZh : place.nameEn])), [countries, locale]);
  const focusedBoundsApplied = focusedPlace && parsed.bounds?.every((value, index) => Math.abs(value - focusedPlace.bounds[index]) <= 0.00001);
  const focusedPolygonApplied = Boolean(focusedPlace && selectedPolygon?.place.code === focusedPlace.code);
  const waitingForPolygon = Boolean(focusedPlace && !focusedPlace.feature && !focusedPlace.clipLimitation
    && pendingProvinceRef.current === focusedPlace.code && countryGeometryState !== 'error');
  const nameOf = place => locale === 'zh-CN' && place.nameZh ? place.nameZh : place.nameEn;
  const subtitleOf = place => {
    const parent = countryNames.get(place.displayParentCode || place.parentCode);
    return place.kind === 'province' && !place.canonicalRegionCode ? parent || place.parentCode : [parent, t(place.levelLabel)].filter(Boolean).join(' · ');
  };
  const limitationMessages = {
    polar: 'This region extends beyond local UTM clipping coverage. Its bounding rectangle remains available for catalogue search.',
    'date-line': 'This region crosses the date line. Draw separate rectangles on either side for catalogue search; local polygon clipping is unavailable.',
    complex: 'This boundary exceeds the local polygon vertex limit. Use its bounding rectangle for catalogue search; local polygon clipping is unavailable.',
  };

  const selectPlace = (place, polygon = true) => {
    if (!place) return;
    setSelectedPolygon(null);
    setDrawError(false);
    setDrawing(false);
    if (place.clipLimitation === 'date-line') { setFields(['', '', '', '']); return; }
    try {
      const bounds = validateBounds(place.bounds);
      setFields(bounds.map(String));
      if (polygon && place.feature && !place.clipLimitation) {
        const geometry = new GeoJSON().writeGeometryObject(place.feature.getGeometry(), { dataProjection: 'EPSG:4326', featureProjection: 'EPSG:4326' });
        setSelectedPolygon({ geometry, place: administrativeSelection(place, locale) });
      }
    } catch { setFields(['', '', '', '']); setDrawError(true); }
  };
  const focusPlace = (place, { select = true, clearQuery = true, fit = true } = {}) => {
    if (!place) return;
    if (place.kind === 'province' && !place.feature) {
      const loaded = provinceSourceRef.current?.getFeatures().find(feature => feature.get('adm1_code') === place.code);
      if (loaded) { focusPlace(administrativePlace(loaded, 'province', 'Natural Earth 1:10m'), { select, clearQuery, fit }); return; }
    }
    focused.current?.clear();
    if (place.feature) focused.current?.addFeature(new Feature(place.feature.getGeometry().clone()));
    setFocusedPlace(place);
    focusedProvinceRef.current = place.kind === 'province' ? place.code : null;
    if (clearQuery) setPlaceQuery('');
    if (select) selectPlace(place);
    if (fit) map.current?.getView().fit(place.bounds, { padding: [45, 45, 45, 45], maxZoom: place.kind === 'country' ? 6 : 8, duration: 0 });
    if (place.kind === 'country') {
      pendingProvinceRef.current = null;
      if (provinceCodesRef.current.has(place.code)) setActiveCountry(place.code);
    }
    if (place.kind === 'province' && !place.feature) {
      pendingProvinceRef.current = place.code;
      setActiveCountry(place.parentCode);
    }
    if (place.kind === 'province' && place.feature) pendingProvinceRef.current = null;
  };
  focusPlaceRef.current = focusPlace;

  useEffect(() => {
    const controller = new AbortController();
    fetch('./basemaps/admin1-10m/index.json', { signal: controller.signal }).then(response => {
      if (!response.ok) throw new Error(`Administrative index HTTP ${response.status}`);
      return response.json();
    }).then(data => {
      if (!Array.isArray(data.areas) || data.areas.length !== 4596) throw new Error('Administrative index is incomplete');
      const entries = data.areas.map(indexedAdministrativePlace);
      provinceCodesRef.current = new Set(entries.map(place => place.parentCode));
      setProvinces(entries); setProvinceState('ready');
    }).catch(error => { if (error.name !== 'AbortError') setProvinceState('error'); });
    return () => controller.abort();
  }, []);

  useEffect(() => {
    if (focusedPlace?.kind === 'country' && provinceCodesRef.current.has(focusedPlace.code)) setActiveCountry(focusedPlace.code);
  }, [focusedPlace, provinces]);

  useEffect(() => {
    if (!target.current) return;
    const land = new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false });
    const countrySource = new VectorSource({ url: './basemaps/natural-earth-50m-admin-0-countries.geojson', format: new GeoJSON(), wrapX: false });
    const provinceSource = new VectorSource({ wrapX: false });
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
      pendingProvinceRef.current = null;
      const extent = dragBox.getGeometry().getExtent().map(roundCoordinate);
      try { setFields(validateBounds(extent).map(String)); setSelectedPolygon(null); setDrawing(false); setDrawError(false); }
      catch { setDrawError(true); }
    });
    land.on('featuresloadend', () => setMapState('ready'));
    land.on('featuresloaderror', () => setMapState('error'));
    countrySource.on('featuresloadend', () => { setCountries(countrySource.getFeatures().map(feature => administrativePlace(feature, 'country'))); setCountryState('ready'); });
    countrySource.on('featuresloaderror', () => setCountryState('error'));
    instance.on('singleclick', event => {
      if (drawingMode.current) return;
      const picked = instance.forEachFeatureAtPixel(event.pixel, (feature, layer) => {
        if (layer === provinceLayer) return administrativePlace(feature, 'province', 'Natural Earth 1:10m');
        if (layer === countryLayer) return administrativePlace(feature, 'country');
        return undefined;
      }, { hitTolerance: 3 });
      if (picked) focusPlaceRef.current?.(picked);
    });
    let fitted = false;
    const resize = new ResizeObserver(() => {
      instance.updateSize();
      if (!fitted && instance.getSize()?.every(size => size > 0)) {
        instance.getView().fit(initialBbox || [-180, -90, 180, 90], { padding: [56, 56, 56, 56], maxZoom: 8, duration: 0 });
        fitted = true;
      }
    });
    resize.observe(target.current);
    const theme = new MutationObserver(() => {
      mapStyles.current = styles(target.current);
      land.changed(); countrySource.changed(); provinceSource.changed(); highlighted.changed(); chosen.changed();
    });
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    map.current = instance; box.current = dragBox; selection.current = chosen; focused.current = highlighted; provinceSourceRef.current = provinceSource;
    return () => {
      resize.disconnect(); theme.disconnect();
      instance.setTarget(undefined); instance.dispose();
      map.current = null; box.current = null; selection.current = null; focused.current = null; provinceSourceRef.current = null; mapStyles.current = null;
    };
  }, []);

  useEffect(() => {
    if (!activeCountry || !provinceSourceRef.current) return;
    const controller = new AbortController();
    setCountryGeometryState('loading'); setCountryGeometryError('');
    provinceSourceRef.current.clear();
    fetch(`./basemaps/admin1-10m/${activeCountry}.geojson`, { signal: controller.signal }).then(response => {
      if (!response.ok) throw new Error(`Administrative boundary HTTP ${response.status}`);
      return response.json();
    }).then(data => {
      if (controller.signal.aborted) return;
      if (data.type !== 'FeatureCollection' || !Array.isArray(data.features)) throw new Error('Invalid country boundary data');
      const features = new GeoJSON().readFeatures(data, { dataProjection: 'EPSG:4326', featureProjection: 'EPSG:4326' });
      const source = provinceSourceRef.current;
      if (!source) return;
      source.clear(); source.addFeatures(features);
      setCountryGeometryState('ready');
      const feature = features.find(item => item.get('adm1_code') === focusedProvinceRef.current);
      if (feature) {
        // Resolve the visible place, but do not replace a later manual edit or
        // drawing with geometry requested by the previous selection.
        const apply = pendingProvinceRef.current === focusedProvinceRef.current;
        focusPlaceRef.current?.(administrativePlace(feature, 'province', 'Natural Earth 1:10m'), { select: apply, clearQuery: false, fit: apply });
      }
    }).catch(error => {
      if (error.name !== 'AbortError') { setCountryGeometryState('error'); setCountryGeometryError(error.message); }
    });
    return () => controller.abort();
  }, [activeCountry]);

  useEffect(() => { drawingMode.current = drawing; box.current?.setActive(drawing); }, [drawing]);
  useEffect(() => {
    if (!selection.current) return;
    selection.current.clear();
    if (selectedPolygon) selection.current.addFeature(new GeoJSON().readFeature({ type: 'Feature', properties: {}, geometry: selectedPolygon.geometry }, { dataProjection: 'EPSG:4326', featureProjection: 'EPSG:4326' }));
    else if (parsed.bounds) selection.current.addFeature(new Feature(fromExtent(parsed.bounds)));
  }, [parsed.bounds, selectedPolygon]);

  const fit = bounds => {
    if (!map.current || !bounds) return;
    map.current.updateSize();
    map.current.getView().fit(bounds, { padding: [56, 56, 56, 56], maxZoom: 8, duration: 0 });
  };
  const changeField = (index, value) => { pendingProvinceRef.current = null; setDrawError(false); setSelectedPolygon(null); setFields(current => current.map((item, i) => i === index ? value : item)); };
  const selectedArea = () => ({ bounds: parsed.bounds, geometry: selectedPolygon?.geometry || null, place: selectedPolygon?.place || null });
  const exportArea = () => onExport(selectedArea());
  const useFocusedBounds = () => {
    pendingProvinceRef.current = null;
    selectPlace(focusedPlace, false);
  };
  const useFocusedPolygon = () => {
    pendingProvinceRef.current = null;
    selectPlace(focusedPlace);
  };

  return <div className="aoi-picker">
    <div className="aoi-intro"><Badge tone="blue">{t('LIVE SEARCH AREA')}</Badge><p>{t('Find an administrative area and select its boundary, or draw a WGS 84 rectangle. Search uses the bounding box; local clipping uses the selected polygon.')}</p></div>
    <div className="aoi-layout">
      <div className="aoi-map-section">
        <div className="aoi-place-find">
          <label>{t('Search administrative areas')}<Input value={placeQuery} onChange={event => setPlaceQuery(event.target.value)} placeholder={t('Country, region or subdivision name')} /></label>
          {placeQuery && <div className="aoi-place-results">{placeResults.map(place => <Button key={`${place.kind}-${place.code}`} size="row" onClick={() => focusPlace(place)}><span><strong>{nameOf(place)}</strong><small>{subtitleOf(place)}</small></span></Button>)}{!placeResults.length && <p role="status">{countryState === 'loading' || provinceState === 'loading' ? t('Loading administrative areas…') : t('No matching administrative area.')}</p>}</div>}
          <p>{t('Search countries, regions and 4,596 subdivisions. Outlines use 1:50m data; detailed boundaries load by source region at 1:10m. Boundaries are map references, not legal definitions.')}</p>
        </div>
        <div className="aoi-map-toolbar"><Button selected={drawing} onClick={() => { pendingProvinceRef.current = null; setDrawing(value => !value); }}><SquareDashed size={16}/>{t(drawing ? 'Cancel drawing' : 'Draw rectangle')}</Button><div><Button size="icon" aria-label={t('Zoom in')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() + 1)}><Plus size={16}/></Button><Button size="icon" aria-label={t('Zoom out')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() - 1)}><Minus size={16}/></Button><Button size="icon" aria-label={t('Fit selected area')} disabled={!parsed.bounds} onClick={() => fit(parsed.bounds)}><Maximize size={16}/></Button><Button size="icon" aria-label={t('World view')} onClick={() => fit([-180, -90, 180, 90])}><Globe2 size={16}/></Button></div></div>
        <div className="aoi-map-wrap"><div ref={target} className={'aoi-map' + (drawing ? ' is-drawing' : '')} tabIndex={0} role="application" aria-label={t('WGS 84 reference map. Pan and zoom, click a region to load its subdivisions, or draw a rectangle.')} />{drawing && <p className="aoi-draw-hint" role="status">{t('Drag from one corner to the opposite corner to select an area.')}</p>}</div>
        <div className="aoi-map-caption"><span>{t('Natural Earth · regional outlines 1:50m · subdivisions 1:10m')}</span>{mapState === 'loading' && <span role="status"><Spinner size={14}/>{t('Loading reference map…')}</span>}{mapState === 'error' && <span role="alert">{t('Reference map unavailable; enter coordinates instead.')}</span>}{countryState === 'error' && <span role="alert">{t('Regional boundaries unavailable; draw or enter coordinates instead.')}</span>}{provinceState === 'loading' && <span role="status"><Spinner size={14}/>{t('Loading global subdivision index…')}</span>}{provinceState === 'error' && <span role="alert">{t('Global subdivision index unavailable; regional selection still works.')}</span>}{countryGeometryState === 'loading' && <span role="status"><Spinner size={14}/>{t('Loading subdivisions for {country}…', { country: countryNames.get(activeCountry) || activeCountry })}</span>}{countryGeometryState === 'error' && <span role="alert">{t('Subdivision geometry unavailable: {error}', { error: countryGeometryError })}</span>}{drawError && <span role="alert">{t('Draw a larger rectangle within WGS 84 limits.')}</span>}</div>
      </div>
      <div className="aoi-form">
        {focusedPlace && <div className="aoi-place-focus"><Badge tone="blue">{t(focusedPlace.levelLabel)}</Badge><strong>{nameOf(focusedPlace)}</strong>{focusedPlace.displayParentCode && <small>{subtitleOf(focusedPlace)}</small>}<p>{t(focusedPolygonApplied ? 'The administrative polygon is selected. Search uses its bounding box; local SCL clipping uses the polygon.' : focusedBoundsApplied ? !focusedPlace.feature ? 'The region bounding rectangle is selected. Loading its polygon boundary…' : 'The region bounding rectangle is selected for search.' : !focusedPlace.feature ? 'Loading this region boundary; its bounding rectangle is available now.' : 'The region is highlighted. Choose its polygon or bounding rectangle for the workspace.')}</p>{focusedPlace.clipLimitation && <p role="alert">{t(limitationMessages[focusedPlace.clipLimitation])}</p>}<div className="row-actions"><Button size="sm" selected={focusedPolygonApplied} disabled={!focusedPlace.feature || Boolean(focusedPlace.clipLimitation)} onClick={useFocusedPolygon}>{t('Use region polygon')}</Button><Button size="sm" selected={Boolean(focusedBoundsApplied && !focusedPolygonApplied)} disabled={focusedPlace.clipLimitation === 'date-line'} onClick={useFocusedBounds}>{t('Use bounding rectangle')}</Button></div></div>}
        <h3>{t('Selected bounds')}</h3>
        <p>{t('Longitude and latitude in degrees. West must be less than east; south must be less than north.')}</p>
        <div className="aoi-fields">{directions.map((direction, index) => <label key={direction}>{t(direction)}<Input type="number" step="any" min={index % 2 === 0 ? -180 : -90} max={index % 2 === 0 ? 180 : 90} value={fields[index]} onChange={event => changeField(index, event.target.value)}/></label>)}</div>
        {parsed.error && (fields.every(value => !value.trim())
          ? <p className="muted">{t('Draw on the map, choose a region, or enter coordinates to begin.')}</p>
          : <p className="aoi-error" role="alert">{t(parsed.error)}</p>)}
        {parsed.bounds && <output className="aoi-summary mono">[{parsed.bounds.map(value => number(value, { maximumFractionDigits: 5 })).join(', ')}]</output>}
      </div>
    </div>
    <div className="aoi-actions"><Button onClick={onClose}>{t('Cancel')}</Button><Button icon={Download} disabled={!parsed.bounds || waitingForPolygon} onClick={exportArea}>{t('Download area GeoJSON')}</Button><Button primary icon={Search} disabled={!parsed.bounds || waitingForPolygon} onClick={() => onApply(selectedArea())}>{t('Search this area')}</Button></div>
  </div>;
}
