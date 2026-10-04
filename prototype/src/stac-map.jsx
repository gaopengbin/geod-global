import React, { useEffect, useRef, useState } from 'react';
import Map from 'ol/Map.js';
import View from 'ol/View.js';
import ImageLayer from 'ol/layer/Image.js';
import ImageStatic from 'ol/source/ImageStatic.js';
import VectorLayer from 'ol/layer/Vector.js';
import VectorSource from 'ol/source/Vector.js';
import GeoJSON from 'ol/format/GeoJSON.js';
import { Style, Fill, Stroke, Circle as CircleStyle } from 'ol/style.js';
import Feature from 'ol/Feature.js';
import Point from 'ol/geom/Point.js';
import { transform, transformExtent } from 'ol/proj.js';
import { register } from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import { Maximize, Minus, Plus } from 'lucide-react';
import { Button } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { genericCoordinateToPixel, genericMapMetadata, genericProjectionDefinition } from './stac-map-model.js';
import 'ol/ol.css';

export default function StacRasterMap({ data, onPixel, disabled = false, kind = 'original' }) {
  const { t } = useI18n();
  const target = useRef(null), map = useRef(null), extent = useRef(null), callback = useRef(onPixel);
  callback.current = { onPixel, disabled };
  const [error, setError] = useState('');
  useEffect(() => {
    setError('');
    let instance, resize, observer;
    try {
      genericMapMetadata(data);
      const definition = genericProjectionDefinition(data.crs);
      if (definition) { proj4.defs(data.crs, definition); register(proj4); }
      const mapBounds = transformExtent(data.bounds, data.crs, 'EPSG:4326', 16);
      if (!mapBounds.every(Number.isFinite) || mapBounds[1] < -90 || mapBounds[3] > 90) throw new Error('This file cannot be placed in the supported map projections.');
      extent.current = mapBounds;
      const color = key => getComputedStyle(target.current).getPropertyValue(key).trim();
      const landStyle = () => new Style({ fill: new Fill({ color: color('--canvas') }), stroke: new Stroke({ color: color('--line-strong'), width: 1 }) });
      const land = new VectorLayer({ source: new VectorSource({ url: './basemaps/natural-earth-50m-land.geojson', format: new GeoJSON(), wrapX: false }), style: landStyle() });
      const raster = new ImageLayer({ source: new ImageStatic({ url: data.previewDataUrl, projection: data.crs, imageExtent: data.bounds, interpolate: false }) });
      const point = new VectorSource({ wrapX: false });
      const markerStyle = () => new Style({ image: new CircleStyle({ radius: 5, fill: new Fill({ color: color('--accent') }), stroke: new Stroke({ color: color('--surface'), width: 2 }) }) });
      const marker = new VectorLayer({ source: point, style: markerStyle() });
      instance = new Map({ target: target.current, layers: [land, raster, marker], view: new View({ projection: 'EPSG:4326', center: [(mapBounds[0] + mapBounds[2]) / 2, (mapBounds[1] + mapBounds[3]) / 2], zoom: 3, maxZoom: 28 }), controls: [] });
      map.current = instance;
      instance.getView().fit(mapBounds, { padding: [28, 28, 28, 28], maxZoom: 26 });
      instance.on('singleclick', event => {
        if (callback.current.disabled) return;
        const coordinate = transform(event.coordinate, 'EPSG:4326', data.crs);
        const pixel = genericCoordinateToPixel(coordinate, data);
        if (!pixel) return;
        point.clear(); point.addFeature(new Feature(new Point(event.coordinate)));
        callback.current.onPixel(pixel);
      });
      resize = new ResizeObserver(() => instance.updateSize()); resize.observe(target.current);
      observer = new MutationObserver(() => { land.setStyle(landStyle()); marker.setStyle(markerStyle()); });
      observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    } catch (cause) { setError(cause.message); }
    return () => { resize?.disconnect(); observer?.disconnect(); instance?.setTarget(undefined); map.current = null; };
  }, [data]);
  return <div className="stac-map-container"><div ref={target} className="stac-raster-map" role="region" aria-label={t(kind === 'coverage' ? 'Georeferenced coverage subset' : 'Georeferenced original raster')} tabIndex={0}/>
    {!error && <><div className="stac-map-controls"><Button size="icon" aria-label={t(kind === 'coverage' ? 'Fit coverage subset' : 'Fit original raster')} tooltip={t(kind === 'coverage' ? 'Fit coverage subset' : 'Fit original raster')} onClick={() => map.current?.getView().fit(extent.current, { padding: [28, 28, 28, 28], maxZoom: 26 })}><Maximize size={15}/></Button><Button size="icon" aria-label={t('Zoom in')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() + 1)}><Plus size={15}/></Button><Button size="icon" aria-label={t('Zoom out')} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() - 1)}><Minus size={15}/></Button></div><p className="stac-help">{t(kind === 'coverage' ? 'Click the coverage to read its raw pixel. The map uses the downloaded file grid and a first-band preview.' : 'Click the raster to read its original pixel. The map uses the file grid and a first-band display preview.')}</p></>}
    {error && <p className="stac-help">{t('Map placement is unavailable for this file. Read metadata and raw pixels below.')} {t(error)}</p>}
  </div>;
}
