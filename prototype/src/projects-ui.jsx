import React, { useCallback, useContext, useEffect, useState } from 'react';
import { ArrowLeft, Check, Compass, Download, FolderOpen, Layers, Pencil, RefreshCw, Search, X } from 'lucide-react';
import { RuntimeContext } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Badge, Button, Disclosure, Input, Modal, Spinner, Surface } from './ui/index.jsx';
import { createProject, jobsForProject, MAX_PROJECT_SCENES, projectRequest } from './projects-client.js';
import { RuntimeJobRows } from './runtime-ui.jsx';
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

export function ProjectsLibrary({ focusedProjectId, onOpenProject, onCloseProject, onContinueExploring }) {
  const { t, date, number } = useI18n();
  const { jobs, health, act } = useContext(RuntimeContext);
  const [projects, setProjects] = useState([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [editingId, setEditingId] = useState('');
  const [draftName, setDraftName] = useState('');
  const [renameError, setRenameError] = useState('');
  const [search, setSearch] = useState('');
  const refresh = useCallback(async () => {
    try { setProjects(await runtimeRequest('projects')); setError(''); }
    catch (cause) { setError(cause.message); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { refresh(); }, [refresh]);
  const rename = async (event, project) => {
    event.preventDefault();
    setBusy(`${project.id}:rename`); setRenameError('');
    try {
      const updated = await act('renameProject', { id: project.id, name: draftName.trim() });
      setProjects(current => current.map(item => item.id === updated.id ? updated : item));
      setEditingId('');
    } catch (cause) { setRenameError(cause.message); }
    finally { setBusy(''); }
  };
  const execute = async (project, assetKey, operation) => {
    setBusy(`${project.id}:${assetKey}`); setError('');
    try { await act(operation, { id: project.id, assetKey }); }
    catch (cause) { setError(cause.message); }
    finally { setBusy(''); }
  };
  const shown = focusedProjectId ? projects.filter(project => project.id === focusedProjectId) : projects.filter(project => project.name.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  return <section className={`projects-library${focusedProjectId ? ' projects-library-focused' : ''}`} aria-label={t('Saved scene projects')}>
    <div className="projects-heading">
      <div className={focusedProjectId ? 'project-navigation' : undefined}>
        {focusedProjectId ? <>
          {onContinueExploring && <Button size="sm" disabled={!shown.length} onClick={() => onContinueExploring(shown[0])}><Compass size={15}/>{t('Explore and add scenes to this project')}</Button>}
          <Button size="sm" variant="ghost" onClick={onCloseProject}><ArrowLeft size={15}/>{t('All projects')}</Button>
        </> : <div className="project-search"><Search size={16} aria-hidden="true"/><Input type="search" aria-label={t('Search projects')} placeholder={t('Search project name')} value={search} onChange={event => setSearch(event.target.value)}/></div>}
      </div>
      <div className="project-navigation">
        <Button size="icon" onClick={refresh} aria-label={t('Refresh projects')}><RefreshCw size={16}/></Button>
        {!focusedProjectId && !loading && <span className="project-result-count">{t('{count} projects', { count: number(shown.length) })}</span>}
      </div>
    </div>
    {loading && <p role="status"><Spinner size={16}/>{t('Loading projects…')}</p>}
    {error && <p className="projects-error" role="alert">{error}</p>}
    {!loading && !projects.length && !error && <Surface className="projects-empty"><Layers size={19}/><span>{t('Choose scenes in Explore, then save them as a project.')}</span></Surface>}
    {!loading && !focusedProjectId && projects.length > 0 && !shown.length && <Surface className="projects-empty"><Search size={19}/><span>{t('No projects match this search.')}</span></Surface>}
    {!loading && focusedProjectId && projects.length > 0 && !shown.length && <p role="alert">{t('This project could not be found. Return to all projects or refresh the list.')}</p>}
    <div className="projects-list">{shown.map(project => {
      const focused = focusedProjectId === project.id;
      const related = jobsForProject(project, jobs);
      const pending = related.filter(job => job.status !== 'succeeded');
      const files = related.filter(job => job.status === 'succeeded');
      const scl = projectJobs(project, jobs, 'scl');
      const visual = projectJobs(project, jobs, 'visual');
      const completed = entries => entries.filter(job => job.status === 'succeeded').length;
      const mosaicJobs = jobs.filter(job => job.kind === 'raster_mosaic' && job.mosaic?.projectId === project.id);
      const latestMosaic = key => mosaicJobs.find(job => job.assetKey === key);
      const mixedCrs = new Set(project.scenes.map(scene => scene.crs).filter(Boolean)).size > 1;
      const orderedDates = project.scenes.map(scene => scene.date).sort();
      return <Surface as="article" id={`project-${project.id}`} className={`project-row${focusedProjectId === project.id ? ' project-row-focused' : ''}`} key={project.id}>
        <div className="project-summary">
          <span className="project-icon" aria-hidden="true"><FolderOpen size={16}/></span>
          <div className="project-title-block">
            {editingId === project.id ? <form className="project-rename" onSubmit={event => rename(event, project)}><label>{t('Project name')}<Input autoFocus value={draftName} maxLength={120} required onChange={event => setDraftName(event.target.value)}/></label><Button type="submit" size="icon" variant="primary" disabled={!draftName.trim() || Boolean(busy)} aria-label={t('Save project name')}><Check size={16}/></Button><Button type="button" size="icon" disabled={Boolean(busy)} aria-label={t('Cancel renaming')} onClick={() => { setEditingId(''); setRenameError(''); }}><X size={16}/></Button></form> : <div className="project-name"><h3>{focused ? project.name : <Button variant="link" className="project-open-title" aria-label={`${t('Open project')} · ${project.name}`} onClick={() => onOpenProject?.(project.id)}>{project.name}</Button>}</h3><Button size="icon" variant="quiet" aria-label={t('Rename project {name}', { name: project.name })} onClick={() => { setEditingId(project.id); setDraftName(project.name); setRenameError(''); }}><Pencil size={16}/></Button>{focused && <Badge>{t('Current project')}</Badge>}</div>}
            <p className="project-metadata"><span>{number(project.scenes.length)} {t('scenes')}</span>{!focused && <span>{t('{count} files', { count: number(files.length) })}</span>}<span className="project-date-range">{date(orderedDates[0])}{orderedDates[0] !== orderedDates.at(-1) && <> – {date(orderedDates.at(-1))}</>}</span></p>
            {editingId === project.id && renameError && <p className="projects-error" role="alert">{renameError}</p>}
          </div>
        </div>
        {focused && <>
        <p className="project-guide">{t('Source files and results stay in this project. Download missing files, then clip a scene or mosaic multiple scenes to the saved area.')}</p>
        <Disclosure className="project-scenes" summary={t('Review selected scenes · {count}', { count: project.scenes.length })}>
          <ul>{project.scenes.map(scene => <li key={scene.itemId}><span>{date(scene.date)}</span><code>{scene.itemId}</code><span>{t('Cloud cover')} · {scene.cloud == null ? t('Unknown') : number(scene.cloud / 100, { style: 'percent', maximumFractionDigits: 1 })}</span></li>)}</ul>
        </Disclosure>
        <div className="project-assets">
          {['visual', 'scl'].map(key => {
            const sources = key === 'visual' ? visual : scl;
            const ready = completed(sources) === project.scenes.length;
            const active = sources.some(job => ['queued', 'running'].includes(job.status));
            const available = project.scenes.every(scene => scene.assets?.[key]);
            return <Surface variant="inset" className="project-asset" key={key}>
              <div className="project-asset-summary"><strong>{t(key === 'visual' ? 'True-color imagery' : 'SCL classification')}</strong><span>{t('{done} / {total} downloaded', { done: completed(sources), total: project.scenes.length })}</span></div>
              {available && <div className="project-actions"><Button variant={ready ? "secondary" : "primary"} size="sm" disabled={!health || Boolean(busy) || ready || active} onClick={() => execute(project, key, 'downloadProject')}><Download size={15}/>{t(key === 'visual' ? 'Download true-color' : 'Download SCL')}</Button><Button variant={ready ? "primary" : "secondary"} size="sm" disabled={!health || Boolean(busy) || mixedCrs || !ready || ['queued', 'running'].includes(latestMosaic(key)?.status)} onClick={() => execute(project, key, 'mosaicProject')}><Layers size={15}/>{t(project.scenes.length === 1 ? key === 'visual' ? 'Clip true-color to project area' : 'Clip SCL to project area' : key === 'visual' ? 'Mosaic and clip true-color' : 'Mosaic and clip SCL')}</Button></div>}
            </Surface>;
          })}
        </div>
        {busy.startsWith(`${project.id}:`) && <span className="project-busy" role="status"><Spinner size={15}/>{t(busy.endsWith(':rename') ? 'Saving project name…' : 'Adding scenes to the local queue…')}</span>}
        {mixedCrs && <p className="projects-error">{t('This project crosses UTM zones. Create one project per CRS to mosaic without reprojection.')}</p>}
        <div className="project-files">
          {pending.length > 0 && <><h4>{t('Project downloads and processing')}</h4><RuntimeJobRows jobs={pending} projectName={project.name}/></>}
          <h4>{t('Project files')} <Badge>{number(files.length)}</Badge></h4>
          {files.length > 0 ? <div className="project-file-list"><RuntimeJobRows jobs={files} library projectName={project.name}/></div> : <Surface variant="inset" className="projects-empty"><span>{t('Completed source files and clipping results will appear here.')}</span></Surface>}
        </div>
        </>}
      </Surface>;
    })}</div>
  </section>;
}
