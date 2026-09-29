import React, { useCallback, useContext, useEffect, useState } from 'react';
import { Download, Layers, RefreshCw } from 'lucide-react';
import { RuntimeContext } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Badge, Button, Disclosure, Input, Modal, Spinner, Surface } from './ui/index.jsx';
import { createProject, MAX_PROJECT_SCENES, projectRequest } from './projects-client.js';
import './projects.css';

const projectJobs = (project, jobs, assetKey) => project.scenes.map(scene =>
  jobs.find(job => job.itemId === scene.itemId && job.assetKey === assetKey
    && job.href === scene.assets?.[assetKey]?.href && job.status === 'succeeded')
  || jobs.find(job => job.itemId === scene.itemId && job.assetKey === assetKey
    && job.href === scene.assets?.[assetKey]?.href && ['queued', 'running'].includes(job.status))
).filter(Boolean);

export function SaveProjectButton({ scenes, bounds, geometry, areaName, onSaved }) {
  const { t } = useI18n();
  const { health, refresh } = useContext(RuntimeContext);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const mixedCrs = new Set(scenes.map(scene => scene.crs).filter(Boolean)).size > 1;
  const save = async event => {
    event.preventDefault(); setBusy(true); setError('');
    try {
      const request = projectRequest({ scenes, bounds, geometry, name });
      const project = await createProject(request);
      await refresh();
      setOpen(false);
      onSaved?.(project);
    } catch (cause) { setError(cause.message); }
    finally { setBusy(false); }
  };
  return <>
    <Button size="sm" disabled={!scenes.length} onClick={() => { setName(`${t(areaName)} · ${scenes.length} ${t('scenes')}`); setError(''); setOpen(true); }}>{t('Save selected as project')}</Button>
    {open && <Modal title={t('Save scene project')} onClose={() => !busy && setOpen(false)} closeDisabled={busy}>
      <form className="project-save-form" onSubmit={save}>
        <p>{t('This project keeps the selected scene IDs, source links and area together for later downloads and processing.')}</p>
        <label>{t('Project name')}<Input value={name} maxLength={120} onChange={event => setName(event.target.value)} required/></label>
        <p>{t('{count} scenes selected · maximum {max} per project', { count: scenes.length, max: MAX_PROJECT_SCENES })}</p>
        {scenes.length > MAX_PROJECT_SCENES && <p className="projects-error">{t('Narrow the selection to {max} scenes to save one project.', { max: MAX_PROJECT_SCENES })}</p>}
        {mixedCrs && <p className="projects-error">{t('This project crosses UTM zones. Create one project per CRS to mosaic without reprojection.')}</p>}
        {!health && <p className="projects-error">{t('The local task service is offline.')}</p>}
        {error && <p className="projects-error" role="alert">{error}</p>}
        <footer><Button type="button" onClick={() => setOpen(false)} disabled={busy}>{t('Cancel')}</Button><Button primary type="submit" disabled={busy || !health || scenes.length > MAX_PROJECT_SCENES || !name.trim()}>{busy ? t('Saving…') : t('Save project')}</Button></footer>
      </form>
    </Modal>}
  </>;
}

export function ProjectsLibrary() {
  const { t, date, number } = useI18n();
  const { jobs, health, act } = useContext(RuntimeContext);
  const [projects, setProjects] = useState([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const refresh = useCallback(async () => {
    try { setProjects(await runtimeRequest('projects')); setError(''); }
    catch (cause) { setError(cause.message); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { refresh(); }, [refresh]);
  const execute = async (project, assetKey, operation) => {
    setBusy(`${project.id}:${assetKey}`); setError('');
    try { await act(operation, { id: project.id, assetKey }); }
    catch (cause) { setError(cause.message); }
    finally { setBusy(''); }
  };
  const downloadBoth = async project => {
    setBusy(`${project.id}:both`); setError('');
    try {
      await act('downloadProject', { id: project.id, assetKey: 'scl' });
      await act('downloadProject', { id: project.id, assetKey: 'visual' });
    } catch (cause) { setError(cause.message); }
    finally { setBusy(''); }
  };
  return <section className="projects-library" aria-label={t('Saved scene projects')}>
    <div className="projects-heading"><div><h2>{t('Saved scene projects')}</h2><p>{t('Selected scenes stay together here. Download both source types, then process the completed rasters.')}</p></div><Button size="sm" onClick={refresh} aria-label={t('Refresh projects')}><RefreshCw size={15}/></Button></div>
    {loading && <p role="status"><Spinner size={16}/>{t('Loading projects…')}</p>}
    {error && <p className="projects-error" role="alert">{error}</p>}
    {!loading && !projects.length && !error && <Surface className="projects-empty"><Layers size={19}/><span>{t('Choose scenes in Explore, then save them as a project.')}</span></Surface>}
    <div className="projects-list">{projects.map(project => {
      const scl = projectJobs(project, jobs, 'scl');
      const visual = projectJobs(project, jobs, 'visual');
      const completed = entries => entries.filter(job => job.status === 'succeeded').length;
      const mosaicJobs = jobs.filter(job => job.kind === 'raster_mosaic' && job.mosaic?.projectId === project.id);
      const latestMosaic = key => mosaicJobs.find(job => job.assetKey === key);
      const mixedCrs = new Set(project.scenes.map(scene => scene.crs).filter(Boolean)).size > 1;
      const orderedDates = project.scenes.map(scene => scene.date).sort();
      return <Surface as="article" className="project-row" key={project.id}>
        <div className="project-summary"><div><h3>{project.name}</h3><p>{number(project.scenes.length)} {t('scenes')} · {date(orderedDates[0])} – {date(orderedDates.at(-1))}</p></div><Badge>{t('Local project')}</Badge></div>
        <Disclosure className="project-scenes" summary={t('Review selected scenes · {count}', { count: project.scenes.length })}>
          <ul>{project.scenes.map(scene => <li key={scene.itemId}><span>{date(scene.date)}</span><code>{scene.itemId}</code><span>{scene.cloud == null ? t('Unknown') : number(scene.cloud / 100, { style: 'percent', maximumFractionDigits: 1 })}</span></li>)}</ul>
        </Disclosure>
        <div className="project-assets">
          <div><strong>{t('True-color imagery')}</strong><span>{t('{done} / {total} downloaded', { done: completed(visual), total: project.scenes.length })}</span></div>
          <div><strong>{t('SCL classification')}</strong><span>{t('{done} / {total} downloaded', { done: completed(scl), total: project.scenes.length })}</span></div>
        </div>
        <div className="project-actions">
          <Button size="sm" disabled={!health || Boolean(busy) || (completed(visual) === project.scenes.length && completed(scl) === project.scenes.length)} onClick={() => downloadBoth(project)}><Download size={15}/>{t('Download SCL and true-color')}</Button>
          <Button size="sm" disabled={!health || Boolean(busy) || mixedCrs || completed(visual) !== project.scenes.length || ['queued', 'running'].includes(latestMosaic('visual')?.status)} onClick={() => execute(project, 'visual', 'mosaicProject')}><Layers size={15}/>{t('Mosaic and clip true-color')}</Button>
          <Button size="sm" disabled={!health || Boolean(busy) || mixedCrs || completed(scl) !== project.scenes.length || ['queued', 'running'].includes(latestMosaic('scl')?.status)} onClick={() => execute(project, 'scl', 'mosaicProject')}><Layers size={15}/>{t('Mosaic and clip SCL')}</Button>
          {busy.startsWith(`${project.id}:`) && <span role="status"><Spinner size={15}/>{t('Adding scenes to the local queue…')}</span>}
        </div>
        {mixedCrs && <p className="projects-error">{t('This project crosses UTM zones. Create one project per CRS to mosaic without reprojection.')}</p>}
        {mosaicJobs.length > 0 && <div className="project-output-status">{['visual', 'scl'].map(key => latestMosaic(key) && <span key={key}>{key.toUpperCase()}: {t(latestMosaic(key).status)}{latestMosaic(key).status === 'succeeded' && <> · {t('Output in My Data below')}</>}</span>)}</div>}
      </Surface>;
    })}</div>
  </section>;
}
