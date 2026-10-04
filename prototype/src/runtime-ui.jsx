import { VEGETATION_KEYS, vegetationIdentity, vegetationMatchesJob } from './vegetation.js';
import { VegetationSelectionDetails } from './vegetation-quality-ui.jsx';
import { MODIS_SCIENCE, MODIS_SCIENCE_KEYS } from './modis-science-layers.js';
import { scienceMatchesJob } from './modis-science.js';
import { ScienceDetails } from './modis-science-ui.jsx';
import React, { useCallback, useContext, useEffect, useId, useRef, useState } from 'react';
import { Crop, Download, FolderOpen, Info, Layers, RefreshCw, Search, X, CheckCircle2, AlertCircle, HardDrive, Scan, KeyRound } from 'lucide-react';
import { desktopAvailable, downloadableAssets, formatBytes, formatClassShare, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { ClipRasterButton, DerivedArtifactDetails } from './processing-ui.jsx';
import { ArtifactPackageButton } from './artifact-ui.jsx';
import { addProjectScenesAndQueue, createProjectAndQueue, jobsForProject, MAX_PROJECT_SCENES, projectRequest, queueProjectDownloads } from './projects-client.js';
import { displayLocalPath } from './local-path.js';
import { Badge, Button, Disclosure, EmptyState, Input, Modal, PageHeader, SegmentedControl, Select, Spinner, Surface, TaskRows } from './ui/index.jsx';
import './runtime.css';
import { FileThumbnail } from './file-thumbnail.jsx';
import { providerById, SOURCE_ASSET_KEYS, LANDSAT_BANDS, assetLabel, demTileLabel, copDemLabel } from './providers.js';
import { prepareOriginalScenes } from './protected-sources.js';
import { rgbQualityPolicyLabel, localRgbGroups } from './local-rgb.js';
import { localRasterKeys, reflectanceMatchesJob } from './reflectance.js';
import { elevationMatchesJob, isElevationKey, heightReference, elevationNotice, elevationProductLabel, elevationSourceCheck } from './elevation.js';
import { aerialMatchesJob, aerialTitle } from './aerial.js';
import { StacRasterInspection } from './stac-ui.jsx';
import { WcsRasterInspection } from './wcs-ui.jsx';
import { originalsReleased, PROTECTED_ORIGINAL_NOTICE } from './release-policy.js';

import { modisIdentity, MODIS_QUALITY_KEYS } from './modis.js';
import { LANDSAT_QUALITY_KEYS } from './landsat-quality.js';
import { compositePeriodLabel } from './composite-period.js';
import { RADAR_KEYS, radarMatchesJob, radarAssetIdentity } from './radar.js';
import { viirsIdentity, verifiedViirsScience } from './viirs.js';
import { qualityMatchesJob, QUALITY_KEYS } from './quality.js';
import { QualityLegend } from './quality-ui.jsx';

const STATUS = { queued: 'Queued', running: 'Downloading', succeeded: 'Downloaded', failed: 'Failed', cancelled: 'Cancelled', interrupted: 'Interrupted' };

export function RuntimeProvider({ children }) {
  const [jobs, setJobs] = useState([]);
  const [projects, setProjects] = useState([]);
  const [health, setHealth] = useState(null);
  const [error, setError] = useState('');
  const [checking, setChecking] = useState(true);
  const [busyJobs, setBusyJobs] = useState({});
  const [batchRetry, setBatchRetry] = useState(null);
  const mounted = useRef(true);
  const sequence = useRef(0);
  const pendingActions = useRef(new Set());
  const batchPending = useRef(false);
  const currentJobs = useRef(jobs);
  const currentHealth = useRef(health);
  const refresh = useCallback(async () => {
    const request = ++sequence.current;
    try {
      const [info, records, savedProjects] = await Promise.all([runtimeRequest('health'), runtimeRequest('list'), runtimeRequest('projects')]);
      const snapshot = Array.isArray(records) ? records : records.jobs || [];
      if (mounted.current && request === sequence.current) {
        currentHealth.current = info;
        currentJobs.current = snapshot;
        setHealth(info); setJobs(currentJobs.current); setError('');
        setProjects(savedProjects);
      }
      return snapshot;
    } catch (e) {
      if (mounted.current && request === sequence.current) { currentHealth.current = null; setHealth(null); setError(e.message); }
    } finally {
      if (mounted.current && request === sequence.current) setChecking(false);
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let active = true;
    let timer;
    const poll = async () => { await refresh(); if (active) timer = setTimeout(poll, 1800); };
    poll();
    return () => { active = false; mounted.current = false; sequence.current++; clearTimeout(timer); };
  }, [refresh]);
  const act = useCallback(async (operation, payload) => {
    const id = ['cancel', 'retry'].includes(operation) ? payload?.id : null;
    if (id && pendingActions.current.has(id)) throw new Error('A task action is already in progress.');
    if (id) {
      pendingActions.current.add(id);
      if (mounted.current) setBusyJobs(previous => ({ ...previous, [id]: true }));
    }
    try {
      const result = await runtimeRequest(operation, payload);
      await refresh();
      return result;
    } finally {
      if (id) {
        pendingActions.current.delete(id);
        if (mounted.current) setBusyJobs(previous => ({ ...previous, [id]: false }));
      }
    }
  }, [refresh]);
  const retryAll = useCallback(async () => {
    if (batchPending.current || !currentHealth.current) return;
    const candidates = [...new Map(currentJobs.current.filter(job => ['failed', 'interrupted'].includes(job.status)).map(job => [job.id, job])).values()];
    if (!candidates.length) return;
    batchPending.current = true;
    const result = { running: true, total: candidates.length, processed: 0, queued: 0, skipped: 0, failures: [] };
    const publish = () => { if (mounted.current) setBatchRetry({ ...result, failures: [...result.failures] }); };
    publish();
    try {
      for (const job of candidates) {
        if (!mounted.current || !currentHealth.current) break;
        const latest = currentJobs.current.find(item => item.id === job.id);
        if (!latest || !['failed', 'interrupted'].includes(latest.status) || pendingActions.current.has(job.id)) {
          result.skipped++;
        } else {
          try { await act('retry', { id: job.id }); result.queued++; }
          catch (error) {
            result.failures.push({ id: job.id, title: job.title || job.itemId, message: error.message });
            await refresh();
          }
        }
        result.processed++;
        publish();
      }
    } finally {
      result.running = false;
      batchPending.current = false;
      publish();
    }
  }, [act, refresh]);
  return <RuntimeContext.Provider value={{ jobs, projects, health, error, checking, refresh, act, busyJobs, batchRetry, retryAll }}>{children}</RuntimeContext.Provider>;
}

function RuntimeError({ message, summary, notice = false }) {
  const { t, locale } = useI18n();
  const disk = /^Insufficient workspace disk space for mosaic: need (\d+) bytes, available (\d+) bytes$/.exec(message || '');
  const detail = disk ? t('Insufficient workspace space: need {required}, available {available}.', { required: formatBytes(Number(disk[1]), locale), available: formatBytes(Number(disk[2]), locale) }) : t(message);
  return <div className={notice ? 'runtime-note' : 'runtime-error'} role={notice ? 'note' : 'alert'}><p>{t(summary)}</p>{message && <Disclosure summary={t('Technical details')}><p className="runtime-wrap">{detail}</p></Disclosure>}</div>;
}

function Connection({ compact = false }) {
  const { health, error, checking, refresh } = useContext(RuntimeContext);
  const { t } = useI18n();
  if (health) return <div className="runtime-connection"><Badge tone="success"><CheckCircle2 size={13}/>{t(desktopAvailable() ? 'Desktop task service connected' : 'Local task service connected')}</Badge>{!compact && <span className="runtime-path">{displayLocalPath(health.storageRoot)}</span>}</div>;
  if (checking) return <p className="runtime-connection" role="status"><Spinner size={15}/>{t('Connecting to the task service…')}</p>;
  return <Surface variant="inset" className="runtime-disconnected"><AlertCircle size={16}/><div><strong>{t('Local task service is offline')}</strong><p>{t('Open GeoD Global Desktop, or run {command} beside the browser preview.', { command: 'npm run runtime' })}</p>{error && <Disclosure summary={t('Technical details')}><small>{t(error)}</small></Disclosure>}</div><Button variant="ghost" size="icon" aria-label={t('Reconnect task service')} onClick={refresh}><RefreshCw size={16}/></Button></Surface>;
}

export function DownloadAssetButton(props) {
  const { t } = useI18n();
  const source = providerById(props.scene.provider);
  if (!originalsReleased(source)) return <div className="runtime-protected-source">
    <p className="runtime-help">{t(PROTECTED_ORIGINAL_NOTICE)}</p>
    <Button asChild size="sm" variant="secondary"><a href={`#Settings?account=${encodeURIComponent(source.account)}`}><KeyRound size={15}/>{t('Manage authorization')}</a></Button>
  </div>;
  return <OriginalDownloadWorkflow {...props}/>;
}

// Integrated workflow retained for adapter validation; protected sources are
// withheld by DownloadAssetButton in this release candidate.
export function OriginalDownloadWorkflow({ scene, scenes = [scene], areaBounds, areaPolygon, areaName, project, onProjectUpdated, onOpenProject }) {
  const { health, jobs, refresh } = useContext(RuntimeContext);
  const { t, date, number } = useI18n();
  const [open, setOpen] = useState(false);
  const [choice, setChoice] = useState('visual');
  const [scope, setScope] = useState('all');
  const [name, setName] = useState('');
  const [created, setCreated] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const mounted = useRef(true);
  const [preparedScenes, setPreparedScenes] = useState([]);
  const [preparingProducts, setPreparingProducts] = useState(false);
  const [productError, setProductError] = useState('');
  const chosenTargets = scope === 'current' ? [scene] : scenes;
  const targets = chosenTargets.map(target => preparedScenes.find(item => item.id === target.id) || target);
  const totalScenes = new Set([...(project?.scenes.map(item => item.itemId) || []), ...targets.map(item => item.id)]).size;
  const options = downloadableAssets(targets[0]).filter(item => SOURCE_ASSET_KEYS.includes(item.key)
    && targets.every(target => downloadableAssets(target).some(asset => asset.key === item.key)));
  const available = new Set(options.map(item => item.key));
  const reflectance = LANDSAT_BANDS.every(key => available.has(key));
  const vegetation = VEGETATION_KEYS.every(key => available.has(key));
  const modis = targets.every(target => target.provider === 'planetary-modis');
  const hls = targets.every(target => target.provider === 'nasa-earthdata');
  const elevation = available.has('elevation');
  const srtm = available.has('srtm');
  const viirs = available.has('viirs');
  const aerial = available.has('aerial');
  const radarKeys = RADAR_KEYS.filter(key => available.has(key));
  const safeProduct = targets.every(target => target.provider === 'copernicus');
  const accountProvider = providerById(scene.provider).account;
  const [authorization, setAuthorization] = useState(null);
  const [checkingAuthorization, setCheckingAuthorization] = useState(false);
  const [authorizationError, setAuthorizationError] = useState('');
  const authorized = !accountProvider || desktopAvailable() && ['connected', 'saved'].includes(authorization?.status)
    && Date.parse(authorization.expiresAt) > Date.now() + 30_000;
  useEffect(() => {
    if (!open || created || !accountProvider || !desktopAvailable()) return;
    const controller = new AbortController();
    setCheckingAuthorization(true); setAuthorization(null); setAuthorizationError('');
    runtimeRequest('accounts', undefined, controller.signal).then(accounts => {
      if (!controller.signal.aborted) setAuthorization(accounts.find(account => account.provider === accountProvider));
    }).catch(cause => { if (!controller.signal.aborted) setAuthorizationError(cause.message); })
      .finally(() => { if (!controller.signal.aborted) setCheckingAuthorization(false); });
    return () => controller.abort();
  }, [open, created, accountProvider]);
  useEffect(() => {
    if (!open || created || !safeProduct) return;
    const controller = new AbortController();
    setPreparingProducts(true); setProductError('');
    prepareOriginalScenes(chosenTargets, runtimeRequest, controller.signal).then(prepared => {
      if (!controller.signal.aborted) setPreparedScenes(prepared);
    }).catch(cause => { if (!controller.signal.aborted) setProductError(cause.message); })
      .finally(() => { if (!controller.signal.aborted) setPreparingProducts(false); });
    return () => controller.abort();
  }, [open, created, safeProduct, scope, scenes, scene]);
  const keys = choice === 'all-science' ? [...VEGETATION_KEYS,...MODIS_SCIENCE_KEYS] : choice === 'all-indices' ? VEGETATION_KEYS : choice === 'all-landsat' ? [...LANDSAT_BANDS,...LANDSAT_QUALITY_KEYS] : choice === 'all-modis' ? [...LANDSAT_BANDS, ...MODIS_QUALITY_KEYS] : choice === 'all-polarizations' ? radarKeys : choice === 'all-bands' ? LANDSAT_BANDS : choice === 'both' ? ['visual', 'scl'] : [choice];
  const scientificSelection = keys.some(key=>MODIS_SCIENCE[key]);
  const defaultChoice = entries => VEGETATION_KEYS.every(key => entries.every(target => downloadableAssets(target).some(asset => asset.key === key))) ? 'all-indices' : entries.every(target => RADAR_KEYS.some(key => downloadableAssets(target).some(asset => asset.key === key))) ? 'all-polarizations' : entries.every(target => downloadableAssets(target).some(asset => asset.key === 'viirs')) ? 'viirs' : entries.every(target => downloadableAssets(target).some(asset => asset.key === 'srtm')) ? 'srtm' : entries.every(target => downloadableAssets(target).some(asset => asset.key === 'aerial')) ? 'aerial' : entries.every(target => downloadableAssets(target).some(asset => asset.key === 'elevation')) ? 'elevation' : entries.every(target => target.provider === 'copernicus') ? 'product' : LANDSAT_BANDS.every(key => entries.every(target => downloadableAssets(target).some(asset => asset.key === key)))
    ? 'all-bands' : entries.every(target => downloadableAssets(target).some(asset => asset.key === 'visual')) ? 'visual' : 'scl';
  const createdJobs = created?.jobIds?.map(id => jobs.find(job => job.id === id)).filter(Boolean) || [];
  const allDone = createdJobs.length > 0 && createdJobs.every(job => job.status === 'succeeded');
  const failed = createdJobs.some(job => ['failed', 'cancelled', 'interrupted'].includes(job.status));
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const close = () => { if (!busy) { setOpen(false); setError(''); } };
  const show = () => {
    if (created) { setOpen(true); return; }
    setScope('all');
    setChoice(defaultChoice(scenes));
    setName(project?.name || `${t(areaName)} · ${srtm ? 'SRTMGL1 v003' : elevation ? copDemLabel(scenes) : scenes.length > 1 ? t('{count} scenes', { count: number(scenes.length) }) : compositePeriodLabel(scene, date)}`);
    setPreparedScenes([]); setPreparingProducts(scenes.every(target => target.provider === 'copernicus')); setProductError(''); setError(''); setOpen(true);
  };
  const start = async event => {
    event?.preventDefault();
    if (preparingProducts || productError || !authorized || !options.length || !name.trim() || totalScenes > MAX_PROJECT_SCENES || !keys.every(key => available.has(key))) return;
    setBusy(true); setError('');
    try {
      const result = created?.project
        ? await queueProjectDownloads(created.project, keys, runtimeRequest, created.itemIds)
        : project
          ? await addProjectScenesAndQueue(project.id, projectRequest({ scenes: targets, bounds: project.bounds, geometry: project.geometry, name: project.name }), keys)
          : await createProjectAndQueue(projectRequest({ scenes: targets, bounds: areaBounds, geometry: areaPolygon?.geometry || areaPolygon, name }), keys);
      if (project) onProjectUpdated?.(result.project);
      await refresh();
      if (mounted.current) setCreated({ project: result.project, itemIds: project ? targets.map(scene => scene.id) : undefined, queued: true, jobIds: result.downloads.flatMap(item => item.jobs.map(job => job.id)) });
    } catch (cause) {
      await refresh();
      if (mounted.current) {
        if (cause.project) {
          if (project) onProjectUpdated?.(cause.project);
          setCreated({ project: cause.project, itemIds: project ? targets.map(scene => scene.id) : undefined, queued: false, jobIds: (cause.downloads || []).flatMap(item => item.jobs.map(job => job.id)) });
        }
        setError(cause.message);
      }
    } finally { if (mounted.current) setBusy(false); }
  };
  const openProject = () => { if (created?.project) { close(); onOpenProject?.(created.project.id); } };
  return <>
    <Button variant="primary" disabled={!scenes.length && !created} onClick={show}><Download size={15}/>{created ? t('View project download') : project ? t('Add to this project and download · {count} scenes', { count: number(scenes.length) }) : scenes.length > 1 ? t('Create project and download · {count} scenes', { count: number(scenes.length) }) : t('Create project and download')}</Button>
    {created && <p className="runtime-project-hint">{t('Project saved: {name}', { name: created.project.name })} · {t(allDone ? 'Files ready' : failed ? 'Download needs attention' : created.queued ? 'Downloading in background' : 'Download not started')}</p>}
    {open && <Modal title={t(created ? 'Project download' : project ? 'Download into this project' : 'Create project and download')} onClose={close} closeDisabled={busy} closeLabel={t('Close download')}>
      {created ? <div className="runtime-dialog-body runtime-project-result">
        <CheckCircle2 size={22}/><h3>{created.project.name}</h3>
        <p>{t(created.queued ? allDone ? vegetation ? 'Vegetation index files are saved in this project. Open an index in Workspace to read original DN and index values.' : radarKeys.length ? 'RTC originals are saved in this project. Open a polarization in Workspace to inspect backscatter.' : viirs ? 'The original VIIRS HDF5 products are saved in this project. Open it to view file details.' : modis ? 'The MODIS COG files are saved in this project. Open them as local RGB in Workspace.' : srtm ? 'The original SRTM HGT files are saved in this project. Open them in Workspace to inspect heights.' : safeProduct ? 'The original SAFE products are saved in this project. Open it to view file details.' : reflectance ? 'The original reflectance bands are saved in this project. Open it to view file details.' : 'The files are ready in this project. Open it to inspect and clip them.' : failed ? 'A download needs attention. Open the project to retry or inspect the task.' : 'The project is saved and its source download is running in the background.' : 'The project was saved, but its download was not added to the queue. Retry below.')}</p>
        {createdJobs.length > 0 && <p className="runtime-help">{t('{done} of {total} files downloaded', { done: number(createdJobs.filter(job => job.status === 'succeeded').length), total: number(createdJobs.length) })}</p>}
        {error && <RuntimeError message={error} summary="The project was saved, but the download could not start."/>}
        <footer className="runtime-dialog-actions"><Button disabled={busy} onClick={close}>{t('Continue exploring')}</Button>{!created.queued && <Button disabled={busy || !health} onClick={start}>{t(busy ? 'Starting…' : 'Retry download')}</Button>}<Button variant="primary" disabled={busy} onClick={openProject}>{t('Open project')}</Button></footer>
      </div> : <form className="runtime-dialog-body" onSubmit={start}>
        <p className="runtime-help">{t(project ? 'Chosen scenes will be added to this project. Its clipping area stays the same and completed source downloads are reused.' : 'The chosen scenes, source links and search area will be saved together in one project.')}</p>
        {scenes.length > 1 && <label className="runtime-field">{t('Download scope')}<Select aria-label={t('Download scope')} value={scope} disabled={busy} onChange={event => {
          const next = event.target.value === 'current' ? [scene] : scenes;
          setScope(event.target.value);
          setChoice(defaultChoice(next));
        }}><option value="all">{t('All chosen scenes · {count}', { count: number(scenes.length) })}</option><option value="current">{t('Current scene only · 1')}</option></Select></label>}
        <p className="runtime-help" role="status">{t(project ? '{scenes} scenes · {files} source files to download or reuse' : '{scenes} scenes · {files} source files to download', { scenes: number(targets.length), files: number(targets.length * keys.length) })}</p>
        {totalScenes > MAX_PROJECT_SCENES && <p className="projects-error" role="alert">{t('This selection would give the project {count} scenes. Keep it within {max} scenes.', { count: totalScenes, max: MAX_PROJECT_SCENES })}</p>}
        {preparingProducts && <p className="runtime-help" role="status"><Spinner size={15}/>{t('Resolving original products from the official catalogue…')}</p>}
        {productError && <RuntimeError message={productError} summary="Original products could not be resolved. Close and retry this download."/>}
        {!options.length && !preparingProducts && !productError && <p className="projects-error" role="alert">{t('These scenes have no common supported source file type. Adjust the selection before downloading.')}</p>}
        {project ? <Surface variant="inset" className="runtime-notice"><FolderOpen size={17}/><span><strong>{project.name}</strong><br/>{t('Project after adding · {count} scenes', { count: totalScenes })}</span></Surface> : <label className="runtime-field">{t('Project name')}<Input value={name} maxLength={120} required disabled={busy} onChange={event => setName(event.target.value)}/></label>}
        <label className="runtime-field">{t('Download content')}<Select aria-label={t('Download content')} value={choice} disabled={busy} onChange={event => setChoice(event.target.value)}>
          {radarKeys.length > 0 && <option value="all-polarizations">{t('All available polarizations')} · {radarKeys.join(' / ').toUpperCase()}</option>}
          {radarKeys.map(key => <option key={key} value={key}>{t(assetLabel(key))} · Float32 · 10 m</option>)}
          {aerial && <option value="aerial">{t('Aerial RGB + NIR · original four-band GeoTIFF')}</option>}
          {viirs && <option value="viirs">{t('VIIRS 09A1 v002 · original HDF5 with science and QA layers')}</option>}
          {srtm && <option value="srtm">{t('SRTMGL1 v003 · original Int16 HGT ZIP')}</option>}
          {elevation && <option value="elevation">{t('Surface elevation · {product} · Float32 GeoTIFF', { product: copDemLabel(scenes) })}</option>}
          {available.has('visual') && <option value="visual">{t('True-color GeoTIFF · 10 m')}</option>}
          {safeProduct && <option value="product">{t('Complete Sentinel-2 L2A SAFE product · ZIP')}</option>}
          {available.has('scl') && <option value="scl">{t('SCL classification GeoTIFF · 20 m')}</option>}
          {available.has('visual') && available.has('scl') && <option value="both">{t('Both source files')}</option>}
          {modis && MODIS_QUALITY_KEYS.every(key => available.has(key)) && <option value="all-modis">{t('RGB bands + quality + pixel state · 5 files')}</option>}
          {MODIS_QUALITY_KEYS.filter(key => available.has(key)).map(key => <option key={key} value={key}>{t(assetLabel(key))} · 500 m · {key === 'modis_qc' ? 'UInt32' : 'UInt16'}</option>)}
          {targets.every(target => target.provider === 'planetary-landsat') && LANDSAT_QUALITY_KEYS.every(key => available.has(key)) && <option value="all-landsat">{t('RGB bands + pixel quality + saturation · 5 files')}</option>}
          {LANDSAT_QUALITY_KEYS.filter(key => available.has(key)).map(key => <option key={key} value={key}>{t(assetLabel(key))} · 30 m · UInt16</option>)}
          {vegetation && <option value="all-indices">{t('NDVI + EVI · 16-day · nominal 250 m · 2 COGs')}</option>}
          {vegetation && MODIS_SCIENCE_KEYS.every(key=>available.has(key)) && <option value="all-science">{t('All MODIS science layers · 12 COGs')}</option>}
          {MODIS_SCIENCE_KEYS.filter(key=>available.has(key)).map(key=><option key={key} value={key}>{t(MODIS_SCIENCE[key].label)}</option>)}
          {VEGETATION_KEYS.filter(key => available.has(key)).map(key => <option key={key} value={key}>{t(assetLabel(key))} · 250 m · Int16</option>)}
          {reflectance && <option value="all-bands">{t(modis ? 'Red + green + blue COGs · 8-day · 500 m · Int16' : hls ? 'Red + green + blue source bands · 30 m · Int16' : 'Red + green + blue source bands · 30 m · UInt16')}</option>}
          {LANDSAT_BANDS.filter(key => available.has(key)).map(key => <option key={key} value={key}>{t(assetLabel(key))} · {modis ? '500' : '30'} m · {hls || modis ? 'Int16' : 'UInt16'}</option>)}
        </Select></label>
        <Surface as="div" variant="inset" className="runtime-notice"><HardDrive size={17}/><span>{t(scientificSelection ? 'Downloads the selected MODIS science COGs with original types, units and fill values. The full NASA HDF is not included; no quality mask is applied.' : vegetation ? 'Downloads NDVI and EVI as Planetary Computer converted COGs. Original Int16 values, scale and the 16-day sinusoidal grid are retained. The full NASA HDF and QA layers are not included.' : radarKeys.length ? 'Downloads complete RTC polarization COGs, up to 4 GiB per file. Linear gamma0 and the original 10 m UTM grid are retained. Local dB stretching is display-only.' : viirs ? 'Downloads the complete original VIIRS HDF5. Prepare M5, M4 and M3 in the project for local pixels, RGB and area processing. The original QA layers are retained; no quality mask is applied.' : keys.some(key => LANDSAT_QUALITY_KEYS.includes(key)) ? 'Downloads original Landsat quality bit fields. Decode cloud, shadow, snow and saturation flags in Workspace; quality viewing does not screen RGB.' : modis && keys.some(key => MODIS_QUALITY_KEYS.includes(key)) ? 'Downloads original unsigned quality COGs alongside any selected reflectance bands. View and decode flags in Workspace; this does not apply a cloud or quality mask.' : modis ? 'Downloads NASA MODIS reflectance as Planetary Computer converted COGs, not the original HDF product. The 8-day period and sinusoidal grid are retained. View source bands or local RGB in Workspace.' : srtm ? 'Download the original SRTMGL1 HGT ZIP. Read Int16 heights above EGM96 in Workspace; clip or mosaic aligned tiles in the project.' : aerial ? 'Download the original four-band RGB + NIR aerial COG, up to 4 GiB per file. Display uses RGB; the NIR channel and NAD83 grid are retained.' : elevation ? 'Downloads the original Float32 elevation tile. Open it in Workspace to read heights in metres above EGM2008; clip or mosaic aligned tiles in the project.' : safeProduct ? 'Downloads the complete original SAFE archive, including JP2 bands and metadata. Prepare the original TCI or SCL rasters in the project to view and process them locally.' : reflectance ? hls ? 'HLS source bands keep their original Int16 values and reflectance scale. Open them in Workspace to inspect DN and reflectance. Clip or mosaic each original band in the project.' : 'Landsat source bands keep their original UInt16 values. Open them in Workspace to inspect DN and reflectance. Clip or mosaic each original band in the project.' : 'GeoD saves complete source files in the local workspace. Open the project later to inspect or clip them; downloading does not crop the source.')}</span></Surface>
        {safeProduct && available.has('product') && <p className="runtime-help">{t('Original product size · {size}', { size: formatBytes(targets.reduce((sum, target) => sum + (target.assets.product?.bytes || 0), 0)) })}</p>}
        {accountProvider && <Surface variant="inset" className="runtime-source-account">
          <p role="status">{checkingAuthorization ? <><Spinner size={15}/>{t('Checking source authorization…')}</> : t(safeProduct ? authorized ? 'Copernicus authorization is available. Product access will be checked when the task starts.' : desktopAvailable() ? 'Connect Copernicus before downloading protected original products.' : 'Open the desktop app to authorize and download protected original products.' : authorized ? 'Earthdata authorization is available. File access will be checked when the task starts.' : desktopAvailable() ? 'Connect NASA Earthdata before downloading protected files.' : 'Open the desktop app to authorize and download protected NASA files.')}</p>
          {!checkingAuthorization && !authorized && <Button asChild size="sm"><a href={`#Settings?account=${accountProvider}`}>{t('Manage authorization')}</a></Button>}
          {authorizationError && <p role="alert" className="runtime-error">{t(authorizationError)}</p>}
        </Surface>}
        <Disclosure summary={t('Source and file checks · advanced')}><dl className="runtime-details"><dt>{t('Scene')}</dt><dd className="mono runtime-wrap">{targets.map(target => target.id).join(', ')}</dd><dt>{t('Source')}</dt><dd>{[...new Set(targets.map(target => providerById(target.provider).name))].join(' / ')} · {[...new Set(targets.map(target => target.dataset || 'Sentinel-2 L2A'))].join(' / ')}</dd><dt>{t('Checks')}</dt><dd>{t(scientificSelection ? 'Official MOD13Q1/MYD13Q1 v061 identity, transfer size and SHA-256. Local inspection checks the original sample type, science layer, unit, fill, period and grid.' : vegetation ? 'Official MOD13Q1/MYD13Q1 v061 identity, byte count, GeoTIFF signature and SHA-256. Inspect local index files to read original Int16 values.' : viirs ? 'Official VIIRS identity and composite period, HDF5 signature, transfer size and SHA-256. Science layers are not decoded.' : srtm ? 'Official SRTMGL1 identity, ZIP member CRC, original HGT byte count and SHA-256. Terrain accuracy is not assessed.' : safeProduct ? 'Official product identity, byte count, SAFE ZIP directory and SHA-256. JP2 pixels are not decoded.' : elevation ? elevationSourceCheck : reflectance ? 'Transfer size, GeoTIFF signature and SHA-256; scale and offset are saved with the project.' : 'Transfer size, file signature and SHA-256. Inspect local RGB or SCL files to read their original pixels.')}</dd></dl></Disclosure>
        <Connection compact/>
        {error && <RuntimeError message={error} summary="The download could not start. Check the service connection and try again."/>}
        <footer className="runtime-dialog-actions"><Button disabled={busy} onClick={close}>{t('Cancel')}</Button><Button variant="primary" type="submit" disabled={preparingProducts || Boolean(productError) || !authorized || busy || !health || !name.trim() || !targets.length || totalScenes > MAX_PROJECT_SCENES || !keys.every(key => available.has(key))}><Download size={15}/>{t(busy ? project ? 'Adding scenes…' : 'Creating project…' : project ? 'Add scenes and start download' : 'Create project and start download')}</Button></footer>
      </form>}
    </Modal>}
  </>;
}

function RasterDialog({ job, onClose }) {
  const science = Boolean(MODIS_SCIENCE[job.assetKey]);
  const aerial = job.assetKey === 'aerial';
  const rgb = job.assetKey === 'visual' || aerial;
  const elevation = isElevationKey(job.assetKey);
  const radar = RADAR_KEYS.includes(job.assetKey);
  const reflectance = LANDSAT_BANDS.includes(job.assetKey);
  const vegetation = VEGETATION_KEYS.includes(job.assetKey);
  const quality = QUALITY_KEYS.includes(job.assetKey);
  const { t, number, locale } = useI18n();
  const legendId = useId();
  const [attempt, setAttempt] = useState(0);
  const [data, setData] = useState(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    let current = true;
    const controller = new AbortController();
    setLoading(true); setData(null); setError('');
    runtimeRequest('raster', { id: job.id }, controller.signal).then(result => {
      if ((job.sha256 && result.sha256.toLowerCase() !== job.sha256.toLowerCase()) || result.bandCount !== (aerial ? 4 : rgb ? 3 : 1)) throw new Error('The raster checksum or bands do not match this file.');
      if (radar && !radarMatchesJob(job,result)) throw new Error('The radar metadata does not match its original polarization.');
      if (aerial && !aerialMatchesJob(job,result)) throw new Error('NAIP COG metadata does not match the selected RGB + NIR source.');
      if (elevation && !elevationMatchesJob(job, result)) throw new Error('The elevation metadata does not match this source tile.');
      if (reflectance && !reflectanceMatchesJob(job, result)) throw new Error('The reflectance metadata does not match this source band.');
      if (vegetation && !vegetationMatchesJob(job,result)) throw new Error('The index metadata does not match this vegetation product.');
      if (science && !scienceMatchesJob(job,result)) throw new Error('The scientific layer metadata does not match this source.');
      if (quality && !qualityMatchesJob(job, result)) throw new Error('The quality metadata does not match this source layer.');
      if (current) setData(result);
    }).catch(e => {
      if (current && e.name !== 'AbortError') setError(e.name === 'TimeoutError' ? 'Raster inspection timed out. Close other inspections and try again.' : e.message);
    }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; controller.abort(); };
  }, [job.id, job.sha256, attempt]);
  const coordinate = value => number(value, { maximumFractionDigits: elevation ? 8 : 3 });
  return <Modal title={t('Inspect raster')} description={t(science ? 'Original scientific layer values and sinusoidal grid' : vegetation ? 'Vegetation index values and sinusoidal grid' : quality ? 'Original quality flags and file geometry' : radar ? (job.kind === 'raster_mosaic' ? 'Radar result values and file geometry' : 'Original radar backscatter and file geometry') : elevation ? 'Original surface height and geographic grid' : reflectance ? 'Original reflectance band and file geometry' : rgb ? 'True-color preview and original file geometry' : 'Scene classification from local raster pixels')} wide onClose={onClose} closeLabel={t('Close raster inspection')}>
    <div className="runtime-dialog-body">
      <p className="mono runtime-wrap runtime-raster-id">{job.itemId} · {science || vegetation || quality || radar || aerial || elevation || reflectance ? t(assetLabel(job.assetKey)) : rgb ? 'RGB' : 'SCL'}</p>
      {loading && <div className="runtime-raster-loading" role="status" aria-live="polite"><Spinner size={24}/><strong>{t('Reading the local GeoTIFF…')}</strong><p>{t('Verifying the file checksum, decoding pixels and reading spatial metadata.')}</p></div>}
      {error && <><RuntimeError message={error} summary="The raster could not be inspected. Keep the local task service running and retry. If the file changed or is missing, download it again."/><Button onClick={() => setAttempt(value => value + 1)}><RefreshCw size={15}/>{t('Retry inspection')}</Button></>}
      {data && <>
        <div className="runtime-raster-grid">
          <figure className="runtime-raster-figure"><Surface as="div" variant="inset" className="runtime-raster-image"><img src={data.previewDataUrl} width={data.previewWidth} height={data.previewHeight} alt={t(science ? 'Verified local scientific layer preview' : vegetation ? 'Verified local vegetation index preview' : quality ? 'Original MODIS quality flags from the verified local file' : radar ? 'Verified local radar preview in dB' : elevation ? 'Verified local elevation tile in grayscale' : reflectance ? 'Verified local reflectance band in grayscale' : rgb ? 'True-color pixels from the verified local file' : 'Sentinel-2 scene classification decoded from the local SCL raster')} onError={() => { setData(null); setError('The decoded raster preview could not be displayed.'); }}/></Surface><figcaption>{t(science ? 'Scientific layer preview · {width} × {height} pixels. Read original values and units in Workspace.' : vegetation ? 'Index preview · {width} × {height} pixels. Read original DN and scaled NDVI or EVI in Workspace.' : quality ? 'Quality flag preview · {width} × {height} pixels. Read and decode original unsigned values in Workspace.' : radar ? 'Radar dB preview · {width} × {height} pixels. Read original linear gamma0 in Workspace.' : elevation ? 'Grayscale elevation preview · {width} × {height} pixels. Read original heights in Workspace.' : reflectance ? 'Grayscale preview · {width} × {height} pixels. Read original DN and reflectance in Workspace.' : rgb ? 'Display preview · {width} × {height} pixels. Inspect pixels in Workspace to read the original RGB channels.' : 'Nearest-neighbor preview · {width} × {height} pixels. Colors show source classification values.', { width: number(data.previewWidth), height: number(data.previewHeight) })}</figcaption></figure>
          <section className="runtime-raster-metadata" aria-label={t('Raster metadata')}><h3>{t('Raster metadata')}</h3><dl className="runtime-details"><dt>{t('Dimensions')}</dt><dd>{t('{width} × {height} pixels', { width: number(data.width), height: number(data.height) })}</dd><dt>{t('Bands')}</dt><dd>{number(data.bandCount)}</dd><dt>{t('Data type')}</dt><dd>{data.dataType}</dd><dt>{t('Coordinate system')}</dt><dd>{data.crs}</dd><dt>{t(elevation ? 'Pixel size (degrees)' : 'Pixel size (metres)')}</dt><dd>{coordinate(data.pixelSize[0])} × {coordinate(data.pixelSize[1])}</dd><dt>{t(elevation ? 'Bounds (degrees)' : 'Bounds (metres)')}</dt><dd className="runtime-raster-bounds">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, index) => <span key={label}>{t(label)}: {coordinate(data.bounds[index])}</span>)}</dd><dt>{t('No-data value')}</dt><dd>{data.elevation?.nodataIsNan ? 'NaN' : data.nodata === null ? t('Not specified') : number(data.nodata)}</dd></dl></section>
        </div>
        {vegetation && <section className="runtime-raster-metadata"><h3>{t(assetLabel(job.assetKey))}</h3><dl className="runtime-details"><dt>{t('Index conversion')}</dt><dd>DN × {data.vegetation.scale}</dd><dt>{t('Display range')}</dt><dd>−0.2 – 1.0 · {t('Display colors only')}</dd><dt>{t('Valid display samples')}</dt><dd>{number(data.vegetation.validSampleCount)} / {number(data.vegetation.sampleCount)}</dd><dt>{t('Out-of-range display samples')}</dt><dd>{number(data.vegetation.outOfRangeSampleCount)}</dd></dl></section>}
         {reflectance && <section className="runtime-raster-metadata" aria-label={t('Reflectance display')}><h3>{t('Reflectance display')}</h3><dl className="runtime-details"><dt>{t('Reflectance conversion')}</dt><dd>DN × {data.reflectance.scale} + ({data.reflectance.offset})</dd><dt>{t('Display stretch')}</dt><dd>{number(data.reflectance.displayRange[0])} – {number(data.reflectance.displayRange[1])} DN · {t('Sampled 2–98 percentiles')}</dd><dt>{t('Valid display samples')}</dt><dd>{number(data.reflectance.validSampleCount)} / {number(data.reflectance.sampleCount)}</dd></dl></section>}
        {elevation && <section className="runtime-raster-metadata"><h3>{t('Surface elevation')}</h3><dl className="runtime-details"><dt>{t('Height reference')}</dt><dd>{heightReference(data)} · {t('metres')}</dd><dt>{t('Display stretch')}</dt><dd>{number(data.elevation.displayRange[0], {maximumFractionDigits: 3})} – {number(data.elevation.displayRange[1], {maximumFractionDigits: 3})} m · {t('Sampled 2–98 percentiles')}</dd></dl></section>}
        {radar && <section className="runtime-raster-metadata"><h3>{t('Radar backscatter')}</h3><dl className="runtime-details"><dt>{t('Polarization')}</dt><dd>{data.radar.polarization}</dd><dt>{t('Original values')}</dt><dd>γ⁰ · {t('Linear intensity')}</dd><dt>{t('Display stretch')}</dt><dd>{number(data.radar.displayRange[0],{maximumFractionDigits:2})} – {number(data.radar.displayRange[1],{maximumFractionDigits:2})} dB</dd><dt>{t('Display source')}</dt><dd>{t(data.radar.overview ? 'Embedded overview' : 'Original raster')}</dd></dl></section>}
        {quality && <QualityLegend metadata={data}/>}
        {science && <ScienceDetails metadata={data}/>}
        {!science && !vegetation && !quality && !rgb && !reflectance && !elevation && !radar && <section className="runtime-raster-legend" aria-labelledby={legendId}><h3 id={legendId}>{t('Scene classes')}</h3><p>{t('Counts cover the current raster, including no-data pixels.')}</p><ul>{data.classes.map(item => <li key={item.value}><span className="runtime-raster-swatch" style={{ backgroundColor: item.color }} aria-hidden="true"/><span className="runtime-raster-class">{number(item.value)} · {t(item.label)}</span><span className="runtime-raster-count">{t('{count} pixels', { count: number(item.count) })}<small>{formatClassShare(item.count, data.width * data.height, locale)}</small></span></li>)}</ul></section>}
        <VegetationSelectionDetails job={job} metadata={data}/><Surface as="div" variant="inset" className="runtime-notice"><CheckCircle2 size={17}/><span>{t(science ? 'Colors are display-only. Original scientific values, units and fill remain unchanged; no quality mask is applied.' : vegetation ? data.vegetation.qualitySelection ? 'Quality-screened vegetation index: accepted original DN retained; rejected pixels are NoData. Display colors do not change index values.' : 'Index colors use the fixed −0.2 to 1.0 display range. Original signed DN and scaled values remain unchanged, including out-of-range values. NoData is transparent. No QA or cloud mask is applied.' : quality ? 'The preview shows one quality field. Counts cover every valid raster pixel in this file; all bit fields remain available in Workspace. No cloud or quality mask is applied.' : radar ? 'Preview brightness uses 10 × log10(gamma0) and a sampled stretch. NoData is transparent; zero remains a valid linear value and has no finite dB. Original Float32 values stay unchanged. No additional calibration, speckle filter or quality mask is applied.' : elevation ? elevationNotice(job) : aerial ? 'RGB is shown for display; the fourth band is near-infrared, not transparency. Read all four original channel values in Workspace. No reflectance calibration or quality mask is applied.' : reflectance ? 'The preview uses a sampled grayscale stretch for display only. Original DN and unbounded calibrated reflectance are read from the verified full-resolution file. No quality mask is applied.' : rgb ? 'The local file checksum and original geometry are verified. Display previews may use an embedded overview; source RGB values are read from the full-resolution file.' : job.kind === 'raster_clip' ? 'The preview and counts come from the derived GeoTIFF after SHA-256 verification. Source pixels were clipped without resampling or reprojection.' : 'The preview and class counts were decoded from this local file after SHA-256 verification. Source classifications are not an independent accuracy assessment. No clipping or reprojection is applied.')}</span></Surface>
        <Disclosure className="runtime-raster-provenance" summary={t('File and provenance')}><dl className="runtime-details"><dt>{t('File')}</dt><dd className="mono runtime-wrap">{displayLocalPath(job.outputPath)}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{data.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd>{elevation && <><dt>{t('Data licence')}</dt><dd><a href={job.assetKey === 'srtm' ? 'https://doi.org/10.5067/MEASURES/SRTM/SRTMGL1.003' : 'https://registry.opendata.aws/copernicus-dem/'} target="_blank" rel="noreferrer">{elevationProductLabel(data)}</a></dd></>}</dl></Disclosure>
      </>}
    </div><footer className="runtime-dialog-actions"><Button onClick={onClose}>{t('Close')}</Button></footer>
  </Modal>;
}

export function RuntimeJobRows({ jobs, library = false, areaBounds, areaPolygon, projectName, projectId }) {
  const { act, health, projects = [], jobs: allJobs = jobs, busyJobs = {}, batchRetry } = useContext(RuntimeContext);
  const { t, locale, number, date } = useI18n();
  const [busy, setBusy] = useState({});
  const [errors, setErrors] = useState({});
  const [expanded, setExpanded] = useState({});
  const [inspect, setInspect] = useState(null);
  const [stacInspect, setStacInspect] = useState(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const run = async (job, operation) => {
    setBusy(previous => ({ ...previous, [job.id]: true }));
    setErrors(previous => ({ ...previous, [job.id]: '' }));
    try { await act(operation, { id: job.id }); }
    catch (e) { if (mounted.current) { setErrors(previous => ({ ...previous, [job.id]: e.message })); setExpanded(previous => ({ ...previous, [job.id]: true })); } }
    finally { if (mounted.current) setBusy(previous => ({ ...previous, [job.id]: false })); }
  };
  const bytes = value => Number.isFinite(value) && value >= 0 ? formatBytes(value, locale) : t('Unknown size');
  const rgbGroups = localRgbGroups(allJobs);
  const items = jobs.map(job => {
    const coverage = job.assetKey === 'wcs_coverage' && Boolean(job.wcsSource);
    const customRaster = coverage || job.assetKey === 'stac_asset' && Boolean(job.stacSource);
    const rgbGroup = rgbGroups.find(group => group.sourceJobs.some(source => source.id === job.id));
    const active = ['queued', 'running'].includes(job.status);
    const derived = job.kind !== 'download';
    const scientificRgb = job.kind === 'raster_rgb';
    const mosaic = job.kind === 'raster_mosaic';
    const prepared = job.kind === 'raster_prepare';
    const projectClip = mosaic && job.mosaic?.sources?.length === 1;
    const notice = ['interrupted', 'cancelled'].includes(job.status);
    const jobError = typeof job.error === 'string' ? job.error : job.error?.message;
    const previousMosaicLimit = mosaic && /^Project output exceeds (?:8 million pixels|128 MiB)/.test(jobError || '');
    const failureSummary = job.status === 'interrupted'
      ? derived ? 'This task was interrupted when the app stopped. Projects and completed files are kept. Retry starts the task from the beginning.' : 'This download was interrupted. Retry checks saved bytes and resumes when the source supports recovery.'
      : job.status === 'cancelled' ? 'This task was cancelled. Retry starts it from the beginning.'
        : previousMosaicLimit ? 'This task hit the previous size limit. Retry with block processing; completed source files will be reused.'
          : mosaic && jobError?.startsWith('Insufficient workspace disk space') ? 'Free some space in the workspace drive, then retry. Completed source files will be reused.'
          : mosaic ? 'Mosaic processing did not complete. Check the source files and retry.'
          : prepared ? job.viirsPrepare ? 'VIIRS preparation did not complete. Check the original product and retry.' : 'SAFE preparation did not complete. Check the original product and retry.'
          : scientificRgb ? 'RGB creation did not complete. Check all three source bands and retry.'
          : derived ? 'Raster processing did not complete. Check the source file and retry the clip.'
            : 'This download did not complete. Retry checks saved bytes and resumes when the source supports recovery.';
    const canInspect = job.status === 'succeeded' && localRasterKeys.includes(job.assetKey)
      && (/^image\/(?:tiff|geotiff)(?:;|$)/i.test(job.mediaType || '') || job.assetKey === 'srtm' && job.mediaType === 'application/zip');
    const owner = projects.find(project => jobsForProject(project, allJobs).some(item => item.id === job.id));
    const radar = RADAR_KEYS.includes(job.assetKey);
    const reflectance = LANDSAT_BANDS.includes(job.assetKey);
    const vegetation = VEGETATION_KEYS.includes(job.assetKey);
    const science = MODIS_SCIENCE[job.assetKey];
    const outputCalibration = job.mosaicOutput?.calibration;
    const rasterBand = outputCalibration ? { dataType: science?.dataType || (outputCalibration.signed ? 'int16' : 'uint16'), ...outputCalibration, spatialResolution: job.mosaicOutput.pixelSize[0] } : owner?.scenes.find(scene => scene.itemId === job.itemId && scene.assets?.[job.assetKey]?.href === job.href)?.assets[job.assetKey]?.rasterBand;
    const progress = Number.isFinite(job.totalBytes) && job.totalBytes > 0 && Number.isFinite(job.bytesDownloaded)
      ? Math.max(0, Math.min(100, job.bytesDownloaded / job.totalBytes * 100)) : null;
    const resumed = !derived && job.transfer?.mode === 'resumed' && Number.isSafeInteger(job.transfer.resumedBytes)
      && job.transfer.resumedBytes > 0 && job.transfer.resumedBytes <= job.bytesDownloaded;
    const hls = !customRaster && (outputCalibration?.product === 'hls-l30-v2' || /^HLS\.L30\./.test(job.itemId || ''));
    const modis = customRaster ? null : modisIdentity(job.itemId) || vegetationIdentity(job.itemId);
    const viirs = customRaster ? null : viirsIdentity(job.itemId);
    const viirsOriginal = viirs && job.assetKey === 'viirs';
    const viirsScience = verifiedViirsScience(job);
    const safeProduct = job.assetKey === 'product';
    const elevation = isElevationKey(job.assetKey);
    const quality = QUALITY_KEYS.includes(job.assetKey);
    const type = MODIS_SCIENCE[job.assetKey] ? `${t(assetLabel(job.assetKey))} · GeoTIFF · ${{int8:'Int8',int16:'Int16',uint16:'UInt16'}[MODIS_SCIENCE[job.assetKey].dataType]}` : vegetation ? `${t(assetLabel(job.assetKey))} · GeoTIFF · Int16` : quality ? `${t(assetLabel(job.assetKey))} · GeoTIFF · ${job.assetKey === 'modis_qc' ? 'UInt32' : 'UInt16'}` : scientificRgb ? t('Scientific RGB · GeoTIFF') : coverage ? t('Coverage subset · GeoTIFF') : customRaster ? t('Custom raster · original file') : radar ? `${t(assetLabel(job.assetKey))} · Float32` : viirsOriginal ? t('VIIRS original product · HDF5') : prepared ? `${t(assetLabel(job.assetKey))} · GeoTIFF${job.viirsPrepare ? ' · Int16' : ''}` : safeProduct ? t('Sentinel-2 SAFE product · ZIP') : mosaic ? t(projectClip ? 'Project clip · GeoTIFF' : 'Project mosaic · GeoTIFF') : derived ? t('Clipped raster · GeoTIFF') : job.assetKey === 'aerial' ? 'NAIP · RGB + NIR · GeoTIFF' : job.assetKey === 'srtm' ? 'SRTMGL1 v003 · HGT ZIP · Int16' : elevation ? `${t('Surface elevation')} · GeoTIFF · Float32` : reflectance ? `${t(assetLabel(job.assetKey))} · GeoTIFF · ${hls || modis || viirs || outputCalibration?.signed ? 'Int16' : 'UInt16'}` : t(job.assetKey === 'scl' ? 'SCL raster · GeoTIFF' : job.assetKey === 'visual' ? 'True-color image · GeoTIFF' : 'Preview image · JPEG');
    const landsatId = /^LC0[89]_L2SP_(\d{3})(\d{3})_(\d{4})(\d{2})(\d{2})_/.exec(job.itemId || '');
    const sceneId = /^S2[A-C]_(\d{2}[A-Z]{3})_(\d{4})(\d{2})(\d{2})_/.exec(job.itemId || '');
    const safeId = /^S2[A-C]_MSIL2A_(\d{4})(\d{2})(\d{2})T\d{6}_N\d{4}_R\d{3}_T(\d{2}[A-Z]{3})_/.exec(job.itemId || '');
    const radarIdentity = radarAssetIdentity(job.href,job.assetKey);
    const readableTitle = customRaster ? job.title || job.itemId : radarIdentity && !derived ? `${date(radarIdentity.start)} · ${radarIdentity.platform} · ${job.assetKey.toUpperCase()}` : viirs ? `VIIRS ${viirs.platform} · ${compositePeriodLabel(job,date)}` : modis ? `MODIS ${modis.platform}${vegetation ? ` · h${String(modis.h).padStart(2,'0')}v${String(modis.v).padStart(2,'0')}` : ''} · ${compositePeriodLabel(job,date)}` : job.assetKey === 'aerial' && !derived ? aerialTitle(job.itemId,date) : elevation && !mosaic ? demTileLabel(job.itemId) : !derived && sceneId && (!job.title || job.title === job.itemId || job.title.startsWith(`${job.itemId} ·`))
      ? `${date(`${sceneId[2]}-${sceneId[3]}-${sceneId[4]}`)} · ${sceneId[1]}` : !derived && landsatId ? `${date(`${landsatId[3]}-${landsatId[4]}-${landsatId[5]}`)} · ${landsatId[1]}/${landsatId[2]}` : job.title || job.itemId;
    return {
      id: job.id,
      title: scientificRgb ? job.title || t('Scientific RGB') : customRaster ? readableTitle : projectName && mosaic ? `${t(assetLabel(job.assetKey))} · ${t(projectClip ? 'Area clip' : 'Mosaic and clip')}` : safeId ? `${date(`${safeId[1]}-${safeId[2]}-${safeId[3]}`)} · ${safeId[4]}` : readableTitle,
      description: library ? type : scientificRgb ? t('Create scientific RGB') : coverage ? t('Coverage subset download') : customRaster ? t('Original raster download') : viirsOriginal ? t('VIIRS original product download') : mosaic ? t(projectClip ? 'Project clip task' : 'Project mosaic task') : prepared ? t(job.viirsPrepare ? 'Prepare VIIRS band' : 'Prepare SAFE raster') : safeProduct ? t('Original SAFE product download') : job.assetKey === 'aerial' ? t('Aerial imagery download') : elevation ? t('Elevation download') : radar ? `RTC · ${job.assetKey.toUpperCase()}` : MODIS_SCIENCE[job.assetKey] ? t(assetLabel(job.assetKey)) : quality ? t('Quality layer download') : vegetation ? t(assetLabel(job.assetKey)) : reflectance ? t(assetLabel(job.assetKey)) : t(derived ? 'Raster clip task' : job.assetKey === 'scl' ? 'SCL download' : job.assetKey === 'visual' ? 'True-color download' : 'Preview download'),
      icon: scientificRgb ? Layers : derived ? Crop : Download,
      preview: library && (customRaster || localRasterKeys.includes(job.assetKey)) ? <FileThumbnail key={`${job.id}:${job.sha256}`} job={job}/> : null,
      status: job.status,
      statusTone: library ? 'neutral' : undefined,
      statusLabel: t(library ? scientificRgb ? 'Scientific RGB' : prepared ? 'Prepared source' : mosaic && !projectClip ? 'Mosaic output' : derived ? 'Clipped output' : 'Source file' : derived && job.status === 'succeeded' ? 'Generated' : derived && job.status === 'running' ? 'Processing' : STATUS[job.status] || job.status),
      progress: mosaic || prepared || scientificRgb ? progress : derived ? null : progress,
      progressLabel: t(derived ? 'Processing progress' : 'Download progress'),
       meta: library ? bytes(job.bytesDownloaded) : mosaic && job.status === 'running' ? t('{progress}% completed', { progress: number(Math.round(progress || 0)) }) : derived ? null : <>{bytes(job.bytesDownloaded)}{active && job.totalBytes ? ` / ${bytes(job.totalBytes)}` : ''}{resumed ? ` · ${t('Resumed')}` : ''}</>,
      details: library ? <Disclosure className="runtime-file-details" icon={Info} summary={t('File details and provenance')}>
        <div className="runtime-file-inspect-actions">
          {customRaster && job.status === 'succeeded' && <Button size="sm" disabled={!health} onClick={() => setStacInspect(job)}><Scan size={16}/>{t(coverage ? 'Inspect coverage subset' : 'Inspect original raster')}</Button>}
          {scientificRgb && <Button asChild size="sm"><a href={`#Workspace?file=${encodeURIComponent(job.id)}`}><Layers size={16}/>{t('Open in workspace')}</a></Button>}
          {canInspect && !scientificRgb && <Button size="sm" disabled={!health} onClick={() => setInspect(job)}><Scan size={16}/>{t('Inspect raster')}</Button>}
          {!derived && !projectName && job.assetKey === 'scl' && <ClipRasterButton job={job} areaBounds={areaBounds} areaPolygon={areaPolygon}/>}
          {desktopAvailable() && <Button size="sm" disabled={!health || busy[job.id]} onClick={() => run(job, 'reveal')}><FolderOpen size={15}/>{t('Show in folder')}</Button>}
        </div>
        {derived && !mosaic && !prepared && !scientificRgb && <DerivedArtifactDetails job={job}/>}
        {scientificRgb && <dl className="runtime-details">{job.rgbSpec.sources.map(source=><React.Fragment key={source.jobId}><dt>{t(assetLabel(source.band))}</dt><dd className="runtime-wrap"><a href={source.href} target="_blank" rel="noreferrer">{source.attribution}</a><br/><span className="mono">SHA-256 · {source.sha256}</span></dd></React.Fragment>)}</dl>}
        {scientificRgb && job.rgbSpec.qualityMask && <dl className="runtime-details"><dt>{t('Quality screening')}</dt><dd>{t(rgbQualityPolicyLabel(job.rgbSpec.qualityMask.policy))}{job.rgbSpec.qualityMask.excludeSnow?` · ${t('Also exclude snow and ice')}`:''}</dd><dt>{t(job.rgbSpec.qualityMask.coupled?'Pixels without qualified RGB':'Quality-rejected pixels')}</dt><dd>{number(job.rgbOutput?.qualityMask?.rejectedPixels || 0)}</dd><dt>{t('Valid pixels removed')}</dt><dd>{number(job.rgbOutput?.qualityMask?.removedValidPixels || 0)}</dd>{job.rgbSpec.qualityMask.sources.map(source=><React.Fragment key={source.jobId}><dt>{t(assetLabel(source.band))}</dt><dd className="runtime-wrap"><a href={source.href} target="_blank" rel="noreferrer">{source.attribution}</a><br/><span className="mono">SHA-256 · {source.sha256}</span></dd></React.Fragment>)}</dl>}
        {scientificRgb && job.rgbSpec.qualityMask?.coupled && <Disclosure summary={t('Original scene selection')}><p>{t('Overlaps use the newest qualified complete RGB scene. An older qualified scene fills flagged or incomplete newer pixels.')}</p><dl className="runtime-details"><dt>{t('Quality fallback pixels')}</dt><dd>{number(job.rgbOutput?.qualityMask?.coupled?.fallbackPixels || 0)}</dd></dl>{job.rgbSpec.qualityMask.coupled.scenes.map((scene,index)=><React.Fragment key={scene.sources[0].jobId}><p className="mono runtime-wrap">{scene.sources[0].itemId} · {t('{count} retained pixels',{count:number(job.rgbOutput?.qualityMask?.coupled?.sceneValidPixels[index] || 0)})}</p><dl className="runtime-details">{scene.sources.map(source=><React.Fragment key={source.jobId}><dt>{t(assetLabel(source.band))}</dt><dd className="runtime-wrap"><a href={source.href} target="_blank" rel="noreferrer">{t('Original source file')}</a><br/><span className="mono">SHA-256 · {source.sha256}</span></dd></React.Fragment>)}</dl></React.Fragment>)}</Disclosure>}
        {mosaic && <p>{t('{count} verified sources · {width} × {height} pixels · {crs}', { count: job.mosaicOutput?.sourceCount || 0, width: job.mosaicOutput?.width || 0, height: job.mosaicOutput?.height || 0, crs: job.mosaicOutput?.crs || '' })}</p>}
        {job.mosaic?.viSelection && <VegetationSelectionDetails job={job}/>}
        {rasterBand && <dl className="runtime-details"><dt>{t('Data type')}</dt><dd>{rasterBand.dataType}</dd><dt>{t(science ? 'Original value conversion' : 'Reflectance conversion')}</dt><dd className="mono">DN × {rasterBand.scale} + ({rasterBand.offset})</dd>{science && <><dt>{t('Unit')}</dt><dd>{t(science.unit)}</dd></>}<dt>{t('No-data value')}</dt><dd>{rasterBand.nodata}</dd><dt>{t('Spatial resolution')}</dt><dd>{rasterBand.spatialResolution} m</dd></dl>}
        {viirsScience && <dl className="runtime-details"><dt>{t('Checked bands')}</dt><dd>M5 / M4 / M3 · Int16</dd><dt>{t('Source grid')}</dt><dd>{viirsScience.width} × {viirsScience.height} · {viirsScience.crs}</dd><dt>{t('Reflectance conversion')}</dt><dd className="mono">DN × 0.0001</dd><dt>{t('No-data value')}</dt><dd>-28672</dd><dt>{t('Quality masks')}</dt><dd>{t('Not applied; QA layers remain in the original HDF5.')}</dd></dl>}
        <dl className="runtime-details"><dt>{t(customRaster ? 'Item identifier' : 'Scene')}</dt><dd className="mono runtime-wrap">{job.itemId}</dd><dt>{t('File')}</dt><dd className="mono runtime-wrap">{displayLocalPath(job.outputPath)}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{job.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd><dt>{t('Updated')}</dt><dd>{date(job.updatedAt)}</dd><dt>{t('Validation')}</dt><dd>{t(science ? 'Original science DN, type, unit, fill and sinusoidal grid are retained. Display counts sample the preview; no quality mask is applied.' : scientificRgb ? job.rgbSpec.qualityMask ? 'Pinned product-specific quality rules applied; accepted RGB DN retained, rejected pixels set to NoData; every output sample checked' : 'Original 16-bit RGB values, calibration, NoData and grid are retained. Every output sample was checked against the pinned bands.' : coverage ? 'This GeoTIFF is a server-generated WCS coverage subset. Inspect the downloaded file for its actual grid, bands and raw samples.' : customRaster ? 'Original asset bytes and SHA-256 are retained. Inspect the local file for its data types, bands, geometry and raw pixel values.' : radar && derived ? 'RTC project result: Float32 linear gamma0, polarization, grid and NoData retained; every output sample checked. No averaging, calibration or speckle filtering is applied.' : radar ? 'Original RTC polarization file: transfer size, signature and SHA-256 checked. Inspect local gamma0 and file geometry in Workspace; the application does not independently assess the provider terrain correction.' : viirsOriginal ? viirsScience ? 'Embedded VIIRS v002 identity, period, grid and M5/M4/M3 samples checked. Other science and QA layers remain in the original file; no quality mask is applied.' : 'Official VIIRS identity and composite period, HDF5 signature, transfer size and SHA-256. Science layers are not decoded.' : prepared ? job.viirsPrepare ? 'Pinned VIIRS HDF5 and every prepared Int16 sample checked. Original grid, NoData and reflectance conversion are retained; no QA mask is applied.' : 'Original SAFE checksum and ZIP CRC verified. JP2 samples and XML geometry are retained in the prepared GeoTIFF.' : safeProduct ? 'Official product identity, byte count, SAFE ZIP directory and SHA-256. JP2 pixels are not decoded.' : job.assetKey === 'srtm' && !derived ? 'Official SRTMGL1 identity, ZIP member CRC, original HGT byte count and SHA-256. Terrain accuracy is not assessed.' : elevation ? derived ? 'Processed elevation: original Float32 heights, Point grid and EGM2008 retained; output samples checked. Gaps and masks are NaN NoData.' : elevationSourceCheck : vegetation ? job.mosaic?.viSelection ? 'Processed vegetation index: NDVI, EVI and same-scene QA selected together; original signed DN, scale, NoData and grid retained.' : derived ? 'Processed vegetation index: original signed DN, scale, NoData and grid are retained. Newest valid composites win overlaps; no QA mask is applied.' : 'Original vegetation index: verified signed DN, scale and sinusoidal grid are retained. No QA mask is applied.': reflectance && derived ? 'Processed band: original DN, NoData and reflectance conversion are retained. Inspect the result in Workspace.' : quality ? 'Original quality bit fields: open in Workspace to check the full-resolution grid, counts and raw flags.' : reflectance ? 'Original source band: inspect the verified full-resolution DN, reflectance and file geometry in Workspace.' : derived ? 'Generated locally from the checked source clip. Inspect the result to read output pixels and spatial metadata.' : 'Transfer size and file signature checked. Use Inspect raster on an SCL file to decode pixels and read spatial metadata.')}</dd></dl>
        {['raster_clip','raster_rgb'].includes(job.kind) && <ArtifactPackageButton job={job}/>}
        {derived && !desktopAvailable() && <Button onClick={() => { const link = document.createElement('a'); link.href = `http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/file`; link.download = `${job.id}.tif`; link.click(); }}><Download size={15}/>{t('Download result GeoTIFF')}</Button>}
        {(mosaic || scientificRgb) && !desktopAvailable() && <Button onClick={() => { const link = document.createElement('a'); link.href = `http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/metadata`; link.download = `${job.id}.metadata.json`; link.click(); }}><Download size={15}/>{t('Download provenance JSON')}</Button>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
      </Disclosure> : <Disclosure className="runtime-task-details" icon={Info} summary={t('Task details')} open={Boolean(expanded[job.id])} onOpenChange={open => setExpanded(previous => ({ ...previous, [job.id]: open }))}>
        {(job.error || notice) && <RuntimeError message={typeof job.error === 'string' ? job.error : job.error?.message} notice={notice} summary={failureSummary}/>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
        <dl className="runtime-details"><dt>{t(customRaster ? 'Item identifier' : 'Scene')}</dt><dd className="mono runtime-wrap">{job.itemId}</dd><dt>{t('Data file')}</dt><dd>{type}</dd>{job.validation && <><dt>{t('Validation')}</dt><dd className="runtime-wrap">{t(job.validation)}</dd></>}</dl>
        {!derived && job.transfer && <dl className="runtime-details"><dt>{t('Download recovery')}</dt><dd>{t(resumed ? 'Resumed {size} of verified source bytes.' : job.transfer.mode === 'restarted' ? 'Started again because saved bytes or the source response did not match.' : 'Downloaded from the beginning.', { size: bytes(job.transfer.resumedBytes || 0) })}</dd></dl>}
      </Disclosure>,
      actions: library ? <>
        {customRaster && job.status === 'succeeded' && <Button size="icon" variant="secondary" disabled={!health} aria-label={t(coverage ? 'Inspect coverage subset' : 'Inspect original raster')} tooltip={t(coverage ? 'Inspect coverage subset' : 'Inspect original raster')} onClick={() => setStacInspect(job)}><Scan size={16}/></Button>}
        {canInspect && <Button asChild size="icon" variant="secondary" tooltip={t(rgbGroup ? 'Open local RGB' : 'Open in workspace')}><a aria-label={t(rgbGroup ? 'Open local RGB' : 'Open in workspace')} href={`#Workspace?${rgbGroup ? `rgb=${encodeURIComponent(rgbGroup.sourceJobs[0].id)}` : `file=${encodeURIComponent(job.id)}`}${projectId || owner ? `&project=${encodeURIComponent(projectId || owner.id)}` : ''}`}><Layers size={16} aria-hidden="true"/></a></Button>}
        {owner && !projectName && <Button asChild size="icon" variant="secondary" tooltip={t('Open project')}><a aria-label={t('Open project')} href={`#My%20Data?project=${encodeURIComponent(owner.id)}`}><FolderOpen size={16} aria-hidden="true"/></a></Button>}
      </> : active || ['failed', 'cancelled', 'interrupted'].includes(job.status) ? <>
        {active && <Button size="icon" variant="secondary" aria-label={t(derived ? 'Cancel processing' : 'Cancel download')} tooltip={t(derived ? 'Cancel processing' : 'Cancel download')} disabled={!health || busy[job.id] || busyJobs[job.id]} onClick={() => run(job, 'cancel')}><X size={16} aria-hidden="true"/></Button>}
        {['failed', 'cancelled', 'interrupted'].includes(job.status) && <Button size="icon" variant="secondary" aria-label={t(derived ? 'Retry from start' : 'Retry download')} tooltip={t(derived ? 'Retry from start' : 'Retry download')} disabled={!health || busy[job.id] || busyJobs[job.id] || batchRetry?.running} onClick={() => run(job, 'retry')}><RefreshCw size={16} aria-hidden="true"/></Button>}
        {(hls || viirs || job.assetKey === 'srtm' || safeProduct) && ['failed', 'interrupted'].includes(job.status) && /authoriz|Earthdata|Settings/i.test(job.error || '') && <Button asChild size="icon" variant="secondary" tooltip={t('Manage authorization')}><a href={`#Settings?account=${hls || viirs || job.assetKey === 'srtm' ? 'nasa-earthdata' : 'copernicus'}`} aria-label={t('Manage authorization')}><KeyRound size={16} aria-hidden="true"/></a></Button>}
      </> : null,
    };
  });
  return <><TaskRows className="runtime-jobs" layout={library ? 'files' : 'tasks'} items={items} ariaLabel={t(library ? 'Local source files and outputs' : 'Local file tasks')}/>{inspect && <RasterDialog job={inspect} onClose={() => setInspect(null)}/>} {stacInspect && (stacInspect.wcsSource ? <WcsRasterInspection job={stacInspect} onClose={() => setStacInspect(null)}/> : <StacRasterInspection job={stacInspect} onClose={() => setStacInspect(null)}/>)}</>;
}

export function RuntimeTasks({ areaBounds, areaPolygon }) {
  const { jobs, health, checking, batchRetry, retryAll } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [view, setView] = useState('active');
  const groups = {
    active: jobs.filter(job => ['queued', 'running'].includes(job.status)),
    attention: jobs.filter(job => ['failed', 'interrupted'].includes(job.status)),
    history: jobs.filter(job => ['succeeded', 'cancelled'].includes(job.status)),
  };
  return <section className="runtime-section tasks-page" aria-label={t('Local file tasks')}>
    <div className="tasks-header">
    <PageHeader className="tasks-heading" title={t('Tasks')} status={<Connection compact/>}/>
    <div className="tasks-toolbar"><SegmentedControl variant="navigation" value={view} onValueChange={setView} aria-label={t('Task view')} items={[
      { value: 'active', 'aria-label': `${t('In progress')} · ${number(groups.active.length)}`, label: <>{t('In progress')}<span className="tasks-tab-count" aria-hidden="true">{number(groups.active.length)}</span></> },
      { value: 'attention', 'aria-label': `${t('Needs attention')} · ${number(groups.attention.length)}`, label: <>{t('Needs attention')}<span className="tasks-tab-count" aria-hidden="true">{number(groups.attention.length)}</span></> },
      { value: 'history', 'aria-label': `${t('History')} · ${number(groups.history.length)}`, label: <>{t('History')}<span className="tasks-tab-count" aria-hidden="true">{number(groups.history.length)}</span></> },
    ]}/></div>
    </div>
    {(view === 'attention' && groups.attention.length > 0 || batchRetry) && retryAll && <Surface variant="inset" className="tasks-recovery">
      <div className="tasks-recovery-heading">
        <strong>{t('Restart unfinished tasks')}</strong>
        <div className="tasks-recovery-actions">
          {(groups.attention.length > 0 || batchRetry?.running) && <Button variant="primary" disabled={!health || batchRetry?.running} onClick={retryAll}>{batchRetry?.running ? <Spinner size={15}/> : <RefreshCw size={15}/>} {t('Retry all · {count}', { count: number(batchRetry?.running ? batchRetry.total : groups.attention.length) })}</Button>}
          {batchRetry?.queued > 0 && groups.active.length > 0 && <Button onClick={() => setView('active')}>{t('View running tasks')}</Button>}
        </div>
      </div>
      <div className="tasks-recovery-copy">
        <p>{t('Downloads resume when verified recovery is available; processing starts again. Completed files and cancelled tasks are kept.')}</p>
        {batchRetry && <p role="status">{t(batchRetry.running ? 'Submitting retries: {done} of {total}…' : 'Queued again: {queued} / {total}.', { done: number(batchRetry.processed), queued: number(batchRetry.queued), total: number(batchRetry.total) })}</p>}
        {batchRetry?.skipped > 0 && <p>{t('Already handled or changed status: {count}. No duplicate submission.', { count: number(batchRetry.skipped) })}</p>}
      </div>
      {batchRetry?.failures.length > 0 && <RuntimeError summary="Some tasks could not be queued again. They remain available for retry." message={batchRetry.failures.map(item => `${item.title}: ${item.message}`).join('\n')}/>}
      {batchRetry && !batchRetry.running && batchRetry.processed < batchRetry.total && <p role="status">{t('The connection was lost. Remaining tasks were not submitted; reconnect to retry them.')}</p>}
    </Surface>}
    {!jobs.length && !health ? <Surface variant="inset" className="runtime-empty" role={checking ? 'status' : undefined}><p>{checking && <Spinner size={16}/>} {t(checking ? 'Loading local tasks…' : 'Reconnect the task service to read your tasks.')}</p></Surface> : groups[view].length ? <RuntimeJobRows jobs={groups[view]} areaBounds={areaBounds} areaPolygon={areaPolygon}/> : <EmptyState icon={view === 'attention' ? CheckCircle2 : Download} title={t(view === 'active' ? 'No tasks running' : view === 'attention' ? 'No tasks need attention' : 'No task history yet')} description={t(view === 'active' ? 'Downloads and processing run here. Your completed files stay in My Data.' : view === 'attention' ? 'Failed or interrupted tasks appear here with a retry action.' : 'Completed and cancelled tasks appear here.')} action={<Button asChild><a href="#My%20Data">{t('Open My Data')}</a></Button>}/>}
  </section>;
}

export function RuntimeLibrary({ areaBounds, areaPolygon }) {
  const { jobs, health, checking } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('all');
  const [dataType, setDataType] = useState('all');
  const completed = jobs.filter(job => job.status === 'succeeded');
  const filtered = completed.filter(job => (dataType === 'all' || (dataType === 'science' ? Boolean(MODIS_SCIENCE[job.assetKey]) : dataType === 'vegetation' ? VEGETATION_KEYS.includes(job.assetKey) : dataType === 'quality' ? QUALITY_KEYS.includes(job.assetKey) : dataType === 'reflectance' ? [...LANDSAT_BANDS,'reflectance_rgb'].includes(job.assetKey) : dataType === 'elevation' ? isElevationKey(job.assetKey) : job.assetKey === dataType)) && (kind === 'all' || (kind === 'derived') === !['download', 'raster_prepare'].includes(job.kind)) && [job.title, job.itemId, job.id].some(value => String(value || '').toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
  return <section className="runtime-section runtime-library" aria-label={t('Local source files and outputs')}>
    {!health && <Connection compact/>}
    {completed.length > 0 && <div className="library-toolbar">
      <div className="library-search"><Search size={16} aria-hidden="true"/><Input type="search" aria-label={t('Search local data')} value={search} placeholder={t('Search name, scene or job ID')} onChange={event => setSearch(event.target.value)}/></div>
      <Select aria-label={t('File origin')} value={kind} onChange={event => setKind(event.target.value)}><option value="all">{t('All origins')}</option><option value="derived">{t('Derived outputs')}</option><option value="download">{t('Source files')}</option></Select>
      <Select aria-label={t('Data type')} value={dataType} onChange={event => setDataType(event.target.value)}><option value="all">{t('All data types')}</option><option value="visual">{t('True-color imagery')}</option><option value="scl">{t('SCL classification')}</option><option value="reflectance">{t('Reflectance bands')}</option><option value="science">{t('MODIS scientific layers')}</option><option value="vegetation">{t('Vegetation indices')}</option><option value="quality">{t('Quality flags')}</option><option value="product">{t('Sentinel-2 SAFE product')}</option><option value="elevation">{t('Surface elevation')}</option><option value="aerial">{t('Aerial imagery · RGB + NIR')}</option><option value="stac_asset">{t('Custom raster assets')}</option><option value="wcs_coverage">{t('Coverage subsets')}</option></Select>
      <p className="runtime-results-count" role="status">{t('{shown} of {total} files', { shown: number(filtered.length), total: number(completed.length) })}</p>
    </div>}
    {!jobs.length && !health ? <Surface variant="inset" className="runtime-empty" role={checking ? 'status' : undefined}><p>{checking && <Spinner size={16}/>} {t(checking ? 'Loading local files…' : 'Reconnect the task service to read your local files.')}</p></Surface> : filtered.length ? <RuntimeJobRows jobs={filtered} areaBounds={areaBounds} areaPolygon={areaPolygon} library/> : <Surface variant="inset" className="runtime-empty"><p>{t(completed.length ? 'No local files match these filters.' : 'Completed downloads appear here with their local path, source and checksum.')}</p></Surface>}
  </section>;
}
