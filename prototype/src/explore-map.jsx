import React, { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import WebGLTileLayer from 'ol/layer/WebGLTile.js';
import VectorLayer from 'ol/layer/Vector.js';
import VectorSource from 'ol/source/Vector.js';
import DragBox from 'ol/interaction/DragBox.js';
import { always } from 'ol/events/condition.js';
import Feature from 'ol/Feature.js';
import GeoJSON from 'ol/format/GeoJSON.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import { intersects as extentsIntersect } from 'ol/extent.js';
import { transformExtent } from 'ol/proj.js';
import { register } from 'ol/proj/proj4.js';
import { Fill, Stroke, Style } from 'ol/style.js';
import proj4 from 'proj4';
import { focusRasterExtent, intersectBounds, utmDefinition } from './workspace-map-geometry.js';
import { Button, Progress, Spinner, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { prepareAssetAccess } from './providers.js';
import { imageryHrefs } from './explore-imagery.js';
import { createImagerySource } from './explore-imagery-source.js';
import 'ol/ol.css';
import './explore-map.css';

const OVERVIEW_PROJECTION = 'EPSG:3857';
const overviewLand = new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false });
const overviewCountries = new VectorSource({ url: './basemaps/natural-earth-50m-admin-0-countries.geojson', format: new GeoJSON(), wrapX: false });
const landStyle = new Style({ fill: new Fill({ color: '#30443c' }), stroke: new Stroke({ color: '#64877e', width: 0.7 }) });
const countryStyle = new Style({ stroke: new Stroke({ color: '#a6bab0', width: 1.2 }) });
const footprintStyle = new Style({ stroke: new Stroke({ color: '#9ed4eb', width: 1.25, lineDash: [5, 4] }), fill: new Fill({ color: 'rgba(102, 183, 225, 0.045)' }) });
const selectedFootprintStyle = new Style({ stroke: new Stroke({ color: '#55b8ff', width: 2 }), fill: new Fill({ color: 'rgba(45, 135, 255, 0.07)' }) });
const queuedFootprintStyle = new Style({ stroke: new Stroke({ color: '#f4b544', width: 2.5 }), fill: new Fill({ color: 'rgba(244, 181, 68, 0.13)' }) });
const loadedFootprintStyle = new Style({ stroke: new Stroke({ color: '#6ed7a3', width: 2 }), fill: new Fill({ color: 'rgba(68, 193, 135, 0.08)' }) });
const focusedFootprintStyle = new Style({ stroke: new Stroke({ color: '#f044d2', width: 4.5 }), fill: new Fill({ color: 'rgba(240, 68, 210, 0.1)' }) });

function sceneFootprint(scene) {
  let feature;
  if (['Polygon', 'MultiPolygon'].includes(scene.geometry?.type)) {
    try {
      feature = new GeoJSON().readFeature({ type: 'Feature', properties: {}, geometry: scene.geometry },
        { dataProjection: 'EPSG:4326', featureProjection: OVERVIEW_PROJECTION });
    } catch { /* Fall back to the catalog bounds below. */ }
  }
  if (feature && !validExtent(feature.getGeometry()?.getExtent())) feature = null;
  if (!feature && Array.isArray(scene.bbox) && scene.bbox.length === 4
    && scene.bbox.every(Number.isFinite) && scene.bbox[0] < scene.bbox[2] && scene.bbox[1] < scene.bbox[3]) {
    feature = new Feature(fromExtent(transformExtent(scene.bbox, 'EPSG:4326', OVERVIEW_PROJECTION)));
  }
  const extent = feature?.getGeometry()?.getExtent();
  if (!validExtent(extent)) return null;
  feature.setId(scene.id);
  return feature;
}

function validExtent(extent) {
  return extent?.length === 4 && extent.every(Number.isFinite) && extent[0] < extent[2] && extent[1] < extent[3];
}

function fitAllInitially(union, selected) {
  return validExtent(union) && validExtent(selected)
    && (union[2] - union[0] > (selected[2] - selected[0]) * 1.35
      || union[3] - union[1] > (selected[3] - selected[1]) * 1.35);
}

export function sceneExtent(scene) {
  const [height, width] = scene?.grid?.shape || [];
  const [xSize, xSkew, xOrigin, ySkew, ySize, yOrigin] = scene?.grid?.transform || [];
  if (![height, width, xSize, xSkew, xOrigin, ySkew, ySize, yOrigin].every(Number.isFinite)
    || height <= 0 || width <= 0 || xSize <= 0 || ySize >= 0 || xSkew !== 0 || ySkew !== 0) {
    throw new Error('The selected scene has no supported georeferenced true-color grid.');
  }
  return [xOrigin, yOrigin + height * ySize, xOrigin + width * xSize, yOrigin];
}

function fitExtent(map, extent) {
  const size = map.getSize();
  if (size?.[0] && size?.[1]) map.getView().fit(extent, { size, padding: [90, 90, 125, 90], maxZoom: 16, duration: 250 });
}

export const ExploreMap = forwardRef(function ExploreMap({ scene, scenes, loadedScenes = [], selectedIds = [], focusedIds = [], activeSceneId, activeDay, reference, split, area, areaGeometry, showArea, boxSelect = false, onFootprintsPick, onFootprintsChange }, ref) {
  const { t } = useI18n();
  const target = useRef(null);
  const map = useRef(null);
  const referenceLayer = useRef(null);
  const areaLayer = useRef(null);
  const footprintSource = useRef(null);
  const footprintUnionRef = useRef(null);
  const onFootprintsPickRef = useRef(onFootprintsPick);
  const boxSelectRef = useRef(boxSelect);
  const savedViewRef = useRef(null);
  const sceneExtentRef = useRef(null);
  const areaExtentRef = useRef(null);
  const focusExtentRef = useRef(null);
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState({ metadataReady: false, requested: 0, completed: 0, active: 0, elapsed: 0, stalled: false });
  const [otherLoading, setOtherLoading] = useState({ metadata: 0, total: 0, requested: 0, completed: 0, active: 0 });
  const [referenceLoading, setReferenceLoading] = useState({ metadataReady: false, ready: false, requested: 0, completed: 0, active: 0 });
  const [retry, setRetry] = useState(0);
  const areaKey = area?.join(',');
  onFootprintsPickRef.current = onFootprintsPick;
  boxSelectRef.current = boxSelect;
  const shouldLoad = loadedScenes.some(item => item.id === scene?.id) && (!activeDay || scene?.date?.slice(0, 10) === activeDay);
  const retryMap = async () => {
    try {
      await prepareAssetAccess([...loadedScenes, reference].flatMap(imageryHrefs), { force: true });
      setRetry(value => value + 1);
    } catch (failure) { setError(failure.message); }
  };

  useImperativeHandle(ref, () => ({
    zoomIn() { const view = map.current?.getView(); if (view) { view.cancelAnimations(); view.animate({ resolution: view.getResolution() / 2, duration: 240 }); } },
    zoomOut() { const view = map.current?.getView(); if (view) { view.cancelAnimations(); view.animate({ resolution: view.getResolution() * 2, duration: 240 }); } },
    fit() { if (map.current && (footprintUnionRef.current || focusExtentRef.current || sceneExtentRef.current)) fitExtent(map.current, footprintUnionRef.current || focusExtentRef.current || sceneExtentRef.current); },
  }), []);

  useEffect(() => {
    setError(''); setReady(false);
    setLoading({ metadataReady: false, requested: 0, completed: 0, active: 0, elapsed: 0, stalled: false });
    if (!target.current) return;
    let cancelled = false;
    let instance;
    let source;
    let resize;
    let pendingTiles = 0;
    let requestedTiles = 0;
    let completedTiles = 0;
    let metadataReady = false;
    let initialReady = false;
    let failed = false;
    let fitted = false;
    let loadingClock;
    const startedAt = Date.now();
    let lastActivityAt = startedAt;
    const publishLoading = () => {
      if (cancelled || failed) return;
      const now = Date.now();
      setLoading({ metadataReady, requested: requestedTiles, completed: completedTiles, active: pendingTiles,
        elapsed: Math.floor((now - startedAt) / 1000), stalled: now - lastActivityAt >= 12000 });
    };
    const fail = message => {
      if (cancelled || failed) return;
      failed = true;
      clearInterval(loadingClock);
      setError(message);
    };
    if (shouldLoad) loadingClock = setInterval(() => {
      if (failed || (initialReady && pendingTiles === 0)) return;
      publishLoading();
      if (Date.now() - lastActivityAt >= 45000) fail('The imagery source stopped responding. Try another scene or retry.');
    }, 1000);
    try {
      const projected = transformExtent(area, 'EPSG:4326', OVERVIEW_PROJECTION, 8);
      if (shouldLoad && !imageryHrefs(scene).length) throw new Error('This scene has no supported georeferenced true-color grid.');
      let mapExtent = projected;
      if (shouldLoad) {
        const extent = sceneExtent(scene);
        proj4.defs(scene.crs, utmDefinition(scene.crs));
        register(proj4);
        mapExtent = transformExtent(extent, scene.crs, OVERVIEW_PROJECTION, 16);
      }
      const imagery = shouldLoad ? createImagerySource(scene) : null;
      source = imagery?.source;
      const base = source ? new WebGLTileLayer({ source, style: imagery.style, className: 'explore-base-layer', extent: mapExtent, zIndex: 9 }) : null;
      const land = new VectorLayer({ source: overviewLand, style: landStyle, zIndex: 0 });
      const countries = new VectorLayer({ source: overviewCountries, style: countryStyle, minResolution: 750, zIndex: 2 });
      const footprints = new VectorSource({ wrapX: false });
      const footprintLayer = new VectorLayer({ source: footprints, style: feature => feature.get('hidden') ? null : feature.get('focused') ? focusedFootprintStyle : feature.get('queued') ? queuedFootprintStyle : feature.get('loaded') ? loadedFootprintStyle : feature.get('active') ? selectedFootprintStyle : footprintStyle, zIndex: 11 });
      const overlays = new VectorLayer({ source: new VectorSource(), className: 'explore-area-layer', zIndex: 12 });
      const previous = savedViewRef.current?.areaKey === areaKey ? savedViewRef.current : null;
      const view = new View({ projection: OVERVIEW_PROJECTION,
        center: previous?.center || [(mapExtent[0] + mapExtent[2]) / 2, (mapExtent[1] + mapExtent[3]) / 2],
        resolution: previous?.resolution || 100, minResolution: 2.5, maxResolution: 156543.03392804097, enableRotation: false });
      instance = new OLMap({ target: target.current, controls: [], layers: [land, ...(base ? [base] : []), countries, footprintLayer, overlays], view });
      map.current = instance; areaLayer.current = overlays; footprintSource.current = footprints; sceneExtentRef.current = mapExtent;
      resize = new ResizeObserver(() => { instance.updateSize(); });
      resize.observe(target.current);
      instance.on('singleclick', event => {
        if (boxSelectRef.current) return;
        const matches = footprintSource.current?.getFeaturesAtCoordinate(event.coordinate).map(feature => feature.getId()) || [];
        onFootprintsPickRef.current?.([...new Set(matches)]);
      });
      source?.on('tileloadstart', () => { pendingTiles += 1; requestedTiles += 1; lastActivityAt = Date.now(); publishLoading(); });
      source?.on('tileloadend', () => {
        pendingTiles = Math.max(0, pendingTiles - 1);
        completedTiles += 1;
        lastActivityAt = Date.now();
        publishLoading();
        if (!cancelled && !failed && !initialReady) { initialReady = true; setReady(true); }
      });
      source?.on('tileloaderror', () => { pendingTiles = Math.max(0, pendingTiles - 1); fail('The true-color COG tiles could not load. Check your connection or retry.'); });
      source?.on('error', () => fail('The true-color COG metadata could not load. Check your connection or retry.'));
      source?.getView().then(() => { metadataReady = true; lastActivityAt = Date.now(); publishLoading(); })
        .catch(() => fail('The true-color COG metadata could not load. Check your connection or retry.'));
      if (!source) { metadataReady = true; initialReady = true; setReady(true); }
      // Focus on the part of the searched area covered by this scene, including when the search spans multiple UTM zones.
      areaExtentRef.current = intersectBounds(projected, mapExtent) ? projected : null;
      focusExtentRef.current = focusRasterExtent(mapExtent, projected);
      requestAnimationFrame(() => {
        if (cancelled) return;
        const size = instance.getSize();
        if (!previous) fitExtent(instance, !shouldLoad ? footprintUnionRef.current || projected : fitAllInitially(footprintUnionRef.current, mapExtent) ? footprintUnionRef.current : focusExtentRef.current);
        else if (size?.[0] && size?.[1] && !extentsIntersect(instance.getView().calculateExtent(size), mapExtent)) fitExtent(instance, focusExtentRef.current);
        fitted = true;
      });
    } catch (cause) {
      fail(cause.message);
    }
    return () => {
      cancelled = true; clearInterval(loadingClock); resize?.disconnect();
      if (instance) {
        if (fitted) savedViewRef.current = { areaKey, center: instance.getView().getCenter()?.slice(), resolution: instance.getView().getResolution() };
        instance.setTarget(undefined); instance.dispose();
      }
      source?.dispose();
      map.current = null; areaLayer.current = null; footprintSource.current = null; referenceLayer.current = null;
      sceneExtentRef.current = null; areaExtentRef.current = null; focusExtentRef.current = null;
    };
  }, [scene?.id, areaKey, retry, shouldLoad]);

  useEffect(() => {
    const source = footprintSource.current;
    if (!source) return;
    source.clear();
    let union = null;
    let count = 0;
    const selectedSet = new Set(selectedIds);
    const loadedSet = new Set(loadedScenes.map(item => item.id));
    const focusedSet = new Set(focusedIds);
    const focusedFootprints = new Set();
    const renderedFootprints = new Map();
    const focusedFeatures = [];
    for (const item of scenes) {
      const feature = sceneFootprint(item);
      if (!feature) continue;
      const extent = feature.getGeometry().getExtent();
      const footprintKey = JSON.stringify(feature.getGeometry().getCoordinates());
      const focused = focusedSet.has(item.id) && !focusedFootprints.has(footprintKey);
      if (focused) focusedFootprints.add(footprintKey);
      const queued = selectedSet.has(item.id) && !loadedSet.has(item.id);
      const loaded = loadedSet.has(item.id);
      const active = item.id === activeSceneId;
      const priority = focused ? 4 : queued ? 3 : loaded ? 2 : active ? 1 : 0;
      // Keep every scene hit-testable while drawing identical footprints only once.
      const rendered = renderedFootprints.get(footprintKey);
      const hidden = rendered && rendered.priority >= priority;
      if (!hidden) {
        rendered?.feature.set('hidden', true);
        renderedFootprints.set(footprintKey, { feature, priority });
      }
      feature.setProperties({ queued, loaded, active, focused, hidden: !!hidden });
      if (focused) focusedFeatures.push(feature);
      else source.addFeature(feature);
      union = union ? [Math.min(union[0], extent[0]), Math.min(union[1], extent[1]), Math.max(union[2], extent[2]), Math.max(union[3], extent[3])] : extent.slice();
      count += 1;
    }
    focusedFeatures.forEach(feature => source.addFeature(feature));
    footprintUnionRef.current = union;
    onFootprintsChange?.(count);
  }, [scenes, selectedIds, focusedIds, loadedScenes, activeSceneId, areaKey, retry, onFootprintsChange]);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !boxSelect) return;
    const dragBox = new DragBox({ condition: always, className: 'explore-scene-dragbox' });
    dragBox.on('boxend', () => {
      const extent = dragBox.getGeometry().getExtent();
      const matches = footprintSource.current?.getFeaturesInExtent(extent)
        .filter(feature => feature.getGeometry()?.intersectsExtent(extent)).map(feature => feature.getId()) || [];
      onFootprintsPickRef.current?.([...new Set(matches)]);
    });
    instance.addInteraction(dragBox);
    return () => instance.removeInteraction(dragBox);
  }, [boxSelect, scene?.id, areaKey, retry, shouldLoad]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    const entries = [];
    let active = true;
    const otherScenes = loadedScenes.filter(item => item.id !== scene.id && (!activeDay || item.date.slice(0, 10) === activeDay));
    let metadata = 0, requested = 0, completed = 0, pending = 0, lastActivity = Date.now();
    const publish = () => { lastActivity = Date.now(); if (active) setOtherLoading({ metadata, total: otherScenes.length, requested, completed, active: pending }); };
    const watch = otherScenes.length ? setInterval(() => {
      if (active && (metadata < otherScenes.length || pending > 0) && Date.now() - lastActivity >= 45000)
        setError('A selected COG could not load. Remove it from the selection and retry.');
    }, 1000) : null;
    publish();
    otherScenes.forEach((item, index) => {
      try {
        const extent = sceneExtent(item);
        proj4.defs(item.crs, utmDefinition(item.crs));
        register(proj4);
        const { source, style } = createImagerySource(item);
        const layer = new WebGLTileLayer({ source, style, className: 'explore-mosaic-layer',
          extent: transformExtent(extent, item.crs, OVERVIEW_PROJECTION, 16), zIndex: 3 + index * 0.25 });
        source.on('tileloadstart', () => { requested += 1; pending += 1; publish(); });
        source.on('tileloadend', () => { completed += 1; pending = Math.max(0, pending - 1); publish(); });
        source.on('error', () => { if (active) setError('A selected COG could not load. Remove it from the selection and retry.'); });
        source.on('tileloaderror', () => { pending = Math.max(0, pending - 1); publish(); if (active) setError('A selected COG could not load. Remove it from the selection and retry.'); });
        source.getView().then(() => { metadata += 1; publish(); }).catch(() => { if (active) setError('A selected COG could not load. Remove it from the selection and retry.'); });
        instance.addLayer(layer);
        entries.push({ layer, source });
      } catch { if (active) setError('A selected COG has no supported georeferenced grid.'); }
    });
    return () => {
      active = false;
      clearInterval(watch);
      entries.forEach(({ layer, source }) => { instance.removeLayer(layer); layer.setSource(null); source.dispose(); });
    };
  }, [scene?.id, areaKey, loadedScenes, activeDay, retry, shouldLoad]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    setReferenceLoading({ metadataReady: false, ready: false, requested: 0, completed: 0, active: 0 });
    if (referenceLayer.current) { instance.removeLayer(referenceLayer.current); const oldSource = referenceLayer.current.getSource(); referenceLayer.current.setSource(null); oldSource?.dispose(); referenceLayer.current = null; }
    if (!reference) return;
    let active = true;
    let referenceSource, layer;
    let metadataReady = false, ready = false, requested = 0, completed = 0, pending = 0, lastActivity = Date.now();
    const publish = () => { lastActivity = Date.now(); if (active) setReferenceLoading({ metadataReady, ready, requested, completed, active: pending }); };
    const referenceError = () => { if (active) setError('The reference COG tiles could not load. Try another scene.'); };
    const watch = setInterval(() => { if (active && (!ready || pending > 0) && Date.now() - lastActivity >= 45000) referenceError(); }, 1000);
    prepareAssetAccess(imageryHrefs(reference)).then(() => {
      if (!active) return;
      const imagery = createImagerySource(reference);
      referenceSource = imagery.source;
      layer = new WebGLTileLayer({ source: referenceSource, style: imagery.style, className: 'explore-reference-layer',
        extent: transformExtent(sceneExtent(reference), reference.crs, OVERVIEW_PROJECTION, 16), zIndex: 10 });
      referenceSource.on('tileloadstart', () => { requested += 1; pending += 1; publish(); });
      referenceSource.on('tileloadend', () => { completed += 1; pending = Math.max(0, pending - 1); ready = true; publish(); });
      referenceSource.on('tileloaderror', referenceError);
      referenceSource.on('error', referenceError);
      referenceSource.getView().then(() => { metadataReady = true; publish(); }).catch(referenceError);
      instance.addLayer(layer); referenceLayer.current = layer;
    }).catch(referenceError);
    return () => { active = false; clearInterval(watch); if (layer) { instance.removeLayer(layer); layer.setSource(null); } referenceSource?.dispose(); if (referenceLayer.current === layer) referenceLayer.current = null; };
  }, [scene?.id, areaKey, reference?.id, retry]);

  useEffect(() => {
    const source = areaLayer.current?.getSource();
    if (!source) return;
    source.clear();
    if (!showArea || !areaExtentRef.current) return;
    let feature;
    try {
      feature = areaGeometry
        ? new GeoJSON().readFeature({ type: 'Feature', properties: {}, geometry: areaGeometry }, { dataProjection: 'EPSG:4326', featureProjection: OVERVIEW_PROJECTION })
        : new Feature(fromExtent(areaExtentRef.current));
    } catch { feature = new Feature(fromExtent(areaExtentRef.current)); }
    feature.setStyle(new Style({ stroke: new Stroke({ color: '#f8fbff', width: 3, lineDash: [8, 5] }), fill: new Fill({ color: 'rgba(45, 135, 255, 0.10)' }) }));
    source.addFeature(feature);
  }, [scene?.id, areaKey, areaGeometry, showArea, retry]);

  const requested = loading.requested + otherLoading.requested + (reference ? referenceLoading.requested : 0);
  const completed = loading.completed + otherLoading.completed + (reference ? referenceLoading.completed : 0);
  const activeRequests = loading.active + otherLoading.active + (reference ? referenceLoading.active : 0);
  const metadataReady = otherLoading.metadata + (shouldLoad && loading.metadataReady ? 1 : 0) + (reference && referenceLoading.metadataReady ? 1 : 0);
  const metadataTotal = otherLoading.total + (shouldLoad ? 1 : 0) + (reference ? 1 : 0);
  return <div className="explore-map-root" data-map-ready={ready ? 'true' : 'false'} data-reference-ready={reference && referenceLoading.ready ? 'true' : 'false'} data-box-select={boxSelect ? 'true' : 'false'} style={{ '--compare-mask-right': `${100 - split}%` }}>
    <div className="explore-map-target" ref={target} aria-label={t(loadedScenes.length ? 'Georeferenced true-color imagery map' : 'Scene footprint map')} />
    {!error && ((shouldLoad && !ready) || otherLoading.metadata < otherLoading.total || activeRequests > 0 || (reference && !referenceLoading.ready)) && <Surface className={`explore-map-message explore-map-progress${ready ? ' explore-map-progress-compact' : ''}`}>
      <div className="explore-map-progress-heading"><Spinner size={17}/><strong>{t(reference && !referenceLoading.ready ? 'Loading reference imagery…' : loadedScenes.length > 1 ? 'Loading selected COG layers · {count}' : 'Loading true-color COG', { count: loadedScenes.length })}</strong><span>{t('Metadata {ready}/{total}', { ready: metadataReady, total: metadataTotal })}</span></div>
      <p role="status">{t(requested === 0 && metadataReady < metadataTotal ? 'Reading imagery metadata…' : requested === 0 ? 'Locating tiles for the current view…' : ready ? 'Map visible; loading remaining tiles…' : 'Loading visible imagery tiles…')}</p>
      <Progress value={requested > 0 ? completed : null} max={requested || 100} aria-label={t('Completed imagery tile requests')} />
      <div className="explore-map-progress-detail"><span>{requested > 0 ? <>{t('Tiles returned')} {completed}/{requested} · {t('Active requests')} {activeRequests}</> : t('Waiting for the imagery source')}</span><span>{loading.elapsed}{t(' seconds')}</span></div>
      {loading.stalled && <div className="explore-map-progress-stalled"><span>{t('Imagery source is responding slowly. You can keep waiting or retry.')}</span><Button size="xs" onClick={retryMap}><RefreshCw size={13}/>{t('Retry map')}</Button></div>}
    </Surface>}
    {error && <Surface className="explore-map-message explore-map-error" role="alert"><strong>{t('Map unavailable')}</strong><span>{t(error)}</span><Button onClick={retryMap}><RefreshCw size={14}/>{t('Retry map')}</Button></Surface>}
  </div>;
});
