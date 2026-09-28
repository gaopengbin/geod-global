import React, { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import WebGLTileLayer from 'ol/layer/WebGLTile.js';
import VectorLayer from 'ol/layer/Vector.js';
import GeoTIFF from 'ol/source/GeoTIFF.js';
import VectorSource from 'ol/source/Vector.js';
import Feature from 'ol/Feature.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import { transformExtent } from 'ol/proj.js';
import { register } from 'ol/proj/proj4.js';
import { getRenderPixel } from 'ol/render.js';
import { Fill, Stroke, Style } from 'ol/style.js';
import proj4 from 'proj4';
import { utmDefinition } from './workspace-map-geometry.js';
import { Button } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import 'ol/ol.css';
import './explore-map.css';

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

export const ExploreMap = forwardRef(function ExploreMap({ scene, reference, split, area, showArea }, ref) {
  const { t } = useI18n();
  const target = useRef(null);
  const map = useRef(null);
  const referenceLayer = useRef(null);
  const areaLayer = useRef(null);
  const sceneExtentRef = useRef(null);
  const areaExtentRef = useRef(null);
  const splitRef = useRef(split);
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [retry, setRetry] = useState(0);
  const areaKey = area?.join(',');
  splitRef.current = split;

  useImperativeHandle(ref, () => ({
    zoomIn() { const view = map.current?.getView(); if (view) view.animate({ resolution: view.getResolution() / 1.5, duration: 180 }); },
    zoomOut() { const view = map.current?.getView(); if (view) view.animate({ resolution: view.getResolution() * 1.5, duration: 180 }); },
    fit() { if (map.current && (areaExtentRef.current || sceneExtentRef.current)) fitExtent(map.current, areaExtentRef.current || sceneExtentRef.current); },
  }), []);

  useEffect(() => {
    setError(''); setReady(false);
    if (!target.current) return;
    let cancelled = false;
    let instance;
    let source;
    let resize;
    let pendingTiles = 0;
    let loadedTiles = 0;
    let readyTimer;
    let initialReady = false;
    try {
      if (!scene?.assets?.visual?.href) throw new Error('This scene has no true-color COG asset.');
      const extent = sceneExtent(scene);
      utmDefinition(scene.crs);
      proj4.defs(scene.crs, utmDefinition(scene.crs));
      register(proj4);
      source = new GeoTIFF({ sources: [{ url: scene.assets.visual.href }] });
      const base = new WebGLTileLayer({ source, className: 'explore-base-layer', extent });
      const overlays = new VectorLayer({ source: new VectorSource(), className: 'explore-area-layer', zIndex: 10 });
      const view = new View({ projection: scene.crs, center: [(extent[0] + extent[2]) / 2, (extent[1] + extent[3]) / 2], resolution: 100, minResolution: 2.5, maxResolution: 2000, enableRotation: false });
      instance = new OLMap({ target: target.current, controls: [], layers: [base, overlays], view });
      map.current = instance; areaLayer.current = overlays; sceneExtentRef.current = extent;
      resize = new ResizeObserver(() => { instance.updateSize(); });
      resize.observe(target.current);
      source.on('tileloadstart', () => { pendingTiles += 1; clearTimeout(readyTimer); });
      source.on('tileloadend', () => {
        pendingTiles = Math.max(0, pendingTiles - 1);
        loadedTiles += 1;
        if (!initialReady && pendingTiles === 0) readyTimer = setTimeout(() => {
          if (!cancelled && pendingTiles === 0 && loadedTiles > 0) { initialReady = true; setReady(true); }
        }, 200);
      });
      source.on('tileloaderror', () => { pendingTiles = Math.max(0, pendingTiles - 1); if (!cancelled) setError('The true-color COG tiles could not load. Check your connection or retry.'); });
      source.on('error', () => { if (!cancelled) setError('The true-color COG metadata could not load. Check your connection or retry.'); });
      source.getView().catch(() => { if (!cancelled) setError('The true-color COG metadata could not load. Check your connection or retry.'); });
      // The submitted WGS84 bounds are transformed into the raster CRS, so the map opens at the searched area.
      const projected = transformExtent(area, 'EPSG:4326', scene.crs, 8);
      const intersects = projected[0] < extent[2] && projected[2] > extent[0] && projected[1] < extent[3] && projected[3] > extent[1];
      areaExtentRef.current = intersects ? projected : null;
      requestAnimationFrame(() => { if (!cancelled) fitExtent(instance, areaExtentRef.current || extent); });
    } catch (cause) {
      if (!cancelled) setError(cause.message);
    }
    return () => {
      cancelled = true; clearTimeout(readyTimer); resize?.disconnect();
      if (instance) { instance.setTarget(undefined); instance.dispose(); }
      source?.dispose();
      map.current = null; areaLayer.current = null; referenceLayer.current = null;
      sceneExtentRef.current = null; areaExtentRef.current = null;
    };
  }, [scene?.id, areaKey, retry]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    if (referenceLayer.current) { instance.removeLayer(referenceLayer.current); const oldSource = referenceLayer.current.getSource(); referenceLayer.current.setSource(null); oldSource?.dispose(); referenceLayer.current = null; }
    if (!reference) return;
    let active = true;
    const referenceSource = new GeoTIFF({ sources: [{ url: reference.assets.visual.href }] });
    const layer = new WebGLTileLayer({ source: referenceSource, className: 'explore-reference-layer', extent: sceneExtent(reference), zIndex: 1 });
    layer.on('prerender', event => {
      const gl = event.context;
      const size = instance.getSize();
      if (!size) return;
      const bottomLeft = getRenderPixel(event, [0, size[1]]);
      const topRight = getRenderPixel(event, [size[0], 0]);
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(bottomLeft[0], bottomLeft[1], Math.round((topRight[0] - bottomLeft[0]) * splitRef.current / 100), topRight[1] - bottomLeft[1]);
    });
    layer.on('postrender', event => event.context.disable(event.context.SCISSOR_TEST));
    const referenceError = () => { if (active) setError('The reference COG tiles could not load. Try another scene.'); };
    referenceSource.on('tileloaderror', referenceError);
    referenceSource.on('error', referenceError);
    referenceSource.getView().catch(referenceError);
    instance.addLayer(layer); referenceLayer.current = layer;
    return () => { active = false; instance.removeLayer(layer); layer.setSource(null); referenceSource.dispose(); if (referenceLayer.current === layer) referenceLayer.current = null; };
  }, [scene?.id, areaKey, reference?.id, retry]);

  useEffect(() => { map.current?.render(); }, [split]);

  useEffect(() => {
    const source = areaLayer.current?.getSource();
    if (!source) return;
    source.clear();
    if (!showArea || !areaExtentRef.current) return;
    const feature = new Feature(fromExtent(areaExtentRef.current));
    feature.setStyle(new Style({ stroke: new Stroke({ color: '#f8fbff', width: 3, lineDash: [8, 5] }), fill: new Fill({ color: 'rgba(45, 135, 255, 0.10)' }) }));
    source.addFeature(feature);
  }, [scene?.id, areaKey, showArea, retry]);

  return <div className="explore-map-root" data-map-ready={ready ? 'true' : 'false'}>
    <div className="explore-map-target" ref={target} aria-label={t('Georeferenced true-color Sentinel-2 map')} />
    {!ready && !error && <div className="explore-map-message" role="status">{t('Loading georeferenced imagery…')}</div>}
    {error && <div className="explore-map-message explore-map-error" role="alert"><strong>{t('Map unavailable')}</strong><span>{t(error)}</span><Button onClick={() => setRetry(value => value + 1)}><RefreshCw size={14}/>{t('Retry map')}</Button></div>}
  </div>;
});
