import React, { useCallback, useContext, useEffect, useId, useRef, useState } from 'react';
import { Download, FolderOpen, RefreshCw, X, CheckCircle2, AlertCircle, HardDrive, Scan, LoaderCircle } from 'lucide-react';
import { desktopAvailable, downloadableAssets, formatBytes, formatClassShare, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { ClipRasterButton, DerivedArtifactDetails } from './processing-ui.jsx';
import { ArtifactPackageButton } from './artifact-ui.jsx';
import './runtime.css';

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
  return <div className="runtime-error" role="alert"><p>{t(summary)}</p>{message && <details><summary>{t('Technical details')}</summary><p className="runtime-wrap">{t(message)}</p></details>}</div>;
}

function Connection({ compact = false }) {
  const { health, error, checking, refresh } = useContext(RuntimeContext);
  const { t } = useI18n();
  if (health) return <p className="runtime-connection"><span className="runtime-dot" />{t(desktopAvailable() ? 'Desktop task service connected' : 'Local task service connected')}{!compact && <span className="runtime-path">{health.storageRoot}</span>}</p>;
  if (checking) return <p className="runtime-connection" role="status"><LoaderCircle size={15} className="runtime-spinner"/>{t('Connecting to the task service…')}</p>;
  return <div className="runtime-disconnected"><AlertCircle size={16}/><div><strong>{t('Local task service is offline')}</strong><p>{t('Open GeoD Global Desktop, or run {command} beside the browser preview.', { command: 'npm run runtime' })}</p>{error && <details><summary>{t('Technical details')}</summary><small>{t(error)}</small></details>}</div><button className="icon-btn" aria-label={t('Reconnect task service')} onClick={refresh}><RefreshCw size={16}/></button></div>;
}

export function DownloadAssetButton({ scene }) {
  const { health, act } = useContext(RuntimeContext);
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [assetKey, setAssetKey] = useState('scl');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const dialog = useRef(null);
  const mounted = useRef(true);
  const titleId = useId();
  const options = downloadableAssets(scene);
  const asset = options.find(item => item.key === assetKey) || options[0];
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => {
    const node = dialog.current;
    if (open && node) node.showModal();
    return () => { if (node?.open) node.close(); };
  }, [open]);
  const close = () => { if (!busy) { setOpen(false); setError(''); } };
  const start = async () => {
    if (!asset) return;
    setBusy(true); setError('');
    try {
      await act('create', { itemId: scene.id, assetKey: asset.key, href: asset.href, mediaType: asset.type, title: `${scene.id} · ${asset.key.toUpperCase()}` });
      if (mounted.current) { setOpen(false); location.hash = 'Tasks'; }
    } catch (e) { if (mounted.current) setError(e.message); }
    finally { if (mounted.current) setBusy(false); }
  };
  return <>
    <button className="button primary" disabled={!options.length} onClick={() => { setAssetKey(options[0]?.key); setOpen(true); }}><Download size={15}/>{t('Download source asset')}</button>
    {open && <dialog ref={dialog} className="runtime-dialog" onCancel={event => { event.preventDefault(); close(); }} onClick={event => { if (event.target === event.currentTarget) close(); }} aria-labelledby={titleId}>
      <header><h2 id={titleId}>{t('Download source asset')}</h2><button className="icon-btn" aria-label={t('Close download')} disabled={busy} onClick={close}><X size={20}/></button></header>
      <div className="runtime-dialog-body">
        <p className="mono runtime-wrap">{scene.id}</p>
        <label className="runtime-field">{t('Asset')}<select aria-label={t('Download asset')} disabled={busy} value={asset?.key || ''} onChange={event => setAssetKey(event.target.value)}>{options.map(option => <option value={option.key} key={option.key}>{t(option.key === 'scl' ? 'Scene classification · GeoTIFF · 20 m' : option.key === 'visual' ? 'True color · GeoTIFF · 10 m' : 'Thumbnail · JPEG · overview only')}</option>)}</select></label>
        <div className="notice"><HardDrive size={17}/><span>{t('The complete source file is saved locally. Area clipping and reprojection are not applied. Large files may take time; the current limit is 512 MiB per file.')}</span></div>
        <dl className="runtime-details"><dt>{t('Source')}</dt><dd>Earth Search / Sentinel-2 L2A</dd><dt>{t('Asset')}</dt><dd><a href={asset?.href} target="_blank" rel="noreferrer">{asset?.title || asset?.key}</a></dd><dt>{t('Save under')}</dt><dd className="runtime-wrap">{health?.storageRoot || t('Local task service required')}</dd><dt>{t('Checks')}</dt><dd>{t('Byte count, file signature and SHA-256. Inspect a downloaded SCL raster separately to read its pixels and spatial metadata.')}</dd></dl>
        <Connection compact/>
        {error && <RuntimeError message={error} summary="The download could not start. Check the service connection and try again."/>}
      </div><footer><button className="button" disabled={busy} onClick={close}>{t('Cancel')}</button><button className="button primary" disabled={busy || !health || !asset} onClick={start}><Download size={15}/>{t(busy ? 'Starting…' : 'Start download')}</button></footer>
    </dialog>}
  </>;
}

function RasterDialog({ job, onClose }) {
  const { t, number, locale } = useI18n();
  const dialog = useRef(null);
  const titleId = useId();
  const descriptionId = useId();
  const [attempt, setAttempt] = useState(0);
  const [data, setData] = useState(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    const node = dialog.current;
    node.showModal();
    return () => { if (node.open) node.close(); };
  }, []);
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
  return <dialog ref={dialog} className="runtime-dialog runtime-raster-dialog" aria-labelledby={titleId} aria-describedby={descriptionId} onCancel={event => { event.preventDefault(); onClose(); }} onClick={event => { if (event.target === event.currentTarget) onClose(); }}>
    <header><div><h2 id={titleId}>{t('Inspect raster')}</h2><p id={descriptionId}>{t('Scene classification from local raster pixels')}</p></div><button className="icon-btn" aria-label={t('Close raster inspection')} onClick={onClose}><X size={20}/></button></header>
    <div className="runtime-dialog-body">
      <p className="mono runtime-wrap runtime-raster-id">{job.itemId} · SCL</p>
      {loading && <div className="runtime-raster-loading" role="status" aria-live="polite"><LoaderCircle size={24} className="runtime-spinner"/><strong>{t('Reading the local GeoTIFF…')}</strong><p>{t('Verifying the file checksum, decoding pixels and reading spatial metadata.')}</p></div>}
      {error && <><RuntimeError message={error} summary="The raster could not be inspected. Keep the local task service running and retry. If the file changed or is missing, download it again."/><button className="button" onClick={() => setAttempt(value => value + 1)}><RefreshCw size={15}/>{t('Retry inspection')}</button></>}
      {data && <>
        <div className="runtime-raster-grid">
          <figure className="runtime-raster-figure"><div className="runtime-raster-image"><img src={data.previewDataUrl} width={data.previewWidth} height={data.previewHeight} alt={t('Sentinel-2 scene classification decoded from the local SCL raster')} onError={() => { setData(null); setError('The decoded raster preview could not be displayed.'); }}/></div><figcaption>{t('Nearest-neighbor preview · {width} × {height} pixels. Colors show source classification values.', { width: number(data.previewWidth), height: number(data.previewHeight) })}</figcaption></figure>
          <section className="runtime-raster-metadata" aria-label={t('Raster metadata')}><h3>{t('Raster metadata')}</h3><dl className="runtime-details"><dt>{t('Dimensions')}</dt><dd>{t('{width} × {height} pixels', { width: number(data.width), height: number(data.height) })}</dd><dt>{t('Bands')}</dt><dd>{number(data.bandCount)}</dd><dt>{t('Data type')}</dt><dd>{data.dataType}</dd><dt>{t('Coordinate system')}</dt><dd>{data.crs}</dd><dt>{t('Pixel size (metres)')}</dt><dd>{coordinate(data.pixelSize[0])} × {coordinate(data.pixelSize[1])}</dd><dt>{t('Bounds (metres)')}</dt><dd className="runtime-raster-bounds">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, index) => <span key={label}>{t(label)}: {coordinate(data.bounds[index])}</span>)}</dd><dt>{t('No-data value')}</dt><dd>{data.nodata === null ? t('Not specified') : number(data.nodata)}</dd></dl></section>
        </div>
        <section className="runtime-raster-legend" aria-labelledby={`${titleId}-legend`}><h3 id={`${titleId}-legend`}>{t('Scene classes')}</h3><p>{t('Counts cover the current raster, including no-data pixels.')}</p><ul>{data.classes.map(item => <li key={item.value}><span className="runtime-raster-swatch" style={{ backgroundColor: item.color }} aria-hidden="true"/><span className="runtime-raster-class">{number(item.value)} · {t(item.label)}</span><span className="runtime-raster-count">{t('{count} pixels', { count: number(item.count) })}<small>{formatClassShare(item.count, data.width * data.height, locale)}</small></span></li>)}</ul></section>
        <div className="notice"><CheckCircle2 size={17}/><span>{t(job.kind === 'raster_clip' ? 'The preview and counts come from the derived GeoTIFF after SHA-256 verification. Source pixels were clipped without resampling or reprojection.' : 'The preview and class counts were decoded from this local file after SHA-256 verification. Source classifications are not an independent accuracy assessment. No clipping or reprojection is applied.')}</span></div>
        <details className="runtime-raster-provenance"><summary>{t('File and provenance')}</summary><dl className="runtime-details"><dt>{t('File')}</dt><dd className="mono runtime-wrap">{job.outputPath}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{data.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd></dl></details>
      </>}
    </div><footer><button className="button" onClick={onClose}>{t('Close')}</button></footer>
  </dialog>;
}

function JobCard({ job, library = false, areaBounds }) {
  const { act } = useContext(RuntimeContext);
  const { t, locale, number, date } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [inspect, setInspect] = useState(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const active = ['queued', 'running'].includes(job.status);
  const derived = job.kind === 'raster_clip';
  const canInspect = job.status === 'succeeded' && job.assetKey === 'scl' && /^image\/(?:tiff|geotiff)(?:;|$)/i.test(job.mediaType || '');
  const run = async operation => {
    setBusy(true); setError('');
    try { await act(operation, { id: job.id }); }
    catch (e) { if (mounted.current) setError(e.message); }
    finally { if (mounted.current) setBusy(false); }
  };
  const progress = job.totalBytes > 0 ? Math.min(100, job.bytesDownloaded / job.totalBytes * 100) : null;
  const bytes = value => Number.isFinite(value) && value >= 0 ? formatBytes(value, locale) : t('Unknown size');
  return <article className="runtime-job">
    <div className="runtime-job-heading"><div><h3>{job.title || job.itemId}</h3><p>{job.assetKey.toUpperCase()} · {job.id}</p></div><span className={'badge ' + (job.status === 'succeeded' ? 'green' : ['failed', 'interrupted'].includes(job.status) ? 'red' : 'blue')}>{t(derived && job.status === 'succeeded' ? 'Generated' : derived && job.status === 'running' ? 'Processing' : STATUS[job.status] || job.status)}</span></div>
    {active && <progress aria-label={t(derived ? 'Processing progress' : 'Download progress')} value={derived ? undefined : progress ?? undefined} max="100"/>}
    <div className="runtime-job-status"><span>{derived ? t('Local raster processing') : <>{bytes(job.bytesDownloaded)}{job.totalBytes ? ` / ${bytes(job.totalBytes)}` : ''}{active && progress !== null ? ` · ${t('{percent}% transferred', { percent: number(Math.floor(progress)) })}` : ''}</>}</span><span>{job.status === 'succeeded' ? <><CheckCircle2 size={14}/>{t('File saved · SHA-256 recorded')}</> : t(active ? derived ? 'Copying source pixels into a derived GeoTIFF' : 'Downloading original asset' : 'No completed artifact from this attempt')}</span></div>
    <DerivedArtifactDetails job={job}/>
    {job.error && <RuntimeError message={typeof job.error === 'string' ? job.error : job.error.message} summary={derived ? 'Raster processing did not complete. Check the source file and retry the recipe.' : 'This download did not complete. Retry from the beginning when the source and local service are available.'}/>}
    {job.status === 'succeeded' && <details open={library}><summary>{t('File and provenance')}</summary><dl className="runtime-details"><dt>{t('File')}</dt><dd className="mono runtime-wrap">{job.outputPath}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{job.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd><dt>{t('Updated')}</dt><dd>{date(job.updatedAt)}</dd><dt>{t('Validation')}</dt><dd>{t(derived ? 'Generated locally from the pinned source recipe. Inspect the result to read output pixels and spatial metadata.' : 'Transfer size and file signature checked. Use Inspect raster on an SCL file to decode pixels and read spatial metadata.')}</dd></dl></details>}
    <div className="row-actions">
      {active && <button className="button" disabled={busy} onClick={() => run('cancel')}><X size={15}/>{t(derived ? 'Cancel processing' : 'Cancel download')}</button>}
      {['failed', 'cancelled', 'interrupted'].includes(job.status) && <button className="button" disabled={busy} onClick={() => run('retry')}><RefreshCw size={15}/>{t('Retry from start')}</button>}
      {canInspect && <button className="button primary" onClick={() => setInspect(true)}><Scan size={15}/>{t('Inspect raster')}</button>}
      {canInspect && !derived && <ClipRasterButton job={job} areaBounds={areaBounds}/>}
      {job.status === 'succeeded' && desktopAvailable() && <button className="button" disabled={busy} onClick={() => run('reveal')}><FolderOpen size={15}/>{t('Show in folder')}</button>}
    </div>{error && <RuntimeError message={error} summary="The task action failed. Check the service connection and try again."/>}
    {derived && job.status === 'succeeded' && <ArtifactPackageButton job={job}/>}
    {inspect && <RasterDialog job={job} onClose={() => setInspect(false)}/>}
  </article>;
}

export function RuntimeTasks({ areaBounds }) {
  const { jobs } = useContext(RuntimeContext);
  const { t } = useI18n();
  return <section className="runtime-section" aria-label={t('Local file tasks')}><h2>{t('Downloads and processing')}</h2><Connection/>{jobs.length ? <div className="runtime-jobs">{jobs.map(job => <JobCard key={job.id} job={job} areaBounds={areaBounds}/>)}</div> : <p className="runtime-empty">{t('Select a scene, then choose “Download source asset”. Download tasks and file checks are saved by the local service.')}</p>}</section>;
}

export function RuntimeLibrary({ areaBounds }) {
  const { jobs } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('all');
  const completed = jobs.filter(job => job.status === 'succeeded');
  const filtered = completed.filter(job => (kind === 'all' || (kind === 'derived') === (job.kind === 'raster_clip')) && [job.title, job.itemId, job.id].some(value => String(value || '').toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
  return <section className="runtime-section" aria-label={t('Local source files and outputs')}><h2>{t('Local source files and outputs')} <span className="badge">{number(completed.length)}</span></h2><Connection/>{completed.length > 0 && <><div className="library-filters"><label className="runtime-field">{t('Search local data')}<input type="search" value={search} placeholder={t('Search name, scene or job ID')} onChange={event => setSearch(event.target.value)}/></label><label className="runtime-field">{t('Data type')}<select value={kind} onChange={event => setKind(event.target.value)}><option value="all">{t('All files')}</option><option value="derived">{t('Derived outputs')}</option><option value="download">{t('Downloaded sources')}</option></select></label></div><p>{t('{shown} of {total} files', { shown: number(filtered.length), total: number(completed.length) })}</p></>}{filtered.length ? <div className="runtime-jobs">{filtered.map(job => <JobCard key={job.id} job={job} areaBounds={areaBounds} library/>)}</div> : <p className="runtime-empty">{t(completed.length ? 'No local files match these filters.' : 'Completed downloads appear here with their local path, source and checksum.')}</p>}</section>;
}
