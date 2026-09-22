import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { Crop, FileJson, LoaderCircle, Play, RefreshCw, Save, Upload, X, CheckCircle2, AlertCircle } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { useRuntime } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { parseRecipeJSON, planMatches, processingRequest, RECIPE_SCHEMA, validateRecipe } from './processing-client.js';
import './processing.css';

function Failure({ error }) {
  const { t } = useI18n();
  return <div className="runtime-error" role="alert"><p>{t('This operation could not finish. Check the local service and source file, then retry.')}</p><details><summary>{t('Technical details')}</summary><p className="runtime-wrap">{t(error)}</p></details></div>;
}

function useModal(ref) {
  useEffect(() => {
    const node = ref.current;
    node.showModal();
    return () => { if (node.open) node.close(); };
  }, [ref]);
}

function PlanDetails({ review, metadata }) {
  const { t, number } = useI18n();
  const plan = review.plan;
  return <section className="processing-plan" aria-label={t('Verified processing plan')}>
    <h3><CheckCircle2 size={17}/>{t('Verified processing plan')}</h3>
    {metadata && <figure className="processing-window-preview"><div><img src={metadata.previewDataUrl} width={metadata.previewWidth} height={metadata.previewHeight} alt={t('Local SCL source pixels with the planned crop window')}/><span className="processing-window-overlay" style={{ left: `${plan.window[0] / metadata.width * 100}%`, top: `${plan.window[1] / metadata.height * 100}%`, width: `${plan.window[2] / metadata.width * 100}%`, height: `${plan.window[3] / metadata.height * 100}%` }} aria-hidden="true"/></div><figcaption>{t('The outlined rectangle is the verified pixel window on the local source preview.')}</figcaption></figure>}
    <dl className="runtime-details"><dt>{t('Output dimensions')}</dt><dd>{t('{width} × {height} pixels', { width: number(plan.width), height: number(plan.height) })}</dd><dt>{t('Output CRS')}</dt><dd>{plan.crs}</dd><dt>{t('Pixel size (metres)')}</dt><dd>{plan.pixelSize.map(value => number(value, { maximumFractionDigits: 3 })).join(' × ')}</dd><dt>{t('Actual bounds (metres)')}</dt><dd className="processing-bound-list">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, i) => <span key={label}>{t(label)}: {number(plan.bounds[i], { maximumFractionDigits: 3 })}</span>)}</dd><dt>{t('Pixel window')}</dt><dd>{t('Offset {x}, {y} · {width} × {height}', { x: number(plan.window[0]), y: number(plan.window[1]), width: number(plan.window[2]), height: number(plan.window[3]) })}</dd><dt>{t('Output format')}</dt><dd>GeoTIFF · UInt8 · SCL</dd></dl>
    <p>{t('Copies source pixels without resampling. The output preserves the source coordinate system and pixel size.')}</p>
    {plan.warnings.length > 0 && <div className="processing-warnings"><strong>{t('Plan adjustments')}</strong><ul>{plan.warnings.map((warning, i) => <li key={i}>{t(warning)}</li>)}</ul></div>}
  </section>;
}

export function ClipRasterButton({ job, areaBounds }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <><button className="button" onClick={() => setOpen(true)}><Crop size={15}/>{t('Clip raster')}</button>{open && <RecipeEditorDialog sourceJob={job} areaBounds={areaBounds} onClose={() => setOpen(false)}/>}</>;
}

export function RecipeEditorDialog({ sourceJob, initialRecipe, areaBounds, onClose, onSaved }) {
  const { t } = useI18n();
  const { refresh } = useRuntime();
  const dialog = useRef(null);
  const titleId = useId();
  const mounted = useRef(true);
  const version = useRef(0);
  const request = useRef(null);
  const [form, setForm] = useState(() => ({ name: initialRecipe?.name || `${sourceJob?.itemId || 'SCL'} · ${t('Raster clip')}`, crs: initialRecipe?.operation.crs || 'source', bounds: initialRecipe?.operation.bounds.map(String) || ['', '', '', ''] }));
  const [metadata, setMetadata] = useState(null);
  const [loading, setLoading] = useState(!initialRecipe);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [review, setReview] = useState(null);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [edited, setEdited] = useState(false);
  const source = initialRecipe?.source || { jobId: sourceJob?.id, sha256: sourceJob?.sha256 };
  const mutating = busy === 'save' || busy === 'run';
  useModal(dialog);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; version.current++; request.current?.abort(); };
  }, []);
  useEffect(() => {
    if (initialRecipe || !sourceJob) return;
    let active = true;
    const controller = new AbortController();
    setLoading(true); setError('');
    runtimeRequest('raster', { id: sourceJob.id }, controller.signal).then(data => {
      if (data.sha256.toLowerCase() !== sourceJob.sha256.toLowerCase()) throw new Error('The raster checksum does not match this download.');
      if (active) { setMetadata(data); setForm(old => ({ ...old, crs: 'source', bounds: data.bounds.map(String) })); }
    }).catch(e => { if (active && e.name !== 'AbortError') setError(e.message); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; controller.abort(); };
  }, [sourceJob?.id, sourceJob?.sha256, initialRecipe, loadAttempt]);
  const draft = useMemo(() => {
    try { return { recipe: validateRecipe({ schemaVersion: RECIPE_SCHEMA, name: form.name, source, operation: { type: 'clip', crs: form.crs, bounds: form.bounds.map(value => value.trim() === '' ? NaN : Number(value)) }, output: { format: 'GeoTIFF' } }), error: '' }; }
    catch (e) { return { recipe: null, error: e.message }; }
  }, [form, source.jobId, source.sha256]);
  const ready = draft.recipe && planMatches(draft.recipe, review);
  const update = changes => {
    version.current++; request.current?.abort(); setReview(null); setBusy(''); setMessage(''); setError(''); setEdited(true);
    setForm(old => ({ ...old, ...changes }));
  };
  const changeMode = crs => update({ crs, bounds: (crs === 'source' ? metadata?.bounds : areaBounds)?.map(String) || ['', '', '', ''] });
  const close = () => { if (!mutating) onClose(); };
  const execute = async operation => {
    if (!draft.recipe || (operation !== 'plan' && !ready)) return;
    const currentVersion = ++version.current;
    request.current?.abort();
    request.current = new AbortController();
    setBusy(operation); setError(''); setMessage('');
    if (operation === 'plan') setReview(null);
    try {
      const result = await processingRequest(operation, draft.recipe, request.current.signal);
      if (!mounted.current || currentVersion !== version.current) return;
      if (operation === 'plan') setReview(result);
      if (operation === 'save') { setMessage('Recipe saved by the local service. It will remain available after restart.'); onSaved?.(); }
      if (operation === 'run') { await refresh(); if (mounted.current) { onClose(); location.hash = 'Tasks'; } }
    } catch (e) {
      if (mounted.current && currentVersion === version.current && e.name !== 'AbortError') {
        setError(e.name === 'TimeoutError' ? 'The request timed out. Check Tasks or saved recipes before trying again.' : e.message);
        setReview(null);
      }
    } finally { if (mounted.current && currentVersion === version.current) setBusy(''); }
  };
  return <dialog ref={dialog} className="runtime-dialog processing-dialog" aria-labelledby={titleId} onCancel={event => { event.preventDefault(); close(); }} onClick={event => { if (event.target === event.currentTarget) close(); }}>
    <header><div><h2 id={titleId}>{t(initialRecipe ? 'Review executable recipe' : 'Clip raster')}</h2><p>{t('Create a local GeoTIFF from a pixel-aligned rectangle.')}</p></div><button className="icon-btn" disabled={mutating} onClick={close} aria-label={t('Close processing plan')}><X size={20}/></button></header>
    <div className="runtime-dialog-body">
      <div className="processing-source"><strong>{t('Pinned source file')}</strong><span className="mono runtime-wrap">{sourceJob?.itemId || source.jobId}</span><small className="mono runtime-wrap">SHA-256 · {source.sha256}</small></div>
      {loading ? <p role="status" className="processing-loading"><LoaderCircle className="runtime-spinner" size={18}/>{t('Reading source geometry…')}</p> : <>
        <label className="runtime-field">{t('Recipe name')}<input value={form.name} disabled={mutating} onChange={event => update({ name: event.target.value })}/></label>
        <div className="processing-coordinate-heading"><label className="runtime-field">{t('Input coordinates')}<select value={form.crs} disabled={mutating} onChange={event => changeMode(event.target.value)}><option value="source">{t('Source CRS · metres')}{metadata ? ` · ${metadata.crs}` : ''}</option><option value="EPSG:4326">{t('WGS84 · longitude / latitude')}</option></select></label>{Array.isArray(areaBounds) && areaBounds.length === 4 && <button className="button" disabled={mutating} onClick={() => update({ crs: 'EPSG:4326', bounds: areaBounds.map(String) })}>{t('Use workspace area')}</button>}</div>
        <p className="processing-hint">{t(form.crs === 'source' ? 'Order: minimum X, minimum Y, maximum X, maximum Y in the source projection.' : 'Order: west longitude, south latitude, east longitude, north latitude in degrees.')}</p>
        {form.crs === 'EPSG:4326' && <p className="processing-hint">{t('UTM coverage: latitude -80° to 84°; longitude span at most 180°. Bounds cannot cross the date line.')}</p>}
        <div className="processing-coordinate-fields">{(form.crs === 'source' ? ['Min X', 'Min Y', 'Max X', 'Max Y'] : ['West longitude', 'South latitude', 'East longitude', 'North latitude']).map((label, index) => <label className="runtime-field" key={index}>{t(label)}<input type="number" step="any" value={form.bounds[index]} disabled={mutating} onChange={event => update({ bounds: form.bounds.map((value, i) => i === index ? event.target.value : value) })}/></label>)}</div>
        <div className="notice"><Crop size={17}/><span>{t('The request is intersected with source coverage and aligned to whole pixels. WGS84 bounds produce an enclosing rectangle in the source projection. Polygon clipping and reprojection are not applied.')}</span></div>
        {edited && draft.error && <p className="runtime-error" role="alert">{t(draft.error)}</p>}
        <button className="button primary processing-preview-button" disabled={!draft.recipe || Boolean(busy)} onClick={() => execute('plan')}>{busy === 'plan' ? <LoaderCircle className="runtime-spinner" size={15}/> : <Crop size={15}/>} {t(busy === 'plan' ? 'Checking plan…' : 'Check processing plan')}</button>
        {ready ? <PlanDetails review={review} metadata={metadata}/> : <p className="processing-hint">{t('Save and Run unlock only after checking these exact settings. Editing a setting requires a new plan.')}</p>}
      </>}
      {error && <><Failure error={error}/>{!metadata && !initialRecipe && <button className="button" disabled={loading} onClick={() => setLoadAttempt(value => value + 1)}><RefreshCw size={15}/>{t('Retry reading source')}</button>}</>}
      {message && <p className="processing-success" role="status"><CheckCircle2 size={16}/>{t(message)}</p>}
    </div><footer><button className="button" disabled={mutating} onClick={close}>{t('Close')}</button><button className="button" disabled={!ready || Boolean(busy)} onClick={() => execute('save')}><Save size={15}/>{t(busy === 'save' ? 'Saving…' : 'Save executable recipe')}</button><button className="button primary" disabled={!ready || Boolean(busy)} onClick={() => execute('run')}><Play size={15}/>{t(busy === 'run' ? 'Starting…' : 'Run clip')}</button></footer>
  </dialog>;
}

function ImportRecipeDialog({ onClose, onReview }) {
  const { t } = useI18n();
  const dialog = useRef(null);
  const titleId = useId();
  const [text, setText] = useState('');
  const [recipe, setRecipe] = useState(null);
  const [error, setError] = useState('');
  useModal(dialog);
  const validate = () => { try { setRecipe(parseRecipeJSON(text)); setError(''); } catch (e) { setRecipe(null); setError(e.message); } };
  return <dialog ref={dialog} className="runtime-dialog processing-dialog" aria-labelledby={titleId} onCancel={event => { event.preventDefault(); onClose(); }}>
    <header><h2 id={titleId}>{t('Import executable recipe')}</h2><button className="icon-btn" onClick={onClose} aria-label={t('Close recipe import')}><X size={20}/></button></header><div className="runtime-dialog-body"><p>{t('Paste a geod-raster-recipe/v1 document. Importing does not save a recipe or start a task. The pinned source job and file must exist on this device.')}</p><label className="runtime-field">{t('Recipe JSON')}<textarea className="processing-json-input mono" rows={12} value={text} spellCheck={false} onChange={event => { setText(event.target.value); setRecipe(null); setError(''); }}/></label><button className="button" onClick={validate} disabled={!text.trim()}><FileJson size={15}/>{t('Validate JSON')}</button>{error && <p className="runtime-error" role="alert">{t(error)}</p>}{recipe && <section className="processing-import-preview"><h3>{t('Validated recipe preview')}</h3><pre>{JSON.stringify(recipe, null, 2)}</pre><p>{t('Next, check the actual source geometry and processing plan before saving or running.')}</p></section>}</div><footer><button className="button" onClick={onClose}>{t('Close')}</button><button className="button primary" disabled={!recipe} onClick={() => onReview(recipe)}>{t('Review processing plan')}</button></footer>
  </dialog>;
}

export function ExecutableRecipes({ onReviewJSON, areaBounds }) {
  const { t, date, number } = useI18n();
  const { jobs } = useRuntime();
  const [recipes, setRecipes] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [reload, setReload] = useState(0);
  const [importing, setImporting] = useState(false);
  const [selected, setSelected] = useState(null);
  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    setLoading(true); setError('');
    processingRequest('list', undefined, controller.signal).then(records => {
      if (!Array.isArray(records)) throw new Error('The service returned an invalid recipe list.');
      const checked = records.map(record => ({ ...record, recipe: validateRecipe(record.recipe) }));
      if (active) setRecipes(checked);
    }).catch(e => { if (active && e.name !== 'AbortError') setError(e.message); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; controller.abort(); };
  }, [reload]);
  return <section className="processing-recipes" aria-label={t('Executable local recipes')}><div className="processing-section-heading"><div><h2>{t('Executable local recipes')} <span className="badge">{number(recipes.length)}</span></h2><p>{t('Real rectangular SCL clips, saved by the local service with a pinned source checksum.')}</p></div><div className="row-actions"><button className="button" disabled={loading} onClick={() => setReload(value => value + 1)}><RefreshCw size={15}/>{t('Refresh recipes')}</button><button className="button" onClick={() => setImporting(true)}><Upload size={15}/>{t('Import JSON')}</button></div></div>
    {loading && <p className="processing-loading" role="status"><LoaderCircle className="runtime-spinner" size={16}/>{t('Loading saved recipes…')}</p>}
    {error && <Failure error={error}/>}
    {!loading && !error && !recipes.length && <p className="runtime-empty">{t('Open a downloaded SCL file in My Data and choose Clip raster. Check the plan, then save it here for reuse.')}</p>}
    <div className="processing-recipe-list">{recipes.map(saved => <article className="processing-recipe-card" key={saved.id}><div><h3>{saved.recipe.name}</h3><p>{t('Rectangular clip')} · {saved.recipe.operation.crs === 'source' ? t('Source CRS') : 'EPSG:4326'} · GeoTIFF</p><small>{t('Saved {date}', { date: date(saved.updatedAt) })}</small><p className="mono runtime-wrap">{saved.recipe.source.jobId}</p></div><div className="row-actions"><button className="button" onClick={() => onReviewJSON('geod-raster-recipe.json', saved.recipe)}><FileJson size={15}/>{t('Review / export JSON')}</button><button className="button primary" onClick={() => setSelected(saved.recipe)}><Play size={15}/>{t('Review and run')}</button></div></article>)}</div>
    {importing && <ImportRecipeDialog onClose={() => setImporting(false)} onReview={recipe => { setImporting(false); setSelected(recipe); }}/>}
    {selected && <RecipeEditorDialog initialRecipe={selected} sourceJob={jobs.find(job => job.id === selected.source.jobId)} areaBounds={areaBounds} onSaved={() => setReload(value => value + 1)} onClose={() => setSelected(null)}/>}
  </section>;
}

export function DerivedArtifactDetails({ job }) {
  const { t, number } = useI18n();
  if (job.kind !== 'raster_clip') return null;
  return <div className="processing-derived"><span className="badge blue">{t('Derived GeoTIFF')}</span>{job.crop && <p>{t('{width} × {height} pixels', { width: number(job.crop.width), height: number(job.crop.height) })} · {job.crop.crs}</p>}<p>{t('Parent job')}: <span className="mono runtime-wrap">{job.parentId || job.recipe?.source?.jobId}</span></p>{job.recipe?.source?.sha256 && <details><summary>{t('Pinned source checksum')}</summary><p className="mono runtime-wrap">{job.recipe.source.sha256}</p></details>}{job.manifestPath && <details><summary>{t('Output provenance manifest')}</summary><p>{t('Keep this JSON file with the GeoTIFF when moving or sharing the output.')}</p><p className="mono runtime-wrap">{job.manifestPath}</p></details>}</div>;
}
