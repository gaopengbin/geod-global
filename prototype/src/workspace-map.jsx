import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Crosshair, Crop, Eye, EyeOff, FolderOpen, Info, Layers, Maximize, Minus, MousePointer2, PanelLeftClose, PanelLeftOpen, Plus, RefreshCw, SquareDashed, Trash2, X } from 'lucide-react';
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
import { asArray } from 'ol/color.js';
import { register } from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import { useRuntime } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { jobsForProject } from './projects-client.js';
import { displayLocalPath } from './local-path.js';
import { localRasterKeys } from './reflectance.js';
import { QualityPixelDetails, QualityLegend } from './quality-ui.jsx';
import { ScientificRgbDialog } from './scientific-rgb-ui.jsx';
import { localRgbGroups, verifiedCompositeMetadata, verifyCompositePixel } from './local-rgb.js';
import { assetLabel, demTileLabel } from './providers.js';
import { modisIdentity, modisPeriodLabel } from './modis.js';
import { radarAssetIdentity } from './radar.js';
import { viirsIdentity } from './viirs.js';
import { vegetationIdentity } from './vegetation.js';
import { VegetationSelectionDetails } from './vegetation-quality-ui.jsx';
import { ScienceDetails, SciencePixelReadout } from './modis-science-ui.jsx';
import { compositePeriodLabel } from './composite-period.js';
import { AERIAL_VIEWS, aerialPixelColor, aerialTitle, aerialView, aerialViewCaption, aerialViewLabel, verifyAerialView } from './aerial.js';
import { isElevationKey, heightReference, elevationNotice, elevationProductLabel } from './elevation.js';
import { useI18n } from './i18n.jsx';
import { RecipeEditorDialog } from './processing-ui.jsx';
import { Badge, Button, Disclosure, EmptyState, Input, Modal, Select, Spinner, Surface, ResizableGroup, ResizablePanel, ResizeHandle } from './ui/index.jsx';
import { MAX_MAP_LAYERS, coordinateToPixel, mapClipRecipe, previewPixelWindow, rasterProjectionDefinition, validBounds, verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';
import 'ol/ol.css';
import './workspace-map.css';

function mapInteractionStyles(target) {
  const theme = getComputedStyle(target);
  const accent = theme.getPropertyValue('--accent').trim();
  const ink = theme.getPropertyValue('--ink').trim();
  const surface = theme.getPropertyValue('--surface').trim();
  return {
    selection: [new Style({ stroke: new Stroke({ color: ink, width: 5 }) }), new Style({ stroke: new Stroke({ color: accent, width: 3 }), fill: new Fill({ color: [...asArray(accent).slice(0, 3), 0.12] }) })],
    boundary: [new Style({ stroke: new Stroke({ color: ink, width: 4, lineDash: [8, 5] }) }), new Style({ stroke: new Stroke({ color: surface, width: 2, lineDash: [8, 5] }) })],
    pixel: [new Style({ image: new CircleStyle({ radius: 8, fill: new Fill({ color: ink }) }) }), new Style({ image: new CircleStyle({ radius: 6, fill: new Fill({ color: accent }), stroke: new Stroke({ color: surface, width: 2 }) }) })],
  };
}

function MapError({ message, onRetry, busy }) {
  const { t } = useI18n();
  return <Surface className="wm-error" role="alert"><p>{t('The map request could not finish. Check the local service and retry.')}</p><Disclosure summary={t('Technical details')}><p>{t(message)}</p></Disclosure>{onRetry && <Button disabled={busy} onClick={onRetry}><RefreshCw size={14}/>{t('Retry map request')}</Button>}</Surface>;
}

function layerName(job, date, t) {
  if (job.kind === 'raster_rgb') return job.title;
  const radar = radarAssetIdentity(job.href,job.assetKey);
  if (radar) return job.kind === 'raster_mosaic' ? `${t(assetLabel(job.assetKey))} · ${t(job.mosaic?.sources.length === 1 ? 'Area clip' : 'Mosaic and clip')}` : `${date(radar.start)} · ${radar.platform} · ${job.assetKey.toUpperCase()}`;
  if (viirsIdentity(job.itemId)) return `VIIRS ${viirsIdentity(job.itemId).platform} · ${compositePeriodLabel(job, date)}`;
  if (modisIdentity(job.itemId)) return `MODIS ${modisIdentity(job.itemId).platform} · ${modisPeriodLabel(job, date)}`;
  if (job.assetKey === 'rgb') return layerName(job.sourceJobs[0], date, t);
  if (job.assetKey === 'aerial') return job.kind === 'raster_mosaic' ? `${t(assetLabel(job.assetKey))} · ${t(job.mosaic?.sources.length === 1 ? 'Area clip' : 'Mosaic and clip')}` : aerialTitle(job.itemId,date);
  if (isElevationKey(job.assetKey)) return job.kind === 'raster_mosaic' ? `${t('Surface elevation')} · ${t(job.mosaic?.sources.length === 1 ? 'Area clip' : 'Mosaic and clip')}` : demTileLabel(job.itemId);
  if (job.kind === 'raster_prepare') {
    const item = /^S2[A-C]_MSIL2A_(\d{4})(\d{2})(\d{2})T\d{6}_N\d{4}_R\d{3}_T(\d{2}[A-Z]{3})_/.exec(job.itemId || '');
    return item ? `${date(`${item[1]}-${item[2]}-${item[3]}`)} · ${item[4]}` : job.title || job.itemId;
  }
  if (job.kind === 'raster_mosaic' && (job.mosaicOutput?.calibration || job.mosaicOutput?.quality || job.mosaicOutput?.landsatQuality)) {
    return `${t(assetLabel(job.assetKey))} · ${t(job.mosaic?.sources.length === 1 ? 'Area clip' : 'Mosaic and clip')}`;
  }
  const vegetation = vegetationIdentity(job.itemId);
  if (vegetation && job.kind === 'download') return `${vegetation.platform} · ${compositePeriodLabel(job,date)} · h${String(vegetation.h).padStart(2,'0')}v${String(vegetation.v).padStart(2,'0')} · ${job.assetKey.toUpperCase()}`;
  const scene = /^S2[A-C]_(\d{2}[A-Z]{3})_(\d{4})(\d{2})(\d{2})_/.exec(job.itemId || '');
  const landsat = /^LC0[89]_L2SP_(\d{3})(\d{3})_(\d{4})(\d{2})(\d{2})_/.exec(job.itemId || '');
  const hls = /^HLS\.L30\.T(\d{2}[A-Z]{3})\.(\d{4})(\d{3})T\d{6}\.v2\.0$/.exec(job.itemId || '');
  if (job.kind !== 'download' || (job.title && job.title !== job.itemId && !job.title.startsWith(`${job.itemId} ·`))) return job.title || job.itemId;
  if (scene) return `${date(`${scene[2]}-${scene[3]}-${scene[4]}`)} · ${scene[1]}`;
  if (landsat) return `${date(`${landsat[3]}-${landsat[4]}-${landsat[5]}`)} · ${landsat[1]}/${landsat[2]}`;
  if (hls) {
    const acquisition = new Date(Date.UTC(Number(hls[2]), 0, Number(hls[3])));
    if (Number(hls[3]) > 0 && acquisition.getUTCFullYear() === Number(hls[2])) return `${date(acquisition.toISOString())} · ${hls[1]}`;
  }
  return job.title || job.itemId;
}

function LayerDetails({ entry, onClose }) {
  const { t, date, number } = useI18n();
  const { job, metadata } = entry;
  return <Modal title={t('Layer details')} description={layerName(job, date, t)} onClose={onClose} closeLabel={t('Close')}>
    <div className="runtime-dialog-body"><dl className="wm-layer-details-grid">
      {modisIdentity(job.itemId) && <><dt>{t('Composite period')}</dt><dd>{modisPeriodLabel(job, date)}</dd></>}
      {(metadata.quality?.product === 'modis-09a1-v061' || metadata.reflectance?.product === 'modis-09a1-v061' || metadata.composite?.product === 'modis-09a1-v061') && <><dt>{t('Data licence')}</dt><dd><a href="https://lpdaac.usgs.gov/data/data-citation-and-policies/" target="_blank" rel="noreferrer">NASA LP DAAC · MOD/MYD09A1 v061</a></dd></>}
      {(metadata.vegetation || metadata.science) && <><dt>{t('Data licence')}</dt><dd><a href="https://lpdaac.usgs.gov/data/data-citation-and-policies/" target="_blank" rel="noreferrer">NASA LP DAAC · MOD13Q1 / MYD13Q1 v061</a></dd>{vegetationIdentity(job.itemId) && <><dt>{t('Composite period')}</dt><dd>{compositePeriodLabel(job,date)}</dd></>}</>}
      <dt>{t('Scene')}</dt><dd className="mono">{job.itemId}</dd>
      <dt>{t('Source grid')}</dt><dd>{metadata.crs}</dd>
      <dt>{t('Dimensions')}</dt><dd>{number(metadata.width)} × {number(metadata.height)}</dd>
      <dt>{t('Pixel size')}</dt><dd>{metadata.pixelSize.map(value => number(value, { maximumFractionDigits: metadata.elevation ? 8 : 3 })).join(' × ')} {metadata.elevation ? '°' : 'm'}</dd>
      {metadata.elevation && <><dt>{t('Height reference')}</dt><dd>{heightReference(metadata)} · {t('metres')}</dd></>}
      {metadata.elevation && <><dt>{t('Data licence')}</dt><dd><a href={job.assetKey === 'srtm' ? 'https://doi.org/10.5067/MEASURES/SRTM/SRTMGL1.003' : 'https://registry.opendata.aws/copernicus-dem/'} target="_blank" rel="noreferrer">{elevationProductLabel(metadata)}</a></dd></>}
      {job.outputPath && <><dt>{t('File')}</dt><dd className="mono">{displayLocalPath(job.outputPath)}</dd></>}
      {job.attribution && <><dt>{t('Data attribution')}</dt><dd>{job.attribution}</dd></>}
      {job.href && job.kind !== 'raster_rgb' && <><dt>{t('Source link')}</dt><dd><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd></>}
    </dl><Disclosure summary={t('File verification')}><dl className="wm-layer-details-grid">
      {(!metadata.composite || metadata.artifact) && <><dt>{t('Task ID')}</dt><dd className="mono">{job.id}</dd><dt>SHA-256</dt><dd className="mono">{metadata.artifact?.sha256 || metadata.sha256}</dd></>}
      {metadata.composite?.sources.map(source => <React.Fragment key={source.jobId}><dt>{t(assetLabel(source.band))}</dt><dd className="mono">{source.jobId}<br/>SHA-256 · {source.sha256}{job.rgbSpec?.sources.find(pin => pin.jobId === source.jobId)?.href && <><br/><a href={job.rgbSpec.sources.find(pin => pin.jobId === source.jobId).href} target="_blank" rel="noreferrer">{t('Source link')}</a></>}</dd></React.Fragment>)}
    </dl></Disclosure></div>
  </Modal>;
}

export function WorkspaceMap() {
  const { t, number, date } = useI18n();
  const { jobs, projects = [], health, checking, refresh } = useRuntime();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [requestedFile, setRequestedFile] = useState(() => new URLSearchParams(location.hash.split('?')[1] || '').get('file'));
  const [resolvedFileRequest, setResolvedFileRequest] = useState(null);
  const [requestedRgb, setRequestedRgb] = useState(() => new URLSearchParams(location.hash.split('?')[1] || '').get('rgb'));
  const [requestedProject, setRequestedProject] = useState(() => new URLSearchParams(location.hash.split('?')[1] || '').get('project'));
  const [entries, setEntries] = useState([]);
  const [activeId, setActiveId] = useState('');
  const [detailsId, setDetailsId] = useState('');
  const [candidate, setCandidate] = useState('');
  const [loadingId, setLoadingId] = useState('');
  const [viewBusy, setViewBusy] = useState(false);
  const [loadError, setLoadError] = useState('');
  const [failedJob, setFailedJob] = useState(null);
  const [mode, setMode] = useState('inspect');
  const [panel, setPanel] = useState(null);
  const [cursor, setCursor] = useState(null);
  const [boundsInput, setBoundsInput] = useState(['', '', '', '']);
  const [pixel, setPixel] = useState(null);
  const [pixelBusy, setPixelBusy] = useState(false);
  const [pixelError, setPixelError] = useState('');
  const [lastPoint, setLastPoint] = useState(null);
  const [editor, setEditor] = useState(null);
  const [rgbExport,setRgbExport] = useState(null);
  const mapTarget = useRef(null);
  const map = useRef(null);
  const overlays = useRef(null);
  const drawing = useRef(null);
  const mapStyles = useRef(null);
  const imageLayers = useRef(new Map());
  const pendingFit = useRef(null);
  const activeRef = useRef(null);
  const handlers = useRef({});
  const mounted = useRef(true);
  const loadRequest = useRef(null);
  const viewRequest = useRef(null);
  const viewSequence = useRef(0);
  const pixelRequest = useRef(null);
  const pixelSequence = useRef(0);
  const layerSequence = useRef(0);
  const active = entries.find(entry => entry.job.id === activeId) || null;
  const detailEntry = entries.find(entry => entry.job.id === detailsId);
  const crs = entries[0]?.metadata.crs || '';
  const bounds = useMemo(() => boundsInput.map(value => value.trim() === '' ? NaN : Number(value)), [boundsInput]);
  const pixelWindow = active && validBounds(bounds) ? previewPixelWindow(bounds, active.metadata) : null;
  const rgbGroups = useMemo(() => localRgbGroups(jobs), [jobs]);
  const candidates = [...rgbGroups, ...jobs.filter(job => job.status === 'succeeded' && localRasterKeys.includes(job.assetKey) && job.sha256 && !entries.some(entry => entry.job.id === job.id))].filter(job => !entries.some(entry => entry.job.id === job.id));
  const selectedCandidate = candidates.find(job => job.id === candidate) || candidates[0];
  const owningProjects = active ? projects.filter(project => jobsForProject(project, jobs).some(job => job.id === active.job.id || active.job.sourceJobs?.some(source => source.id === job.id))) : [];
  const activeProject = owningProjects.find(project => project.id === requestedProject) || owningProjects[0];
  const clipAvailable = active?.job.assetKey === 'scl';
  activeRef.current = active;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; loadRequest.current?.abort(); viewRequest.current?.abort(); viewSequence.current++; pixelRequest.current?.abort(); pixelSequence.current++; layerSequence.current++; };
  }, []);

  const addLayer = async job => {
    if (!job || loadingId || entries.length >= MAX_MAP_LAYERS) return;
    const sequence = ++layerSequence.current;
    loadRequest.current?.abort();
    loadRequest.current = new AbortController();
    setLoadingId(job.id); setLoadError(''); setFailedJob(null);
    try {
      const data = job.kind === 'raster_rgb'
        ? verifiedCompositeMetadata(job, await runtimeRequest('rgb', { id:job.id }, loadRequest.current.signal))
        : job.sourceJobs
        ? verifiedCompositeMetadata(job, await runtimeRequest('composite', { jobIds: job.sourceJobs.map(source => source.id) }, loadRequest.current.signal))
        : verifiedMapMetadata(job, await runtimeRequest('raster', { id: job.id }, loadRequest.current.signal));
      if (!mounted.current || sequence !== layerSequence.current) return;
      if (crs && data.crs !== crs) throw new Error('This raster uses a different CRS. Remove the current layers before loading it.');
      pendingFit.current = job.id;
      setEntries(old => [...old, { job, metadata: data, visible: true, opacity: 1, ...(data.aerial ? {viewPreviews:{rgb:data}} : {}) }]);
      setActiveId(job.id);
    } catch (error) {
      if (mounted.current && sequence === layerSequence.current && error.name !== 'AbortError') { setLoadError(error.message); setFailedJob(job); }
    } finally { if (mounted.current && sequence === layerSequence.current) setLoadingId(''); }
  };

  const switchAerialView = async view => {
    const entry=active;
    if (!entry?.metadata.aerial || !AERIAL_VIEWS.includes(view) || viewBusy || loadingId || view===aerialView(entry.metadata)) return;
    setLoadError(''); setFailedJob(null);
    const cached=entry.viewPreviews?.[view];
    if (cached) { setEntries(old=>old.map(item=>item.job.id===entry.job.id ? {...item,metadata:cached} : item)); return; }
    const sequence=++viewSequence.current;
    viewRequest.current?.abort(); viewRequest.current=new AbortController(); setViewBusy(true);
    try {
      const data=verifyAerialView(entry.metadata,verifiedMapMetadata(entry.job,await runtimeRequest('raster',{id:entry.job.id,aerialView:view},viewRequest.current.signal)),view);
      if (!mounted.current || sequence!==viewSequence.current) return;
      setEntries(old=>old.map(item=>item.job.id===entry.job.id ? {...item,metadata:data,viewPreviews:{...item.viewPreviews,[view]:data}} : item));
    } catch (error) {
      if (mounted.current && sequence===viewSequence.current && error.name!=='AbortError') setLoadError(error.message);
    } finally { if (mounted.current && sequence===viewSequence.current) setViewBusy(false); }
  };

  useEffect(() => {
    const restore = () => {
      const params = new URLSearchParams(location.hash.split('?')[1] || '');
      setRequestedFile(params.get('file'));
      setRequestedRgb(params.get('rgb'));
      setRequestedProject(params.get('project'));
    };
    window.addEventListener('hashchange', restore);
    return () => window.removeEventListener('hashchange', restore);
  }, []);
  // A completed job can be newer than the page's last polling snapshot.
  // Refresh before declaring a deep-linked file missing; keep a pending job
  // requested until a later snapshot makes its committed result available.
  useEffect(() => {
    if (!requestedFile || !health || jobs.some(job => job.id === requestedFile && job.status === 'succeeded' && localRasterKeys.includes(job.assetKey) && job.sha256)) return;
    let active = true;
    Promise.resolve(refresh()).then(snapshot => {
      if (active && Array.isArray(snapshot)) setResolvedFileRequest({ id: requestedFile, job: snapshot.find(job => job.id === requestedFile) });
    });
    return () => { active = false; };
  }, [requestedFile, Boolean(health), refresh]);
  useEffect(() => {
    if ((!requestedFile && !requestedRgb) || !health || loadingId) return;
    if (requestedRgb) {
      const target = rgbGroups.find(job => job.sourceJobs[0].id === requestedRgb);
      if (target) {
        setRequestedRgb(null);
        if (entries.some(entry => entry.job.id === target.id)) setActiveId(target.id);
        else { setCandidate(target.id); addLayer(target); }
      } else if (!checking) { setLoadError('Download all three original bands before opening local RGB.'); setRequestedRgb(null); }
      return;
    }
    const loaded = entries.find(entry => entry.job.id === requestedFile);
    if (loaded) { setActiveId(loaded.job.id); setRequestedFile(null); return; }
    const freshJob = resolvedFileRequest?.id === requestedFile ? resolvedFileRequest.job : null;
    const target = candidates.find(job => job.id === requestedFile)
      || (freshJob?.status === 'succeeded' && localRasterKeys.includes(freshJob.assetKey) && freshJob.sha256 ? freshJob : null);
    if (target) { setCandidate(target.id); setRequestedFile(null); addLayer(target); }
    else if (!checking && resolvedFileRequest?.id === requestedFile
      && !['queued','running'].includes((jobs.find(job => job.id === requestedFile) || freshJob)?.status)) {
      setLoadError('This file is unavailable. Choose a completed local raster.'); setRequestedFile(null);
    }
  }, [requestedFile, requestedRgb, Boolean(health), loadingId, jobs, checking, resolvedFileRequest]);

  const inspect = async coordinate => {
    const entry = activeRef.current;
    if (!entry?.visible || !coordinate || pixelBusy) return;
    setPanel('pixel');
    setLastPoint(coordinate); setPixel(null); setPixelError('');
    if (!coordinateToPixel(coordinate, entry.metadata)) { setPixelError('Choose a point inside the active raster.'); return; }
    const sequence = ++pixelSequence.current;
    pixelRequest.current?.abort();
    pixelRequest.current = new AbortController();
    setPixelBusy(true);
    try {
      const result = entry.job.kind === 'raster_rgb'
        ? verifyCompositePixel(await runtimeRequest('rgbPixel', {id:entry.job.id,x:coordinate[0],y:coordinate[1]}, pixelRequest.current.signal),entry.job,entry.metadata,coordinate)
        : entry.job.sourceJobs
        ? verifyCompositePixel(await runtimeRequest('compositePixel', { jobIds: entry.job.sourceJobs.map(source => source.id), x: coordinate[0], y: coordinate[1] }, pixelRequest.current.signal), entry.job, entry.metadata, coordinate)
        : verifyPixelResult(await runtimeRequest('pixel', { id: entry.job.id, x: coordinate[0], y: coordinate[1] }, pixelRequest.current.signal), entry.job, entry.metadata, coordinate);
      if (mounted.current && sequence === pixelSequence.current && activeRef.current?.job.id === entry.job.id) setPixel(result);
    } catch (error) {
      if (mounted.current && sequence === pixelSequence.current && error.name !== 'AbortError') setPixelError(error.message);
    } finally { if (mounted.current && sequence === pixelSequence.current) setPixelBusy(false); }
  };

  handlers.current = { inspect, rectangle: rectangle => { setBoundsInput(rectangle.map(value => String(Math.round(value * 1000) / 1000))); setMode('inspect'); setPanel('clip'); } };

  useEffect(() => {
    if (!crs || !mapTarget.current) return;
    proj4.defs(crs, rasterProjectionDefinition(crs));
    register(proj4);
    const source = new VectorSource();
    mapStyles.current = mapInteractionStyles(mapTarget.current);
    const overlay = new VectorLayer({ source, zIndex: 1000 });
    const initialBounds = entries[0].metadata.bounds;
    const initialCenter = [(initialBounds[0] + initialBounds[2]) / 2, (initialBounds[1] + initialBounds[3]) / 2];
    const geographic = crs === 'EPSG:4326';
    const instance = new OLMap({ target: mapTarget.current, controls: [], layers: [overlay],
      view: new View({ projection: crs, center: initialCenter, resolution: geographic ? 0.01 : 1000,
        minResolution: geographic ? Math.min(...entries[0].metadata.pixelSize) / 4 : 0.25,
        maxResolution: geographic ? 360 / 256 : 100000, enableRotation: false }) });
    const draw = new Draw({ type: 'Circle', geometryFunction: createBox(), stopClick: true, style: () => mapStyles.current.selection });
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
    const theme = new MutationObserver(() => {
      mapStyles.current = mapInteractionStyles(mapTarget.current);
      source.changed(); draw.getOverlay().changed();
    });
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    map.current = instance; overlays.current = source; drawing.current = draw;
    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      resize.disconnect(); theme.disconnect(); draw.abortDrawing();
      for (const layer of imageLayers.current.values()) layer.setSource(null);
      imageLayers.current.clear(); instance.setTarget(undefined); instance.dispose();
      map.current = null; overlays.current = null; drawing.current = null; mapStyles.current = null;
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
      } else if (layer.getSource()?.getUrl() !== entry.metadata.previewDataUrl) {
        layer.setSource(new ImageStatic({ url: entry.metadata.previewDataUrl, projection: crs, imageExtent: entry.metadata.bounds, interpolate: false }));
      }
      layer.setVisible(entry.visible); layer.setOpacity(entry.opacity); layer.setZIndex(index);
    });
    const fitEntry = entries.find(entry => entry.job.id === pendingFit.current);
    if (fitEntry) { instance.updateSize(); instance.getView().fit(fitEntry.metadata.bounds, { padding: [32, 32, 32, 32], duration: 240 }); pendingFit.current = null; }
  }, [entries, crs]);

  useEffect(() => {
    pixelSequence.current++; pixelRequest.current?.abort(); setPixel(null); setPixelError(''); setPixelBusy(false); setLastPoint(null);
    setBoundsInput(['', '', '', '']); setMode('inspect'); setPanel(null);
  }, [activeId]);

  useEffect(() => { if (drawing.current) { drawing.current.abortDrawing(); drawing.current.setActive(mode === 'draw' && Boolean(active?.visible)); } }, [mode, active?.visible, crs]);

  useEffect(() => {
    if (!overlays.current) return;
    overlays.current.clear();
    if (active?.visible) {
      const extent = new Feature(fromExtent(active.metadata.bounds)); extent.setStyle(() => mapStyles.current.boundary); overlays.current.addFeature(extent);
      if (pixelWindow) { const rectangle = new Feature(fromExtent(bounds)); rectangle.setStyle(() => mapStyles.current.selection); overlays.current.addFeature(rectangle); }
      if (pixel) { const point = new Feature(new Point(pixel.coordinate)); point.setStyle(() => mapStyles.current.pixel); overlays.current.addFeature(point); }
    }
  }, [active, bounds, pixel, crs]);

  const patchEntry = (id, values) => setEntries(old => old.map(entry => entry.job.id === id ? { ...entry, ...values } : entry));
  const removeLayer = id => {
    setEntries(old => old.filter(entry => entry.job.id !== id));
    if (activeId === id) setActiveId(entries.find(entry => entry.job.id !== id)?.job.id || '');
    setLoadError(''); setFailedJob(null);
  };
  const fit = entry => { if (entry && map.current) map.current.getView().fit(entry.metadata.bounds, { padding: [32, 32, 32, 32], duration: 240 }); };
  const zoom = delta => { const view = map.current?.getView(); if (view) { view.cancelAnimations(); view.animate({ zoom: view.getZoom() + delta, duration: 240 }); } };
  const reviewClip = () => {
    try { setEditor({ job: active.job, metadata: active.metadata, recipe: mapClipRecipe(active.job, active.metadata, bounds, `${active.job.itemId} · ${t('Map selection')}`) }); }
    catch (error) { setLoadError(error.message); setFailedJob(null); }
  };
  const formatCoordinate = coordinate => coordinate.map(value => number(value, { maximumFractionDigits: active?.metadata.elevation ? 6 : 2 })).join(' · ');
  const inspectable = Boolean(active?.visible && health && !pixelBusy);

  return <main className={`wm-workspace${sidebarOpen ? '' : ' wm-sidebar-collapsed'}`} aria-label={t('Local raster map workspace')}>
    <ResizableGroup storageKey="local-map" className="wm-panels" panelIds={[...(sidebarOpen ? ['layers-pane'] : []), 'local-map-pane']}>
    {sidebarOpen && <ResizablePanel id="layers-pane" defaultSize={280} minSize={240} maxSize={520} groupResizeBehavior="preserve-pixel-size"><aside className="wm-sidebar">
      <div className="wm-heading"><h1>{t('Your layers')}</h1><Button size="icon" variant="quiet" aria-label={t('Hide layers')} onClick={() => setSidebarOpen(false)}><PanelLeftClose size={18}/></Button></div>
      <div className="wm-add-layer"><label className="runtime-field">{t('Raster to add to the map')}<Select value={selectedCandidate?.id || ''} disabled={!candidates.length || Boolean(loadingId)} onChange={event => setCandidate(event.target.value)}>{!candidates.length && <option value="">{t('No additional local rasters')}</option>}{candidates.map(job => <option value={job.id} key={job.id}>{job.mosaicOutput?.calibration || job.mosaicOutput?.elevation || job.mosaicOutput?.quality || job.mosaicOutput?.landsatQuality ? layerName(job, date, t) : `${t(job.assetKey === 'rgb' ? 'Local RGB' : assetLabel(job.assetKey))} · ${layerName(job, date, t)}`}</option>)}</Select></label><Button primary disabled={!selectedCandidate || Boolean(loadingId) || entries.length >= MAX_MAP_LAYERS || !health} onClick={() => addLayer(selectedCandidate)}>{loadingId ? <Spinner size={16}/> : <Plus size={16}/>}{t(loadingId ? 'Reading local raster…' : 'Add layer')}</Button><small>{t('Up to {count} layers in the same CRS. Unload a layer to release its preview.', { count: number(MAX_MAP_LAYERS) })}</small></div>
      {!health && <Surface className="wm-connection" role="status"><p>{t(checking ? 'Connecting to the task service…' : 'Local task service is offline')}</p><Button onClick={refresh}><RefreshCw size={14}/>{t('Reconnect task service')}</Button></Surface>}
      {loadError && <MapError message={loadError} onRetry={failedJob ? () => addLayer(failedJob) : undefined} busy={Boolean(loadingId)}/>}
      <div className="wm-layer-list" aria-label={t('Map layers')}>{entries.map(entry => {
        const name = layerName(entry.job, date, t);
        const pixelSizes = entry.metadata.pixelSize.map(value => number(value, { maximumFractionDigits: entry.metadata.elevation ? 8 : 3 }));
        const resolution = pixelSizes[0] === pixelSizes[1] ? pixelSizes[0] : pixelSizes.join(' × ');
        const output = entry.job.kind === 'raster_rgb' ? 'Scientific RGB' : entry.job.kind === 'raster_prepare' ? 'Prepared source' : entry.job.kind === 'raster_mosaic' && entry.job.mosaic?.sources?.length !== 1 ? 'Mosaic output' : 'Clipped output';
        return <Surface as="article" key={entry.job.id} data-visible={entry.visible} className={'wm-layer ' + (entry.job.id === activeId ? 'active' : '')}>
          <div className="wm-layer-title"><label title={name}><Input type="radio" name="active-map-layer" checked={entry.job.id === activeId} onChange={() => setActiveId(entry.job.id)}/><span>{name}</span></label><Button size="icon" variant="quiet" aria-label={t(entry.visible ? 'Hide layer {name}' : 'Show layer {name}', { name })} tooltip={t(entry.visible ? 'Hide layer {name}' : 'Show layer {name}', { name })} onClick={() => patchEntry(entry.job.id, { visible: !entry.visible })}>{entry.visible ? <Eye size={16}/> : <EyeOff size={16}/>}</Button></div>
          <div className="wm-layer-metadata"><span>{t(entry.job.assetKey === 'rgb' ? 'Local RGB' : assetLabel(entry.job.assetKey))}</span><span>{resolution} {entry.metadata.elevation ? '°' : 'm'}</span>{!['download', 'local_composite', 'raster_rgb'].includes(entry.job.kind) && <Badge>{t(output)}</Badge>}</div>
          <div className="wm-layer-footer"><label className="wm-opacity" title={t('Opacity')}><Input className="wm-opacity-control" type="range" min="0" max="100" step="5" aria-label={t('Layer opacity {name}', { name })} value={Math.round(entry.opacity * 100)} onChange={event => patchEntry(entry.job.id, { opacity: Number(event.target.value) / 100 })}/><output>{Math.round(entry.opacity * 100)}%</output></label>
          <div className="wm-layer-actions">{entry.job.kind === 'local_composite' && <Button size="icon" variant="quiet" aria-label={t('Create scientific RGB')} tooltip={t('Create scientific RGB')} onClick={()=>setRgbExport(entry.job)}><Layers size={16}/></Button>}<Button size="icon" variant="quiet" aria-label={t('Fit layer')} tooltip={t('Fit layer')} onClick={() => fit(entry)}><Maximize size={16} aria-hidden="true"/></Button><Button size="icon" variant="quiet" aria-label={t('Layer details')} tooltip={t('Layer details')} onClick={() => setDetailsId(entry.job.id)}><Info size={16} aria-hidden="true"/></Button><Button size="icon" variant="quiet" aria-label={t('Unload')} tooltip={t('Unload')} onClick={() => removeLayer(entry.job.id)}><Trash2 size={16} aria-hidden="true"/></Button></div></div>
        </Surface>;
      })}</div>
    </aside></ResizablePanel>}
    {sidebarOpen && <ResizeHandle label={t('Resize layers panel')} hint={t('Drag to resize · Double-click to reset · Arrow keys to adjust')}/>}
    <ResizablePanel id="local-map-pane" minSize={320}>
    <section className="wm-main">
      <div className="wm-toolbar">
        <div className="wm-mode">
          {!sidebarOpen && <Button size="sm" onClick={() => setSidebarOpen(true)}><PanelLeftOpen size={16}/>{t('Your layers')}</Button>}
          <Button size="icon" aria-label={t('Inspect pixels')} tooltip={t('Inspect pixels')} selected={mode === 'inspect' && panel === 'pixel'} aria-pressed={mode === 'inspect' && panel === 'pixel'} disabled={!active?.visible} onClick={() => { setMode('inspect'); setPanel(panel === 'pixel' ? null : 'pixel'); }}><MousePointer2 size={16} aria-hidden="true"/></Button>
          {clipAvailable && <>
            <Button size="icon" aria-label={t('Draw rectangle')} tooltip={t('Draw rectangle')} selected={mode === 'draw'} aria-pressed={mode === 'draw'} disabled={!active?.visible} onClick={() => { setMode(mode === 'draw' ? 'inspect' : 'draw'); setPanel(null); }}><SquareDashed size={16} aria-hidden="true"/></Button>
            <Button size="icon" aria-label={t('Clip area')} tooltip={t('Clip area')} selected={panel === 'clip'} aria-expanded={panel === 'clip'} aria-controls="wm-tool-panel" disabled={!active?.visible} onClick={() => { setMode('inspect'); setPanel(panel === 'clip' ? null : 'clip'); }}><Crop size={16} aria-hidden="true"/></Button>
          </>}
          {activeProject && <Button asChild size="icon" tooltip={t('Open project')}><a aria-label={t('Open project')} href={`#My%20Data?project=${encodeURIComponent(activeProject.id)}`}><FolderOpen size={16} aria-hidden="true"/></a></Button>}
          {active?.metadata.aerial && <div className="wm-display-control"><Select className="wm-aerial-view" aria-label={t('Aerial display')} value={aerialView(active.metadata)} disabled={viewBusy || Boolean(loadingId)} onChange={event=>switchAerialView(event.target.value)}>{AERIAL_VIEWS.map(view=><option value={view} key={view}>{t(aerialViewLabel(view))}</option>)}</Select>{viewBusy && <Spinner size={14}/>}</div>}
        </div>
        <div className="wm-navigation">
          <Button size="icon" aria-label={t('Zoom in')} disabled={!entries.length} onClick={() => zoom(1)}><Plus size={17}/></Button>
          <Button size="icon" aria-label={t('Zoom out')} disabled={!entries.length} onClick={() => zoom(-1)}><Minus size={17}/></Button>
          <Button size="icon" aria-label={t('Fit active raster')} title={t('Fit active raster')} disabled={!active} onClick={() => fit(active)}><Maximize size={16}/></Button>
        </div>
      </div>
      <div className="wm-map-container">
        <div ref={mapTarget} className={'wm-map ' + (mode === 'draw' ? 'drawing' : '')} tabIndex={0} role="application" aria-label={t('Raster map. Arrow keys pan, plus and minus zoom, Enter reads the centre pixel, Escape cancels drawing.')} onKeyDown={event => { if (event.key === 'Escape') { drawing.current?.abortDrawing(); setMode('inspect'); } if (event.key === 'Enter' && mode === 'inspect') { event.preventDefault(); inspect(map.current?.getView().getCenter()); } }}/>
        {!entries.length && <EmptyState className="wm-empty" icon={Layers} title={t('Build a map from your local rasters')} description={t(selectedCandidate ? 'Choose a local raster, then open it on the map.' : 'Download a source raster in Explore, then open it here.')} action={selectedCandidate ? <Button primary disabled={Boolean(loadingId) || !health} onClick={() => addLayer(selectedCandidate)}>{loadingId ? <Spinner size={16}/> : <Plus size={16}/>} {t(loadingId ? 'Reading local raster…' : 'Add selected raster to map')}</Button> : <Button asChild><a href="#Explore">{t('Explore data')}</a></Button>}/>}
        {entries.length > 0 && panel !== 'pixel' && <div className="wm-map-caption"><Badge>{crs}</Badge><Badge>{t(active?.metadata.science ? 'Local science layer · display colors only · original values and units available' : active?.metadata.vegetation ? 'Local vegetation index · display colors only · original values available' : active?.metadata.quality ? 'Quality flags · original unsigned values available' : active?.metadata.radar ? 'Local radar · dB display only · original gamma0 available' : active?.metadata.composite ? active.metadata.artifact ? active.job.rgbSpec?.qualityMask ? 'Quality-masked RGB · accepted original DN available' : 'Scientific RGB · display stretch only · original DN available' : 'Local RGB · display stretch only · original DN available' : active?.metadata.elevation ? 'Local elevation · display stretch only · original heights available' : active?.metadata.reflectance ? 'Local band · display stretch only · original DN available' : active?.metadata.aerial ? aerialViewCaption(aerialView(active.metadata)) : active?.job.assetKey === 'visual' ? 'Local true-color overview · original pixels available' : 'Georeferenced SCL overview · nearest-neighbour display')}</Badge></div>}
        {mode === 'draw' && <p className="wm-draw-hint" role="status">{t('Click two opposite corners. Escape cancels. Review the rectangle before running a clip.')}</p>}
        {panel && <Surface as="aside" id="wm-tool-panel" className="wm-tool-panel" aria-label={t(panel === 'pixel' ? 'Source pixel inspector' : 'Map clip selection')}>
          <div className="wm-tool-heading"><h2>{panel === 'pixel' ? t('Source pixel inspector') : <><Crop size={16}/>{t('Map clip selection')}</>}</h2><Button size="icon" variant="quiet" aria-label={t('Close map tools')} onClick={() => setPanel(null)}><X size={16}/></Button></div>
          {panel === 'pixel' ? <section className="wm-pixel-panel" aria-label={t('Source pixel inspector')}><p>{t('Click the active raster to read its full-resolution file. The overview image is only a display preview.')}</p>
            {pixelBusy && <p className="wm-reading" role="status"><Spinner size={16}/>{t('Reading the original pixel…')}</p>}
            {pixelError && <MapError message={pixelError} onRetry={lastPoint && active?.visible ? () => inspect(lastPoint) : undefined} busy={pixelBusy}/>}
            {pixel && (active?.metadata.science ? <SciencePixelReadout pixel={pixel} metadata={active.metadata}/> : <div className="wm-pixel-value" role="status">{!active?.metadata.composite && !active?.metadata.reflectance && !active?.metadata.elevation && <span className="wm-swatch" style={{ background: active?.metadata.aerial ? aerialPixelColor(pixel,aerialView(active.metadata)) : pixel.color }}/>}<div><strong>{active?.metadata.vegetation ? pixel.isNoData ? t('NoData') : `${active.metadata.vegetation.index.toUpperCase()} ${number(pixel.indexValue,{maximumFractionDigits:4})}` : active?.metadata.quality ? `QA ${number(pixel.value)} · ${t(pixel.label)}` : active?.metadata.radar ? pixel.isNoData ? t('NoData') : `γ⁰ ${number(pixel.value,{maximumSignificantDigits:7})}` : active?.metadata.composite ? `DN · R ${number(pixel.values[0])} · G ${number(pixel.values[1])} · B ${number(pixel.values[2])}` : active?.metadata.elevation ? pixel.isNoData ? t('NoData') : `${number(pixel.value, { maximumFractionDigits: 6 })} m` : active?.metadata.reflectance ? `DN ${number(pixel.value)}` : pixel.values ? `R ${number(pixel.values[0])} · G ${number(pixel.values[1])} · B ${number(pixel.values[2])}${pixel.nearInfrared !== undefined ? ` · NIR ${number(pixel.nearInfrared)}` : ''}` : `${t(pixel.label)} · ${number(pixel.value)}`}</strong>{active?.metadata.radar && !pixel.isNoData && <p>{pixel.decibels === undefined ? t('Zero intensity has no finite dB value.') : `${number(pixel.decibels,{maximumFractionDigits:3})} dB`}</p>}{active?.metadata.composite && <p>{t('Reflectance')} · {pixel.reflectances.map((value, index) => `${['R','G','B'][index]} ${value === null ? t('NoData') : number(value, { maximumFractionDigits: 6 })}`).join(' · ')}</p>}{active?.metadata.vegetation && <p>DN {number(pixel.value)}{!pixel.isNoData && (pixel.value < -2000 || pixel.value > 10000) ? ` · ${t('Outside product valid range')}` : ''}</p>}{active?.metadata.reflectance && !pixel.isNoData && <p>{t('Reflectance')} · {number(pixel.reflectance, { maximumFractionDigits: 6 })}</p>}<p>{t('Column {column}, row {row}', { column: number(pixel.pixel[0]), row: number(pixel.pixel[1]) })} · {t('zero-based')}</p><small className="mono">{formatCoordinate(pixel.coordinate)} {active?.metadata.elevation ? '°' : 'm'}</small>{pixel.isNoData && <p>{t('This pixel is NoData.')}</p>}</div></div>)}
            {active?.metadata.science && <ScienceDetails metadata={active.metadata}/>} {active?.metadata.quality && <><QualityPixelDetails pixel={pixel}/><p>{t('Quality flags describe the product pixel. They do not change RGB or reflectance, and no quality mask is applied.')}</p><QualityLegend metadata={active.metadata}/></>}{active?.metadata.radar && <p>{t('Radar display uses an embedded overview where available. Pixel queries read original Float32 gamma0; dB is derived only for positive values. No additional speckle filtering is applied.')}</p>}{active?.metadata.composite && <p>{t(active.metadata.artifact ? active.job.rgbSpec?.qualityMask ? 'The saved RGB includes quality screening. Accepted DN are unchanged and rejected pixels are NoData. Display stretching only changes the preview.' : 'Pixels are read from the saved RGB file. The display stretch does not change DN or reflectance; no cloud or quality mask is applied.' : 'RGB uses the three local originals. The sampled 2–98 percentile stretch is display-only; original DN and reflectance stay unchanged. No cloud or quality mask is applied.')}</p>}{active?.metadata.aerial && <p>{t('Display changes keep original RGB + NIR values. No reflectance calibration or quality mask is applied.')}</p>}{active?.metadata.elevation && <p>{t(elevationNotice(active.job))}</p>}{active?.metadata.vegetation && <><p>{t(active.metadata.vegetation.qualitySelection ? 'Quality-screened vegetation index: accepted original DN retained; rejected pixels are NoData. Display colors do not change index values.' : 'Index colors use the fixed −0.2 to 1.0 display range. Original signed DN and scaled values remain unchanged, including out-of-range values. NoData is transparent. No QA or cloud mask is applied.')}</p><VegetationSelectionDetails job={active.job} metadata={active.metadata}/></>}{active?.metadata.reflectance && <p>{t('Grayscale stretch is for display only. Reflectance keeps negative and above-one values; no quality mask is applied.')}</p>}{active?.job.assetKey === 'scl' && <Disclosure className="wm-legend" summary={t('SCL class legend')}><ul>{active.metadata.classes.map(item => <li key={item.value}><i style={{ background: item.color }}/><span>{number(item.value)} · {t(item.label)}</span></li>)}</ul></Disclosure>}
          </section> : <section className="wm-clip-panel" aria-label={t('Map clip selection')}><p>{t('Draw a rectangle or enter bounds in source metres, then check the selected clip before running it.')}</p><div className="wm-bounds">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, index) => <label className="runtime-field" key={label}>{t(label)}<Input type="number" step="any" value={boundsInput[index]} disabled={!active} onChange={event => setBoundsInput(old => old.map((value, i) => i === index ? event.target.value : value))}/></label>)}</div>
            {pixelWindow && <p className="wm-window-hint">{t('Preview window: {width} × {height} pixels at {x}, {y}. The native plan verifies these bounds and the source checksum.', { width: number(pixelWindow[2]), height: number(pixelWindow[3]), x: number(pixelWindow[0]), y: number(pixelWindow[1]) })}</p>}
            {active && boundsInput.every(value => value !== '') && !pixelWindow && <p className="wm-error" role="alert">{t('Enter an ordered rectangle that overlaps the active raster.')}</p>}
            <div className="wm-clip-actions"><Button size="sm" disabled={!active} onClick={() => setBoundsInput(active.metadata.bounds.map(String))}>{t('Use full raster extent')}</Button><Button size="sm" primary disabled={!active || !pixelWindow || !health} onClick={reviewClip}><Crop size={15}/>{t('Review selected clip')}</Button></div>
          </section>}
        </Surface>}
      </div>
      <div className="wm-coordinate-bar"><span className="mono">{cursor && crs ? `${crs} · ${formatCoordinate(cursor)} ${active?.metadata.elevation ? '°' : 'm'}` : t('Move across the map to read source coordinates')}</span><Button disabled={!inspectable} onClick={() => inspect(map.current?.getView().getCenter())}><Crosshair size={14}/>{t('Read centre pixel')}</Button></div>
    </section>
    </ResizablePanel>
    </ResizableGroup>
    {detailEntry && <LayerDetails entry={detailEntry} onClose={() => setDetailsId('')}/>}
    {rgbExport && <ScientificRgbDialog group={rgbExport} jobs={jobs} projectId={activeProject?.id} onClose={()=>setRgbExport(null)} onQueued={async()=>{setRgbExport(null);await refresh();}}/>}
    {editor && <RecipeEditorDialog sourceJob={editor.job} initialRecipe={editor.recipe} initialMetadata={editor.metadata} onClose={() => setEditor(null)}/>}
  </main>;
}
