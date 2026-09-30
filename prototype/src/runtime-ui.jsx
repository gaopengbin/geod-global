import React, { useCallback, useContext, useEffect, useId, useRef, useState } from 'react';
import { Crop, Download, FolderOpen, RefreshCw, Search, X, CheckCircle2, AlertCircle, HardDrive, Scan } from 'lucide-react';
import { desktopAvailable, downloadableAssets, formatBytes, formatClassShare, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { ClipRasterButton, DerivedArtifactDetails } from './processing-ui.jsx';
import { ArtifactPackageButton } from './artifact-ui.jsx';
import { addProjectScenesAndQueue, createProjectAndQueue, MAX_PROJECT_SCENES, projectRequest, queueProjectDownloads } from './projects-client.js';
import { displayLocalPath } from './local-path.js';
import { Badge, Button, Disclosure, Input, Modal, Select, Spinner, Surface, TaskRows } from './ui/index.jsx';
import './runtime.css';
import { FileThumbnail } from './file-thumbnail.jsx';

const STATUS = { queued: 'Queued', running: 'Downloading', succeeded: 'Downloaded', failed: 'Failed', cancelled: 'Cancelled', interrupted: 'Interrupted' };

export function RuntimeProvider({ children }) {
  const [jobs, setJobs] = useState([]);
  const [health, setHealth] = useState(null);
  const [error, setError] = useState('');
  const [checking, setChecking] = useState(true);
  const mounted = useRef(true);
  const sequence = useRef(0);
  const refresh = useCallback(async () => {
    const request = ++sequence.current;
    try {
      const [info, records] = await Promise.all([runtimeRequest('health'), runtimeRequest('list')]);
      if (mounted.current && request === sequence.current) {
        setHealth(info); setJobs(Array.isArray(records) ? records : records.jobs || []); setError('');
      }
    } catch (e) {
      if (mounted.current && request === sequence.current) { setHealth(null); setError(e.message); }
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
    const result = await runtimeRequest(operation, payload);
    await refresh();
    return result;
  }, [refresh]);
  return <RuntimeContext.Provider value={{ jobs, health, error, checking, refresh, act }}>{children}</RuntimeContext.Provider>;
}

function RuntimeError({ message, summary }) {
  const { t } = useI18n();
  return <div className="runtime-error" role="alert"><p>{t(summary)}</p>{message && <Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(message)}</p></Disclosure>}</div>;
}

function Connection({ compact = false }) {
  const { health, error, checking, refresh } = useContext(RuntimeContext);
  const { t } = useI18n();
  if (health) return <div className="runtime-connection"><Badge tone="success"><CheckCircle2 size={13}/>{t(desktopAvailable() ? 'Desktop task service connected' : 'Local task service connected')}</Badge>{!compact && <span className="runtime-path">{displayLocalPath(health.storageRoot)}</span>}</div>;
  if (checking) return <p className="runtime-connection" role="status"><Spinner size={15}/>{t('Connecting to the task service…')}</p>;
  return <Surface variant="inset" className="runtime-disconnected"><AlertCircle size={16}/><div><strong>{t('Local task service is offline')}</strong><p>{t('Open GeoD Global Desktop, or run {command} beside the browser preview.', { command: 'npm run runtime' })}</p>{error && <Disclosure summary={t('Technical details')}><small>{t(error)}</small></Disclosure>}</div><Button variant="ghost" size="icon" aria-label={t('Reconnect task service')} onClick={refresh}><RefreshCw size={16}/></Button></Surface>;
}

export function DownloadAssetButton({ scene, scenes = [scene], areaBounds, areaPolygon, areaName, project, onProjectUpdated, onOpenProject }) {
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
  const targets = scope === 'current' ? [scene] : scenes;
  const totalScenes = new Set([...(project?.scenes.map(item => item.itemId) || []), ...targets.map(item => item.id)]).size;
  const options = downloadableAssets(targets[0]).filter(item => (item.key === 'scl' || item.key === 'visual')
    && targets.every(target => downloadableAssets(target).some(asset => asset.key === item.key)));
  const available = new Set(options.map(item => item.key));
  const keys = choice === 'both' ? ['visual', 'scl'] : [choice];
  const createdJobs = created?.jobIds?.map(id => jobs.find(job => job.id === id)).filter(Boolean) || [];
  const allDone = createdJobs.length > 0 && createdJobs.every(job => job.status === 'succeeded');
  const failed = createdJobs.some(job => ['failed', 'cancelled', 'interrupted'].includes(job.status));
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const close = () => { if (!busy) { setOpen(false); setError(''); } };
  const show = () => {
    if (created) { setOpen(true); return; }
    setScope('all');
    setChoice(scenes.every(target => downloadableAssets(target).some(asset => asset.key === 'visual')) ? 'visual' : 'scl');
    setName(project?.name || `${t(areaName)} · ${scenes.length > 1 ? t('{count} scenes', { count: number(scenes.length) }) : date(scene.date)}`);
    setError(''); setOpen(true);
  };
  const start = async event => {
    event?.preventDefault();
    if (!options.length || !name.trim() || totalScenes > MAX_PROJECT_SCENES || !keys.every(key => available.has(key))) return;
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
        <p>{t(created.queued ? allDone ? 'The files are ready in this project. Open it to inspect and clip them.' : failed ? 'A download needs attention. Open the project to retry or inspect the task.' : 'The project is saved and its source download is running in the background.' : 'The project was saved, but its download was not added to the queue. Retry below.')}</p>
        {createdJobs.length > 0 && <p className="runtime-help">{t('{done} of {total} files downloaded', { done: number(createdJobs.filter(job => job.status === 'succeeded').length), total: number(createdJobs.length) })}</p>}
        {error && <RuntimeError message={error} summary="The project was saved, but the download could not start."/>}
        <footer className="runtime-dialog-actions"><Button disabled={busy} onClick={close}>{t('Continue exploring')}</Button>{!created.queued && <Button disabled={busy || !health} onClick={start}>{t(busy ? 'Starting…' : 'Retry download')}</Button>}<Button variant="primary" disabled={busy} onClick={openProject}>{t('Open project')}</Button></footer>
      </div> : <form className="runtime-dialog-body" onSubmit={start}>
        <p className="runtime-help">{t(project ? 'Chosen scenes will be added to this project. Its clipping area stays the same and completed source downloads are reused.' : 'The chosen scenes, source links and search area will be saved together in one project.')}</p>
        {scenes.length > 1 && <label className="runtime-field">{t('Download scope')}<Select aria-label={t('Download scope')} value={scope} disabled={busy} onChange={event => {
          const next = event.target.value === 'current' ? [scene] : scenes;
          setScope(event.target.value);
          setChoice(next.every(target => downloadableAssets(target).some(asset => asset.key === 'visual')) ? 'visual' : 'scl');
        }}><option value="all">{t('All chosen scenes · {count}', { count: number(scenes.length) })}</option><option value="current">{t('Current scene only · 1')}</option></Select></label>}
        <p className="runtime-help" role="status">{t(project ? '{scenes} scenes · {files} source files to download or reuse' : '{scenes} scenes · {files} source files to download', { scenes: number(targets.length), files: number(targets.length * keys.length) })}</p>
        {totalScenes > MAX_PROJECT_SCENES && <p className="projects-error" role="alert">{t('This selection would give the project {count} scenes. Keep it within {max} scenes.', { count: totalScenes, max: MAX_PROJECT_SCENES })}</p>}
        {!options.length && <p className="projects-error" role="alert">{t('These scenes have no common supported source file type. Adjust the selection before downloading.')}</p>}
        {project ? <Surface variant="inset" className="runtime-notice"><FolderOpen size={17}/><span><strong>{project.name}</strong><br/>{t('Project after adding · {count} scenes', { count: totalScenes })}</span></Surface> : <label className="runtime-field">{t('Project name')}<Input value={name} maxLength={120} required disabled={busy} onChange={event => setName(event.target.value)}/></label>}
        <label className="runtime-field">{t('Download content')}<Select aria-label={t('Download content')} value={choice} disabled={busy} onChange={event => setChoice(event.target.value)}>
          {available.has('visual') && <option value="visual">{t('True-color GeoTIFF · 10 m')}</option>}
          {available.has('scl') && <option value="scl">{t('SCL classification GeoTIFF · 20 m')}</option>}
          {available.has('visual') && available.has('scl') && <option value="both">{t('Both source files')}</option>}
        </Select></label>
        <Surface as="div" variant="inset" className="runtime-notice"><HardDrive size={17}/><span>{t('GeoD saves complete source files in the local workspace. Open the project later to inspect or clip them; downloading does not crop the source.')}</span></Surface>
        <Disclosure summary={t('Source and file checks · advanced')}><dl className="runtime-details"><dt>{t('Scene')}</dt><dd className="mono runtime-wrap">{targets.map(target => target.id).join(', ')}</dd><dt>{t('Source')}</dt><dd>Earth Search / Sentinel-2 L2A</dd><dt>{t('Checks')}</dt><dd>{t('Transfer size, file signature and SHA-256. Pixel inspection is available for SCL files only.')}</dd></dl></Disclosure>
        <Connection compact/>
        {error && <RuntimeError message={error} summary="The download could not start. Check the service connection and try again."/>}
        <footer className="runtime-dialog-actions"><Button disabled={busy} onClick={close}>{t('Cancel')}</Button><Button variant="primary" type="submit" disabled={busy || !health || !name.trim() || !targets.length || totalScenes > MAX_PROJECT_SCENES || !keys.every(key => available.has(key))}><Download size={15}/>{t(busy ? project ? 'Adding scenes…' : 'Creating project…' : project ? 'Add scenes and start download' : 'Create project and start download')}</Button></footer>
      </form>}
    </Modal>}
  </>;
}

function RasterDialog({ job, onClose }) {
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
      if (job.sha256 && result.sha256.toLowerCase() !== job.sha256.toLowerCase()) throw new Error('The raster checksum does not match this download.');
      if (current) setData(result);
    }).catch(e => {
      if (current && e.name !== 'AbortError') setError(e.name === 'TimeoutError' ? 'Raster inspection timed out. Close other inspections and try again.' : e.message);
    }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; controller.abort(); };
  }, [job.id, job.sha256, attempt]);
  const coordinate = value => number(value, { maximumFractionDigits: 3 });
  return <Modal title={t('Inspect raster')} description={t('Scene classification from local raster pixels')} wide onClose={onClose} closeLabel={t('Close raster inspection')}>
    <div className="runtime-dialog-body">
      <p className="mono runtime-wrap runtime-raster-id">{job.itemId} · SCL</p>
      {loading && <div className="runtime-raster-loading" role="status" aria-live="polite"><Spinner size={24}/><strong>{t('Reading the local GeoTIFF…')}</strong><p>{t('Verifying the file checksum, decoding pixels and reading spatial metadata.')}</p></div>}
      {error && <><RuntimeError message={error} summary="The raster could not be inspected. Keep the local task service running and retry. If the file changed or is missing, download it again."/><Button onClick={() => setAttempt(value => value + 1)}><RefreshCw size={15}/>{t('Retry inspection')}</Button></>}
      {data && <>
        <div className="runtime-raster-grid">
          <figure className="runtime-raster-figure"><Surface as="div" variant="inset" className="runtime-raster-image"><img src={data.previewDataUrl} width={data.previewWidth} height={data.previewHeight} alt={t('Sentinel-2 scene classification decoded from the local SCL raster')} onError={() => { setData(null); setError('The decoded raster preview could not be displayed.'); }}/></Surface><figcaption>{t('Nearest-neighbor preview · {width} × {height} pixels. Colors show source classification values.', { width: number(data.previewWidth), height: number(data.previewHeight) })}</figcaption></figure>
          <section className="runtime-raster-metadata" aria-label={t('Raster metadata')}><h3>{t('Raster metadata')}</h3><dl className="runtime-details"><dt>{t('Dimensions')}</dt><dd>{t('{width} × {height} pixels', { width: number(data.width), height: number(data.height) })}</dd><dt>{t('Bands')}</dt><dd>{number(data.bandCount)}</dd><dt>{t('Data type')}</dt><dd>{data.dataType}</dd><dt>{t('Coordinate system')}</dt><dd>{data.crs}</dd><dt>{t('Pixel size (metres)')}</dt><dd>{coordinate(data.pixelSize[0])} × {coordinate(data.pixelSize[1])}</dd><dt>{t('Bounds (metres)')}</dt><dd className="runtime-raster-bounds">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, index) => <span key={label}>{t(label)}: {coordinate(data.bounds[index])}</span>)}</dd><dt>{t('No-data value')}</dt><dd>{data.nodata === null ? t('Not specified') : number(data.nodata)}</dd></dl></section>
        </div>
        <section className="runtime-raster-legend" aria-labelledby={legendId}><h3 id={legendId}>{t('Scene classes')}</h3><p>{t('Counts cover the current raster, including no-data pixels.')}</p><ul>{data.classes.map(item => <li key={item.value}><span className="runtime-raster-swatch" style={{ backgroundColor: item.color }} aria-hidden="true"/><span className="runtime-raster-class">{number(item.value)} · {t(item.label)}</span><span className="runtime-raster-count">{t('{count} pixels', { count: number(item.count) })}<small>{formatClassShare(item.count, data.width * data.height, locale)}</small></span></li>)}</ul></section>
        <Surface as="div" variant="inset" className="runtime-notice"><CheckCircle2 size={17}/><span>{t(job.kind === 'raster_clip' ? 'The preview and counts come from the derived GeoTIFF after SHA-256 verification. Source pixels were clipped without resampling or reprojection.' : 'The preview and class counts were decoded from this local file after SHA-256 verification. Source classifications are not an independent accuracy assessment. No clipping or reprojection is applied.')}</span></Surface>
        <Disclosure className="runtime-raster-provenance" summary={t('File and provenance')}><dl className="runtime-details"><dt>{t('File')}</dt><dd className="mono runtime-wrap">{displayLocalPath(job.outputPath)}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{data.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd></dl></Disclosure>
      </>}
    </div><footer className="runtime-dialog-actions"><Button onClick={onClose}>{t('Close')}</Button></footer>
  </Modal>;
}

export function RuntimeJobRows({ jobs, library = false, areaBounds, areaPolygon, projectName }) {
  const { act } = useContext(RuntimeContext);
  const { t, locale, number, date } = useI18n();
  const [busy, setBusy] = useState({});
  const [errors, setErrors] = useState({});
  const [inspect, setInspect] = useState(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const run = async (job, operation) => {
    setBusy(previous => ({ ...previous, [job.id]: true }));
    setErrors(previous => ({ ...previous, [job.id]: '' }));
    try { await act(operation, { id: job.id }); }
    catch (e) { if (mounted.current) setErrors(previous => ({ ...previous, [job.id]: e.message })); }
    finally { if (mounted.current) setBusy(previous => ({ ...previous, [job.id]: false })); }
  };
  const bytes = value => Number.isFinite(value) && value >= 0 ? formatBytes(value, locale) : t('Unknown size');
  const items = jobs.map(job => {
    const active = ['queued', 'running'].includes(job.status);
    const derived = job.kind !== 'download';
    const mosaic = job.kind === 'raster_mosaic';
    const projectClip = mosaic && job.mosaic?.sources?.length === 1;
    const canInspect = job.status === 'succeeded' && job.assetKey === 'scl' && /^image\/(?:tiff|geotiff)(?:;|$)/i.test(job.mediaType || '');
    const progress = Number.isFinite(job.totalBytes) && job.totalBytes > 0 && Number.isFinite(job.bytesDownloaded)
      ? Math.max(0, Math.min(100, job.bytesDownloaded / job.totalBytes * 100)) : null;
    const type = mosaic ? t(projectClip ? 'Project clip · GeoTIFF' : 'Project mosaic · GeoTIFF') : derived ? t('Clipped raster · GeoTIFF') : t(job.assetKey === 'scl' ? 'SCL raster · GeoTIFF' : job.assetKey === 'visual' ? 'True-color image · GeoTIFF' : 'Preview image · JPEG');
    return {
      id: job.id,
      title: projectName && mosaic ? `${projectName} · ${t(projectClip ? 'Area clip' : 'Mosaic and clip')} · ${job.assetKey === 'scl' ? 'SCL' : t('True-color imagery')}` : job.title || job.itemId,
      description: library ? type : `${t(mosaic ? projectClip ? 'Project clip task' : 'Project mosaic task' : derived ? 'Raster clip task' : 'Source download task')}${mosaic && projectName ? '' : ` · ${job.itemId}`}`,
      icon: derived ? Crop : Download,
      preview: library && ['scl', 'visual'].includes(job.assetKey) ? <FileThumbnail key={`${job.id}:${job.sha256}`} job={job}/> : null,
      status: job.status,
      statusTone: library ? 'neutral' : undefined,
      statusLabel: t(library ? mosaic && !projectClip ? 'Mosaic output' : derived ? 'Clipped output' : 'Source file' : derived && job.status === 'succeeded' ? 'Generated' : derived && job.status === 'running' ? 'Processing' : STATUS[job.status] || job.status),
      progress: mosaic ? progress : derived ? null : progress,
      progressLabel: t(derived ? 'Processing progress' : 'Download progress'),
      meta: library ? t('Saved on {date}', { date: date(job.updatedAt) }) : mosaic && job.status === 'running' ? <>{t(job.validation || 'Checking downloaded sources')} · {number(job.bytesDownloaded)} / {number(job.totalBytes || 0)} {t('steps')}</> : derived ? t('Local raster processing') : <>{bytes(job.bytesDownloaded)}{job.totalBytes ? ` / ${bytes(job.totalBytes)}` : ''}{active && progress !== null ? ` · ${t('{percent}% transferred', { percent: number(Math.floor(progress)) })}` : ''}</>,
      details: library ? <Disclosure className="runtime-file-details" summary={t('File details and provenance')}>
        {derived && !mosaic && <DerivedArtifactDetails job={job}/>}
        {mosaic && <p>{t('{count} verified sources · {width} × {height} pixels · {crs}', { count: job.mosaicOutput?.sourceCount || 0, width: job.mosaicOutput?.width || 0, height: job.mosaicOutput?.height || 0, crs: job.mosaicOutput?.crs || '' })}</p>}
        <dl className="runtime-details"><dt>{t('Scene')}</dt><dd className="mono runtime-wrap">{job.itemId}</dd><dt>{t('File')}</dt><dd className="mono runtime-wrap">{displayLocalPath(job.outputPath)}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{job.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd><dt>{t('Updated')}</dt><dd>{date(job.updatedAt)}</dd><dt>{t('Validation')}</dt><dd>{t(derived ? 'Generated locally from the checked source clip. Inspect the result to read output pixels and spatial metadata.' : 'Transfer size and file signature checked. Use Inspect raster on an SCL file to decode pixels and read spatial metadata.')}</dd></dl>
        {job.kind === 'raster_clip' && <ArtifactPackageButton job={job}/>}
        {desktopAvailable() && <Button disabled={busy[job.id]} onClick={() => run(job, 'reveal')}><FolderOpen size={15}/>{t('Show in folder')}</Button>}
        {derived && !desktopAvailable() && <Button onClick={() => { const link = document.createElement('a'); link.href = `http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/file`; link.download = `${job.id}.tif`; link.click(); }}><Download size={15}/>{t('Download result GeoTIFF')}</Button>}
        {mosaic && !desktopAvailable() && <Button onClick={() => { const link = document.createElement('a'); link.href = `http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/metadata`; link.download = `${job.id}.metadata.json`; link.click(); }}><Download size={15}/>{t('Download provenance JSON')}</Button>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
      </Disclosure> : job.error || errors[job.id] ? <div className="runtime-job-details">
        {job.error && <RuntimeError message={typeof job.error === 'string' ? job.error : job.error.message} summary={derived ? 'Raster processing did not complete. Check the source file and retry the clip.' : 'This download did not complete. Retry from the beginning when the source and local service are available.'}/>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
      </div> : null,
      actions: library ? canInspect && <>
        <Button size="sm" onClick={() => setInspect(job)}><Scan size={16}/>{t('Inspect raster')}</Button>
        {!derived && !projectName && <ClipRasterButton job={job} areaBounds={areaBounds} areaPolygon={areaPolygon}/>}
      </> : active || ['failed', 'cancelled', 'interrupted'].includes(job.status) ? <>
        {active && <Button disabled={busy[job.id]} onClick={() => run(job, 'cancel')}><X size={15}/>{t(derived ? 'Cancel processing' : 'Cancel download')}</Button>}
        {['failed', 'cancelled', 'interrupted'].includes(job.status) && <Button disabled={busy[job.id]} onClick={() => run(job, 'retry')}><RefreshCw size={15}/>{t('Retry from start')}</Button>}
      </> : null,
    };
  });
  return <><TaskRows className="runtime-jobs" layout={library ? 'files' : 'tasks'} items={items} ariaLabel={t(library ? 'Local source files and outputs' : 'Local file tasks')}/>{inspect && <RasterDialog job={inspect} onClose={() => setInspect(null)}/>}</>;
}

export function RuntimeTasks({ areaBounds, areaPolygon }) {
  const { jobs } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const pending = jobs.filter(job => job.status !== 'succeeded');
  const completed = jobs.filter(job => job.status === 'succeeded');
  return <section className="runtime-section" aria-label={t('Local file tasks')}><header className="runtime-section-heading"><h2>{t('Tasks needing attention')}</h2><Connection compact/></header>
    {pending.length ? <RuntimeJobRows jobs={pending} areaBounds={areaBounds} areaPolygon={areaPolygon}/> : <Surface variant="inset" className="runtime-empty"><p>{t('No tasks need attention. Start a download in Explore; finished files are in My Data.')}</p></Surface>}
    {completed.length > 0 && <Disclosure className="runtime-task-history" summary={t('Completed task history · {count}', { count: number(completed.length) })}><RuntimeJobRows jobs={completed} areaBounds={areaBounds} areaPolygon={areaPolygon}/></Disclosure>}
  </section>;
}

export function RuntimeLibrary({ areaBounds, areaPolygon }) {
  const { jobs, health } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('all');
  const completed = jobs.filter(job => job.status === 'succeeded');
  const filtered = completed.filter(job => (kind === 'all' || (kind === 'derived') === (job.kind !== 'download')) && [job.title, job.itemId, job.id].some(value => String(value || '').toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
  return <section className="runtime-section runtime-library" aria-label={t('Local source files and outputs')}>
    {!health && <Connection compact/>}
    {completed.length > 0 && <div className="library-toolbar">
      <div className="library-search"><Search size={16} aria-hidden="true"/><Input type="search" aria-label={t('Search local data')} value={search} placeholder={t('Search name, scene or job ID')} onChange={event => setSearch(event.target.value)}/></div>
      <Select aria-label={t('File origin')} value={kind} onChange={event => setKind(event.target.value)}><option value="all">{t('All origins')}</option><option value="derived">{t('Derived outputs')}</option><option value="download">{t('Downloaded sources')}</option></Select>
      <p className="runtime-results-count" role="status">{t('{shown} of {total} files', { shown: number(filtered.length), total: number(completed.length) })}</p>
    </div>}
    {filtered.length ? <RuntimeJobRows jobs={filtered} areaBounds={areaBounds} areaPolygon={areaPolygon} library/> : <Surface variant="inset" className="runtime-empty"><p>{t(completed.length ? 'No local files match these filters.' : 'Completed downloads appear here with their local path, source and checksum.')}</p></Surface>}
  </section>;
}
