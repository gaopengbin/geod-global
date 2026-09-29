import React, { useCallback, useContext, useEffect, useId, useRef, useState } from 'react';
import { Crop, Download, FolderOpen, RefreshCw, X, CheckCircle2, AlertCircle, HardDrive, Scan } from 'lucide-react';
import { desktopAvailable, downloadableAssets, formatBytes, formatClassShare, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { ClipRasterButton, DerivedArtifactDetails } from './processing-ui.jsx';
import { ArtifactPackageButton } from './artifact-ui.jsx';
import { Badge, Button, Disclosure, Input, Modal, Select, Spinner, Surface, TaskRows } from './ui/index.jsx';
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
  return <div className="runtime-error" role="alert"><p>{t(summary)}</p>{message && <Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(message)}</p></Disclosure>}</div>;
}

function Connection({ compact = false }) {
  const { health, error, checking, refresh } = useContext(RuntimeContext);
  const { t } = useI18n();
  if (health) return <div className="runtime-connection"><Badge tone="success"><CheckCircle2 size={13}/>{t(desktopAvailable() ? 'Desktop task service connected' : 'Local task service connected')}</Badge>{!compact && <span className="runtime-path">{health.storageRoot}</span>}</div>;
  if (checking) return <p className="runtime-connection" role="status"><Spinner size={15}/>{t('Connecting to the task service…')}</p>;
  return <Surface variant="inset" className="runtime-disconnected"><AlertCircle size={16}/><div><strong>{t('Local task service is offline')}</strong><p>{t('Open GeoD Global Desktop, or run {command} beside the browser preview.', { command: 'npm run runtime' })}</p>{error && <Disclosure summary={t('Technical details')}><small>{t(error)}</small></Disclosure>}</div><Button variant="ghost" size="icon" aria-label={t('Reconnect task service')} onClick={refresh}><RefreshCw size={16}/></Button></Surface>;
}

export function DownloadAssetButton({ scene }) {
  const { health, act } = useContext(RuntimeContext);
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [assetKey, setAssetKey] = useState('scl');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const mounted = useRef(true);
  const options = downloadableAssets(scene);
  const asset = options.find(item => item.key === assetKey) || options[0];
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
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
    <Button variant="primary" disabled={!options.length} onClick={() => { setAssetKey(options[0]?.key); setOpen(true); }}><Download size={15}/>{t('Choose a file to download')}</Button>
    {open && <Modal title={t('Download source asset')} onClose={close} closeDisabled={busy} closeLabel={t('Close download')}>
      <div className="runtime-dialog-body">
        <p className="mono runtime-wrap">{scene.id}</p>
        <label className="runtime-field">{t('Asset')}<Select aria-label={t('Download asset')} disabled={busy} value={asset?.key || ''} onChange={event => setAssetKey(event.target.value)}>{options.map(option => <option value={option.key} key={option.key}>{t(option.key === 'scl' ? 'SCL classification · can clip · GeoTIFF · 20 m' : option.key === 'visual' ? 'True-color image · download only · GeoTIFF · 10 m' : 'JPEG thumbnail · preview only')}</option>)}</Select></label>
        <Surface as="div" variant="inset" className="runtime-notice"><HardDrive size={17}/><span>{t(asset?.key === 'scl' ? 'Downloads the full SCL classification raster. After it finishes, open My Data to inspect or clip it; this download does not crop the file.' : asset?.key === 'visual' ? 'Downloads the full true-color GeoTIFF. Local clipping of true-color imagery is not available yet.' : 'Downloads only the JPEG preview, not the full-resolution raster. It cannot be clipped.')}</span></Surface>
        <p className="runtime-help">{t('Files are saved to the local workspace, not your browser Downloads folder. The current limit is 512 MiB per file.')}</p>
        <dl className="runtime-details"><dt>{t('Source')}</dt><dd>Earth Search / Sentinel-2 L2A</dd><dt>{t('Asset')}</dt><dd><a href={asset?.href} target="_blank" rel="noreferrer">{asset?.title || asset?.key}</a></dd><dt>{t('Save under')}</dt><dd className="runtime-wrap">{health?.storageRoot || t('Local task service required')}</dd><dt>{t('Checks')}</dt><dd>{t('Transfer size, file signature and SHA-256. Pixel inspection is available for SCL files only.')}</dd></dl>
        <Connection compact/>
        {error && <RuntimeError message={error} summary="The download could not start. Check the service connection and try again."/>}
      </div><footer className="runtime-dialog-actions"><Button disabled={busy} onClick={close}>{t('Cancel')}</Button><Button variant="primary" disabled={busy || !health || !asset} onClick={start}><Download size={15}/>{t(busy ? 'Starting…' : 'Start download')}</Button></footer>
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
        <Disclosure className="runtime-raster-provenance" summary={t('File and provenance')}><dl className="runtime-details"><dt>{t('File')}</dt><dd className="mono runtime-wrap">{job.outputPath}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{data.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd></dl></Disclosure>
      </>}
    </div><footer className="runtime-dialog-actions"><Button onClick={onClose}>{t('Close')}</Button></footer>
  </Modal>;
}

function RuntimeJobRows({ jobs, library = false, areaBounds, areaPolygon }) {
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
    const derived = job.kind === 'raster_clip';
    const canInspect = job.status === 'succeeded' && job.assetKey === 'scl' && /^image\/(?:tiff|geotiff)(?:;|$)/i.test(job.mediaType || '');
    const progress = Number.isFinite(job.totalBytes) && job.totalBytes > 0 && Number.isFinite(job.bytesDownloaded)
      ? Math.max(0, Math.min(100, job.bytesDownloaded / job.totalBytes * 100)) : null;
    const type = derived ? t('Clipped raster · GeoTIFF') : t(job.assetKey === 'scl' ? 'SCL raster · GeoTIFF' : job.assetKey === 'visual' ? 'True-color image · GeoTIFF' : 'Preview image · JPEG');
    return {
      id: job.id,
      title: job.title || job.itemId,
      description: library ? type : `${t(derived ? 'Raster clip task' : 'Source download task')} · ${job.itemId}`,
      icon: derived ? Crop : Download,
      status: job.status,
      statusLabel: t(library ? derived ? 'Clipped output' : 'Source file' : derived && job.status === 'succeeded' ? 'Generated' : derived && job.status === 'running' ? 'Processing' : STATUS[job.status] || job.status),
      progress: derived ? null : progress,
      progressLabel: t(derived ? 'Processing progress' : 'Download progress'),
      meta: library ? date(job.updatedAt) : derived ? t('Local raster processing') : <>{bytes(job.bytesDownloaded)}{job.totalBytes ? ` / ${bytes(job.totalBytes)}` : ''}{active && progress !== null ? ` · ${t('{percent}% transferred', { percent: number(Math.floor(progress)) })}` : ''}</>,
      details: library ? <Disclosure className="runtime-file-details" summary={t('File details and provenance')}>
        <DerivedArtifactDetails job={job}/>
        <dl className="runtime-details"><dt>{t('Scene')}</dt><dd className="mono runtime-wrap">{job.itemId}</dd><dt>{t('File')}</dt><dd className="mono runtime-wrap">{job.outputPath}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{job.sha256}</dd><dt>{t('Source')}</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd><dt>{t('Updated')}</dt><dd>{date(job.updatedAt)}</dd><dt>{t('Validation')}</dt><dd>{t(derived ? 'Generated locally from the checked source clip. Inspect the result to read output pixels and spatial metadata.' : 'Transfer size and file signature checked. Use Inspect raster on an SCL file to decode pixels and read spatial metadata.')}</dd></dl>
        {derived && <ArtifactPackageButton job={job}/>}
        {desktopAvailable() && <Button disabled={busy[job.id]} onClick={() => run(job, 'reveal')}><FolderOpen size={15}/>{t('Show in folder')}</Button>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
      </Disclosure> : job.error || errors[job.id] ? <div className="runtime-job-details">
        {job.error && <RuntimeError message={typeof job.error === 'string' ? job.error : job.error.message} summary={derived ? 'Raster processing did not complete. Check the source file and retry the clip.' : 'This download did not complete. Retry from the beginning when the source and local service are available.'}/>}
        {errors[job.id] && <RuntimeError message={errors[job.id]} summary="The task action failed. Check the service connection and try again."/>}
      </div> : null,
      actions: library ? canInspect && <>
        <Button variant="primary" onClick={() => setInspect(job)}><Scan size={15}/>{t('Inspect raster')}</Button>
        {!derived && <ClipRasterButton job={job} areaBounds={areaBounds} areaPolygon={areaPolygon}/>}
      </> : active || ['failed', 'cancelled', 'interrupted'].includes(job.status) ? <>
        {active && <Button disabled={busy[job.id]} onClick={() => run(job, 'cancel')}><X size={15}/>{t(derived ? 'Cancel processing' : 'Cancel download')}</Button>}
        {['failed', 'cancelled', 'interrupted'].includes(job.status) && <Button disabled={busy[job.id]} onClick={() => run(job, 'retry')}><RefreshCw size={15}/>{t('Retry from start')}</Button>}
      </> : null,
    };
  });
  return <><TaskRows className="runtime-jobs" items={items} ariaLabel={t(library ? 'Local source files and outputs' : 'Local file tasks')}/>{inspect && <RasterDialog job={inspect} onClose={() => setInspect(null)}/>}</>;
}

export function RuntimeTasks({ areaBounds, areaPolygon }) {
  const { jobs } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const pending = jobs.filter(job => job.status !== 'succeeded');
  const completed = jobs.filter(job => job.status === 'succeeded');
  return <section className="runtime-section" aria-label={t('Local file tasks')}><h2>{t('Tasks needing attention')}</h2><Connection compact/>
    {pending.length ? <RuntimeJobRows jobs={pending} areaBounds={areaBounds} areaPolygon={areaPolygon}/> : <Surface variant="inset" className="runtime-empty"><p>{t('No tasks need attention. Start a download in Explore; finished files are in My Data.')}</p></Surface>}
    {completed.length > 0 && <Disclosure className="runtime-task-history" summary={t('Completed task history · {count}', { count: number(completed.length) })}><RuntimeJobRows jobs={completed} areaBounds={areaBounds} areaPolygon={areaPolygon}/></Disclosure>}
  </section>;
}

export function RuntimeLibrary({ areaBounds, areaPolygon }) {
  const { jobs } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('all');
  const completed = jobs.filter(job => job.status === 'succeeded');
  const filtered = completed.filter(job => (kind === 'all' || (kind === 'derived') === (job.kind === 'raster_clip')) && [job.title, job.itemId, job.id].some(value => String(value || '').toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
  return <section className="runtime-section runtime-library" aria-label={t('Local source files and outputs')}><h2>{t('Available files')} <Badge>{number(completed.length)}</Badge></h2><Connection compact/>{completed.length > 0 && <><div className="library-filters"><label className="runtime-field">{t('Search local data')}<Input type="search" value={search} placeholder={t('Search name, scene or job ID')} onChange={event => setSearch(event.target.value)}/></label><label className="runtime-field">{t('Data type')}<Select value={kind} onChange={event => setKind(event.target.value)}><option value="all">{t('All files')}</option><option value="derived">{t('Derived outputs')}</option><option value="download">{t('Downloaded sources')}</option></Select></label></div><p className="runtime-results-count">{t('{shown} of {total} files', { shown: number(filtered.length), total: number(completed.length) })}</p></>}{filtered.length ? <RuntimeJobRows jobs={filtered} areaBounds={areaBounds} areaPolygon={areaPolygon} library/> : <Surface variant="inset" className="runtime-empty"><p>{t(completed.length ? 'No local files match these filters.' : 'Completed downloads appear here with their local path, source and checksum.')}</p></Surface>}</section>;
}
