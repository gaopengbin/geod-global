import React, { useContext, useEffect, useRef, useState } from 'react';
import { Database, Download, FolderOpen, Plus, Search, Scan, ShieldCheck, Trash2 } from 'lucide-react';
import { Badge, Button, DatePicker, Disclosure, Input, Modal, Select, Spinner, Surface, Table, THead, TBody, TR, TH, TD } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { stacRequest } from './stac-client.js';
import { validQueryBounds } from './features-client.js';
import './stac.css';

const selectionKey = (snapshotId, assetKey) => JSON.stringify([snapshotId, assetKey]);
const StacRasterMap = React.lazy(() => import('./stac-map.jsx'));
const parseBounds = text => text.split(',').map(value => value.trim() === '' ? NaN : Number(value));
function SourceMetadata({ item, assetKey }) {
  const { t, date } = useI18n();
  const asset = item.assets.find(asset => asset.key === assetKey);
  const direct = item.assets.some(asset => asset.metadata?.kind === 'direct-raster');
  return <Disclosure summary={t('Original item metadata')}><dl className="stac-details">
    <dt>{t('Item identifier')}</dt><dd>{item.itemId}</dd><dt>{t('Collection')}</dt><dd>{item.collectionId || t('Not declared')}</dd>
    <dt>{t('Observation time')}</dt><dd>{item.datetime ? date(item.datetime) : item.startDatetime || item.endDatetime ? `${item.startDatetime || '…'} / ${item.endDatetime || '…'}` : t('Not declared')}</dd>
    <dt>{t('Retrieved')}</dt><dd>{date(item.retrievedAt)}</dd><dt>{t(direct ? 'Connection receipt SHA-256' : 'Metadata SHA-256')}</dt><dd className="mono">{item.documentSha256}</dd>
    {direct && <><dt>{t('Receipt scope')}</dt><dd>{t('The connection receipt records the URL and file-header probe. The downloaded file has a separate SHA-256.')}</dd></>}
    {item.provenance && <><dt>{t('Metadata document')}</dt><dd>{item.provenance.documentUrl}</dd>{item.provenance.collection && <><dt>{t('Declared license')}</dt><dd>{item.provenance.collection.license || t('Not declared')}</dd><dt>{t('Original collection metadata')}</dt><dd><pre>{JSON.stringify(item.provenance.collection, null, 2)}</pre></dd></>}{item.provenance.search && <><dt>{t('Original search')}</dt><dd><pre>{JSON.stringify(item.provenance.search, null, 2)}</pre></dd></>}<dt>{t('Metadata receipts')}</dt><dd><pre>{JSON.stringify(item.provenance.metadataDocuments, null, 2)}</pre></dd></>}
    {item.provenance?.documentRequest && <><dt>{t('Original page request')}</dt><dd><pre>{JSON.stringify(item.provenance.documentRequest, null, 2)}</pre></dd></>}
    <dt>{t('Original properties')}</dt><dd><pre>{JSON.stringify(item.properties, null, 2)}</pre></dd>
    {asset && <><dt>{t('Original asset metadata')}</dt><dd><pre>{JSON.stringify(asset.metadata, null, 2)}</pre></dd></>}
  </dl></Disclosure>;
}

export function StacSourcesPanel({ onOpen, bounds }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <><Surface className="settings-panel stac-settings"><div><Database size={20}/><h2>{t('Custom raster sources')}</h2></div>
    <p>{t('Connect a STAC catalog, a single STAC item, or a public GeoTIFF URL. Choose the original files and keep their source metadata with a project.')}</p>
    <Button onClick={() => onOpen ? onOpen() : setOpen(true)}><Plus size={15}/>{t('Manage raster sources')}</Button>
  </Surface>{open && <StacSourceDialog areaBounds={bounds} onClose={() => setOpen(false)}/>}</>;
}

export function StacSourceDialog({ areaBounds, currentProject, onClose, onSaved }) {
  const { t, locale, date, number } = useI18n();
  const { projects = [], refresh = async () => {} } = useContext(RuntimeContext) || {};
  const [connections, setConnections] = useState([]);
  const [connectionId, setConnectionId] = useState('new');
  const [kind, setKind] = useState('api');
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [collectionId, setCollectionId] = useState('');
  const [region, setRegion] = useState((currentProject?.bounds || areaBounds || []).join(', '));
  const [useTime, setUseTime] = useState(false);
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');
  const [items, setItems] = useState([]);
  const [page, setPage] = useState(null);
  const [selected, setSelected] = useState({});
  const [destination, setDestination] = useState(currentProject?.id || 'new');
  const [projectName, setProjectName] = useState('');
  const [saved, setSaved] = useState(null);
  const [queued, setQueued] = useState(false);
  const [busy, setBusy] = useState('loading');
  const [error, setError] = useState('');
  const request = useRef(null);
  const connection = connections.find(item => item.id === connectionId);
  const collection = connection?.collections.find(item => item.id === collectionId);
  const bounds = parseBounds(region);
  const selections = Object.values(selected);
  const choices = currentProject && !projects.some(item => item.id === currentProject.id) ? [currentProject, ...projects] : projects;
  const targetProject = choices.find(item => item.id === destination);
  const projectBounds = targetProject?.bounds || bounds;
  const clearResults = () => { setItems([]); setPage(null); setSelected({}); setSaved(null); setQueued(false); setError(''); };
  useEffect(() => {
    const abort = new AbortController(); request.current = abort;
    stacRequest('list', {}, abort.signal).then(setConnections).catch(cause => { if (!abort.signal.aborted) setError(cause.message); }).finally(() => { if (!abort.signal.aborted) setBusy(''); });
    return () => { abort.abort(); request.current?.abort(); };
  }, []);
  async function perform(label, work) {
    if (busy && busy !== 'loading') return;
    request.current?.abort(); const abort = new AbortController(); request.current = abort;
    setBusy(label); setError('');
    try { await work(abort.signal); }
    catch (cause) { if (!abort.signal.aborted) setError(cause.message); }
    finally { if (!abort.signal.aborted) setBusy(''); }
  }
  async function loadConnection(next, signal) {
    clearResults(); setConnectionId(next.id); setCollectionId(next.collections[0]?.id || '');
    if (next.kind !== 'api') setItems(await Promise.all(next.snapshotIds.map(id => stacRequest('snapshot', { id }, signal))));
  }
  const connect = event => { event.preventDefault(); perform('connect', async signal => {
    const next = await stacRequest('connect', { name: name.trim(), url: url.trim(), kind }, signal);
    setConnections(previous => [...previous.filter(item => item.id !== next.id), next]); await loadConnection(next, signal);
  }); };
  const search = more => perform('search', async signal => {
    if (!more) { setItems([]); setSelected({}); setSaved(null); setQueued(false); setPage(null); }
    const result = await stacRequest('search', { connectionId, collectionId, bounds, limit: 20,
      ...(useTime ? { datetime: `${start}T00:00:00Z/${end}T23:59:59Z` } : {}), ...(more ? { cursor: page.nextCursor } : {}) }, signal);
    setItems(previous => more ? [...new Map([...previous, ...result.items].map(item => [JSON.stringify([item.collectionId, item.itemId]), item])).values()] : result.items); setPage(result);
  });
  const save = () => perform('save', async signal => {
    const project = await stacRequest('project', { ...(destination === 'new' ? { name: projectName.trim() } : { projectId: destination }), bounds: projectBounds, selections }, signal);
    setSaved(project); setQueued(false); await refresh(); onSaved?.(project);
  });
  const download = () => perform('download', async signal => { const result = await stacRequest('downloads', { projectId: saved.id, selections: saved.canonicalSelections || selections }, signal); setQueued(result.jobs?.length && result.jobs.every(job => job.status === 'succeeded') ? 'ready' : 'queued'); await refresh(); });
  const updateFilter = change => { clearResults(); change(); };
  return <Modal title={t('Custom raster sources')} closeLabel={t('Close')} onClose={onClose} closeDisabled={Boolean(busy)} className="stac-source-dialog" wide>
    <div className="stac-source-body">
      <p className="stac-help">{t('Choose original raster files. The catalog describes the source; local inspection reads the downloaded file itself.')}</p>
      <div className="stac-row"><label className="stac-field"><span>{t('Saved raster source')}</span><Select aria-label={t('Saved raster source')} disabled={Boolean(busy)} value={connectionId} onChange={event => {
        if (event.target.value === 'new') { clearResults(); setConnectionId('new'); return; }
        const next = connections.find(item => item.id === event.target.value); perform('source', signal => loadConnection(next, signal));
      }}><option value="new">{t('Add a raster source')}</option>{connections.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</Select></label>
      {connection && <Button size="icon" disabled={Boolean(busy)} aria-label={t('Forget raster connection')} onClick={() => perform('forget', async signal => { await stacRequest('forget', { id: connectionId }, signal); setConnections(previous => previous.filter(item => item.id !== connectionId)); setConnectionId('new'); clearResults(); })}><Trash2 size={16}/></Button>}</div>
      {!connection && <form className="stac-connect" onSubmit={connect}>
        <div className="stac-grid"><label className="stac-field"><span>{t('Source type')}</span><Select aria-label={t('Source type')} value={kind} disabled={Boolean(busy)} onChange={event => setKind(event.target.value)}><option value="api">STAC API</option><option value="item">{t('STAC item')}</option><option value="raster">{t('COG / GeoTIFF URL')}</option></Select></label>
          <label className="stac-field"><span>{t('Source name')}</span><Input aria-label={t('Source name')} value={name} maxLength={80} disabled={Boolean(busy)} onChange={event => setName(event.target.value)} required/></label></div>
        <label className="stac-field"><span>{t('Source URL')}</span><Input aria-label={t('Source URL')} placeholder="https://…" value={url} disabled={Boolean(busy)} onChange={event => setUrl(event.target.value)} required/></label>
        <p className="stac-help">{t('Use a public HTTPS source you are authorized to access. Account-protected providers remain in Data source authorization.')}</p>
        <Button type="submit" variant="primary" disabled={Boolean(busy) || !name.trim() || !url.trim()}><Plus size={15}/>{t('Connect raster source')}</Button>
      </form>}
      {connection && <>
        <div className="stac-connection-summary"><Badge>{connection.kind === 'api' ? 'STAC API' : t(connection.kind === 'item' ? 'STAC item' : 'COG / GeoTIFF URL')}</Badge><span>{connection.url}</span></div>
        {connection.kind === 'api' && <>
          <label className="stac-field"><span>{t('Collection')}</span><Select aria-label={t('Raster collection')} disabled={Boolean(busy) || !connection.collections.length} value={collectionId} onChange={event => updateFilter(() => setCollectionId(event.target.value))}>{connection.collections.map(item => <option key={item.id} value={item.id}>{item.title || item.id}</option>)}</Select></label>
          {collection && <Disclosure summary={t('Collection details')}><p className="stac-help">{collection.description}</p><dl className="stac-details"><dt>{t('Collection')}</dt><dd>{collection.id}</dd><dt>{t('Declared license')}</dt><dd>{collection.license || t('Not declared')}</dd></dl></Disclosure>}
          <label className="stac-field"><span>{t('Search bounds · west, south, east, north')}</span><Input aria-label={t('Raster search bounds')} value={region} disabled={Boolean(busy)} onChange={event => updateFilter(() => setRegion(event.target.value))}/></label>
          <label className="stac-check"><Input type="checkbox" checked={useTime} disabled={Boolean(busy)} onChange={event => updateFilter(() => setUseTime(event.target.checked))}/>{t('Limit by observation date')}</label>
          {useTime && <div className="stac-grid"><label className="stac-field"><span>{t('Start date')}</span><DatePicker aria-label={t('Raster start date')} locale={locale} value={start} disabled={Boolean(busy)} onChange={event => updateFilter(() => setStart(event.target.value))}/></label><label className="stac-field"><span>{t('End date')}</span><DatePicker aria-label={t('Raster end date')} locale={locale} value={end} disabled={Boolean(busy)} onChange={event => updateFilter(() => setEnd(event.target.value))}/></label></div>}
          {!connection.capabilities.searchGet && !connection.capabilities.searchPost && <p className="stac-help">{t('This source does not advertise supported Item Search. Its connection metadata is retained.')}</p>}
          <Button variant="primary" disabled={Boolean(busy) || !(connection.capabilities.searchGet || connection.capabilities.searchPost) || !collectionId || !validQueryBounds(bounds) || useTime && (!start || !end || start > end)} onClick={() => search(false)}><Search size={15}/>{t('Search raster items')}</Button>
        </>}
        <div className="stac-items">{items.map(item => <Surface as="article" variant="inset" className="stac-item" key={item.id}>
          <div className="stac-item-heading"><strong>{item.title || item.itemId}</strong>{item.datetime && <span>{date(item.datetime)}</span>}</div>
          {!item.assets.some(asset => asset.metadata?.kind === 'direct-raster') && <p className="stac-item-id">{item.itemId}</p>}
          {item.warnings?.map((warning, index) => <p className="stac-help" key={index}>{t(warning)}</p>)}
          <div className="stac-assets">{item.assets.map(asset => { const key = selectionKey(item.id, asset.key); return <div className="stac-asset" key={asset.key}><label className="stac-check"><Input type="checkbox" aria-label={`${t('Select asset')} · ${item.itemId} · ${asset.key}`} checked={Boolean(selected[key])} disabled={Boolean(busy) || !asset.eligible || selections.length >= 32 && !selected[key]} onChange={event => {
            const checked = event.target.checked; setSelected(previous => { const next = { ...previous }; if (checked) next[key] = { snapshotId: item.id, assetKey: asset.key }; else delete next[key]; return next; }); setSaved(null); setQueued(false);
          }}/><span><strong>{asset.title || asset.key}</strong><small>{asset.key} · {asset.mediaType || t('Media type not declared')}</small></span></label>
          {!asset.eligible && <p className="stac-help">{asset.reason || t('This asset is not available for raster download.')}</p>}
          <Disclosure summary={t('Asset source details')}><dl className="stac-details"><dt>{t('Source URL')}</dt><dd>{asset.href}</dd><dt>{t('Declared roles')}</dt><dd>{asset.roles.join(', ') || t('Not declared')}</dd><dt>{t('Original asset metadata')}</dt><dd><pre>{JSON.stringify(asset.metadata, null, 2)}</pre></dd></dl></Disclosure></div>; })}</div>
          <SourceMetadata item={item}/>
        </Surface>)}</div>
        {page && <div className="stac-search-footer"><span className="stac-help" role="status">{t(page.limitReached ? 'Search limit reached. Narrow the area, dates or collection.' : page.complete ? '{count} items loaded · search complete' : '{count} items loaded · more available', { count: number(items.length) })}</span>{page.nextCursor && <Button disabled={Boolean(busy)} onClick={() => search(true)}>{t('Load more items')}</Button>}</div>}
        {items.length > 0 && <Surface variant="inset" className="stac-destination">
          <strong>{t('{count} original assets selected', { count: number(selections.length) })}</strong>
          <label className="stac-field"><span>{t('Save into project')}</span><Select aria-label={t('Save into project')} value={destination} disabled={Boolean(busy)} onChange={event => { setDestination(event.target.value); setSaved(null); setQueued(false); }}><option value="new">{t('New project')}</option>{choices.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></label>
          {destination === 'new' && <><label className="stac-field"><span>{t('Project name')}</span><Input aria-label={t('Raster project name')} value={projectName} maxLength={120} disabled={Boolean(busy)} onChange={event => { setProjectName(event.target.value); setSaved(null); }}/></label>{connection.kind !== 'api' && <label className="stac-field"><span>{t('Project bounds · west, south, east, north')}</span><Input aria-label={t('Raster project bounds')} value={region} disabled={Boolean(busy)} onChange={event => { setRegion(event.target.value); setSaved(null); }}/></label>}</>}
          <p className="stac-help">{t('The area organizes this project. Downloads keep the complete original asset; no clipping or band conversion is applied.')}</p>
          <div className="stac-actions"><Button variant={saved ? 'secondary' : 'primary'} disabled={Boolean(busy) || !selections.length || !validQueryBounds(projectBounds) || destination === 'new' && !projectName.trim() || Boolean(saved)} onClick={save}><FolderOpen size={15}/>{t(saved ? 'Selection saved' : 'Save asset selection')}</Button>
            {saved && <Button variant="primary" disabled={Boolean(busy) || Boolean(queued)} onClick={download}><Download size={15}/>{t(queued === 'ready' ? 'Original files ready' : queued ? 'Downloads queued' : 'Download selected originals')}</Button>}</div>
          {saved && <p className="stac-help" role="status">{t('Saved in {name}.', { name: saved.name })} <a href={`#My%20Data?project=${encodeURIComponent(saved.id)}`} onClick={onClose}>{t('Open project')}</a></p>}
        </Surface>}
      </>}
      {busy && <p className="stac-status" role="status"><Spinner size={15}/>{t(busy === 'search' ? 'Reading raster items…' : busy === 'download' ? 'Queueing originals…' : 'Loading raster source…')}</p>}
      {error && <p className="stac-error" role="alert">{error}</p>}
    </div>
  </Modal>;
}

export function StacProjectAssets({ project, jobs, onChanged }) {
  const { t } = useI18n();
  const { refresh, health } = useContext(RuntimeContext) || {};
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [verification, setVerification] = useState(null);
  const [open, setOpen] = useState(false);
  const selections = project.stacItems || [];
  const matching = item => jobs.filter(job => job.stacSource?.snapshotId === item.snapshotId && job.stacSource?.assetKey === item.assetKey);
  const missing = selections.filter(item => !matching(item).some(job => ['queued', 'running', 'succeeded'].includes(job.status)));
  const completed = selections.filter(item => matching(item).some(job => job.status === 'succeeded'));
  async function download() { setBusy(true); setError(''); setVerification(null); try { await stacRequest('downloads', { projectId: project.id, selections: missing.map(({ snapshotId, assetKey }) => ({ snapshotId, assetKey })) }); await refresh?.(); } catch (cause) { setError(cause.message); } finally { setBusy(false); } }
  async function verify() {
    setBusy(true); setError(''); setVerification(null);
    try {
      const result = await stacRequest('downloads', { projectId: project.id, selections: completed.map(({ snapshotId, assetKey }) => ({ snapshotId, assetKey })) });
      setVerification({ verified: result.jobs.filter(job => job.status === 'succeeded').length, queued: result.jobs.filter(job => job.status === 'queued').length, running: result.jobs.filter(job => job.status === 'running').length });
      await refresh?.();
    } catch (cause) { setError(cause.message); }
    finally { setBusy(false); }
  }
  return <section className="stac-project-assets"><div className="stac-actions"><strong>{t('Custom raster assets')}</strong><Button size="sm" onClick={() => setOpen(true)}><Plus size={15}/>{t('Add raster assets')}</Button>{completed.length > 0 && <Button size="icon" variant="secondary" aria-label={t('Verify downloaded originals')} tooltip={t('Verify downloaded originals')} disabled={!health || busy} onClick={verify}>{busy ? <Spinner size={15}/> : <ShieldCheck size={15}/>}</Button>}{selections.length > 0 && <Button size="sm" variant="primary" disabled={!health || busy || !missing.length} onClick={download}><Download size={15}/>{t('Download missing originals')}</Button>}</div>
    {verification && <p className="stac-help" role="status">{t('{verified} verified · {queued} repair downloads queued', verification)}{verification.running > 0 && <> · {t('{count} repairs already running', { count: verification.running })}</>}</p>}
    {selections.length > 0 && <Disclosure summary={t('Review original assets · {count}', { count: selections.length })}><ul className="stac-project-list">{selections.map(item => <li key={selectionKey(item.snapshotId, item.assetKey)}><strong>{item.title}</strong><span>{item.serviceName} · {item.itemId} · {item.assetKey}</span><span>{item.mediaType || t('Media type not declared')}</span><span>{item.href}</span></li>)}</ul></Disclosure>}
    {error && <p className="stac-error" role="alert">{error}</p>}
    {open && <StacSourceDialog currentProject={project} onClose={() => setOpen(false)} onSaved={onChanged}/>}
  </section>;
}

function StacInspectionSource({ source, job }) { return <SourceMetadata item={source} assetKey={job.stacSource.assetKey}/>; }
export function StacRasterInspection({ job, onClose }) {
  return <RasterFileInspector job={job} onClose={onClose} request={stacRequest} sourceOperation="snapshot" sourceId={job.stacSource.snapshotId} SourceDetails={StacInspectionSource}/>;
}
export function RasterFileInspector({ job, onClose, request, sourceOperation, sourceId, SourceDetails, kind = 'original' }) {
  const { t, number } = useI18n();
  const [data, setData] = useState(null);
  const [snapshot, setSnapshot] = useState(null);
  const [error, setError] = useState('');
  const [pixel, setPixel] = useState(null);
  const [position, setPosition] = useState({ column: '0', row: '0' });
  const [busy, setBusy] = useState(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true; const abort = new AbortController();
    Promise.all([request('inspect', { id: job.id }, abort.signal), request(sourceOperation, { id: sourceId }, abort.signal)])
      .then(([inspection, source]) => { if (job.sha256 && inspection.sha256 !== job.sha256) throw new Error('The inspected file differs from the saved download.'); setData(inspection); setSnapshot(source); })
      .catch(cause => { if (!abort.signal.aborted) setError(cause.message); });
    return () => { active.current = false; abort.abort(); };
  }, [job.id, job.sha256, request, sourceOperation, sourceId]);
  const column = Number(position.column), row = Number(position.row);
  const validPosition = position.column.trim() !== '' && position.row.trim() !== '' && Number.isSafeInteger(column) && Number.isSafeInteger(row) && column >= 0 && row >= 0 && column < data?.width && row < data?.height;
  async function readPixel(position) { if (busy) return; setPosition({ column: String(position.column), row: String(position.row) }); setBusy(true); setError(''); setPixel(null); try {
    const next = await request('pixel', { id: job.id, ...position });
    if (next.sha256 !== data.sha256 || next.values.length !== data.bands.length) throw new Error('The pixel does not match the inspected raster.');
    if (active.current) setPixel(next);
  } catch (cause) { if (active.current) setError(cause.message); } finally { if (active.current) setBusy(false); } }
  const sample = event => { event.preventDefault(); readPixel({ column, row }); };
  return <Modal title={t(kind === 'coverage' ? 'Inspect coverage subset' : 'Inspect original raster')} closeLabel={t('Close')} onClose={onClose} wide className="stac-inspection"><div className="stac-source-body">
    <p className="stac-item-id">{job.title || job.itemId}{job.stacSource && ` · ${job.stacSource.assetKey}`}</p>
    {kind === 'coverage' && <p className="stac-help">{t('Server-generated coverage subset. Pixel values come from the downloaded file, without inferred scientific units.')}</p>}
    {!data && !error && <p role="status"><Spinner size={15}/>{t('Reading local raster…')}</p>}
    {data && <>{data.previewDataUrl && <React.Suspense fallback={<p role="status">{t('Loading local map…')}</p>}><StacRasterMap data={data} onPixel={readPixel} disabled={busy} kind={kind}/></React.Suspense>}<p className="stac-help">{number(data.width)} × {number(data.height)} · {t('Bands')}: {data.bands.length} · {data.crs || t('Not declared')}</p>      <form className="stac-pixel-form" onSubmit={sample}><label className="stac-field"><span>{t('Column (from 0)')}</span><Input aria-label={t('Pixel column')} inputMode="numeric" disabled={busy} value={position.column} onChange={event => { setPosition({ ...position, column: event.target.value }); setPixel(null); }}/></label><label className="stac-field"><span>{t('Row (from 0)')}</span><Input aria-label={t('Pixel row')} inputMode="numeric" disabled={busy} value={position.row} onChange={event => { setPosition({ ...position, row: event.target.value }); setPixel(null); }}/></label><Button type="submit" disabled={busy || !validPosition}>{t('Read raw pixel')}</Button></form>
      {pixel && <Surface variant="inset" className="stac-pixel-result" role="status">{pixel.values.map((value, index) => <span key={index}>{t('Band')} {index + 1}: <strong>{value === null ? t('Non-finite value') : String(value)}{pixel.noData[index] ? ` · ${t('NoData')}` : ''}</strong></span>)}</Surface>}
<Disclosure summary={t('File metadata and display')}><div className="stac-inspection-grid"><div>{data.previewDataUrl ? <><img className="stac-preview" src={data.previewDataUrl} alt={t('First-band display preview')}/><p className="stac-help">{t('Display preview of band 1. The stretch affects this preview only; raw pixels remain unchanged.')}{data.displayRange && ` ${data.displayRange.join(' – ')}`}</p></> : <Surface variant="inset" className="stac-preview-empty"><Scan size={24}/><p>{t('No display preview is available. File metadata remains available below.')}</p></Surface>}</div>
      <dl className="stac-details"><dt>{t('Raster dimensions')}</dt><dd>{number(data.width)} × {number(data.height)}</dd><dt>{t('Bands')}</dt><dd>{number(data.bands.length)}</dd><dt>{t('CRS from file')}</dt><dd>{data.crs || t('Not declared')}</dd><dt>{t('File bounds')}</dt><dd>{data.bounds?.join(', ') || t('Not declared')}</dd><dt>{t('Pixel interpretation')}</dt><dd>{data.pixelInterpretation || t('Not declared')}</dd><dt>{t('File transform')}</dt><dd>{data.transform?.join(', ') || t('Not declared')}</dd></dl></div>
      <div className="stac-table-wrap"><Table className="stac-table"><THead><TR><TH>{t('Band')}</TH><TH>{t('Data type')}</TH><TH>{t('No-data value')}</TH></TR></THead><TBody>{data.bands.map(band => <TR key={band.index}><TD>{band.index}</TD><TD>{band.dataType}</TD><TD>{band.nodata === null ? t('Not declared') : String(band.nodata)}</TD></TR>)}</TBody></Table></div>
      {data.limitations.length > 0 && <ul className="stac-limitations">{data.limitations.map((item, index) => <li key={index}>{t(item)}</li>)}</ul>}
      <dl className="stac-details"><dt>{t('File SHA-256')}</dt><dd className="mono">{data.sha256}</dd>{job.stacSource && <><dt>{t('Original asset key')}</dt><dd>{job.stacSource.assetKey}</dd></>}<dt>{t('Source URL')}</dt><dd>{job.href}</dd></dl>
</Disclosure>    </>}
    {snapshot && <SourceDetails source={snapshot} job={job}/>}
    {error && <p role="alert" className="stac-error">{error}</p>}
  </div></Modal>;
}
