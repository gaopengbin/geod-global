import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { Download, FolderOpen, RefreshCw, X, CheckCircle2, AlertCircle, HardDrive } from 'lucide-react';
import { desktopAvailable, downloadableAssets, formatBytes, runtimeRequest } from './runtime-client.js';
import './runtime.css';

const RuntimeContext = createContext(null);
export function RuntimeProvider({ children }) {
  const [jobs, setJobs] = useState([]);
  const [health, setHealth] = useState(null);
  const [error, setError] = useState('');
  const mounted = useRef(true);
  const refresh = useCallback(async () => {
    try {
      const [info, records] = await Promise.all([runtimeRequest('health'), runtimeRequest('list')]);
      if (mounted.current) { setHealth(info); setJobs(Array.isArray(records) ? records : records.jobs || []); setError(''); }
    } catch (e) {
      if (mounted.current) { setHealth(null); setError(e.message); }
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let timer;
    const poll = async () => { await refresh(); if (mounted.current) timer = setTimeout(poll, 1800); };
    poll();
    return () => { mounted.current = false; clearTimeout(timer); };
  }, [refresh]);
  const act = useCallback(async (operation, payload) => {
    const result = await runtimeRequest(operation, payload);
    await refresh();
    return result;
  }, [refresh]);
  return <RuntimeContext.Provider value={{ jobs, health, error, refresh, act }}>{children}</RuntimeContext.Provider>;
}

function Connection({ compact = false }) {
  const { health, error, refresh } = useContext(RuntimeContext);
  if (health) return <p className="runtime-connection"><span className="runtime-dot" />{desktopAvailable() ? 'Desktop task service' : 'Local task service'} connected{!compact && <span className="runtime-path">{health.storageRoot}</span>}</p>;
  return <div className="runtime-disconnected"><AlertCircle size={16}/><div><strong>Local task service is offline</strong><p>Open GeoD Global Desktop, or run <code>npm run runtime</code> beside the browser preview.</p>{error && <small>{error}</small>}</div><button className="icon-btn" aria-label="Reconnect task service" onClick={refresh}><RefreshCw size={16}/></button></div>;
}

export function DownloadAssetButton({ scene }) {
  const { health, act } = useContext(RuntimeContext);
  const [open, setOpen] = useState(false);
  const [assetKey, setAssetKey] = useState('scl');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const dialog = useRef(null);
  const options = downloadableAssets(scene);
  const asset = options.find(item => item.key === assetKey) || options[0];
  useEffect(() => { if (open && dialog.current) dialog.current.showModal(); }, [open]);
  const close = () => { if (!busy) { setOpen(false); setError(''); } };
  const start = async () => {
    setBusy(true); setError('');
    try {
      await act('create', { itemId: scene.id, assetKey: asset.key, href: asset.href, mediaType: asset.type, title: `${scene.id} · ${asset.key.toUpperCase()}` });
      setOpen(false);
      location.hash = 'Tasks';
    } catch (e) { setError(e.message); }
    finally { setBusy(false); }
  };
  return <>
    <button className="button primary" disabled={!options.length} onClick={() => { setAssetKey(options[0]?.key); setOpen(true); }}><Download size={15}/>Download source asset</button>
    {open && <dialog ref={dialog} className="runtime-dialog" onCancel={event => { if (busy) event.preventDefault(); else close(); }} onClick={event => { if (event.target === event.currentTarget) close(); }} aria-labelledby="download-title">
      <header><h2 id="download-title">Download source asset</h2><button className="icon-btn" aria-label="Close download" disabled={busy} onClick={close}><X size={20}/></button></header>
      <div className="runtime-dialog-body">
        <p className="mono runtime-wrap">{scene.id}</p>
        <label className="runtime-field">Asset<select aria-label="Download asset" disabled={busy} value={asset?.key || ''} onChange={event => setAssetKey(event.target.value)}>{options.map(option => <option value={option.key} key={option.key}>{option.key === 'scl' ? 'Scene classification · GeoTIFF · 20 m' : option.key === 'visual' ? 'True color · GeoTIFF · 10 m' : 'Thumbnail · JPEG · overview only'}</option>)}</select></label>
        <div className="notice"><HardDrive size={17}/><span>The complete source file is saved locally. Area clipping and reprojection are not applied. Large files may take time; the current limit is 512 MiB per file.</span></div>
        <dl className="runtime-details"><dt>Source</dt><dd>Earth Search / Sentinel-2 L2A</dd><dt>Asset</dt><dd><a href={asset?.href} target="_blank" rel="noreferrer">{asset?.title || asset?.key}</a></dd><dt>Save under</dt><dd className="runtime-wrap">{health?.storageRoot || 'Local task service required'}</dd><dt>Checks</dt><dd>Byte count, file signature and SHA-256. This does not validate raster geometry or scientific values.</dd></dl>
        <Connection compact/>
        {error && <p className="runtime-error" role="alert">{error}</p>}
      </div><footer><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button primary" disabled={busy || !health || !asset} onClick={start}><Download size={15}/>{busy ? 'Starting…' : 'Start download'}</button></footer>
    </dialog>}
  </>;
}

function JobCard({ job, library = false }) {
  const { act } = useContext(RuntimeContext);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const active = ['queued', 'running'].includes(job.status);
  const run = async operation => { setBusy(true); setError(''); try { await act(operation, { id: job.id }); } catch (e) { setError(e.message); } finally { setBusy(false); } };
  const progress = job.totalBytes > 0 ? Math.min(100, job.bytesDownloaded / job.totalBytes * 100) : null;
  return <article className="runtime-job">
    <div className="runtime-job-heading"><div><h3>{job.title || job.itemId}</h3><p>{job.assetKey.toUpperCase()} · {job.id}</p></div><span className={'badge ' + (job.status === 'succeeded' ? 'green' : ['failed', 'interrupted'].includes(job.status) ? 'red' : 'blue')}>{job.status === 'succeeded' ? 'Downloaded' : job.status}</span></div>
    {active && <progress aria-label="Download progress" value={progress ?? undefined} max="100"/>}
    <div className="runtime-job-status"><span>{formatBytes(job.bytesDownloaded)}{job.totalBytes ? ` / ${formatBytes(job.totalBytes)}` : ''}{active && progress !== null ? ` · ${Math.floor(progress)}% transferred` : ''}</span><span>{job.status === 'succeeded' ? <><CheckCircle2 size={14}/> File saved · SHA-256 recorded</> : active ? 'Downloading original asset' : 'No completed artifact from this attempt'}</span></div>
    {job.error && <p className="runtime-error">{typeof job.error === 'string' ? job.error : job.error.message}</p>}
    {job.status === 'succeeded' && <details open={library}><summary>File and provenance</summary><dl className="runtime-details"><dt>File</dt><dd className="mono runtime-wrap">{job.outputPath}</dd><dt>SHA-256</dt><dd className="mono runtime-wrap">{job.sha256}</dd><dt>Source</dt><dd className="runtime-wrap"><a href={job.href} target="_blank" rel="noreferrer">{job.href}</a></dd><dt>Validation</dt><dd>Transfer size and file signature checked. Raster structure, projection and values are not validated by the app yet.</dd></dl></details>}
    <div className="row-actions">
      {active && <button className="button" disabled={busy} onClick={() => run('cancel')}><X size={15}/>Cancel download</button>}
      {['failed', 'cancelled', 'interrupted'].includes(job.status) && <button className="button" disabled={busy} onClick={() => run('retry')}><RefreshCw size={15}/>Retry from start</button>}
      {job.status === 'succeeded' && desktopAvailable() && <button className="button" disabled={busy} onClick={() => run('reveal')}><FolderOpen size={15}/>Show in folder</button>}
    </div>{error && <p role="alert" className="runtime-error">{error}</p>}
  </article>;
}

export function RuntimeTasks() {
  const { jobs } = useContext(RuntimeContext);
  return <section className="runtime-section" aria-label="Real download tasks"><h2>Downloads</h2><Connection/>{jobs.length ? <div className="runtime-jobs">{jobs.map(job => <JobCard key={job.id} job={job}/>)}</div> : <p className="runtime-empty">Select a scene, then choose “Download source asset”. Download tasks and file checks are saved by the local service.</p>}</section>;
}

export function RuntimeLibrary() {
  const { jobs } = useContext(RuntimeContext);
  const completed = jobs.filter(job => job.status === 'succeeded');
  return <section className="runtime-section" aria-label="Downloaded source files"><h2>Downloaded source files <span className="badge">{completed.length}</span></h2><Connection/>{completed.length ? <div className="runtime-jobs">{completed.map(job => <JobCard key={job.id} job={job} library/>)}</div> : <p className="runtime-empty">Completed downloads appear here with their local path, source and checksum.</p>}</section>;
}
