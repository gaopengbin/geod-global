import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Crosshair, Crop, Eye, EyeOff, Layers, LoaderCircle, Maximize, Minus, MousePointer2, Plus, RefreshCw, SquareDashed, Trash2 } from 'lucide-react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import ImageLayer from 'ol/layer/Image.js';
import VectorLayer from 'ol/layer/Vector.js';
import ImageStatic from 'ol/source/ImageStatic.js';
import VectorSource from 'ol/source/Vector.js';
import Draw, { createBox } from 'ol/interaction/Draw.js';
import Feature from 'ol/Feature.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import Point from 'ol/geom/Point.js';
import { Fill, Stroke, Style, Circle as CircleStyle } from 'ol/style.js';
import { register } from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import { useRuntime } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { RecipeEditorDialog } from './processing-ui.jsx';
import { MAX_MAP_LAYERS, coordinateToPixel, mapClipRecipe, previewPixelWindow, utmDefinition, validBounds, verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';
import 'ol/ol.css';
import './workspace-map.css';

const selectionStyle = new Style({ stroke: new Stroke({ color: '#2563eb', width: 3 }), fill: new Fill({ color: 'rgba(37,99,235,0.10)' }) });
const boundaryStyle = new Style({ stroke: new Stroke({ color: '#f8fafc', width: 2, lineDash: [8, 5] }) });
const pixelStyle = new Style({ image: new CircleStyle({ radius: 6, fill: new Fill({ color: '#ffffff' }), stroke: new Stroke({ color: '#0f172a', width: 2 }) }) });

function MapError({ message, onRetry, busy }) {
  const { t } = useI18n();
  return <div className="wm-error" role="alert"><p>{t('The map request could not finish. Check the local service and retry.')}</p><details><summary>{t('Technical details')}</summary><p>{t(message)}</p></details>{onRetry && <button className="button" disabled={busy} onClick={onRetry}><RefreshCw size={14}/>{t('Retry map request')}</button>}</div>;
}

export function WorkspaceMap() {
  const { t, number } = useI18n();
  const { jobs, health, checking, refresh } = useRuntime();
  const [entries, setEntries] = useState([]);
  const [activeId, setActiveId] = useState('');
  const [candidate, setCandidate] = useState('');
  const [loadingId, setLoadingId] = useState('');
  const [loadError, setLoadError] = useState('');
  const [failedJob, setFailedJob] = useState(null);
  const [mode, setMode] = useState('inspect');
  const [cursor, setCursor] = useState(null);
  const [boundsInput, setBoundsInput] = useState(['', '', '', '']);
  const [pixel, setPixel] = useState(null);
  const [pixelBusy, setPixelBusy] = useState(false);
  const [pixelError, setPixelError] = useState('');
  const [lastPoint, setLastPoint] = useState(null);
  const [editor, setEditor] = useState(null);
  const mapTarget = useRef(null);
  const map = useRef(null);
  const overlays = useRef(null);
  const drawing = useRef(null);
  const imageLayers = useRef(new Map());
  const pendingFit = useRef(null);
  const activeRef = useRef(null);
  const handlers = useRef({});
  const mounted = useRef(true);
  const loadRequest = useRef(null);
  const pixelRequest = useRef(null);
  const pixelSequence = useRef(0);
  const layerSequence = useRef(0);
  const active = entries.find(entry => entry.job.id === activeId) || null;
  const crs = entries[0]?.metadata.crs || '';
  const bounds = useMemo(() => boundsInput.map(value => value.trim() === '' ? NaN : Number(value)), [boundsInput]);
  const pixelWindow = active && validBounds(bounds) ? previewPixelWindow(bounds, active.metadata) : null;
  const candidates = jobs.filter(job => job.status === 'succeeded' && job.assetKey === 'scl' && job.sha256 && !entries.some(entry => entry.job.id === job.id));
  const selectedCandidate = candidates.find(job => job.id === candidate) || candidates[0];
  activeRef.current = active;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; loadRequest.current?.abort(); pixelRequest.current?.abort(); pixelSequence.current++; layerSequence.current++; };
  }, []);

  const addLayer = async job => {
    if (!job || loadingId || entries.length >= MAX_MAP_LAYERS) return;
    const sequence = ++layerSequence.current;
    loadRequest.current?.abort();
    loadRequest.current = new AbortController();
    setLoadingId(job.id); setLoadError(''); setFailedJob(null);
    try {
      const data = verifiedMapMetadata(job, await runtimeRequest('raster', { id: job.id }, loadRequest.current.signal));
      if (!mounted.current || sequence !== layerSequence.current) return;
      if (crs && data.crs !== crs) throw new Error('This raster uses a different CRS. Remove the current layers before loading it.');
      pendingFit.current = job.id;
      setEntries(old => [...old, { job, metadata: data, visible: true, opacity: 1 }]);
      setActiveId(job.id);
    } catch (error) {
      if (mounted.current && sequence === layerSequence.current && error.name !== 'AbortError') { setLoadError(error.message); setFailedJob(job); }
    } finally { if (mounted.current && sequence === layerSequence.current) setLoadingId(''); }
  };

  const inspect = async coordinate => {
    const entry = activeRef.current;
    if (!entry?.visible || !coordinate || pixelBusy) return;
    setLastPoint(coordinate); setPixel(null); setPixelError('');
    if (!coordinateToPixel(coordinate, entry.metadata)) { setPixelError('Choose a point inside the active raster.'); return; }
    const sequence = ++pixelSequence.current;
    pixelRequest.current?.abort();
    pixelRequest.current = new AbortController();
    setPixelBusy(true);
    try {
      const result = verifyPixelResult(await runtimeRequest('pixel', { id: entry.job.id, x: coordinate[0], y: coordinate[1] }, pixelRequest.current.signal), entry.job, entry.metadata, coordinate);
      if (mounted.current && sequence === pixelSequence.current && activeRef.current?.job.id === entry.job.id) setPixel(result);
    } catch (error) {
      if (mounted.current && sequence === pixelSequence.current && error.name !== 'AbortError') setPixelError(error.message);
    } finally { if (mounted.current && sequence === pixelSequence.current) setPixelBusy(false); }
  };

  handlers.current = { inspect, rectangle: rectangle => { setBoundsInput(rectangle.map(value => String(Math.round(value * 1000) / 1000))); setMode('inspect'); } };

  useEffect(() => {
    if (!crs || !mapTarget.current) return;
    proj4.defs(crs, utmDefinition(crs));
    register(proj4);
    const source = new VectorSource();
    const overlay = new VectorLayer({ source, zIndex: 1000 });
    const instance = new OLMap({ target: mapTarget.current, controls: [], layers: [overlay],
      view: new View({ projection: crs, center: [500000, 0], resolution: 1000, minResolution: 0.25, maxResolution: 100000, enableRotation: false }) });
    const draw = new Draw({ type: 'Circle', geometryFunction: createBox(), stopClick: true, style: selectionStyle });
    draw.setActive(false);
    instance.addInteraction(draw);
    draw.on('drawend', event => handlers.current.rectangle(event.feature.getGeometry().getExtent()));
    let frame = null;
    instance.on('pointermove', event => {
      if (event.dragging) return;
      if (frame !== null) cancelAnimationFrame(frame);
      const point = [...event.coordinate];
      frame = requestAnimationFrame(() => { setCursor(point); frame = null; });
    });
    instance.on('singleclick', event => { if (!draw.getActive()) handlers.current.inspect([...event.coordinate]); });
    const resize = new ResizeObserver(() => instance.updateSize());
    resize.observe(mapTarget.current);
    map.current = instance; overlays.current = source; drawing.current = draw;
    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      resize.disconnect(); draw.abortDrawing();
      for (const layer of imageLayers.current.values()) layer.setSource(null);
      imageLayers.current.clear(); instance.setTarget(undefined); instance.dispose();
      map.current = null; overlays.current = null; drawing.current = null;
    };
  }, [crs]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    for (const [id, layer] of imageLayers.current) {
      if (!entries.some(entry => entry.job.id === id)) { instance.removeLayer(layer); layer.setSource(null); imageLayers.current.delete(id); }
    }
    entries.forEach((entry, index) => {
      let layer = imageLayers.current.get(entry.job.id);
      if (!layer) {
        layer = new ImageLayer({ source: new ImageStatic({ url: entry.metadata.previewDataUrl, projection: crs, imageExtent: entry.metadata.bounds, interpolate: false }) });
        imageLayers.current.set(entry.job.id, layer); instance.addLayer(layer);
      }
      layer.setVisible(entry.visible); layer.setOpacity(entry.opacity); layer.setZIndex(index);
    });
    const fitEntry = entries.find(entry => entry.job.id === pendingFit.current);
    if (fitEntry) { instance.updateSize(); instance.getView().fit(fitEntry.metadata.bounds, { padding: [32, 32, 32, 32], duration: 0 }); pendingFit.current = null; }
  }, [entries, crs]);

  useEffect(() => {
    pixelSequence.current++; pixelRequest.current?.abort(); setPixel(null); setPixelError(''); setPixelBusy(false); setLastPoint(null);
    setBoundsInput(['', '', '', '']); setMode('inspect');
  }, [activeId]);

  useEffect(() => { if (drawing.current) { drawing.current.abortDrawing(); drawing.current.setActive(mode === 'draw' && Boolean(active?.visible)); } }, [mode, active?.visible, crs]);

  useEffect(() => {
    if (!overlays.current) return;
    overlays.current.clear();
    if (active?.visible) {
      const extent = new Feature(fromExtent(active.metadata.bounds)); extent.setStyle(boundaryStyle); overlays.current.addFeature(extent);
      if (pixelWindow) { const rectangle = new Feature(fromExtent(bounds)); rectangle.setStyle(selectionStyle); overlays.current.addFeature(rectangle); }
      if (pixel) { const point = new Feature(new Point(pixel.coordinate)); point.setStyle(pixelStyle); overlays.current.addFeature(point); }
    }
  }, [active, bounds, pixel, crs]);

  const patchEntry = (id, values) => setEntries(old => old.map(entry => entry.job.id === id ? { ...entry, ...values } : entry));
  const removeLayer = id => {
    setEntries(old => old.filter(entry => entry.job.id !== id));
    if (activeId === id) setActiveId(entries.find(entry => entry.job.id !== id)?.job.id || '');
    setLoadError(''); setFailedJob(null);
  };
  const fit = entry => { if (entry && map.current) map.current.getView().fit(entry.metadata.bounds, { padding: [32, 32, 32, 32], duration: 0 }); };
  const reviewClip = () => {
    try { setEditor({ job: active.job, metadata: active.metadata, recipe: mapClipRecipe(active.job, active.metadata, bounds, `${active.job.itemId} · ${t('Map selection')}`) }); }
    catch (error) { setLoadError(error.message); setFailedJob(null); }
  };
  const formatCoordinate = coordinate => coordinate.map(value => number(value, { maximumFractionDigits: 2 })).join(' · ');
  const inspectable = Boolean(active?.visible && health && !pixelBusy);

  return <main className="wm-workspace" aria-label={t('Local raster map workspace')}>
    <aside className="wm-sidebar">
      <div className="wm-heading"><div><span className="eyebrow">{t('LOCAL RASTER MAP')}</span><h1>{t('Your layers')}</h1></div><Layers size={22}/></div>
      <p className="wm-intro">{t('Place downloaded and derived SCL rasters in their original coordinate system.')}</p>
      <div className="wm-add-layer"><label className="runtime-field">{t('Completed local raster')}<select value={selectedCandidate?.id || ''} disabled={!candidates.length || Boolean(loadingId)} onChange={event => setCandidate(event.target.value)}>{!candidates.length && <option value="">{t('No additional SCL rasters')}</option>}{candidates.map(job => <option value={job.id} key={job.id}>{job.kind === 'raster_clip' ? t('Derived') : t('Source')} · {job.itemId} · {job.id.slice(0, 8)}</option>)}</select></label><button className="button primary" disabled={!selectedCandidate || Boolean(loadingId) || entries.length >= MAX_MAP_LAYERS || !health} onClick={() => addLayer(selectedCandidate)}>{loadingId ? <LoaderCircle size={16} className="runtime-spinner"/> : <Plus size={16}/>}{t(loadingId ? 'Reading local raster…' : 'Add layer')}</button><small>{t('Up to {count} layers in the same CRS. Unload a layer to release its preview.', { count: number(MAX_MAP_LAYERS) })}</small></div>
      {!health && <div className="wm-connection" role="status"><p>{t(checking ? 'Connecting to the task service…' : 'Local task service is offline')}</p><button className="button" onClick={refresh}><RefreshCw size={14}/>{t('Reconnect task service')}</button></div>}
      {loadError && <MapError message={loadError} onRetry={failedJob ? () => addLayer(failedJob) : undefined} busy={Boolean(loadingId)}/>}
      <div className="wm-layer-list" aria-label={t('Map layers')}>{entries.map((entry, index) => <article key={entry.job.id} className={'wm-layer ' + (entry.job.id === activeId ? 'active' : '')}>
        <div className="wm-layer-title"><label><input type="radio" name="active-map-layer" checked={entry.job.id === activeId} onChange={() => setActiveId(entry.job.id)}/><span>{entry.job.kind === 'raster_clip' ? t('Derived raster') : t('Source raster')} {number(index + 1)}</span></label><button className="icon-btn" aria-label={t(entry.visible ? 'Hide layer {name}' : 'Show layer {name}', { name: entry.job.id.slice(0, 8) })} onClick={() => patchEntry(entry.job.id, { visible: !entry.visible })}>{entry.visible ? <Eye size={16}/> : <EyeOff size={16}/>}</button></div>
        <p className="wm-layer-name mono">{entry.job.itemId}</p><small className="mono">{entry.job.id.slice(0, 8)} · {entry.metadata.crs}</small><p>{number(entry.metadata.width)} × {number(entry.metadata.height)} · {entry.metadata.pixelSize.map(value => number(value)).join(' × ')} m</p>
        <label className="wm-opacity">{t('Opacity')}<input type="range" min="0" max="100" step="5" aria-label={t('Layer opacity {name}', { name: entry.job.id.slice(0, 8) })} value={Math.round(entry.opacity * 100)} onChange={event => patchEntry(entry.job.id, { opacity: Number(event.target.value) / 100 })}/><span>{Math.round(entry.opacity * 100)}%</span></label>
        <div className="wm-layer-actions"><button className="button" onClick={() => fit(entry)}><Maximize size={14}/>{t('Fit layer')}</button><button className="button" onClick={() => removeLayer(entry.job.id)}><Trash2 size={14}/>{t('Unload')}</button></div>
      </article>)}</div>
      {entries.length > 0 && <p className="wm-note">{t('The last added layer is drawn on top. The selected layer supplies pixel values and clip input.')}</p>}
      {active && <details className="wm-provenance"><summary>{t('Active raster provenance')}</summary><p className="mono">{active.job.id}</p><p>{active.job.attribution}</p><p className="mono">SHA-256 · {active.metadata.sha256}</p></details>}
    </aside>
    <section className="wm-main">
      <div className="wm-toolbar"><div className="wm-mode"><button className={'button ' + (mode === 'inspect' ? 'selected' : '')} aria-pressed={mode === 'inspect'} disabled={!active?.visible} onClick={() => setMode('inspect')}><MousePointer2 size={15}/>{t('Inspect pixels')}</button><button className={'button ' + (mode === 'draw' ? 'selected' : '')} aria-pressed={mode === 'draw'} disabled={!active?.visible} onClick={() => setMode(mode === 'draw' ? 'inspect' : 'draw')}><SquareDashed size={15}/>{t('Draw rectangle')}</button></div><div className="wm-navigation"><button className="icon-btn" aria-label={t('Zoom in')} disabled={!entries.length} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() + 1)}><Plus size={17}/></button><button className="icon-btn" aria-label={t('Zoom out')} disabled={!entries.length} onClick={() => map.current?.getView().setZoom(map.current.getView().getZoom() - 1)}><Minus size={17}/></button><button className="button" disabled={!active} onClick={() => fit(active)}><Maximize size={15}/>{t('Fit active raster')}</button></div></div>
      <div className="wm-map-container">
        <div ref={mapTarget} className={'wm-map ' + (mode === 'draw' ? 'drawing' : '')} tabIndex={0} role="application" aria-label={t('Raster map. Arrow keys pan, plus and minus zoom, Enter reads the centre pixel, Escape cancels drawing.')} onKeyDown={event => { if (event.key === 'Escape') { drawing.current?.abortDrawing(); setMode('inspect'); } if (event.key === 'Enter' && mode === 'inspect') { event.preventDefault(); inspect(map.current?.getView().getCenter()); } }}/>
        {!entries.length && <div className="wm-empty"><Layers size={36}/><h2>{t('Build a map from your local rasters')}</h2><p>{t('Add a completed SCL file to inspect real pixels, compare a crop with its source, or draw the next clip.')}</p><a className="button" href="#My%20Data">{t('Open My Data')}</a></div>}
        {entries.length > 0 && <div className="wm-map-caption"><span>{crs}</span><span>{t('Georeferenced SCL overview · nearest-neighbour display')}</span></div>}
        {mode === 'draw' && <p className="wm-draw-hint" role="status">{t('Click two opposite corners. Escape cancels. Review the rectangle before running a clip.')}</p>}
      </div>
      <div className="wm-coordinate-bar"><span className="mono">{cursor && crs ? `${crs} · ${formatCoordinate(cursor)} m` : t('Move across the map to read source coordinates')}</span><button className="button" disabled={!inspectable} onClick={() => inspect(map.current?.getView().getCenter())}><Crosshair size={14}/>{t('Read centre pixel')}</button></div>
      <div className="wm-panels">
        <section className="wm-pixel-panel" aria-label={t('Source pixel inspector')}><h2>{t('Source pixel inspector')}</h2><p>{t('Click the active raster to read its full-resolution file. The overview image is only a display preview.')}</p>
          {pixelBusy && <p className="wm-reading" role="status"><LoaderCircle size={16} className="runtime-spinner"/>{t('Reading the original pixel…')}</p>}
          {pixelError && <MapError message={pixelError} onRetry={lastPoint && active?.visible ? () => inspect(lastPoint) : undefined} busy={pixelBusy}/>}
          {pixel && <div className="wm-pixel-value" role="status"><span className="wm-swatch" style={{ background: pixel.color }}/><div><strong>{t(pixel.label)} · {number(pixel.value)}</strong><p>{t('Column {column}, row {row}', { column: number(pixel.pixel[0]), row: number(pixel.pixel[1]) })} · {t('zero-based')}</p><small className="mono">{formatCoordinate(pixel.coordinate)} m</small>{pixel.isNoData && <p>{t('This pixel is NoData.')}</p>}</div></div>}
          {active && <details className="wm-legend"><summary>{t('SCL class legend')}</summary><ul>{active.metadata.classes.map(item => <li key={item.value}><i style={{ background: item.color }}/><span>{number(item.value)} · {t(item.label)}</span></li>)}</ul></details>}
        </section>
        <section className="wm-clip-panel" aria-label={t('Map clip selection')}><h2><Crop size={16}/>{t('Map clip selection')}</h2><p>{t('Draw a rectangle or enter bounds in source metres. Reviewing opens the existing verified recipe workflow.')}</p><div className="wm-bounds">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, index) => <label className="runtime-field" key={label}>{t(label)}<input type="number" step="any" value={boundsInput[index]} disabled={!active} onChange={event => setBoundsInput(old => old.map((value, i) => i === index ? event.target.value : value))}/></label>)}</div>
          {pixelWindow && <p className="wm-window-hint">{t('Preview window: {width} × {height} pixels at {x}, {y}. The native plan verifies these bounds and the source checksum.', { width: number(pixelWindow[2]), height: number(pixelWindow[3]), x: number(pixelWindow[0]), y: number(pixelWindow[1]) })}</p>}
          {active && boundsInput.every(value => value !== '') && !pixelWindow && <p className="wm-error" role="alert">{t('Enter an ordered rectangle that overlaps the active raster.')}</p>}
          <div className="wm-clip-actions"><button className="button" disabled={!active} onClick={() => setBoundsInput(active.metadata.bounds.map(String))}>{t('Use full raster extent')}</button><button className="button primary" disabled={!active || !pixelWindow || !health} onClick={reviewClip}><Crop size={15}/>{t('Review selected clip')}</button></div>
        </section>
      </div>
      <p className="wm-footer-note">{t('Local previews only. No online basemap is requested. Display layers share their source CRS; output pixels are not reprojected.')}</p>
    </section>
    {editor && <RecipeEditorDialog sourceJob={editor.job} initialRecipe={editor.recipe} initialMetadata={editor.metadata} onClose={() => setEditor(null)}/>}
  </main>;
}
