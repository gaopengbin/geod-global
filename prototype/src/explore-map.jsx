import React, { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import WebGLTileLayer from 'ol/layer/WebGLTile.js';
import VectorLayer from 'ol/layer/Vector.js';
import GeoTIFF from 'ol/source/GeoTIFF.js';
import VectorSource from 'ol/source/Vector.js';
import Feature from 'ol/Feature.js';
import GeoJSON from 'ol/format/GeoJSON.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import { transform, transformExtent } from 'ol/proj.js';
import { register } from 'ol/proj/proj4.js';
import { Fill, Stroke, Style } from 'ol/style.js';
import proj4 from 'proj4';
import { focusRasterExtent, intersectBounds, utmDefinition } from './workspace-map-geometry.js';
import { Button, Progress, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import 'ol/ol.css';
import './explore-map.css';

const OVERVIEW_PROJECTION = 'EPSG:3857';
const overviewLand = new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false });
const overviewCountries = new VectorSource({ url: './basemaps/natural-earth-50m-admin-0-countries.geojson', format: new GeoJSON(), wrapX: false });
const landStyle = new Style({ fill: new Fill({ color: '#30443c' }), stroke: new Stroke({ color: '#64877e', width: 0.7 }) });
const countryStyle = new Style({ stroke: new Stroke({ color: '#a6bab0', width: 1.2 }) });

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

export const ExploreMap = forwardRef(function ExploreMap({ scene, reference, split, area, areaGeometry, showArea }, ref) {
  const { t } = useI18n();
  const target = useRef(null);
  const map = useRef(null);
  const referenceLayer = useRef(null);
  const areaLayer = useRef(null);
  const sceneExtentRef = useRef(null);
  const areaExtentRef = useRef(null);
  const focusExtentRef = useRef(null);
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState({ metadataReady: false, requested: 0, completed: 0, active: 0, elapsed: 0, stalled: false });
  const [retry, setRetry] = useState(0);
  const areaKey = area?.join(',');

  useImperativeHandle(ref, () => ({
    zoomIn() { const view = map.current?.getView(); if (view) { view.cancelAnimations(); view.setResolution(view.getResolution() / 2); } },
    zoomOut() { const view = map.current?.getView(); if (view) { view.cancelAnimations(); view.setResolution(view.getResolution() * 2); } },
    fit() { if (map.current && (focusExtentRef.current || sceneExtentRef.current)) fitExtent(map.current, focusExtentRef.current || sceneExtentRef.current); },
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
    loadingClock = setInterval(() => {
      if (failed || (initialReady && pendingTiles === 0)) return;
      publishLoading();
      if (Date.now() - lastActivityAt >= 45000) fail('The imagery source stopped responding. Try another scene or retry.');
    }, 1000);
    try {
      if (!scene?.assets?.visual?.href) throw new Error('This scene has no true-color COG asset.');
      const extent = sceneExtent(scene);
      utmDefinition(scene.crs);
      proj4.defs(scene.crs, utmDefinition(scene.crs));
      register(proj4);
      source = new GeoTIFF({ sources: [{ url: scene.assets.visual.href }] });
      const mapExtent = transformExtent(extent, scene.crs, OVERVIEW_PROJECTION, 16);
      const base = new WebGLTileLayer({ source, className: 'explore-base-layer', extent: mapExtent, zIndex: 1 });
      const land = new VectorLayer({ source: overviewLand, style: landStyle, zIndex: 0 });
      const countries = new VectorLayer({ source: overviewCountries, style: countryStyle, minResolution: 750, zIndex: 2 });
      const overlays = new VectorLayer({ source: new VectorSource(), className: 'explore-area-layer', zIndex: 10 });
      const view = new View({ projection: OVERVIEW_PROJECTION,
        center: transform([(extent[0] + extent[2]) / 2, (extent[1] + extent[3]) / 2], scene.crs, OVERVIEW_PROJECTION),
        resolution: 100, minResolution: 2.5, maxResolution: 156543.03392804097, enableRotation: false });
      instance = new OLMap({ target: target.current, controls: [], layers: [land, base, countries, overlays], view });
      map.current = instance; areaLayer.current = overlays; sceneExtentRef.current = mapExtent;
      resize = new ResizeObserver(() => { instance.updateSize(); });
      resize.observe(target.current);
      source.on('tileloadstart', () => { pendingTiles += 1; requestedTiles += 1; lastActivityAt = Date.now(); publishLoading(); });
      source.on('tileloadend', () => {
        pendingTiles = Math.max(0, pendingTiles - 1);
        completedTiles += 1;
        lastActivityAt = Date.now();
        publishLoading();
        if (!cancelled && !failed && !initialReady) { initialReady = true; setReady(true); }
      });
      source.on('tileloaderror', () => { pendingTiles = Math.max(0, pendingTiles - 1); fail('The true-color COG tiles could not load. Check your connection or retry.'); });
      source.on('error', () => fail('The true-color COG metadata could not load. Check your connection or retry.'));
      source.getView().then(() => { metadataReady = true; lastActivityAt = Date.now(); publishLoading(); })
        .catch(() => fail('The true-color COG metadata could not load. Check your connection or retry.'));
      // Focus on the part of the searched area covered by this scene, including when the search spans multiple UTM zones.
      const projected = transformExtent(area, 'EPSG:4326', OVERVIEW_PROJECTION, 8);
      areaExtentRef.current = intersectBounds(projected, mapExtent) ? projected : null;
      focusExtentRef.current = focusRasterExtent(mapExtent, projected);
      requestAnimationFrame(() => { if (!cancelled) fitExtent(instance, focusExtentRef.current); });
    } catch (cause) {
      fail(cause.message);
    }
    return () => {
      cancelled = true; clearInterval(loadingClock); resize?.disconnect();
      if (instance) { instance.setTarget(undefined); instance.dispose(); }
      source?.dispose();
      map.current = null; areaLayer.current = null; referenceLayer.current = null;
      sceneExtentRef.current = null; areaExtentRef.current = null; focusExtentRef.current = null;
    };
  }, [scene?.id, areaKey, retry]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    if (referenceLayer.current) { instance.removeLayer(referenceLayer.current); const oldSource = referenceLayer.current.getSource(); referenceLayer.current.setSource(null); oldSource?.dispose(); referenceLayer.current = null; }
    if (!reference) return;
    let active = true;
    const referenceSource = new GeoTIFF({ sources: [{ url: reference.assets.visual.href }] });
    const layer = new WebGLTileLayer({ source: referenceSource, className: 'explore-reference-layer',
      extent: transformExtent(sceneExtent(reference), reference.crs, OVERVIEW_PROJECTION, 16), zIndex: 3 });
    const referenceError = () => { if (active) setError('The reference COG tiles could not load. Try another scene.'); };
    referenceSource.on('tileloaderror', referenceError);
    referenceSource.on('error', referenceError);
    referenceSource.getView().catch(referenceError);
    instance.addLayer(layer); referenceLayer.current = layer;
    return () => { active = false; instance.removeLayer(layer); layer.setSource(null); referenceSource.dispose(); if (referenceLayer.current === layer) referenceLayer.current = null; };
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

  return <div className="explore-map-root" data-map-ready={ready ? 'true' : 'false'} style={{ '--compare-mask-right': `${100 - split}%` }}>
    <div className="explore-map-target" ref={target} aria-label={t('Georeferenced true-color Sentinel-2 map')} />
    {!error && (!ready || loading.active > 0) && <div className={`explore-map-message explore-map-progress${ready ? ' explore-map-progress-compact' : ''}`}>
      <div className="explore-map-progress-heading"><Spinner size={17}/><strong>{t('Loading true-color COG')}</strong><span>{scene?.date?.slice(0, 10)}</span></div>
      <p role="status">{t(loading.requested === 0 && !loading.metadataReady ? 'Reading imagery metadata…' : loading.requested === 0 ? 'Locating tiles for the current view…' : ready ? 'Map visible; loading remaining tiles…' : 'Loading visible imagery tiles…')}</p>
      <Progress value={loading.requested > 0 ? loading.completed : null} max={loading.requested || 100} aria-label={t('Completed imagery tile requests')} />
      <div className="explore-map-progress-detail"><span>{loading.requested > 0 ? <>{t('Tiles returned')} {loading.completed}/{loading.requested} · {t('Active requests')} {loading.active}</> : t('Waiting for the imagery source')}</span><span>{loading.elapsed}{t(' seconds')}</span></div>
      {loading.stalled && <div className="explore-map-progress-stalled"><span>{t('Imagery source is responding slowly. You can keep waiting or retry.')}</span><Button size="xs" onClick={() => setRetry(value => value + 1)}><RefreshCw size={13}/>{t('Retry map')}</Button></div>}
    </div>}
    {error && <div className="explore-map-message explore-map-error" role="alert"><strong>{t('Map unavailable')}</strong><span>{t(error)}</span><Button onClick={() => setRetry(value => value + 1)}><RefreshCw size={14}/>{t('Retry map')}</Button></div>}
  </div>;
});
