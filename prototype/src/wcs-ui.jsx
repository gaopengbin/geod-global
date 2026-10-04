import React, { useContext, useEffect, useRef, useState } from 'react';
import { Database, Download, FolderOpen, Plus, Search, ShieldCheck, Trash2 } from 'lucide-react';
import { Badge, Button, Disclosure, Input, Modal, Select, Spinner, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { RasterFileInspector } from './stac-ui.jsx';
import { validQueryBounds } from './features-client.js';
import { wcsRequest } from './wcs-client.js';
import './wcs.css';

const readBounds = value => value.split(',').map(part => part.trim() ? Number(part) : NaN);
function CoverageFields({ description }) {
  const { t } = useI18n();
  return <Disclosure summary={t('Declared range fields · {count}', { count: description.fields.length })}><ul className="wcs-fields">{description.fields.map((field, index) => <li key={index}><strong>{field.name}</strong>{field.description && <p>{field.description}</p>}<dl className="stac-details"><dt>{t('Declared unit')}</dt><dd>{field.unit || t('Not declared')}</dd><dt>{t('Declared nil values')}</dt><dd>{field.nilValues.length ? field.nilValues.map((nil, i) => <span className="wcs-nil" key={i}>{nil.value}{nil.reason && ` · ${nil.reason}`}</span>) : t('Not declared')}</dd></dl></li>)}</ul><p className="stac-help">{t('These are service declarations. Local file inspection reads the delivered bands and NoData without inferring elevation or other scientific meaning.')}</p></Disclosure>;
}
export function WcsPlanDetails({ plan }) {
  const { t } = useI18n();
  const d = plan.description;
  return <Disclosure summary={t('Coverage subset source details')}><p className="stac-help">{t('A WCS service generates this coverage subset on its native grid. It is not an original product archive or a rendered map image.')}</p><dl className="stac-details">
    <dt>{t('Coverage identifier')}</dt><dd>{d.coverageId}</dd><dt>{t('Service')}</dt><dd>{d.serviceName}</dd>
    <dt>{t('Requested region')}</dt><dd>{plan.requestedBounds.join(', ')}</dd><dt>{t('Planned geographic extent')}</dt><dd>{plan.bounds.join(', ')}</dd>
    <dt>{t('Planned native grid')}</dt><dd>{plan.width} × {plan.height} · {d.crs}</dd><dt>{t('Native grid bounds')}</dt><dd>{plan.nativeBounds.join(', ')}</dd>
    <dt>{t('Declared CRS')}</dt><dd>{d.declaredCrs}</dd><dt>{t('Coverage axis labels')}</dt><dd>{d.axisLabels.join(', ')}</dd><dt>{t('Grid axis labels')}</dt><dd>{d.gridAxisLabels.join(', ')}</dd>
    <dt>{t('File transform')}</dt><dd>{plan.transform.join(', ')}</dd><dt>{t('Response format')}</dt><dd>{plan.format}</dd><dt>{t('Subset request')}</dt><dd>{plan.requestUrl}</dd>
    <dt>{t('Description request')}</dt><dd>{d.descriptionUrl}</dd><dt>{t('Description retrieved')}</dt><dd>{d.retrievedAt}</dd>
    <dt>{t('Capabilities SHA-256')}</dt><dd className="mono">{d.capabilitiesSha256}</dd><dt>{t('Coverage description SHA-256')}</dt><dd className="mono">{d.descriptionSha256}</dd><dt>{t('Saved plan identifier')}</dt><dd className="mono">{plan.id}</dd>
    {d.metadataLinks.length > 0 && <><dt>{t('Declared metadata links')}</dt><dd>{d.metadataLinks.map((link, index) => <span className="wcs-nil" key={index}>{link}</span>)}</dd></>}
  </dl><CoverageFields description={d}/>{[...new Set([...d.warnings, ...plan.warnings])].map((warning, index) => <p key={index} className="stac-help">{t(warning)}</p>)}</Disclosure>;
}
function WcsInspectionSource({ source }) { return <WcsPlanDetails plan={source}/>; }
export function WcsRasterInspection({ job, onClose }) {
  return <RasterFileInspector job={job} onClose={onClose} request={wcsRequest} sourceOperation="savedPlan" sourceId={job.wcsSource.planId} SourceDetails={WcsInspectionSource} kind="coverage"/>;
}
export function WcsSourcesPanel({ onOpen, bounds }) {
  const { t } = useI18n(); const [open, setOpen] = useState(false);
  return <><Surface className="settings-panel stac-settings"><div><Database size={20}/><h2>{t('Coverage services (WCS)')}</h2></div><p>{t('Connect a coverage service, review its grid and range fields, then request a bounded GeoTIFF subset into a project.')}</p><Button onClick={() => onOpen ? onOpen() : setOpen(true)}><Plus size={15}/>{t('Manage coverage services')}</Button></Surface>{open && <WcsSourceDialog areaBounds={bounds} onClose={() => setOpen(false)}/>}</>;
}

export function WcsSourceDialog({ areaBounds, currentProject, onClose, onSaved }) {
  const { t, number } = useI18n(); const { projects = [], refresh = async () => {} } = useContext(RuntimeContext) || {};
  const [connections, setConnections] = useState([]), [connectionId, setConnectionId] = useState('new'), [coverageId, setCoverageId] = useState('');
  const [name, setName] = useState(''), [url, setUrl] = useState(''), [filter, setFilter] = useState('');
  const [description, setDescription] = useState(null), [plan, setPlan] = useState(null);
  const [region, setRegion] = useState((currentProject?.bounds || areaBounds || []).join(', '));
  const [destination, setDestination] = useState(currentProject?.id || 'new'), [projectName, setProjectName] = useState('');
  const [saved, setSaved] = useState(null), [delivery, setDelivery] = useState('');
  const [busy, setBusy] = useState('loading'), [error, setError] = useState(''); const active = useRef(null);
  const connection = connections.find(item => item.id === connectionId), bounds = readBounds(region);
  const choices = currentProject && !projects.some(project => project.id === currentProject.id) ? [currentProject, ...projects] : projects;
  const target = choices.find(project => project.id === destination);
  const coverages = connection?.coverages.filter(item => `${item.id} ${item.title}`.toLocaleLowerCase().includes(filter.toLocaleLowerCase())) || [];
  const invalidatePlan = () => { setPlan(null); setSaved(null); setDelivery(''); setError(''); };
  const invalidateDescription = () => { setDescription(null); invalidatePlan(); };
  useEffect(() => {
    const abort = new AbortController(); active.current = abort;
    wcsRequest('list', {}, abort.signal).then(setConnections).catch(cause => { if (!abort.signal.aborted) setError(cause.message); }).finally(() => { if (!abort.signal.aborted) setBusy(''); });
    return () => active.current?.abort();
  }, []);
  async function perform(operation, work) {
    if (busy) return; active.current?.abort(); const abort = new AbortController(); active.current = abort; setBusy(operation); setError('');
    try { await work(abort.signal); } catch (cause) { if (!abort.signal.aborted) setError(cause.message); } finally { if (!abort.signal.aborted) setBusy(''); }
  }
  const choose = next => { setConnectionId(next?.id || 'new'); setCoverageId(next?.coverages[0]?.id || ''); setFilter(''); invalidateDescription(); };
  const filterCoverages = value => { setFilter(value); const matches = connection.coverages.filter(item => `${item.id} ${item.title}`.toLocaleLowerCase().includes(value.toLocaleLowerCase())); if (!matches.some(item => item.id === coverageId)) { setCoverageId(matches[0]?.id || ''); invalidateDescription(); } };
  const connect = event => { event.preventDefault(); perform('connect', async signal => { const value = await wcsRequest('connect', { name: name.trim(), url: url.trim() }, signal); setConnections(previous => [...previous.filter(item => item.id !== value.id), value]); choose(value); }); };
  const describe = () => perform('describe', async signal => { invalidateDescription(); setDescription(await wcsRequest('describe', { connectionId, coverageId }, signal)); });
  const prepare = () => perform('plan', async signal => { invalidatePlan(); setPlan(await wcsRequest('plan', { descriptionId: description.id, bounds }, signal)); });
  const save = () => perform('save', async signal => { const value = await wcsRequest('project', { ...(destination === 'new' ? { name: projectName.trim() } : { projectId: destination }), bounds: target?.bounds || bounds, selections: [{ planId: plan.id }] }, signal); setSaved(value); await refresh(); onSaved?.(value); });
  const download = () => perform('download', async signal => { const result = await wcsRequest('downloads', { projectId: saved.id, selections: [{ planId: plan.id }] }, signal); setDelivery(result.jobs.length && result.jobs.every(job => job.status === 'succeeded') ? 'ready' : 'queued'); await refresh(); });
  return <Modal title={t('Coverage services (WCS)')} closeLabel={t('Close')} onClose={onClose} closeDisabled={Boolean(busy)} wide className="wcs-source-dialog"><div className="stac-source-body">
    <p className="stac-help">{t('Request a server-generated coverage subset. The service preserves its native grid; no band meaning or measurement unit is assumed.')}</p>
    <div className="stac-row"><label className="stac-field"><span>{t('Saved coverage service')}</span><Select aria-label={t('Saved coverage service')} value={connectionId} disabled={Boolean(busy)} onChange={event => choose(connections.find(item => item.id === event.target.value))}><option value="new">{t('Add a coverage service')}</option>{connections.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</Select></label>{connection && <Button size="icon" aria-label={t('Forget coverage connection')} disabled={Boolean(busy)} onClick={() => perform('forget', async signal => { await wcsRequest('forget', { id: connectionId }, signal); setConnections(previous => previous.filter(item => item.id !== connectionId)); choose(null); })}><Trash2 size={16}/></Button>}</div>
    {!connection && <form onSubmit={connect} className="stac-connect"><label className="stac-field"><span>{t('Coverage service name')}</span><Input aria-label={t('Coverage service name')} value={name} maxLength={80} disabled={Boolean(busy)} onChange={event => setName(event.target.value)} required/></label><label className="stac-field"><span>{t('WCS service URL')}</span><Input aria-label={t('WCS service URL')} placeholder="https://…" value={url} disabled={Boolean(busy)} onChange={event => setUrl(event.target.value)} required/></label><p className="stac-help">{t('Enter a public HTTPS WCS 2.0.1 service you are authorized to access.')}</p><Button type="submit" variant="primary" disabled={Boolean(busy) || !name.trim() || !url.trim()}><Plus size={15}/>{t('Connect coverage service')}</Button></form>}
    {connection && <><div className="stac-connection-summary"><Badge>WCS 2.0.1</Badge><span>{connection.title} · {connection.url}</span></div>
      <Disclosure summary={t('Coverage service declarations')}><dl className="stac-details"><dt>{t('Access constraints')}</dt><dd>{connection.accessConstraints || t('Not declared')}</dd><dt>{t('Fees')}</dt><dd>{connection.fees || t('Not declared')}</dd><dt>{t('Attribution')}</dt><dd>{connection.attribution || t('Not declared')}</dd><dt>{t('Capabilities SHA-256')}</dt><dd className="mono">{connection.capabilitiesSha256}</dd><dt>{t('Declared formats')}</dt><dd>{connection.formats.join(', ')}</dd></dl><p className="stac-help">{t('Fees and access statements are source declarations, not an inferred dataset license.')}</p></Disclosure>
      {connection.coverages.length > 12 && <label className="stac-field"><span>{t('Find a coverage')}</span><Input type="search" aria-label={t('Find a coverage')} value={filter} disabled={Boolean(busy)} onChange={event => filterCoverages(event.target.value)}/></label>}
      <label className="stac-field"><span>{t('Coverage')}</span><Select aria-label={t('Coverage')} value={coverageId} disabled={Boolean(busy) || !coverages.length} onChange={event => { setCoverageId(event.target.value); invalidateDescription(); }}>{coverages.map(item => <option key={item.id} value={item.id}>{item.title || item.id}</option>)}</Select></label>
      {!connection.coverages.length && <p className="stac-help">{t('This service did not declare any coverages.')}</p>}
      <Button disabled={Boolean(busy) || !coverageId} onClick={describe}><Search size={15}/>{t('Read coverage description')}</Button>
      {description && <Surface variant="inset" className="wcs-description"><strong>{description.title}</strong><p className="stac-help">{description.width} × {description.height} · {description.crs} · {t('{count} range fields', { count: description.fields.length })}</p><CoverageFields description={description}/>{description.warnings.length > 0 && <Disclosure summary={t('Coverage limitations')}>{description.warnings.map((warning, index) => <p className="stac-help" key={index}>{t(warning)}</p>)}</Disclosure>}</Surface>}
      <label className="stac-field"><span>{t('Subset bounds · west, south, east, north')}</span><Input aria-label={t('Coverage subset bounds')} value={region} disabled={Boolean(busy)} onChange={event => { setRegion(event.target.value); invalidatePlan(); }}/></label>
      <p className="stac-help">{t('The selected area is trimmed on the service grid. A plan is required before saving or downloading.')}</p>
      <Button variant="primary" disabled={Boolean(busy) || !description || !validQueryBounds(bounds)} onClick={prepare}>{t('Plan coverage subset')}</Button>
      {plan && <Surface variant="inset" className="wcs-plan"><div className="wcs-plan-heading"><strong>{t('Planned coverage subset')}</strong><Badge>{plan.format}</Badge></div><p className="wcs-grid-summary">{number(plan.width)} × {number(plan.height)} · {plan.description.crs}</p><p className="stac-help">{t('Predicted native grid. The delivered file is checked after download.')}</p><WcsPlanDetails plan={plan}/>
        <label className="stac-field"><span>{t('Save into project')}</span><Select aria-label={t('Coverage destination project')} value={destination} disabled={Boolean(busy)} onChange={event => { setDestination(event.target.value); setSaved(null); setDelivery(''); }}><option value="new">{t('New project')}</option>{choices.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></label>
        {destination === 'new' && <label className="stac-field"><span>{t('Project name')}</span><Input aria-label={t('Coverage project name')} value={projectName} disabled={Boolean(busy)} maxLength={120} onChange={event => { setProjectName(event.target.value); setSaved(null); setDelivery(''); }}/></label>}
        <div className="stac-actions"><Button variant={saved ? 'secondary' : 'primary'} disabled={Boolean(busy) || Boolean(saved) || destination === 'new' && !projectName.trim()} onClick={save}><FolderOpen size={15}/>{t(saved ? 'Coverage plan saved' : 'Save coverage plan')}</Button>{saved && <Button variant="primary" disabled={Boolean(busy) || Boolean(delivery)} onClick={download}><Download size={15}/>{t(delivery === 'ready' ? 'Coverage subset ready' : delivery ? 'Subset download queued' : 'Download coverage subset')}</Button>}</div>
        {saved && <p className="stac-help" role="status">{t('Saved in {name}.', { name: saved.name })} <a href={`#My%20Data?project=${encodeURIComponent(saved.id)}`} onClick={onClose}>{t('Open project')}</a></p>}
      </Surface>}
    </>}
    {busy && <p className="stac-status" role="status"><Spinner size={15}/>{t('Reading coverage service…')}</p>}{error && <p className="stac-error" role="alert">{error}</p>}
  </div></Modal>;
}

export function WcsProjectAssets({ project, jobs, onChanged }) {
  const { t } = useI18n(); const { health, refresh } = useContext(RuntimeContext) || {};
  const [open, setOpen] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(''), [notice, setNotice] = useState(null);
  const [review, setReview] = useState(null);
  const items = project.wcsItems || [], matching = item => jobs.filter(job => job.wcsSource?.planId === item.planId);
  const missing = items.filter(item => !matching(item).some(job => ['succeeded', 'queued', 'running'].includes(job.status)));
  const complete = items.filter(item => matching(item).some(job => job.status === 'succeeded'));
  async function download(selected) { setBusy(true); setError(''); setNotice(null); try { const result = await wcsRequest('downloads', { projectId: project.id, selections: selected.map(({ planId }) => ({ planId })) }); setNotice({ ready: result.jobs.filter(job => job.status === 'succeeded').length, queued: result.jobs.filter(job => job.status === 'queued').length, running: result.jobs.filter(job => job.status === 'running').length }); await refresh?.(); } catch (cause) { setError(cause.message); } finally { setBusy(false); } }
  async function readPlan(id) { setBusy(true); setError(''); try { setReview(await wcsRequest('savedPlan', { id })); } catch (cause) { setError(cause.message); } finally { setBusy(false); } }
  return <section className="stac-project-assets"><div className="stac-actions"><strong>{t('Coverage subsets')}</strong><Button size="sm" onClick={() => setOpen(true)}><Plus size={15}/>{t('Add coverage subset')}</Button>{complete.length > 0 && <Button size="icon" variant="secondary" aria-label={t('Verify downloaded subsets')} tooltip={t('Verify downloaded subsets')} disabled={!health || busy} onClick={() => download(complete)}><ShieldCheck size={15}/></Button>}{items.length > 0 && <Button size="sm" variant="primary" disabled={!health || busy || !missing.length} onClick={() => download(missing)}><Download size={15}/>{t('Download missing subsets')}</Button>}</div>
    {items.length > 0 && <Disclosure summary={t('Review coverage subsets · {count}', { count: items.length })}><ul className="stac-project-list">{items.map(item => <li key={item.planId}><strong>{item.title}</strong><span>{item.serviceName} · {item.coverageId}</span><span>{item.bounds.join(', ')} · {item.mediaType}</span><div><Button size="sm" disabled={!health || busy} aria-label={`${t('Review saved plan')} · ${item.title}`} onClick={() => readPlan(item.planId)}>{t('Review saved plan')}</Button></div></li>)}</ul><p className="stac-help">{t('These files are service-generated coverage subsets, not original product archives.')}</p></Disclosure>}
    {notice && <p className="stac-help" role="status">{t('{ready} subsets verified · {queued} downloads queued · {running} running', notice)}</p>}{error && <p className="stac-error" role="alert">{error}</p>}
    {open && <WcsSourceDialog currentProject={project} onClose={() => setOpen(false)} onSaved={onChanged}/>}
    {review && <Modal title={t('Saved coverage plan')} closeLabel={t('Close')} onClose={() => setReview(null)} className="wcs-source-dialog" wide><div className="stac-source-body"><strong>{review.description.title}</strong><p className="wcs-grid-summary">{review.width} × {review.height} · {review.description.crs}</p><p className="stac-help">{t('Predicted native grid. The delivered file is checked after download.')}</p><WcsPlanDetails plan={review}/></div></Modal>}
  </section>;
}
