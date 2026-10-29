import React, { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowUpRight, Database, Download, FolderOpen, Layers, ListChecks, Search, X } from 'lucide-react';
import { InfoDetails, Badge, Button, Dialog, DialogContent, DialogTitle, Disclosure, Input, Modal, Select, Spinner, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { validQueryBounds } from './features-client.js';
import { publicDatasetRequest, publicDatasetMonths, validatePublicDatasetItem, publicDatasetAssetLayer, publicDatasetPreviewRange, publicDatasetPreviewPolicy } from './public-dataset-client.js';
import { stacRequest, validateStacSearchPage } from './stac-client.js';
import { PublicDatasetExploreMap } from './public-dataset-explore-map.jsx';
import { ExploreSourceFrame } from './explore-source-workspace.jsx';
import { DeliveryReceipt, ExploreResultRow, SelectionActions } from './acquisition-ui.jsx';
import { formatBytes } from './runtime-client.js';
import {NativeRasterExploreMap} from './native-raster-explore-map.jsx';
import {reviewedArchiveSelection,reviewedArchiveDownloadCandidates,inspectReviewedArchiveDownload,validateOriginalRasterInspection,reviewedClcdSelection,reviewedClcdDownloadCandidates,inspectReviewedClcdDownload,reviewedIhuSelection,reviewedIhuDownloadCandidates,inspectReviewedIhuDownload} from './original-backlog-client.js';
import {SpectralPixelTable} from './enmap-ui.jsx';
import './stac.css';
import './public-dataset.css';

const keyFor = (snapshotId, assetKey) => JSON.stringify([snapshotId, assetKey]);
const MAX_PRODUCT_PREVIEWS = 4;
const previewableAsset = asset => {
  if (!asset?.eligible) return false;
  try { return /(?:image\/(?:tiff|geotiff)|application\/(?:geotiff|x-geotiff))/i.test(asset.mediaType || '') || /\.tiff?$/i.test(new URL(asset.href).pathname); }
  catch { return false; }
};
const annualMissingDataNote = 'The tested 2025 x218y075 quality and valid-count originals have no embedded TIFF NoData tag, although the publisher table declares 255. Native inspection follows each original file, without automatically masking 0 or 255. This does not establish other tiles\' missing-data declarations.';
const boundsFromText = value => value.split(',').map(coordinate => coordinate.trim() === '' ? NaN : Number(coordinate));
const itemIdentity = item => JSON.stringify([item.connectionId, item.collectionId, item.itemId]);
const mergeItems = (previous, next) => {
  const seen = new Set(previous.map(itemIdentity));
  return [...previous, ...next.filter(item => { const key = itemIdentity(item); if (seen.has(key)) return false; seen.add(key); return true; })];
};

export function PublicDatasetDialog(props) { return <PublicDatasetWorkflow {...props}/>; }
export function PublicDatasetExplorer(props) { return <PublicDatasetWorkflow {...props} inline/>; }

function PublicDatasetWorkflow({ datasetId, areaBounds, areaGeometry, currentProject, onClose, onSaved, onOpenProject, onOpenCoverage, inline = false, autoSearch = false, searchRevision = 0, sidebarTarget, listHidden = false, onShowList, onRequestArea, onRequestChange, onBoundsChange, onCatalogChange, onMutationChange, workflowHeader }) {
  const { t, number, locale } = useI18n();
  const { projects = [], jobs = [], refresh = async () => {} } = useContext(RuntimeContext) || {};
  const [datasets, setDatasets] = useState([]);
  const [chosen, setChosen] = useState(datasetId || '');
  const [year, setYear] = useState('');
  const [month, setMonth] = useState('');
  const [layer, setLayer] = useState('');
  const [country, setCountry] = useState('');
  const [selectedRegion, setSelectedRegion] = useState('');
  const [season, setSeason] = useState('');
  const [region, setRegion] = useState((inline ? areaBounds || currentProject?.bounds || [] : currentProject?.bounds || areaBounds || []).join(', '));
  const [plan, setPlan] = useState(null);
  const [connection, setConnection] = useState(null);
  const [items, setItems] = useState([]);
  const [searches, setSearches] = useState([]);
  const [selected, setSelected] = useState({});
  const [destination, setDestination] = useState(currentProject?.id || 'new');
  const [projectName, setProjectName] = useState('');
  const [saved, setSaved] = useState(null);
  const [delivery, setDelivery] = useState('');
  const [deliveryIds,setDeliveryIds]=useState([]);
  const [busy, setBusy] = useState('loading');
  const [error, setError] = useState('');
  const [focusedId, setFocusedId] = useState(null);
  const [rasterLayers, setRasterLayers] = useState([]);
  const [nativeRasters,setNativeRasters]=useState([]),[nativePixel,setNativePixel]=useState(null);
  const [saveOpen, setSaveOpen] = useState(false);
  const previewGeneration = useRef(0);
  const request = useRef(null);
  const searchedArea = useRef(null);
  const previousArea = useRef((areaBounds || []).join(', '));
  const previousRevision = useRef(searchRevision);
  const dataset = datasets.find(item => item.id === chosen);
  const catalogRaster = dataset?.discovery === 'catalog-raster';
  const catalogSearch = dataset?.discovery === 'stac-api' || catalogRaster;
  const observationPeriod = dataset?.discovery === 'stac-api' || ['jaxa-amsr2-soil-moisture', 'jaxa-sgli-lst', 'jaxa-modis-ndvi', 'maxar-open-legacy','vantor-open'].includes(dataset?.id);
  const publishedItems = dataset?.discovery === 'stac-items';
  const selectedLayer = dataset?.layers.find(option => option.id === layer);
  const availableMonths = publicDatasetMonths(dataset,year);
  const availableSeasons = (dataset?.seasons || []).filter(option => !selectedLayer?.seasons?.length || selectedLayer.seasons.includes(option.id));
  const bounds = boundsFromText(region);
  const selections = Object.values(selected);
  const selectedFiles=selections.map(selection=>items.find(item=>item.id===selection.snapshotId)?.assets.find(asset=>asset.key===selection.assetKey));
  const declaredBytes=selectedFiles.map(asset=>asset?.metadata?.['file:size']);
  const estimatedBytes=declaredBytes.length&&declaredBytes.every(value=>Number.isFinite(value)&&value>0)?declaredBytes.reduce((a,b)=>a+b,0):null;
  const layerBoundSelection = ['maxar-open-legacy','cbers-wpm','amazonia-wfi-dn','vantor-open'].includes(dataset?.id);
  const localBandPreview = dataset?.id === 'cbers-wpm';
  const selectionReviewed = !layerBoundSelection || selections.every(selection => {
    const item = items.find(candidate => candidate.id === selection.snapshotId);
    const asset = item?.assets.find(candidate => candidate.key === selection.assetKey);
    return asset?.eligible && publicDatasetAssetLayer(dataset, plan?.request, asset.key);
  });
  const choices = currentProject && !projects.some(project => project.id === currentProject.id) ? [currentProject, ...projects] : projects;
  const target = choices.find(project => project.id === destination);
  const valid = dataset && validQueryBounds(bounds) && (!dataset.years.length || dataset.years.includes(Number(year)))
    && (!dataset.months?.length || availableMonths.includes(Number(month)))
    && (!selectedRegion || (dataset.regions || []).some(option => option.id === selectedRegion))
    && (!dataset.seasons?.length || availableSeasons.some(option => option.id === season))
    && dataset.layers.some(option => option.id === layer) && (!dataset.requiresCountry || /^[A-Z]{3}$/.test(country));
  const payload = () => ({ datasetId: chosen, bounds, ...(dataset.years.length ? { year: Number(year) } : {}), layer,
    ...(dataset.months?.length ? { month: Number(month) } : {}),
    ...(selectedRegion ? { region: selectedRegion } : {}), ...(dataset.seasons?.length ? { season } : {}),
    ...(dataset.requiresCountry ? { country } : {}) });
  const clearRasters = () => { previewGeneration.current += 1; setRasterLayers([]); setNativeRasters([]);setNativePixel(null); };
  const invalidate = () => { clearRasters(); setSaveOpen(false); setPlan(null); setConnection(null); setItems([]); setSearches([]); setSelected({}); setSaved(null); setDelivery(''); setError(''); setFocusedId(null); };
  const edit = change => { request.current?.abort(); setBusy(''); invalidate(); change(); };
  const chooseDataset = next => {
    setChosen(next?.id || ''); setYear(next?.defaultYear == null ? next?.years.at(-1)?.toString() || '' : String(next.defaultYear));
    setMonth(next?.defaultMonth == null ? next?.months?.[0]?.toString() || '' : String(next.defaultMonth));
    const nextLayer = next?.layers.find(option => option.id === next.defaultLayer) || next?.layers[0];
    setLayer(nextLayer?.id || ''); setCountry(next?.id==='worldclim'?'NPL':''); setProjectName(next?.title || ''); setSelectedRegion(next?.defaultRegion || '');
    setSeason(nextLayer?.seasons?.length === 1 ? nextLayer.seasons[0] : next?.defaultSeason || next?.seasons?.[0]?.id || '');
  };
  async function perform(label, work) {
    request.current?.abort(); const abort = new AbortController(); request.current = abort;
    setBusy(label); setError('');
    try { await work(abort.signal); }
    catch (cause) { if (!abort.signal.aborted) setError(cause.message); }
    finally { if (!abort.signal.aborted) setBusy(''); }
  }
  const load = () => perform('loading', async signal => {
    const available = await publicDatasetRequest('list', {}, signal);
    if (signal.aborted) return;
    setDatasets(available); onCatalogChange?.(available);
    const next = available.find(item => item.id === datasetId) || (!datasetId ? available[0] : null);
    chooseDataset(next); if (!next) throw new Error(t('This product connector is not available in the running app.'));
  });
  useEffect(() => { load(); return () => request.current?.abort(); }, []);
  const areaKey = (areaBounds || []).join(', ');
  useEffect(() => {
    if (!inline || (previousArea.current === areaKey && previousRevision.current === searchRevision)) return;
    const areaChanged = previousArea.current !== areaKey;
    previousArea.current = areaKey;
    previousRevision.current = searchRevision;
    if (areaChanged) { setRegion(areaKey); searchedArea.current = null; }
    // Catalog definitions do not depend on the area. Let the initial list finish
    // while cancelling any area-dependent search and clearing its selections.
    if (busy !== 'loading') { request.current?.abort(); setBusy(''); }
    invalidate();
  }, [inline, areaKey, searchRevision]);
  useEffect(() => {
    onRequestChange?.(valid ? payload() : null);
  }, [chosen, year, month, layer, country, region, selectedRegion, season, Boolean(valid), searchRevision, onRequestChange]);
  useEffect(() => {
    onMutationChange?.(['save', 'download'].includes(busy));
    return () => onMutationChange?.(false);
  }, [busy, onMutationChange]);
  const resolve = () => perform('resolve', async signal => { invalidate(); const result = await publicDatasetRequest('resolve', payload(), signal); if (!signal.aborted) setPlan(result); });
  const readSources = async (query, signal) => {
    setConnection(null); setItems([]); setSearches([]); setSelected({}); setSaved(null); setDelivery('');
    setFocusedId(null);
    const next = await publicDatasetRequest('connect', query, signal);
    if (signal.aborted) return;
    const ids = [...new Set(next.stacConnections.filter(source => ['raster', 'item'].includes(source.kind)).flatMap(source => source.snapshotIds))];
    const snapshots = [];
    for (const id of ids) {
      const snapshot = await stacRequest('snapshot', { id }, signal);
      if (signal.aborted) return;
      snapshots.push(validatePublicDatasetItem(snapshot, next.plan, next.stacConnections));
    }
    setPlan(next.plan); setConnection(next); setSearches(next.searches || []); setItems(mergeItems(snapshots, (next.searches || []).flatMap(search => search.page.items)));
  };
  const connect = () => perform('connect', signal => readSources(plan.request, signal));
  // The native connector resolves and archives original metadata itself. A
  // single exploration action can use it without creating any download job.
  const searchProducts = () => perform('connect', signal => { invalidate(); return readSources(payload(), signal); });
  useEffect(() => {
    if (!inline || !autoSearch || !valid || busy || !validQueryBounds(areaBounds)
      || bounds.join(',') !== areaBounds.join(',')) return;
    const stamp = JSON.stringify([chosen, areaKey, searchRevision]);
    if (searchedArea.current === stamp) return;
    searchedArea.current = stamp;
    searchProducts();
  }, [inline, autoSearch, chosen, areaKey, region, Boolean(valid), busy, searchRevision]);
  const focusItem = id => {
    if (!items.some(item => item.id === id)) return;
    setFocusedId(id);
    document.getElementById(`public-dataset-item-${id}`)?.scrollIntoView?.({block:'nearest',behavior:'smooth'});
  };
  const selectedPreviews = useMemo(() => {
    const candidates = selections.flatMap(pin => {
    const item = items.find(candidate => candidate.id === pin.snapshotId);
    const asset = item?.assets.find(candidate => candidate.key === pin.assetKey);
    if (!item || dataset?.id==='vantor-open' || !previewableAsset(asset) || layerBoundSelection && !publicDatasetAssetLayer(dataset, plan?.request, asset.key)) return [];
    const measurement = publicDatasetAssetLayer(dataset, plan?.request, asset.key);
    return [{item,identity:JSON.stringify([item.title || item.itemId,asset.key]),preview:{id:keyFor(pin.snapshotId,pin.assetKey),...pin,itemId:item.itemId,
      title:`${item.title || item.itemId} · ${['cbers-wpm','amazonia-wfi-dn'].includes(dataset?.id) ? `${asset.key} · ` : ''}${measurement ? t(measurement.title) : asset.title || asset.key}`,
      unit:measurement?.unit || null,displayRange:publicDatasetPreviewRange(dataset,asset,item),overviewPolicy:publicDatasetPreviewPolicy(dataset,asset,item),visible:true,opacity:1,retry:0}}];
    });
    const counts = new Map();
    for (const candidate of candidates) counts.set(candidate.identity,(counts.get(candidate.identity) || 0)+1);
    return candidates.map(({item,identity,preview}) => counts.get(identity)>1 && validQueryBounds(item.bbox)
      ? {...preview,title:`${t('Longitude {west}° to {east}°',{west:String(item.bbox[0]),east:String(item.bbox[2])})} · ${preview.title}`}
      : preview);
  }, [selected, items, plan, dataset, layerBoundSelection, t]);
  const previewCount = new Set([...rasterLayers,...selectedPreviews].map(entry=>entry.id)).size;
   const selectedArchives=selections.map(pin=>{const item=items.find(item=>item.id===pin.snapshotId),asset=item?.assets.find(asset=>asset.key===pin.assetKey);return reviewedArchiveSelection(asset)||reviewedClcdSelection(asset)||reviewedIhuSelection(asset)||localBandPreview&&asset?.eligible&&/^BAND[1-4]$/.test(asset.key)||dataset?.id==='vantor-open'&&asset?.eligible&&asset.key==='visual'?{pin,item,asset}:null;}).filter(Boolean);
  const archiveJobs=selectedArchives.map(({pin,item,asset})=>{
    const candidates=reviewedArchiveSelection(asset)?reviewedArchiveDownloadCandidates(asset,jobs,pin.snapshotId):reviewedClcdSelection(asset)?reviewedClcdDownloadCandidates(asset,jobs,pin.snapshotId):reviewedIhuSelection(asset)?reviewedIhuDownloadCandidates(asset,jobs,pin.snapshotId):jobs.filter(job=>job.status==='succeeded'&&job.stacSource?.snapshotId===pin.snapshotId&&job.stacSource?.assetKey===pin.assetKey);
    return {item,asset,candidates,job:candidates[0]};
  });
  const reviewedArchiveDownloadsAvailable=dataset?.id!=='vantor-open'&&selectedArchives.length>0&&archiveJobs.every(({job})=>Boolean(job));
  const loadNativeArchives=()=>perform('inspect',async signal=>{
    setNativeRasters([]);setNativePixel(null);const next=[];
    for(const{item,asset,candidates,job}of archiveJobs){
      if(!job)throw new Error(localBandPreview?'Download the complete original band before loading its local preview.':dataset?.id==='clcd'?'Download the complete CLCD original before loading its classification preview.':dataset?.id==='vantor-open'?'Download the complete visual TIFF before loading its decoded component preview.':'Download the complete original ZIP before loading its TIFF member.');
      const original=reviewedArchiveSelection(asset)?await inspectReviewedArchiveDownload(asset,candidates,stacRequest,signal):reviewedClcdSelection(asset)?await inspectReviewedClcdDownload(asset,candidates,stacRequest,signal):reviewedIhuSelection(asset)?await inspectReviewedIhuDownload(asset,candidates,stacRequest,signal):{job,item,data:await stacRequest('inspect',{id:job.id},signal)};
      if(signal.aborted)return;
      validateOriginalRasterInspection(original.data,original.job,original.item);
      next.push({key:original.job.id,title:item.title,job:original.job,item:original.item,data:original.data,visible:true,opacity:1});
    }
    if(!signal.aborted)setNativeRasters(next);
  });
  const sampleNative=(entry,position)=>perform('pixel',async signal=>{const value=await stacRequest('pixel',{id:entry.job.id,...position},signal);if(value.sha256!==entry.data.sha256||value.values.length!==entry.data.bands.length)throw new Error('The pixel does not match the inspected raster.');if(!signal.aborted)setNativePixel({title:entry.title,...value,sampleEncoding:entry.data.sampleEncoding,spectralEncoding:entry.data.spectralEncoding});});
  const changeNativeBand=(entry,displayBand)=>perform('band',async signal=>{
    const data=await stacRequest('inspect',{id:entry.job.id,displayBand},signal);
    validateOriginalRasterInspection(data,entry.job,entry.item);
    if(!signal.aborted)setNativeRasters(current=>current.map(e=>e.key===entry.key?{...e,data}:e));
  });
  const loadPreviews = () => {
    if(selectedArchives.length)return loadNativeArchives();
    if (busy || !selectedPreviews.length || previewCount > MAX_PRODUCT_PREVIEWS) return;
    const existing = new Set(rasterLayers.map(entry=>entry.id));
    setRasterLayers([...rasterLayers.map(entry=>{const candidate=selectedPreviews.find(candidate=>candidate.id===entry.id);return candidate?{...entry,title:candidate.title,visible:true}:entry;}),
      ...selectedPreviews.filter(entry=>!existing.has(entry.id)).map(entry=>({...entry,generation:++previewGeneration.current}))]);
  };
  const changeRaster = (id, patch) => setRasterLayers(current=>current.map(entry=>entry.id===id?{
    ...entry,...(typeof patch.visible === 'boolean'?{visible:patch.visible}:{}),
    ...(Number.isFinite(patch.opacity)?{opacity:Math.max(0,Math.min(1,patch.opacity))}:{}),
    ...(patch.retry ? {retry:entry.retry+1}:{}),
  }:entry));
  useEffect(()=>{if(nativePixel)document.querySelector('.public-dataset-native-pixel')?.scrollIntoView?.({block:'nearest'});},[nativePixel]);
  const clearSelection = () => { setSelected({}); setSaved(null); setDelivery(''); };
  const selectReviewed = () => {
    if (busy || !plan) return;
    const pins = items.flatMap(item => item.assets.filter(asset=>asset.eligible &&
      (!layerBoundSelection || Boolean(publicDatasetAssetLayer(dataset,plan.request,asset.key))) &&
      (!publicDatasetAssetLayer(dataset,plan.request,asset.key) || publicDatasetAssetLayer(dataset,plan.request,asset.key)?.id===plan.request.layer))
      .map(asset=>({snapshotId:item.id,assetKey:asset.key}))).slice(0,32);
    setSelected(Object.fromEntries(pins.map(pin=>[keyFor(pin.snapshotId,pin.assetKey),pin]))); setSaved(null); setDelivery('');
  };
  const loadMore = search => perform('search', async signal => {
    const source = plan.sources.find(source => source.id === search.sourceId);
    const query = { connectionId: search.connectionId, ...source.search, cursor: search.page.nextCursor };
    const page = await stacRequest('search', query, signal);
    if (signal.aborted) return;
    validateStacSearchPage(page, query, true);
    setItems(previous => mergeItems(previous, page.items));
    setSearches(previous => previous.map(result => result.sourceId === search.sourceId ? { ...result, page } : result));
  });
  const saveSelection = async signal => {
    if (!selectionReviewed) throw new Error(t('Only the reviewed analytical layer can be selected.'));
    const result = await stacRequest('project', { ...(destination === 'new' ? { name: projectName.trim() } : { projectId: destination }),
      bounds: target?.bounds || plan.request.bounds, selections }, signal);
    if (signal.aborted) return;
    setSaved(result); await refresh(); if (!signal.aborted) onSaved?.(result);
    return result;
  };
  const save = () => perform('save',saveSelection);
  const confirmDownload=()=>perform('download',async signal=>{
    if(!selections.length||!selectionReviewed)return;
    const project=saved||await saveSelection(signal);
    if(signal.aborted||!project)return;
    const result=await stacRequest('downloads',{projectId:project.id,selections:project.canonicalSelections||selections},signal);
    if(signal.aborted)return;
    if(!result.jobs?.length)throw new Error('No download tasks were returned. The selection is saved; retry the download.');
    setDeliveryIds(result.jobs.map(job=>job.id));setDelivery('queued');await refresh();
  });
  const download = () => perform('download', async signal => {
    if (!selectionReviewed) throw new Error(t('Only the reviewed analytical layer can be selected.'));
    const result = await stacRequest('downloads', { projectId: saved.id, selections: saved.canonicalSelections || selections }, signal);
    if (signal.aborted) return;
    setDeliveryIds(result.jobs.map(job=>job.id));setDelivery(result.jobs.length && result.jobs.every(job => job.status === 'succeeded') ? 'ready' : 'queued'); await refresh();
  });
  const definition = dataset && <Surface variant="inset" className="public-dataset-definition">
          <div className="public-dataset-heading"><strong>{t(dataset.title)}</strong><Badge>{t(dataset.version)}</Badge></div>
          <p className="stac-help">{t(dataset.description)}</p>
          <dl className="stac-details"><dt>{t('Published grid spacing')}</dt><dd>{t(dataset.nominalResolution)}</dd><dt>{t('Declared unit')}</dt><dd>{t(dataset.unit)}</dd><dt>{t('Value interpretation')}</dt><dd>{t(dataset.valueInterpretation)}</dd><dt>{t('Declared license')}</dt><dd><a href={dataset.licenseUrl} target="_blank" rel="noreferrer">{dataset.license}</a></dd><dt>{t('Attribution')}</dt><dd>{t(dataset.attribution)}</dd></dl>
          <a className="public-dataset-documentation" href={dataset.documentationUrl} target="_blank" rel="noreferrer">{t('Product documentation')}<ArrowUpRight size={13} aria-hidden="true"/></a>
          {dataset.warnings.length > 0 && <section><h4>{t('Product limitations')}</h4>{dataset.warnings.map((warning, index) => <p key={index} className="stac-help">{t(warning)}</p>)}</section>}
          {dataset.limits.length > 0 && <section><h4>{t('Request limits')}</h4>{dataset.limits.map((limit, index) => <p key={index} className="stac-help">{t(limit)}</p>)}</section>}
        </Surface>;
  const body = <div className="stac-source-body">
      {workflowHeader}
      {(!inline || !autoSearch) && <p className="stac-help">{t('Choose a published product, review its files, then save and download into your workspace.')}</p>}
      {!inline && <label className="stac-field"><span>{t('Data product')}</span><Select aria-label={t('Data product')} value={chosen} disabled={Boolean(busy) || !datasets.length} onChange={event => edit(() => chooseDataset(datasets.find(item => item.id === event.target.value)))}>{!chosen && <option value="">{t('Choose a data product')}</option>}{datasets.map(item => <option key={item.id} value={item.id}>{t(item.title)}</option>)}</Select></label>}
      {dataset && <>
        {!inline && definition}
        {dataset.id === 'cbers-wpm' && <p className="stac-help">{t('CBERS WPM preview uses the downloaded original band. Save and download first, then load its local preview.')}</p>}
        {dataset.id === 'openlandmap-nightlights' && <InfoDetails closeLabel={t('Close')} summary={t('Preview display note')}><p className="stac-help">{t('Nighttime-light grayscale uses a 0–200 original-DN display stretch for contrast. Values outside it are clipped only for display; original signed values, valid zero and NoData remain unchanged. This does not establish physical radiance calibration.')}</p></InfoDetails>}
        {dataset.id === 'amazonia-wfi-dn' && <InfoDetails closeLabel={t('Close')} summary={t('Preview display note')}><p className="stac-help">{t('WFI grayscale uses a fixed 0–1023 raw-DN display stretch. Values outside it are clipped only for display; original pixels, TIFF NoData and metadata remain unchanged. BAND1 has complete-file acceptance; the other bands have not. This is not reflectance calibration.')}</p></InfoDetails>}
        <div className="stac-grid">
          <label className="stac-field public-dataset-layer-field"><span>{t('Product layer')}</span><Select aria-label={t('Product layer')} value={layer} disabled={Boolean(busy)} onChange={event => edit(() => {
            const next = dataset.layers.find(option => option.id === event.target.value); setLayer(next.id);
            const allowed = (dataset.seasons || []).filter(option => !next.seasons?.length || next.seasons.includes(option.id));
            setSeason(next.seasons?.length === 1 ? next.seasons[0] : allowed.some(option => option.id === season) ? season : allowed[0]?.id || '');
          })}>{dataset.layers.map(option => <option key={option.id} value={option.id}>{t(option.title)}</option>)}</Select></label>
          {dataset.years.length > 0 && <label className="stac-field"><span>{t(observationPeriod && dataset.months?.length ? 'Observation year' : 'Reference year')}</span><Select aria-label={t(observationPeriod && dataset.months?.length ? 'Observation year' : 'Reference year')} value={year} disabled={Boolean(busy)} onChange={event => edit(() => {setYear(event.target.value);if(dataset.months?.length&&!publicDatasetMonths(dataset,event.target.value).includes(Number(month)))setMonth('');})}>{dataset.years.map(value => <option value={value} key={value}>{value}</option>)}</Select></label>}
          {dataset.months?.length > 0 && <label className="stac-field"><span>{t(observationPeriod ? 'Observation month' : 'Reference month')}</span><Select aria-label={t(observationPeriod ? 'Observation month' : 'Reference month')} value={month} disabled={Boolean(busy)} onChange={event => edit(() => setMonth(event.target.value))}>{!month&&<option value="">{t('Choose an observation month')}</option>}{dataset.months.map(value => <option value={value} key={value} disabled={!availableMonths.includes(value)}>{number(value)}</option>)}</Select></label>}
          {dataset.requiresCountry && <label className="stac-field public-dataset-wide-field"><span>{t('Country code · ISO 3166-1 alpha-3')}</span><Input aria-label={t('Dataset country code')} placeholder="CHN" maxLength={3} value={country} disabled={Boolean(busy)} onChange={event => edit(() => setCountry(event.target.value.toUpperCase()))}/></label>}
          {dataset.regions?.length > 0 && <label className="stac-field public-dataset-wide-field"><span>{t('Published region')}</span><Select aria-label={t('Published region')} value={selectedRegion} disabled={Boolean(busy)} onChange={event => edit(() => setSelectedRegion(event.target.value))}><option value="">{t('Match current map area automatically')}</option>{dataset.regions.map(option => <option key={option.id} value={option.id}>{t(option.title)}</option>)}</Select><small className="stac-help">{t('Region selection retains your map area. Only intersecting published items are used.')}</small></label>}
          {dataset.seasons?.length > 0 && <label className="stac-field"><span>{t('Published season')}</span><Select aria-label={t('Published season')} value={season} disabled={Boolean(busy)} onChange={event => edit(() => setSeason(event.target.value))}>{availableSeasons.map(option => <option key={option.id} value={option.id}>{t(option.title)}</option>)}</Select></label>}
        </div>
        {!inline && selectedLayer && <p className="stac-help">{t(selectedLayer.valueInterpretation)}</p>}
        {dataset.id === 'deafrica-fractional-cover' && ['qa', 'count_valid'].includes(layer) && !items.length && <p className="stac-help">{t(annualMissingDataNote)}</p>}
        {(!inline || !autoSearch) && <label className="stac-field"><span>{t('Search bounds · west, south, east, north')}</span><Input aria-label={t('Dataset search bounds')} value={region} disabled={Boolean(busy)} onChange={event => edit(() => { setRegion(event.target.value); if (inline) onBoundsChange?.(event.target.value); })}/></label>}
        {(!inline || !autoSearch) && <p className="stac-help">{t(catalogRaster ? 'Search current published elevation tiles by map area. Release dates describe publication, not terrain observation time. Downloads retain complete original files.' : dataset.id === 'chelsa-climate' ? 'The selected area organizes one complete quasi-global CMI mean original for the fixed 1981–2010 climate normal. It does not crop, resample or change the original climate period.' : publishedItems ? dataset.seasons?.length ? 'The current map area selects reviewed regional publisher items for the theme and actual season. Downloads retain each complete selected original; classification and confidence assets keep their own grids. No clipping or resampling is applied.' : dataset.id === 'cgls-landcover' ? 'The selected reference year downloads one complete published continental permanent-water fraction file. Your area must lie within the distribution; it does not crop or resample the original.' : 'The map area selects published regional originals for this product and reference epoch. Each file retains its actual grid and raw samples; no clipping, resampling or scientific conversion is applied.' : catalogSearch ? 'The catalog returns original product items for the selected area and period. Downloads retain each complete selected asset; no clipping, RGB conversion or resampling is applied.' : dataset.id === 'chirps' ? 'The selected month downloads one complete global file covering 60°S to 60°N. The area organizes the project; it does not clip, subset or resample the original.' : dataset.id === 'hilda-land-use' ? 'The selected reference year downloads one complete published quasi-global model file. The area organizes the project; it does not clip or resample the original.' : dataset.requiresCountry ? 'The country code selects the complete published country raster. The area organizes the project; it does not crop the downloaded file.' : 'The area selects intersecting published tiles. Downloads retain each complete file; no clipping or resampling is applied.')}</p>}
        {publishedItems && (!inline || !autoSearch) && <p className="stac-help">{t(dataset.regions?.length ? 'Supported distribution: {regions}. The reference year is not the actual season interval; review each published start and end date below.' : 'Only the published regional distribution is available. Review each actual product period below; the reference year is not a single scene observation.', {regions:(dataset.regions || []).map(option => t(option.title)).join(' · ')})}</p>}
        <div className="public-dataset-search-actions"><Button variant="primary" disabled={Boolean(busy) || !valid} onClick={inline && autoSearch ? searchProducts : resolve}><Search size={15}/>{t(inline && autoSearch ? 'Search products' : publishedItems ? 'Review published items' : catalogSearch ? 'Review catalog search' : 'Find published files')}</Button>
        {inline && <InfoDetails closeLabel={t('Close')} className="public-dataset-product-details" summary={t('Product details')}>{definition}{selectedLayer && <p className="stac-help">{t(selectedLayer.valueInterpretation)}</p>}</InfoDetails>}</div>
        {inline && autoSearch && !valid && !busy && <p className="stac-help">{t(dataset.requiresCountry && !/^[A-Z]{3}$/.test(country) ? 'Enter the country code to search this product.' : 'Choose an area to search published products.')}</p>}
      </>}
      {plan && !autoSearch && <Surface variant="inset" className="public-dataset-review">
        <strong>{t(catalogSearch && !connection ? 'Catalog sources · {count}' : 'Published sources · {count}', { count: number(plan.sources.length) })}</strong>
        <p className="stac-help">{t(publishedItems ? 'These are standalone publisher metadata references. Reading archives the real item and original asset declarations; files require separate selection, saving and downloading. Actual grid and raw values are checked in the downloaded original.' : catalogSearch ? 'This is the fixed catalog query. Searching archives real item metadata; original files require separate selection, saving and downloading.' : 'These are product file references. File size and actual grid are checked when connecting and inspecting the download.')}</p>
        <p className="stac-help">{t('Product descriptions are GeoD preset declarations based on the publisher documentation. Download inspection reads the actual file; these declarations do not replace that check.')}</p>
        <ul className="public-dataset-source-list">{plan.sources.map(source => <li key={source.id}><strong>{source.title}</strong><span>{source.stac?.url || source.wcs?.url}</span>{source.search && <><span>{source.search.collectionId}</span><span>{source.search.datetime}</span></>}{source.region && <span>{t('Published region')}: {t(dataset.regions?.find(option => option.id === source.region)?.title || source.region)}</span>}{source.season && <span>{t('Published season')}: {t(dataset.seasons?.find(option => option.id === source.season)?.title || source.season)} · {source.season}</span>}{source.startDatetime && source.endDatetime && <span>{t('Actual product period')}: {source.startDatetime.slice(0, 10)} – {source.endDatetime.slice(0, 10)}</span>}{source.bounds && <span>{source.bounds.join(', ')}</span>}</li>)}</ul>
        {plan.warnings.length > 0 && <InfoDetails closeLabel={t('Close')} summary={t('File scope and usage conditions')}>{plan.warnings.map((warning, index) => <p key={index} className="stac-help">{t(warning)}</p>)}</InfoDetails>}
        {!plan.sources.length && <p className="stac-help" role="status">{t('No published files match this request.')}</p>}
        {!connection && plan.sources.length > 0 && <Button disabled={Boolean(busy)} onClick={connect}><Database size={15}/>{t(publishedItems ? 'Read published item metadata' : catalogSearch ? 'Search product catalog' : 'Check source files')}</Button>}
      </Surface>}
      {connection && !connection.complete && <Surface variant="inset" className="public-dataset-review" role="status">
        <strong>{t(publishedItems ? 'Some published items could not be read' : catalogSearch ? 'Some catalog sources could not be searched' : 'Some source files could not be checked')}</strong><p className="stac-help">{t('The available files can be selected below. Failed files are excluded; this is not a complete coverage claim.')}</p>
        <ul className="public-dataset-source-list">{connection.failures.map(failure => <li key={failure.sourceId}><strong>{plan.sources.find(source => source.id === failure.sourceId)?.title || failure.sourceId}</strong><span>{failure.error}</span></li>)}</ul>
        <Button disabled={Boolean(busy)} onClick={connect}>{t('Retry source checks')}</Button>
      </Surface>}
      {connection?.wcsConnections.map(source => <Surface variant="inset" className="public-dataset-review" key={source.id}><strong>{source.title}</strong><p className="stac-help">{t('Review the coverage grid before requesting a service-generated subset.')}</p><Button disabled={Boolean(busy) || !onOpenCoverage} onClick={() => { const selectedSource = plan.sources.find(item => item.wcs?.url === source.url); onOpenCoverage?.({ connectionId: source.id, coverageId: selectedSource.coverageId, bounds: plan.request.bounds }); }}>{t('Review coverage subset')}</Button></Surface>)}
      {inline && autoSearch && connection && <div className="public-dataset-results-heading"><span role="status">{t('{count} product files found', {count:number(items.length)})}</span>{items.length>0 && <Button variant="quiet" size="icon" icon={ListChecks} disabled={Boolean(busy)} aria-label={t('Select reviewed files')} tooltip={t('Select reviewed files')} onClick={selectReviewed}/>}{inline && autoSearch && plan && <InfoDetails closeLabel={t('Close')} summary={t('Search source details')}><ul className="public-dataset-source-list">{plan.sources.map(source=><li key={source.id}><strong>{source.title}</strong><span>{source.stac?.url || source.wcs?.url}</span>{source.bounds && <span>{source.bounds.join(', ')}</span>}</li>)}</ul>{plan.warnings.map((warning,index)=><p className="stac-help" key={index}>{t(warning)}</p>)}</InfoDetails>}</div>}
      {connection?.complete && !items.length && !connection.wcsConnections.length && (!plan?.sources.length||!searches.length) && <div className="public-dataset-empty" role="status">
        <p className="stac-help">{t(plan?.sources.length===0?'No published files from this connector overlap the selected area.':'No product items match this search.')}</p>
        {plan?.availableAreas?.length>0&&<label className="stac-field"><span>{t('View an available area')}</span><Select aria-label={t('View an available area')} value="" disabled={Boolean(busy)} onChange={event=>{const area=plan.availableAreas.find(item=>item.id===event.target.value);if(area)edit(()=>{setRegion(area.bounds.join(', '));onBoundsChange?.(area.bounds);});}}><option value="">{t('Choose an area to view')}</option>{plan.availableAreas.map(area=><option key={area.id} value={area.id}>{t(area.title)}</option>)}</Select></label>}
      </div>}
      {items.map(item => <Surface as="article" variant="inset" className="public-dataset-review" key={item.id} id={`public-dataset-item-${item.id}`} data-focused={item.id === focusedId || undefined}>
        {inline && autoSearch && <div className="public-dataset-result-heading"><Button variant="quiet" size="row" className="public-dataset-result-focus" title={t(item.title || item.itemId)} aria-label={t('Locate product {name}',{name:item.title || item.itemId})} aria-pressed={item.id===focusedId} onClick={()=>focusItem(item.id)}>{t(item.title || item.itemId)}</Button>
        {item.warnings.length>0 && <InfoDetails closeLabel={t('Close')} summary={t('Source file notes')}>{item.warnings.map((warning,index)=><p key={index}>{t(warning)}</p>)}</InfoDetails>}</div>}
        {!(inline&&autoSearch)&&<strong>{item.title || item.itemId}</strong>}
        {catalogRaster && <p className="stac-help">{t('Publisher release date')}: {item.properties.datasetPreset.catalogRaster.record.publicationDate} · {t('Release date is not the terrain observation date.')}</p>}
        {(catalogSearch || publishedItems) && <p className="stac-item-id">{item.itemId}{(item.datetime || item.startDatetime) && <> · {(publishedItems ? item.startDatetime || item.datetime : item.datetime || item.startDatetime).slice(0, 10)}{(publishedItems || !item.datetime) && item.endDatetime && ` – ${item.endDatetime.slice(0, 10)}`}</>}</p>}
        {publishedItems && item.bbox && <p className="stac-help">{t('Original published item bounds')}: {item.bbox.join(', ')}</p>}
        {!(inline && autoSearch) && item.warnings.length > 0 && <InfoDetails closeLabel={t('Close')} summary={t('Source file notes')}>{item.warnings.map((warning, index) => <p className="stac-help" key={index}>{t(warning)}</p>)}</InfoDetails>}
        {item.assets.filter(asset=>!autoSearch || dataset.id==='cbers-wpm' && asset.key==='BAND0' || asset.eligible && (!layerBoundSelection || Boolean(publicDatasetAssetLayer(dataset, plan.request, asset.key)))).map(asset => {
          const key=keyFor(item.id,asset.key),measurement=publicDatasetAssetLayer(dataset,plan.request,asset.key),selectable=asset.eligible&&(!layerBoundSelection||Boolean(measurement));
          return <ExploreResultRow key={asset.key} title={measurement?t(measurement.title):asset.title||asset.key} selected={Boolean(selected[key])}
            selectionLabel={`${t('Select asset')} · ${item.itemId} · ${asset.key}`} disabled={Boolean(busy)||!selectable||selections.length>=32&&!selected[key]}
            onSelect={event=>{if(!selectable)return;const checked=event.target.checked;setSelected(previous=>{const next={...previous};if(checked)next[key]={snapshotId:item.id,assetKey:asset.key};else delete next[key];return next;});setSaved(null);setDelivery('');}}
            onFocus={inline?()=>focusItem(item.id):undefined} focusLabel={t('Locate product {name}',{name:measurement?t(measurement.title):asset.title||asset.key})}
            meta={<>{asset.key}{asset.metadata?.['file:size']?` · ${formatBytes(asset.metadata['file:size'],locale)}`:''}{!selectable?` · ${t('Not available')}`:''}</>}
            actions={<InfoDetails closeLabel={t('Close')} summary={t('Asset source details')}><p>{asset.key} · {asset.mediaType||t('Media type not declared')}</p>
              {measurement&&<p>{t('Declared unit')}: {t(measurement.unit)} · {t(measurement.valueInterpretation)}</p>}
              {layerBoundSelection&&!measurement&&<p>{t(dataset.id==='cbers-wpm'?'This connector selects 8 m multispectral originals only. The separate 2 m PAN is not part of this download workflow.':'Only the reviewed analytical layer can be selected.')}</p>}
              {dataset.id==='deafrica-fractional-cover'&&['qa','count_valid'].includes(asset.key)&&<p>{t(annualMissingDataNote)}</p>}
              {!asset.eligible&&<p>{dataset.id==='cgls-landcover'&&asset.key!=='permanentwater_cover_fraction'?t('This layer is not connected. Currently only permanent-water cover fraction is supported.'):t(asset.reason)}</p>}
              {dataset.id==='worldcereal'&&asset.key==='confidence'&&dataset.warnings.filter(warning=>warning.startsWith('The reviewed annual confidence original')).map(warning=><p key={warning}>{t(warning)}</p>)}
              <dl className="stac-details">{(asset.originalHref||typeof asset.metadata.href==='string'&&asset.metadata.href!==asset.href)&&<><dt>{t(asset.originalHref?'Original publisher URL':'Original asset reference')}</dt><dd>{asset.originalHref||asset.metadata.href}</dd></>}<dt>{t(asset.originalHref||asset.metadata.href!==asset.href?'Public download URL':'Source URL')}</dt><dd>{asset.href}</dd><dt>{t('Original asset metadata')}</dt><dd><pre>{JSON.stringify(asset.metadata,null,2)}</pre></dd><dt>{t('Original item properties')}</dt><dd><pre>{JSON.stringify(item.properties,null,2)}</pre></dd></dl>
            </InfoDetails>}/>;
        })}
      </Surface>)}
      {searches.map(search => <Surface variant="inset" className="public-dataset-review" key={search.sourceId}>
        <strong>{plan.sources.find(source => source.id === search.sourceId)?.title}</strong>
        <p className="stac-help" role="status">{t(search.page.limitReached ? 'Search limit reached. Narrow the area, dates or collection.' : search.page.complete ? '{count} items loaded · search complete' : '{count} items loaded · more available', {count:number(items.filter(item => item.connectionId === search.connectionId).length)})}</p>
        <p className="stac-help">{t('Catalog completion only describes this metadata search. Item footprints and selected band files do not prove complete, valid-pixel or cloud-free area coverage.')}</p>
        {!items.some(item => item.connectionId === search.connectionId) && <p className="stac-help">{t('No product items match this search.')}</p>}
        {search.page.nextCursor && <Button disabled={Boolean(busy)} onClick={() => loadMore(search)}>{t('Load more items')}</Button>}
      </Surface>)}
      {items.length > 0 && <ProductDelivery inline={inline && autoSearch} open={saveOpen} onOpenChange={setSaveOpen} busy={['save','download'].includes(busy)}><Surface variant="inset" className="public-dataset-review">
        <strong>{t('{count} original assets selected', { count: number(selections.length) })}</strong>
        {inline&&autoSearch&&<p className="stac-help">{t('Estimated size')} · {estimatedBytes?formatBytes(estimatedBytes,locale):t('Size checked during download')}</p>}
        {inline&&autoSearch&&<InfoDetails summary={t('Files and area')} closeLabel={t('Close')}><p>{t(dataset.title)}</p><p>{(target?.bounds||plan?.request.bounds||[]).join(', ')}</p><ul>{selections.map(selection=><li key={keyFor(selection.snapshotId,selection.assetKey)}>{items.find(item=>item.id===selection.snapshotId)?.itemId} · {selection.assetKey}</li>)}</ul><p>{t('The area organizes this project. Downloads keep the complete original asset; no clipping or band conversion is applied.')}</p></InfoDetails>}
        <label className="stac-field"><span>{t('Save into project')}</span><Select aria-label={t('Save into project')} value={destination} disabled={Boolean(busy)} onChange={event => { setDestination(event.target.value); setSaved(null); setDelivery(''); }}><option value="new">{t('New project')}</option>{choices.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></label>
        {destination === 'new' && <label className="stac-field"><span>{t('Project name')}</span><Input aria-label={t('Dataset project name')} value={projectName} disabled={Boolean(busy)} maxLength={120} onChange={event => { setProjectName(event.target.value); setSaved(null); setDelivery(''); }}/></label>}
        {inline&&autoSearch?<>{!delivery&&<Button variant="primary" icon={Download} disabled={Boolean(busy)||!selections.length||!selectionReviewed||destination==='new'&&!projectName.trim()} onClick={confirmDownload}>{busy==='download'?<Spinner size={15}/>:null}{t('Confirm and download')}</Button>}{saved&&<DeliveryReceipt project={saved} jobIds={delivery?deliveryIds:[]} submitted={Boolean(delivery)} onOpenProject={onOpenProject}/>}</>:<><div className="stac-actions"><Button variant={saved ? 'secondary' : 'primary'} disabled={Boolean(busy) || !selections.length || !selectionReviewed || destination === 'new' && !projectName.trim() || Boolean(saved)} onClick={save}><FolderOpen size={15}/>{t(saved ? 'Selection saved' : 'Save asset selection')}</Button>
          {saved && <Button variant="primary" disabled={Boolean(busy) || !selectionReviewed || Boolean(delivery)} onClick={download}><Download size={15}/>{t(delivery === 'ready' ? 'Original files ready' : delivery ? 'Downloads queued' : 'Download selected originals')}</Button>}</div>
        {saved && <p className="stac-help" role="status">{t('Saved in {name}.', { name: saved.name })} <a className="public-dataset-open-project" href={`#My%20Data?project=${encodeURIComponent(saved.id)}`} onClick={event=>{if(onOpenProject){event.preventDefault();onOpenProject(saved.id);}onClose?.();}}>{t('Open project')}</a></p>}</>}
        {inline && autoSearch && error && <p className="stac-error" role="alert">{t(error)}</p>}
      </Surface></ProductDelivery>}
      {nativePixel&&<Disclosure className="public-dataset-native-pixel" defaultOpen summary={t('Original pixel values')}><p className="stac-help" title={nativePixel.title}>{nativePixel.spectralEncoding?t('Pixel · column {column}, row {row}',{column:nativePixel.column,row:nativePixel.row}):`${nativePixel.title} · ${nativePixel.column}, ${nativePixel.row}`}</p>{nativePixel.spectralEncoding?<SpectralPixelTable pixel={nativePixel} encoding={nativePixel.spectralEncoding}/>:nativePixel.values.map((value,i)=><p key={i}>{nativePixel.sampleEncoding?.components[i]||`${t('Band')} ${i+1}`} · {value===null?t('Non-finite value'):String(value)}{nativePixel.noData[i]?' · '+t('NoData'):''}</p>)}</Disclosure>}
      {busy && <p className="stac-status" role="status"><Spinner size={15}/>{t(busy === 'resolve' ? 'Finding published files…' : busy === 'connect' ? publishedItems ? 'Reading published item metadata…' : catalogSearch ? 'Searching product catalog…' : 'Checking published files…' : busy === 'search' ? 'Reading more product items…' : busy === 'download' ? 'Queueing originals…' : 'Loading dataset metadata…')}</p>}
      {error && <p className="stac-error" role="alert">{t(error)}</p>}
      {!busy && !datasets.length && <Button onClick={load}>{t('Retry')}</Button>}
    </div>;
  const actions = inline && autoSearch && items.length>0 && <SelectionActions count={selections.length} onClear={clearSelection}
      preview={<Button variant="secondary" icon={Layers} aria-label={t('Load selected files to map')} disabled={Boolean(busy) || (selectedArchives.length?selectedArchives.length>MAX_PRODUCT_PREVIEWS||archiveJobs.some(({job})=>!job):!selectedPreviews.length || previewCount>MAX_PRODUCT_PREVIEWS)} onClick={loadPreviews}>{t('Preview selected')}</Button>}
      download={<Button variant="primary" icon={Download} disabled={!selections.length || Boolean(busy)} onClick={()=>setSaveOpen(true)}>{t('Save and download')}</Button>}>
      <InfoDetails summary={t('Preview details')} closeLabel={t('Close')}>
      {selectedArchives.length>0&&<p className="stac-help">{t(localBandPreview?'CBERS WPM preview uses the downloaded original band. Save and download first, then load its local preview.':dataset?.id==='enmap'?'Download the complete EnMAP ZIP first. The map reads the original 224-band cube; select any wavelength after loading.':dataset?.id==='merit-hydro'?'IHU 原件使用本地原网格预览：请先保存并下载所选文件；已完成的文件会直接复用，不会重复下载。':dataset?.id==='clcd'?(reviewedArchiveDownloadsAvailable?'Loading checks the complete CLCD original source, checksum and grid; it does not start another download.':'Download the complete CLCD original before loading its classification preview.'):dataset?.id==='vantor-open'?'Download the complete visual TIFF first. The native map displays decoded Y; raw inspection labels Y/Cb/Cr and retains the original file checksum.':reviewedArchiveDownloadsAvailable?'Loading checks the saved whole original source and separate ZIP/TIFF checksum receipts; it does not start another download.':'Download the complete original ZIP first. The map reads the separately verified whole TIFF member; both original checksums are retained.')}</p>}

      {(!selectedArchives.length||selectedArchives.length>MAX_PRODUCT_PREVIEWS)&&<p className="stac-help">{t(previewCount>MAX_PRODUCT_PREVIEWS||selectedArchives.length>MAX_PRODUCT_PREVIEWS ? 'The map supports up to {count} product layers. Reduce the selection or remove a preview layer.' : selections.length && selectedPreviews.length!==selections.length ? 'Some selected originals have no online TIFF preview. They remain available for saving and download.' : 'Preview reads visible blocks only; it does not start an original download. Up to {count} product layers.',{count:MAX_PRODUCT_PREVIEWS})}</p>}
      </InfoDetails>
      {selectedArchives.length>0&&!reviewedArchiveDownloadsAvailable&&<p className="stac-help">{t('Download originals before previewing.')}</p>}
      {saved&&delivery&&<DeliveryReceipt compact project={saved} jobIds={deliveryIds} submitted onOpenProject={onOpenProject}/>}
    </SelectionActions>;
  return inline ? <ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} title={dataset?.title || 'Data product'} ariaLabel="Data product exploration" actions={actions}
    map={nativeRasters.length?<NativeRasterExploreMap entries={nativeRasters} areaBounds={validQueryBounds(bounds)?bounds:areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} onPixel={sampleNative} onDisplayBand={changeNativeBand} bandBusy={Boolean(busy)} onPatch={(key,patch)=>setNativeRasters(old=>old.map(e=>e.key===key?{...e,...patch}:e))} onRemove={key=>setNativeRasters(old=>old.filter(e=>e.key!==key))}/>:<PublicDatasetExploreMap items={items} areaBounds={validQueryBounds(bounds) ? bounds : areaBounds} areaGeometry={areaGeometry} selectedIds={[...new Set(selections.map(selection=>selection.snapshotId))]} focusedId={focusedId} onFocusItem={focusItem} onRequestArea={onRequestArea} areaDisabled={['save','download'].includes(busy)} onShowList={onShowList} listHidden={listHidden} rasterLayers={rasterLayers} onRasterChange={changeRaster} onRemoveRaster={id=>setRasterLayers(current=>current.filter(entry=>entry.id!==id))} onClearRasters={clearRasters} onRaiseRaster={id=>setRasterLayers(current=>[...current.filter(entry=>entry.id!==id),...current.filter(entry=>entry.id===id)])}/>}
    ><div className={`public-dataset-sidebar-content${autoSearch ? ' public-dataset-sidebar-live' : ''}`} aria-label={t('Product search and results')}>{body}</div></ExploreSourceFrame>
    : <Modal title={t('Public dataset connectors')} closeLabel={t('Close')} onClose={onClose} closeDisabled={['save', 'download'].includes(busy)} wide className="public-dataset-dialog">{body}</Modal>;
}

function ProductDelivery({inline,open,onOpenChange,busy,children}) {
  const {t}=useI18n();
  const returnFocus=useRef(null),wasOpen=useRef(false);
  if(open&&!wasOpen.current)returnFocus.current=document.activeElement;
  wasOpen.current=open;
  if (!inline) return <Disclosure summary={t('Save and download')} open onOpenChange={onOpenChange}>{children}</Disclosure>;
  return <Dialog open={open} onOpenChange={next=>{if(!busy)onOpenChange(next);}}><DialogContent aria-describedby={undefined} className="public-dataset-delivery-dialog" onCloseAutoFocus={event=>{event.preventDefault();if(returnFocus.current?.isConnected)returnFocus.current.focus();}} onEscapeKeyDown={event=>{if(busy)event.preventDefault();}} onPointerDownOutside={event=>{if(busy)event.preventDefault();}}>
    <header className="bui-dialog-header"><DialogTitle className="bui-dialog-title">{t('Confirm download')}</DialogTitle><Button variant="quiet" size="icon" disabled={busy} aria-label={t('Close')} onClick={()=>onOpenChange(false)}><X size={18}/></Button></header>
    <div className="bui-dialog-body">{children}</div>
  </DialogContent></Dialog>;
}
