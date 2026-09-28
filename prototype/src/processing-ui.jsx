import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Crop, FileJson, Play, RefreshCw, Save, Upload, CheckCircle2 } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { useRuntime } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { parseRecipeJSON, planMatches, processingRequest, RECIPE_SCHEMA, POLYGON_RECIPE_SCHEMA, validateRecipe } from './processing-client.js';
import { Badge, Button, Disclosure, Input, Modal, Select, Spinner, Surface, Textarea } from './ui/index.jsx';
import './processing.css';

function Failure({ error }) {
  const { t } = useI18n();
  return <div className="runtime-error" role="alert"><p>{t('This operation could not finish. Check the local service and source file, then retry.')}</p><Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(error)}</p></Disclosure></div>;
}

function PlanDetails({ review, metadata }) {
  const { t, number } = useI18n();
  const plan = review.plan;
  return <Surface variant="inset" className="processing-plan" aria-label={t('Verified processing plan')}>
    <h3><CheckCircle2 size={17}/>{t('Verified processing plan')}</h3>
    {metadata && <figure className="processing-window-preview"><div><img src={metadata.previewDataUrl} width={metadata.previewWidth} height={metadata.previewHeight} alt={t('Local SCL source pixels with the planned crop window')}/><span className="processing-window-overlay" style={{ left: `${plan.window[0] / metadata.width * 100}%`, top: `${plan.window[1] / metadata.height * 100}%`, width: `${plan.window[2] / metadata.width * 100}%`, height: `${plan.window[3] / metadata.height * 100}%` }} aria-hidden="true"/></div><figcaption>{t(review.recipe.operation.geometry ? 'The outline is the output window. Pixels outside the polygon will be set to nodata when the job runs.' : 'The outlined rectangle is the verified pixel window on the local source preview.')}</figcaption></figure>}
    <dl className="runtime-details"><dt>{t('Output dimensions')}</dt><dd>{t('{width} × {height} pixels', { width: number(plan.width), height: number(plan.height) })}</dd><dt>{t('Output CRS')}</dt><dd>{plan.crs}</dd><dt>{t('Pixel size (metres)')}</dt><dd>{plan.pixelSize.map(value => number(value, { maximumFractionDigits: 3 })).join(' × ')}</dd><dt>{t('Actual bounds (metres)')}</dt><dd className="processing-bound-list">{['Min X', 'Min Y', 'Max X', 'Max Y'].map((label, i) => <span key={label}>{t(label)}: {number(plan.bounds[i], { maximumFractionDigits: 3 })}</span>)}</dd><dt>{t('Pixel window')}</dt><dd>{t('Offset {x}, {y} · {width} × {height}', { x: number(plan.window[0]), y: number(plan.window[1]), width: number(plan.window[2]), height: number(plan.window[3]) })}</dd>{Number.isSafeInteger(plan.maskedPixels) && <><dt>{t('Pixels outside polygon')}</dt><dd>{number(plan.maskedPixels)}</dd></>}<dt>{t('Output format')}</dt><dd>GeoTIFF · UInt8 · SCL</dd></dl>
    <p>{t(review.recipe.operation.geometry ? 'Copies pixels inside the WGS84 polygon and sets outside pixels to nodata. The output keeps the source CRS and pixel size.' : 'Copies source pixels without resampling. The output preserves the source coordinate system and pixel size.')}</p>
    {plan.warnings.length > 0 && <div className="processing-warnings"><strong>{t('Plan adjustments')}</strong><ul>{plan.warnings.map((warning, i) => <li key={i}>{t(warning)}</li>)}</ul></div>}
  </Surface>;
}

export function ClipRasterButton({ job, areaBounds, areaPolygon }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return <><Button onClick={() => setOpen(true)}><Crop size={15}/>{t('Clip raster')}</Button>{open && <RecipeEditorDialog sourceJob={job} areaBounds={areaBounds} areaPolygon={areaPolygon} onClose={() => setOpen(false)}/>}</>;
}

export function RecipeEditorDialog({ sourceJob, initialRecipe, initialMetadata, areaBounds, areaPolygon, onClose, onSaved }) {
  const { t } = useI18n();
  const { refresh } = useRuntime();
  const mounted = useRef(true);
  const version = useRef(0);
  const request = useRef(null);
  const [form, setForm] = useState(() => ({ name: initialRecipe?.name || `${sourceJob?.itemId || 'SCL'} · ${t('Raster clip')}`, crs: initialRecipe?.operation.crs || 'source', bounds: initialRecipe?.operation.bounds.map(String) || ['', '', '', ''], geometry: initialRecipe?.operation.geometry || null }));
  const [metadata, setMetadata] = useState(() => initialMetadata
    && initialMetadata.sha256?.toLowerCase() === sourceJob?.sha256?.toLowerCase()
    && initialMetadata.sha256?.toLowerCase() === initialRecipe?.source?.sha256?.toLowerCase()
    ? initialMetadata : null);
  const [loading, setLoading] = useState(!initialRecipe);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [review, setReview] = useState(null);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [edited, setEdited] = useState(false);
  const source = initialRecipe?.source || { jobId: sourceJob?.id, sha256: sourceJob?.sha256 };
  const mutating = busy === 'save' || busy === 'run';
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
    try { return { recipe: validateRecipe({ schemaVersion: form.geometry ? POLYGON_RECIPE_SCHEMA : RECIPE_SCHEMA, name: form.name, source, operation: { type: 'clip', crs: form.crs, bounds: form.bounds.map(value => value.trim() === '' ? NaN : Number(value)), ...(form.geometry ? { geometry: form.geometry } : {}) }, output: { format: 'GeoTIFF' } }), error: '' }; }
    catch (e) { return { recipe: null, error: e.message }; }
  }, [form, source.jobId, source.sha256]);
  const ready = draft.recipe && planMatches(draft.recipe, review);
  const update = changes => {
    version.current++; request.current?.abort(); setReview(null); setBusy(''); setMessage(''); setError(''); setEdited(true);
    setForm(old => ({ ...old, ...changes }));
  };
  const changeMode = crs => update({ crs, geometry: null, bounds: (crs === 'source' ? metadata?.bounds : areaBounds)?.map(String) || ['', '', '', ''] });
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
  return <Modal title={t(initialRecipe ? 'Review executable recipe' : 'Clip raster')} description={t('Create a local SCL GeoTIFF from a rectangle or an administrative polygon.')} wide onClose={close} closeDisabled={mutating} closeLabel={t('Close processing plan')}>
    <div className="runtime-dialog-body">
      <Surface as="div" variant="inset" className="processing-source"><strong>{t('Pinned source file')}</strong><span className="mono runtime-wrap">{sourceJob?.itemId || source.jobId}</span><small className="mono runtime-wrap">SHA-256 · {source.sha256}</small></Surface>
      {loading ? <p role="status" className="processing-loading"><Spinner size={18}/>{t('Reading source geometry…')}</p> : <>
        <label className="runtime-field">{t('Recipe name')}<Input value={form.name} disabled={mutating} onChange={event => update({ name: event.target.value })}/></label>
        <div className="processing-coordinate-heading"><label className="runtime-field">{t('Input coordinates')}<Select value={form.crs} disabled={mutating} onChange={event => changeMode(event.target.value)}><option value="source">{t('Source CRS · metres')}{metadata ? ` · ${metadata.crs}` : ''}</option><option value="EPSG:4326">{t('WGS84 · longitude / latitude')}</option></Select></label>{Array.isArray(areaBounds) && areaBounds.length === 4 && <Button className="processing-area-button" disabled={mutating} onClick={() => update({ crs: 'EPSG:4326', geometry: null, bounds: areaBounds.map(String) })}>{t('Use workspace rectangle')}</Button>}{areaPolygon && <Button className="processing-area-button" disabled={mutating} onClick={() => update({ crs: 'EPSG:4326', geometry: areaPolygon.geometry, bounds: areaPolygon.bounds.map(String) })}>{t('Use {name} polygon', { name: areaPolygon.place?.name || t('region') })}</Button>}</div>
        <p className="processing-hint">{t(form.crs === 'source' ? 'Order: minimum X, minimum Y, maximum X, maximum Y in the source projection.' : 'Order: west longitude, south latitude, east longitude, north latitude in degrees.')}</p>
        {form.crs === 'EPSG:4326' && <p className="processing-hint">{t('UTM coverage: latitude -80° to 84°; longitude span at most 180°. Bounds cannot cross the date line.')}</p>}
        <div className="processing-coordinate-fields">{(form.crs === 'source' ? ['Min X', 'Min Y', 'Max X', 'Max Y'] : ['West longitude', 'South latitude', 'East longitude', 'North latitude']).map((label, index) => <label className="runtime-field" key={index}>{t(label)}<Input type="number" step="any" value={form.bounds[index]} disabled={mutating} onChange={event => update({ bounds: form.bounds.map((value, i) => i === index ? event.target.value : value) })}/></label>)}</div>
        <Surface as="div" variant="inset" className="runtime-notice"><Crop size={17}/><span>{t(form.geometry ? 'The administrative boundary masks this local SCL raster in its original UTM grid. Outside pixels become nodata. You can narrow these bounds while keeping the polygon. Polygon jobs are limited to 8 million output pixels and require source nodata.' : 'The request is intersected with source coverage and aligned to whole pixels. WGS84 bounds produce an enclosing rectangle in the source projection; rectangular clips are not polygon-masked.')}</span></Surface>
        {edited && draft.error && <p className="runtime-error" role="alert">{t(draft.error)}</p>}
        <Button variant="primary" className="processing-preview-button" disabled={!draft.recipe || Boolean(busy)} onClick={() => execute('plan')}>{busy === 'plan' ? <Spinner size={15}/> : <Crop size={15}/>} {t(busy === 'plan' ? 'Checking plan…' : 'Check processing plan')}</Button>
        {ready ? <PlanDetails review={review} metadata={metadata}/> : <p className="processing-hint">{t('Save and Run unlock only after checking these exact settings. Editing a setting requires a new plan.')}</p>}
      </>}
      {error && <><Failure error={error}/>{!metadata && !initialRecipe && <Button disabled={loading} onClick={() => setLoadAttempt(value => value + 1)}><RefreshCw size={15}/>{t('Retry reading source')}</Button>}</>}
      {message && <p className="processing-success" role="status"><CheckCircle2 size={16}/>{t(message)}</p>}
    </div><footer className="runtime-dialog-actions"><Button disabled={mutating} onClick={close}>{t('Close')}</Button><Button disabled={!ready || Boolean(busy)} onClick={() => execute('save')}><Save size={15}/>{t(busy === 'save' ? 'Saving…' : 'Save executable recipe')}</Button><Button variant="primary" disabled={!ready || Boolean(busy)} onClick={() => execute('run')}><Play size={15}/>{t(busy === 'run' ? 'Starting…' : 'Run clip')}</Button></footer>
  </Modal>;
}

function ImportRecipeDialog({ onClose, onReview }) {
  const { t } = useI18n();
  const [text, setText] = useState('');
  const [recipe, setRecipe] = useState(null);
  const [error, setError] = useState('');
  const validate = () => { try { setRecipe(parseRecipeJSON(text)); setError(''); } catch (e) { setRecipe(null); setError(e.message); } };
  return <Modal title={t('Import executable recipe')} wide onClose={onClose} closeLabel={t('Close recipe import')}>
    <div className="runtime-dialog-body"><p>{t('Paste a geod-raster-recipe/v1 rectangle or v2 polygon document. Importing does not save a recipe or start a task. The pinned source job and file must exist on this device.')}</p><label className="runtime-field">{t('Recipe JSON')}<Textarea className="processing-json-input mono" rows={12} value={text} spellCheck={false} onChange={event => { setText(event.target.value); setRecipe(null); setError(''); }}/></label><Button onClick={validate} disabled={!text.trim()}><FileJson size={15}/>{t('Validate JSON')}</Button>{error && <p className="runtime-error" role="alert">{t(error)}</p>}{recipe && <section className="processing-import-preview"><h3>{t('Validated recipe preview')}</h3><Surface as="pre" variant="inset">{JSON.stringify(recipe, null, 2)}</Surface><p>{t('Next, check the actual source geometry and processing plan before saving or running.')}</p></section>}</div><footer className="runtime-dialog-actions"><Button onClick={onClose}>{t('Close')}</Button><Button variant="primary" disabled={!recipe} onClick={() => onReview(recipe)}>{t('Review processing plan')}</Button></footer>
  </Modal>;
}

export function ExecutableRecipes({ onReviewJSON, areaBounds, areaPolygon }) {
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
  return <section className="processing-recipes" aria-label={t('Executable local recipes')}><div className="processing-section-heading"><div><h2>{t('Executable local recipes')} <Badge>{number(recipes.length)}</Badge></h2><p>{t('Real rectangular and polygon-masked SCL clips, saved with a pinned source checksum.')}</p></div><div className="row-actions"><Button disabled={loading} onClick={() => setReload(value => value + 1)}><RefreshCw size={15}/>{t('Refresh recipes')}</Button><Button onClick={() => setImporting(true)}><Upload size={15}/>{t('Import JSON')}</Button></div></div>
    {loading && <p className="processing-loading" role="status"><Spinner size={16}/>{t('Loading saved recipes…')}</p>}
    {error && <Failure error={error}/>}
    {!loading && !error && !recipes.length && <Surface variant="inset" className="runtime-empty"><p>{t('Open a downloaded SCL file in My Data and choose Clip raster. Check the plan, then save it here for reuse.')}</p></Surface>}
    <div className="processing-recipe-list">{recipes.map(saved => <Surface as="article" className="processing-recipe-card" key={saved.id}><div><h3>{saved.recipe.name}</h3><p>{t(saved.recipe.operation.geometry ? 'Polygon clip' : 'Rectangular clip')} · {saved.recipe.operation.crs === 'source' ? t('Source CRS') : 'EPSG:4326'} · GeoTIFF</p><small>{t('Saved {date}', { date: date(saved.updatedAt) })}</small><p className="mono runtime-wrap">{saved.recipe.source.jobId}</p></div><div className="row-actions"><Button onClick={() => onReviewJSON('geod-raster-recipe.json', saved.recipe)}><FileJson size={15}/>{t('Review / export JSON')}</Button><Button variant="primary" onClick={() => setSelected(saved.recipe)}><Play size={15}/>{t('Review and run')}</Button></div></Surface>)}</div>
    {importing && <ImportRecipeDialog onClose={() => setImporting(false)} onReview={recipe => { setImporting(false); setSelected(recipe); }}/>}
    {selected && <RecipeEditorDialog initialRecipe={selected} sourceJob={jobs.find(job => job.id === selected.source.jobId)} areaBounds={areaBounds} areaPolygon={areaPolygon} onSaved={() => setReload(value => value + 1)} onClose={() => setSelected(null)}/>}
  </section>;
}

export function DerivedArtifactDetails({ job }) {
  const { t, number } = useI18n();
  if (job.kind !== 'raster_clip') return null;
  return <div className="processing-derived"><Badge tone="blue">{t('Derived GeoTIFF')}</Badge>{job.crop && <p>{t('{width} × {height} pixels', { width: number(job.crop.width), height: number(job.crop.height) })} · {job.crop.crs}</p>}{Number.isSafeInteger(job.crop?.maskedPixels) && <p>{t('Pixels outside polygon')}: {number(job.crop.maskedPixels)}</p>}<p>{t('Parent job')}: <span className="mono runtime-wrap">{job.parentId || job.recipe?.source?.jobId}</span></p>{job.recipe?.source?.sha256 && <Disclosure summary={t('Pinned source checksum')}><p className="mono runtime-wrap">{job.recipe.source.sha256}</p></Disclosure>}{job.manifestPath && <Disclosure summary={t('Output provenance manifest')}><p>{t('Keep this JSON file with the GeoTIFF when moving or sharing the output.')}</p><p className="mono runtime-wrap">{job.manifestPath}</p></Disclosure>}</div>;
}
