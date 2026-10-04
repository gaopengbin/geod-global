import { VEGETATION_KEYS, vegetationMatchesJob } from './vegetation.js';
import { MODIS_SCIENCE_KEYS } from './modis-science-layers.js';
import { ModisScienceControls } from './modis-science-ui.jsx';
import { VegetationQualityControls } from './vegetation-quality-ui.jsx';
import { supportsViSelection } from './vegetation-quality.js';
import React, { useCallback, useContext, useEffect, useState } from 'react';
import { ArrowLeft, Check, Compass, Download, FolderOpen, Layers, Pencil, RefreshCw, Search, X } from 'lucide-react';
import { RuntimeContext } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Badge, Button, Disclosure, Input, Modal, Progress, SegmentedControl, Spinner, Surface } from './ui/index.jsx';
import { createProject, jobsForProject, pendingProjectJobs, projectSourceJobs, MAX_PROJECT_SCENES, projectRequest } from './projects-client.js';
import { RuntimeJobRows } from './runtime-ui.jsx';
import { FileThumbnail } from './file-thumbnail.jsx';
import { SOURCE_ASSET_KEYS, LANDSAT_BANDS, assetLabel, demTileLabel, copDemLabel } from './providers.js';
import { modisIdentity, MODIS_QUALITY_KEYS } from './modis.js';
import { QUALITY_KEYS } from './quality.js';
import { LANDSAT_QUALITY_KEYS } from './landsat-quality.js';
import { verifiedViirsScience } from './viirs.js';
import { RADAR_KEYS } from './radar.js';
import { compositeIdentity, compositePeriodLabel } from './composite-period.js';
import './projects.css';
import { StacProjectAssets } from './stac-ui.jsx';
import { WcsProjectAssets } from './wcs-ui.jsx';
import { projectOriginalsDeferred, PROTECTED_ORIGINAL_NOTICE } from './release-policy.js';

function completedSafe(project, jobs) { return project.scenes.length > 0 && projectSourceJobs(project, jobs, 'product').filter(job => job.status === 'succeeded').length === project.scenes.length; }

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
  const [fileView, setFileView] = useState('all');
  const refresh = useCallback(async () => {
    try { setProjects(await runtimeRequest('projects')); setError(''); }
    catch (cause) { setError(cause.message); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { refresh(); }, [refresh, Boolean(health), focusedProjectId]);
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
  const execute = async (project, assetKey, operation, options = {}) => {
    if (operation === 'downloadProject' && projectOriginalsDeferred(project)) {
      setError(t(PROTECTED_ORIGINAL_NOTICE));
      return false;
    }
    setBusy(`${project.id}:${assetKey}`); setError('');
    try { await act(operation, { id: project.id, assetKey, ...options }); return true; }
    catch (cause) { setError(cause.message); return false; }
    finally { setBusy(''); }
  };
  const shown = focusedProjectId ? projects.filter(project => project.id === focusedProjectId) : projects.filter(project => project.name.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  return <section className={`projects-library${focusedProjectId ? ' projects-library-focused' : ''}`} aria-label={t('Saved scene projects')}>
    <div className="projects-heading">
      <div className={focusedProjectId ? 'project-navigation' : undefined}>
        {focusedProjectId ? <>
          {onContinueExploring && shown[0]?.scenes.length > 0 && <Button size="sm" onClick={() => onContinueExploring(shown[0])}><Compass size={15}/>{t('Explore and add scenes to this project')}</Button>}
          <Button size="sm" variant="secondary" onClick={onCloseProject}><ArrowLeft size={16}/>{t('All projects')}</Button>
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
      const originalsDeferred = projectOriginalsDeferred(project);
      const related = jobsForProject(project, jobs);
      const pending = pendingProjectJobs(related);
      const files = related.filter(job => job.status === 'succeeded');
      const safeProject = project.scenes.length > 0 && project.scenes.every(scene => scene.assets?.product);
      const safeReady = completedSafe(project, jobs);
      const viirsProject = project.scenes.length > 0 && project.scenes.every(scene => scene.assets?.viirs);
      const viirsReady = projectSourceJobs(project, jobs, 'viirs').filter(job => verifiedViirsScience(job)).length === project.scenes.length;
      const assetKeys = safeProject ? ['product', 'visual', 'scl'] : viirsProject ? ['viirs', ...LANDSAT_BANDS] : SOURCE_ASSET_KEYS.filter(key => project.scenes.some(scene => scene.assets?.[key]));
      const srtm = assetKeys.includes('srtm');
      const elevation = assetKeys.includes('elevation') || srtm;
      const radar = assetKeys.some(key => ['vv','vh','hh','hv'].includes(key));
      const aerial = assetKeys.includes('aerial');
      const reflectance = assetKeys.some(key => LANDSAT_BANDS.includes(key));
      const completed = entries => entries.filter(job => job.status === 'succeeded').length;
      const mosaicJobs = jobs.filter(job => job.kind === 'raster_mosaic' && job.mosaic?.projectId === project.id);
      const latestMosaic = key => mosaicJobs.find(job => job.assetKey === key);
      const mixedCrs = new Set(project.scenes.map(scene => scene.crs).filter(Boolean)).size > 1;
      const orderedDates = project.scenes.map(scene => scene.date).sort();
      const lastObservationDate = project.scenes.map(scene => compositeIdentity(scene.itemId)?.endDate || scene.date).sort().at(-1);
      const activeCount = related.filter(job => ['queued', 'running'].includes(job.status)).length;
      const preview = files.find(job => job.assetKey === 'aerial') || files.find(job => job.assetKey === 'visual') || files.find(job => job.assetKey === 'scl') || files.find(job => LANDSAT_BANDS.includes(job.assetKey)) || files.find(job => VEGETATION_KEYS.includes(job.assetKey)) || files.find(job => MODIS_SCIENCE_KEYS.includes(job.assetKey)) || files.find(job => ['elevation','srtm'].includes(job.assetKey)) || files.find(job => RADAR_KEYS.includes(job.assetKey)) || files.find(job => job.assetKey === 'stac_asset' || job.assetKey === 'wcs_coverage');
      const sourceFiles = files.filter(job => ['download', 'raster_prepare'].includes(job.kind));
      const outputs = files.filter(job => !['download', 'raster_prepare'].includes(job.kind));
      const visibleFiles = fileView === 'sources' ? sourceFiles : fileView === 'outputs' ? outputs : files;
      return <Surface as="article" id={`project-${project.id}`} className={`project-row${focusedProjectId === project.id ? ' project-row-focused' : ''}`} key={project.id}>
        <div className="project-summary">
          {!focused && preview ? <FileThumbnail job={preview}/> : <span className="project-icon" aria-hidden="true"><FolderOpen size={16}/></span>}
          <div className="project-title-block">
            {editingId === project.id ? <form className="project-rename" onSubmit={event => rename(event, project)}><label>{t('Project name')}<Input autoFocus value={draftName} maxLength={120} required onChange={event => setDraftName(event.target.value)}/></label><Button type="submit" size="icon" variant="primary" disabled={!draftName.trim() || Boolean(busy)} aria-label={t('Save project name')}><Check size={16}/></Button><Button type="button" size="icon" disabled={Boolean(busy)} aria-label={t('Cancel renaming')} onClick={() => { setEditingId(''); setRenameError(''); }}><X size={16}/></Button></form> : <div className="project-name"><h3>{focused ? project.name : <Button variant="link" className="project-open-title" aria-label={`${t('Open project')} · ${project.name}`} onClick={() => onOpenProject?.(project.id)}>{project.name}</Button>}</h3><Button size="icon" variant="quiet" aria-label={t('Rename project {name}', { name: project.name })} onClick={() => { setEditingId(project.id); setDraftName(project.name); setRenameError(''); }}><Pencil size={16}/></Button></div>}
            <p className="project-metadata">{focused && <Badge>{t('Current project')}</Badge>}{project.scenes.length > 0 && <span>{number(project.scenes.length)} {t(elevation ? 'elevation tiles' : 'scenes')}</span>}{project.stacItems?.length > 0 && <span>{t('{count} raster assets', { count: number(project.stacItems.length) })}</span>}{project.wcsItems?.length > 0 && <span>{t('{count} coverage subsets', { count: number(project.wcsItems.length) })}</span>}{!focused && <span>{t('{count} files', { count: number(files.length) })}</span>}{project.scenes.length > 0 && <span className="project-date-range">{srtm ? 'SRTMGL1 v003' : elevation ? copDemLabel(project.scenes) : date(orderedDates[0])}{!elevation && orderedDates[0] !== lastObservationDate && <> – {date(lastObservationDate)}</>}</span>}</p>
            {!focused && Boolean(activeCount || outputs.length) && <p className="project-card-state"><Badge tone={activeCount ? 'blue' : 'neutral'}>{t(activeCount ? '{count} active tasks' : '{count} processing results', { count: number(activeCount || outputs.length) })}</Badge></p>}
            {editingId === project.id && renameError && <p className="projects-error" role="alert">{renameError}</p>}
          </div>
        </div>
        {focused && <>
        {originalsDeferred && <p className="project-asset-help">{t(PROTECTED_ORIGINAL_NOTICE)}</p>}
        {project.scenes.length > 0 && <Disclosure className="project-scenes" summary={t('Review selected scenes · {count}', { count: project.scenes.length })}>
          <ul>{project.scenes.map(scene => <li key={scene.itemId}><code>{elevation ? demTileLabel(scene.itemId) : scene.itemId}</code><div className="project-scene-metadata">{elevation ? <span>{srtm ? 'SRTMGL1 v003 · Int16 · EGM96' : `${copDemLabel([scene])} · Float32 · EGM2008`}</span> : <><span>{compositePeriodLabel(scene, date)}</span>{!aerial && !radar && !compositeIdentity(scene.itemId) && <span>{t('Cloud cover')} · {scene.cloud == null ? t('Unknown') : number(scene.cloud / 100, { style: 'percent', maximumFractionDigits: 1 })}</span>}</>}</div></li>)}</ul>
        </Disclosure>}
        {(project.stacItems?.length > 0 || project.wcsItems?.length > 0) && <div className="project-source-tools">
          {project.stacItems?.length > 0 && <StacProjectAssets project={project} jobs={related} onChanged={refresh}/>}
          {project.wcsItems?.length > 0 && <WcsProjectAssets project={project} jobs={related} onChanged={refresh}/>}
        </div>}
        {(!project.stacItems?.length || !project.wcsItems?.length) && <Disclosure className="project-add-source" summary={t('Add data from another source')}>
          <div className="project-source-tools">
            {!project.stacItems?.length && <StacProjectAssets project={project} jobs={related} onChanged={refresh}/>}
            {!project.wcsItems?.length && <WcsProjectAssets project={project} jobs={related} onChanged={refresh}/>}
          </div>
        </Disclosure>}
        <div className={`project-assets${reflectance || safeProject ? ' project-assets-reflectance' : ''}${viirsProject ? ' project-assets-viirs' : ''}${LANDSAT_BANDS.every(key=>assetKeys.includes(key)) && (MODIS_QUALITY_KEYS.every(key=>assetKeys.includes(key)) || LANDSAT_QUALITY_KEYS.every(key=>assetKeys.includes(key))) ? ' project-assets-modis-quality' : ''}`}>
          <ModisScienceControls project={project} jobs={jobs} health={health} busy={Boolean(busy)} onAction={(key,operation)=>execute(project,key,operation)}/>
          <VegetationQualityControls project={project} jobs={jobs} health={health} busy={Boolean(busy)} onAction={(key,operation,options)=>execute(project,key,operation,options)}/>
          {assetKeys.filter(key=>!MODIS_SCIENCE_KEYS.includes(key) && !(supportsViSelection(project) && VEGETATION_KEYS.includes(key))).map(key => {
            const sources = projectSourceJobs(project, jobs, key);
            const processable = ['visual', 'scl', ...LANDSAT_BANDS, ...VEGETATION_KEYS, ...RADAR_KEYS, ...QUALITY_KEYS, 'elevation', 'srtm', 'aerial'].includes(key);
            const quality = QUALITY_KEYS.includes(key);
            const sourceBand = LANDSAT_BANDS.includes(key) || VEGETATION_KEYS.includes(key) || RADAR_KEYS.includes(key) || quality;
            const prepare = safeProject && key !== 'product' || viirsProject && sourceBand;
            const originalReady = viirsProject ? viirsReady : safeReady;
            const ready = key === 'viirs' ? viirsReady : completed(sources) === project.scenes.length;
            const processingReady = ready && (key !== 'qa_radsat' || completed(projectSourceJobs(project, jobs, 'qa_pixel')) === project.scenes.length);
            const active = sources.some(job => ['queued', 'running'].includes(job.status));
            const available = prepare || project.scenes.every(scene => scene.assets?.[key]);
            return <Surface variant="inset" className={`project-asset${QUALITY_KEYS.includes(key) ? ' project-asset-quality' : ''}`} key={key}>
              <div className="project-asset-summary"><strong>{t(assetLabel(key))}{viirsProject && sourceBand ? ` · ${{red:'M5',green:'M4',blue:'M3'}[key]}` : ''}</strong><span>{t(prepare ? '{done} / {total} ready' : '{done} / {total} downloaded', { done: completed(sources), total: project.scenes.length })}</span></div>
              <Progress value={completed(sources)} max={project.scenes.length} aria-label={`${t(assetLabel(key))} · ${t('Files ready')}`}/>
              {available && <div className="project-actions"><Button variant={ready ? "secondary" : "primary"} size="sm" disabled={!health || Boolean(busy) || ready || active || prepare && !originalReady || !prepare && originalsDeferred} onClick={() => execute(project, key, prepare ? 'prepareProject' : 'downloadProject')}><Download size={15}/>{t(prepare ? viirsProject ? 'Prepare {band}' : key === 'visual' ? 'Prepare true-color imagery' : 'Prepare SCL' : ['elevation','srtm'].includes(key) ? 'Download elevation tiles' : key === 'aerial' ? 'Download aerial originals' : ['product','viirs'].includes(key) ? 'Download original product' : quality ? 'Download quality file' : sourceBand ? 'Download source band' : processable ? key === 'visual' ? 'Download true-color' : 'Download SCL' : 'Download source band', {band:{red:'M5',green:'M4',blue:'M3'}[key]})}</Button>{processable && <Button variant={processingReady ? "primary" : "secondary"} size="sm" disabled={!health || Boolean(busy) || mixedCrs || !processingReady || ['queued', 'running'].includes(latestMosaic(key)?.status)} onClick={() => execute(project, key, 'mosaicProject')}><Layers size={15}/>{t(quality ? project.scenes.length === 1 ? 'Clip quality to project area' : 'Mosaic and clip quality' : RADAR_KEYS.includes(key) ? project.scenes.length === 1 ? 'Clip radar to project area' : 'Mosaic and clip radar' : key === 'aerial' ? project.scenes.length === 1 ? 'Clip aerial imagery to project area' : 'Mosaic and clip aerial imagery' : ['elevation','srtm'].includes(key) ? project.scenes.length === 1 ? 'Clip elevation to project area' : 'Mosaic and clip elevation' : sourceBand ? project.scenes.length === 1 ? 'Clip band to project area' : 'Mosaic and clip band' : project.scenes.length === 1 ? key === 'visual' ? 'Clip true-color to project area' : 'Clip SCL to project area' : key === 'visual' ? 'Mosaic and clip true-color' : 'Mosaic and clip SCL')}</Button>}</div>}
              {(!available || !ready || active) && <p className="project-asset-help">{t(prepare ? active ? viirsProject ? 'Preparing original HDF5 bands…' : 'Preparing original JP2 rasters…' : ready ? 'Prepared rasters keep the original pixel values and grid.' : 'Download every original product, then prepare the selected raster.' : !available ? 'Some scenes do not provide this file type.' : active ? processable ? 'Downloads are running. Processing becomes available when every source is ready.' : key === 'aerial' ? 'Downloading aerial originals…' : ['product','viirs'].includes(key) ? 'Downloading original products…' : 'Downloading original source bands…' : processable ? 'Download the missing source files before processing.' : key === 'aerial' ? 'Download the missing aerial originals.' : ['product','viirs'].includes(key) ? 'Download the missing original products.' : 'Download the missing source bands.')}</p>}
              {key === 'qa_radsat' && ready && !processingReady && <p className="project-asset-help">{t('Download matching pixel-quality files before processing saturation flags.')}</p>}
            </Surface>;
          })}
        </div>
        {assetKeys.some(key => LANDSAT_QUALITY_KEYS.includes(key)) && <p className="project-asset-help">{t('Landsat quality processing retains complete UInt16 flags on the aligned 30 m grid. Saturation coverage requires matching pixel-quality files.')}</p>}
        {busy.startsWith(`${project.id}:`) && <span className="project-busy" role="status"><Spinner size={15}/>{t(busy.endsWith(':rename') ? 'Saving project name…' : 'Adding scenes to the local queue…')}</span>}
        {assetKeys.some(key => MODIS_QUALITY_KEYS.includes(key)) && <p className="project-asset-help">{t('Quality processing retains whole unsigned bit fields on the original grid. Newer valid composites win overlaps; gaps and polygon masks use product fill. This does not select clearer observations or apply a mask to reflectance.')}</p>}
        {radar && <p className="project-asset-help">{t('RTC processing preserves Float32 linear gamma0, polarization and the aligned file grid. Newer valid scenes win overlaps; gaps and polygon masks are -32768 NoData. No averaging, calibration or speckle filtering is applied.')}</p>}
        {aerial && <p className="project-asset-help">{t('NAIP processing keeps RGB, NIR and the aligned NAD83 grid. Uncovered areas use an independent coverage mask; zero channel values remain valid.')}</p>}
        {srtm && <p className="project-asset-help">{t('SRTM processing preserves Int16 heights, EGM96 and the Point grid. Shared edges occupy one row or column; gaps and masks are -32768 NoData.')}</p>}
        {elevation && !srtm && <p className="project-asset-help">{t('Elevation processing preserves original Float32 heights and the Point grid. Tiles with different pixel spacing must be processed separately; gaps and polygon masks become NaN NoData.')}</p>}
        {assetKeys.some(key => VEGETATION_KEYS.includes(key)) && !supportsViSelection(project) && <p className="project-asset-help">{t('Processing keeps original index values and the grid. The newest valid composite fills overlaps; no quality mask is applied.')}</p>}
        {reflectance && project.scenes.some(scene => modisIdentity(scene.itemId)) && <p className="project-asset-help">{t('MODIS band processing preserves Int16 DN, -28672 NoData, reflectance conversion and the original sinusoidal grid. Newest valid composites win overlaps; cloud and quality masks are not applied.')}</p>}
        {reflectance && !viirsProject && !project.scenes.some(scene => modisIdentity(scene.itemId)) && <p className="project-asset-help">{t('Band processing preserves original DN, NoData and reflectance conversion. Each result uses the project area and one aligned 30 m grid.')}</p>}
        {viirsProject && <p className="project-asset-help">{t('VIIRS HDF5 originals retain all science and quality layers. Prepare M5, M4 and M3 to inspect DN, view local RGB and process the project area on the original sinusoidal grid. QA masks are not applied.')} <a href="#Settings?account=nasa-earthdata">{t('Manage authorization')}</a></p>}
        {viirsProject && !viirsReady && projectSourceJobs(project, jobs, 'viirs').some(job => job.status === 'succeeded' && !verifiedViirsScience(job)) && <p className="project-asset-help">{t('An older VIIRS download has no science validation. Download the original product again before preparing bands; the existing file is retained.')}</p>}
        {assetKeys.includes('product') && <p className="project-asset-help">{t('SAFE archives remain the original source. Prepare TCI (10 m) or SCL (20 m) to view pixels and process the project area.')}</p>}
        {mixedCrs && <p className="projects-error">{t('This project crosses UTM zones. Create one project per CRS to mosaic without reprojection.')}</p>}
        <div className="project-files">
          {pending.length > 0 && <><h4>{t('Project downloads and processing')}</h4><RuntimeJobRows jobs={pending} projectName={project.name} projectId={project.id}/></>}
          <div className="project-files-heading"><h4>{t('Project files')}</h4>
          <SegmentedControl variant="navigation" value={fileView} onValueChange={setFileView} aria-label={t('Project file view')} items={[
            { value: 'all', label: `${t('All files')} · ${number(files.length)}` },
            { value: 'sources', label: `${t('Source files')} · ${number(sourceFiles.length)}` },
            { value: 'outputs', label: `${t('Processing results')} · ${number(outputs.length)}` },
          ]}/>
          </div>
          {visibleFiles.length > 0 ? <div className="project-file-list"><RuntimeJobRows jobs={visibleFiles} library projectName={project.name} projectId={project.id}/></div> : <Surface variant="inset" className="projects-empty"><span>{t(fileView === 'outputs' ? 'Run a project clip or mosaic to create your first result.' : 'Completed source files and clipping results will appear here.')}</span></Surface>}
        </div>
        </>}
      </Surface>;
    })}</div>
  </section>;
}
