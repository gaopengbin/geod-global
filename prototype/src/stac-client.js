import { desktopAvailable } from './runtime-client.js';
import { validQueryBounds } from './features-client.js';
import { isNextPlanetaryProduct, validNextPlanetarySnapshot } from './next-public-products.js';
import {reviewedLargeRasterInspection,reviewedClcdSelection} from './original-backlog-client.js';
import {reviewedEnmapInspection,reviewedEnmapPixel} from './enmap-client.js';

const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i;
const hash = /^[a-f0-9]{64}$/;
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const text = (value, max = 16384) => typeof value === 'string' && value.length <= max;
const time = value => typeof value === 'string' && Number.isFinite(Date.parse(value));
const optionalTime = value => value === null || time(value);
const finiteArray = (value, length) => Array.isArray(value) && value.length === length && value.every(Number.isFinite);
const integer = (value, min = 0) => Number.isSafeInteger(value) && value >= min;
const rawSample = value => value === null || Number.isFinite(value) || typeof value === 'string' && (/^-?(?:0|[1-9]\d*)$/.test(value) && value.length <= 21 || ['NaN', 'Infinity', '-Infinity'].includes(value));
const fail = message => { throw new Error(message); };

export function validStacUrl(value) {
  try {
    const url = new URL(value);
    return text(value, 8192) && url.protocol === 'https:' && !url.username && !url.password && !url.hash
      && !/(^|\.)(localhost|local|internal)$/i.test(url.hostname)
      && !/^[\d.]+$/.test(url.hostname) && !url.hostname.includes(':')
      && ![...url.searchParams.keys()].some(key => /^(?:sig|signature|token|access_token|api_key|apikey|key|password|authorization|credential|expires|se|sp|sv|sr|x-amz-.*|x-goog-.*)$/i.test(key));
  } catch { return false; }
}

export function validateStacConnection(value) {
  if (!object(value) || !uuid.test(value.id) || !text(value.name, 240) || !value.name.trim()
    || !validStacUrl(value.url) || !['api', 'item', 'raster', 'catalog'].includes(value.kind) || !time(value.connectedAt)
    || !object(value.capabilities) || typeof value.capabilities.searchGet !== 'boolean' || typeof value.capabilities.searchPost !== 'boolean'
    || value.searchMethod !== undefined && !['GET', 'POST'].includes(value.searchMethod)
    || !Array.isArray(value.collections) || value.collections.length > 4096 || !Array.isArray(value.snapshotIds)
    || value.snapshotIds.some(id => !hash.test(id)) || new Set(value.snapshotIds).size !== value.snapshotIds.length
    || value.collections.some(c => !object(c) || !text(c.id, 512) || !c.id || !text(c.title) || !text(c.description, 32768) || !text(c.license))
    || new Set(value.collections.map(c => c.id)).size !== value.collections.length) fail('The raster source returned invalid connection metadata.');
  const nodes = value.catalogNodes || [];
  if (!Array.isArray(nodes) || nodes.length > 128 || value.kind !== 'catalog' && nodes.length
    || value.kind === 'catalog' && (!nodes.length || value.collections.length || value.snapshotIds.length
      || value.capabilities.searchGet || value.capabilities.searchPost || value.searchUrl !== null)
    || nodes.some((node, index) => !object(node) || !hash.test(node.key) || !text(node.id, 512) || !node.id
      || !text(node.title, 512) || !text(node.description, 32768) || !['Catalog', 'Collection'].includes(node.kind)
      || !validStacUrl(node.url) || !(node.license === null || text(node.license, 2048))
      || (index === 0 ? node.parentKey !== null || node.url !== value.url : !nodes.slice(0, index).some(parent => parent.key === node.parentKey)))
    || new Set(nodes.map(node => node.key)).size !== nodes.length || new Set(nodes.map(node => node.url)).size !== nodes.length) fail('The raster source returned invalid static directory metadata.');
  return value;
}

function validPublicAssetTransport(value, asset) {
  const proof = value.provenance.reviewedDataset, dataset = proof?.dataset?.id, year = proof?.request?.year;
  if (!text(asset.originalHref, 8192) || asset.metadata.href !== asset.originalHref || !validStacUrl(asset.href)) return false;
  if (isNextPlanetaryProduct(dataset)) return validNextPlanetarySnapshot(value,asset);
  const bucket = 's3://deafrica-input-datasets/', endpoint = 'https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/';
  if (dataset === 'worldcereal') {
    const prefix = `${bucket}esa_worldcereal/`;
    return asset.originalHref.startsWith(prefix) && !/[?#\\]/.test(asset.originalHref)
      && !asset.originalHref.slice(prefix.length).split('/').some(part => !part || part === '.' || part === '..' || /%2e|%2f|%5c/i.test(part))
      && asset.transportAdapter === 'deafrica-worldcereal-public-s3'
      && asset.href === `${endpoint}esa_worldcereal/${asset.originalHref.slice(prefix.length)}`;
  }
  if (dataset === 'cgls-landcover') {
    const stem = `cgls_landcover_${year}`, key = 'permanentwater_cover_fraction';
    return Number.isInteger(year) && year >= 2015 && year <= 2019 && proof.request.layer === 'permanentwater' && asset.key === key
      && asset.transportAdapter === 'deafrica-cgls-reviewed-distribution-path'
      && asset.originalHref === `${bucket}gls/${year}/${stem}_${key}.tif`
      && asset.href === `${endpoint}cgls_landcover/${year}/${stem}_${key}.tif`
      && value.provenance.documentUrl === `${endpoint}cgls_landcover/${year}/${stem}.stac-item.json`;
  }
  if (!['wsf-settlements', 'gmw-mangroves'].includes(dataset) || !asset.originalHref.startsWith(bucket)
    || asset.transportAdapter !== 'deafrica-settlements-mangrove-public-s3') return false;
  const path = asset.originalHref.slice(bucket.length);
  if (dataset === 'wsf-settlements') {
    const version = year === 2015 ? 'v2' : 'v1', match = path.match(/^wsf_(2015|2019)\/(-?\d+_-?\d+)\/WSF(2015|2019)_(v[12])_(-?\d+_-?\d+)\.tif$/);
    if (!match || ![2015, 2019].includes(year) || proof.request.layer !== 'settlement' || asset.key !== `wsf${year}`
      || Number(match[1]) !== year || Number(match[3]) !== year || match[2] !== match[5] || match[4] !== version) return false;
  } else {
    const match = path.match(/^gmw\/(201[789]|2020)\/([NS]\d{2}[EW]\d{3})\/GMW_([NS]\d{2}[EW]\d{3})_(201[789]|2020)_v3\.tif$/);
    if (!match || ![2017, 2018, 2019, 2020].includes(year) || proof.request.layer !== 'mangrove' || asset.key !== 'mangrove'
      || Number(match[1]) !== year || Number(match[4]) !== year || match[2] !== match[3]) return false;
  }
  return asset.href === `${endpoint}${path}` && value.provenance.documentUrl === `${endpoint}${path.slice(0, -4)}.stac-item.json`;
}

export function validateStacSnapshot(value) {
  if (!object(value) || !hash.test(value.id) || !uuid.test(value.connectionId) || !text(value.itemId, 1024) || !value.itemId
    || !text(value.title) || !(value.collectionId === null || text(value.collectionId, 512))
    || ![value.datetime, value.startDatetime, value.endDatetime].every(optionalTime) || !time(value.retrievedAt)
    || !hash.test(value.documentSha256) || !object(value.properties) || !(value.geometry === null || object(value.geometry))
    || !['instant', 'interval', 'missing'].includes(value.temporalStatus) || !Array.isArray(value.warnings) || value.warnings.some(message => !text(message))
    || !object(value.provenance) || !validStacUrl(value.provenance.documentUrl) || !Array.isArray(value.provenance.metadataDocuments)
    || value.provenance.documentRequest !== undefined && (!object(value.provenance.documentRequest)
      || value.provenance.documentRequest.url !== value.provenance.documentUrl || !['GET', 'POST'].includes(value.provenance.documentRequest.method)
      || value.provenance.documentRequest.body !== undefined && (!object(value.provenance.documentRequest.body) || value.provenance.documentRequest.method !== 'POST')
      || new TextEncoder().encode(JSON.stringify(value.provenance.documentRequest)).length > 16384)
    || value.provenance.metadataDocuments.some(receipt => !object(receipt) || !validStacUrl(receipt.url) || !hash.test(receipt.sha256))
    || !(value.provenance.collection === null || object(value.provenance.collection))
    || value.provenance.searchMode !== undefined && value.provenance.searchMode !== 'catalog'
    || !(value.provenance.search === null ? value.provenance.searchMode === undefined : object(value.provenance.search)
      && value.provenance.search.connectionId === value.connectionId
      && (value.provenance.searchMode === 'catalog' ? hash.test(value.provenance.search.collectionId) : value.provenance.search.collectionId === value.collectionId)
      && validQueryBounds(value.provenance.search.bounds))
    || !(value.bbox === null || validQueryBounds(value.bbox))
    || !Array.isArray(value.assets) || value.assets.length > 512 || value.assets.some(asset =>
      !object(asset) || !text(asset.key, 1024) || !asset.key || !text(asset.title) || !text(asset.href, 8192)
      || !(asset.mediaType === null || text(asset.mediaType, 512)) || !Array.isArray(asset.roles) || asset.roles.some(role => !text(role, 256))
      || !object(asset.metadata) || typeof asset.eligible !== 'boolean' || !(asset.reason === null || text(asset.reason)) || asset.eligible && (!validStacUrl(asset.href)
        || ['file:size','bdc:size'].some(key=>asset.metadata[key]!=null && (!integer(asset.metadata[key],1) || asset.metadata[key]>512*1024*1024 && !reviewedClcdSelection(asset)))))
    || new Set(value.assets.map(asset => asset.key)).size !== value.assets.length) fail('The raster source returned invalid item metadata.');
  if (value.temporalStatus !== (value.datetime ? 'instant' : value.startDatetime && value.endDatetime ? 'interval' : 'missing')) fail('The raster source observation time is inconsistent.');
  const transports = value.provenance.assetTransports || [];
  const mapped = value.assets.filter(asset => asset.originalHref !== undefined || asset.transportAdapter !== undefined);
  if (!Array.isArray(transports) || transports.length > value.assets.length || new Set(transports.map(receipt => receipt.assetKey)).size !== transports.length
    || transports.length !== mapped.length || mapped.length && (!object(value.provenance.reviewedDataset) || value.provenance.reviewedDataset.schemaVersion !== 2
      || !(['worldcereal', 'cgls-landcover', 'wsf-settlements', 'gmw-mangroves'].includes(value.provenance.reviewedDataset.dataset?.id) || isNextPlanetaryProduct(value.provenance.reviewedDataset.dataset?.id)) || value.provenance.reviewedDataset.dataset?.discovery !== 'stac-items'
      || value.provenance.search !== null)
    || transports.some(receipt => !object(receipt) || !mapped.some(asset => receipt.assetKey === asset.key && receipt.originalHref === asset.originalHref && receipt.href === asset.href && receipt.adapter === asset.transportAdapter))
    || mapped.some(asset => !validPublicAssetTransport(value, asset))) fail('The public asset transport does not match its original publisher source.');
  return value;
}

export function validateStacSearchPage(value, request, bindQuery = false) {
  if (!object(value) || !Array.isArray(value.items) || value.items.length > 100 || !(value.nextCursor === null || text(value.nextCursor, 16384) && value.nextCursor.length)
    || typeof value.complete !== 'boolean' || typeof value.limitReached !== 'boolean' || value.complete && (value.nextCursor !== null || value.limitReached)
    || value.scannedItems !== undefined && (!integer(value.scannedItems) || value.scannedItems > 1000 || value.scannedItems < value.items.length)
    || new Set(value.items.map(item => item.id)).size !== value.items.length) fail('Invalid raster search page.');
  const period = request.datetime?.split('/');
  const periodStart = period?.[0] === '..' ? -Infinity : Date.parse(period?.[0]);
  const periodEnd = period?.at(-1) === '..' ? Infinity : Date.parse(period?.at(-1));
  if (bindQuery && (!integer(request.limit, 1) || request.limit > 100 || value.items.length > request.limit || !validQueryBounds(request.bounds)
    || request.datetime != null && (period.length > 2 || Number.isNaN(periodStart) || Number.isNaN(periodEnd) || periodStart > periodEnd))) fail('Invalid raster search page.');
  value.items.forEach(item => {
    validateStacSnapshot(item);
    if (item.connectionId !== request.connectionId
      || (item.provenance.searchMode === 'catalog' ? item.provenance.search.collectionId !== request.collectionId || value.scannedItems === undefined : item.collectionId !== request.collectionId)) fail('Raster search returned an item from a different source or collection.');
    const query = item.provenance.search;
    if (bindQuery && (!query || query.connectionId !== request.connectionId || query.collectionId !== request.collectionId
      || query.bounds.some((coordinate, index) => coordinate !== request.bounds[index])
      || query.datetime !== request.datetime || query.limit !== request.limit || query.cursor != null)) fail('Raster search returned an item from a different query.');
    if (bindQuery) {
      // Saved provenance retains the canonical query; the page cursor belongs to documentRequest.
      const bounds = item.bbox;
      const start = Date.parse(item.datetime ?? item.startDatetime), end = Date.parse(item.datetime ?? item.endDatetime);
      if (!bounds || bounds[2] < request.bounds[0] || bounds[0] > request.bounds[2] || bounds[3] < request.bounds[1] || bounds[1] > request.bounds[3]
        || request.datetime != null && (Number.isNaN(start) || Number.isNaN(end) || start > end || end < periodStart || start > periodEnd)) fail('Raster search returned an item outside the requested area or time.');
    }
  });
  return value;
}

export function validateStacInspection(value, id) {
  const reviewedLarge=reviewedLargeRasterInspection(value);
  const envi=reviewedEnmapInspection(value);
  if (!object(value) || value.jobId !== id || !hash.test(value.sha256) || !integer(value.width, 1) || !integer(value.height, 1)
    || !reviewedLarge && (value.width > 131072 || value.height > 131072 || value.width * value.height > 8000000000)
    || value.storageLayout !== undefined && !['tile', 'strip'].includes(value.storageLayout) && !envi
    || (value.width > 65536 || value.height > 65536) && value.storageLayout !== 'tile'
    || !Array.isArray(value.bands) || !value.bands.length || value.bands.length > 16 && !envi
    || value.bands.some((band, index) => !object(band) || band.index !== index + 1 || !text(band.dataType, 80) || !band.dataType
      || !rawSample(band.nodata))
    || !(value.crs === null || text(value.crs)) || !(value.transform === null || finiteArray(value.transform, 6))
    || ['horizontalCrs', 'verticalCrs'].some(key => value[key] !== undefined && value[key] !== null && !text(value[key], 2048))
    || value.sourceModelPixelScale !== undefined && value.sourceModelPixelScale !== null && !finiteArray(value.sourceModelPixelScale, 3)
    || value.sourceModelTiepoint !== undefined && value.sourceModelTiepoint !== null && !finiteArray(value.sourceModelTiepoint, 6)
    || !(value.bounds === null || finiteArray(value.bounds, 4)) || !(value.pixelInterpretation === null || text(value.pixelInterpretation, 80))
    || !Array.isArray(value.limitations) || value.limitations.some(item => !text(item)) || value.displayBand !== 1 && !envi
    || !(value.displayRange === null || finiteArray(value.displayRange, 2) && value.displayRange[0] <= value.displayRange[1])) fail('The local raster inspection is incomplete or invalid.');
  if(value.categoricalEncoding!=null && !reviewedLarge) fail('The categorical preview differs from its exact reviewed original.');
  if(value.spectralEncoding!=null && !envi) fail('EnMAP original cube, header or source receipt changed.');
  if (value.previewDataUrl === null) {
    if (value.previewWidth !== null || value.previewHeight !== null) fail('The local raster preview is invalid.');
  } else if (!text(value.previewDataUrl, 4000000) || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(value.previewDataUrl)
    || !integer(value.previewWidth, 1) || !integer(value.previewHeight, 1) || value.previewWidth > 1024 || value.previewHeight > 1024) fail('The local raster preview is invalid.');
  if(value.sampleEncoding!=null) {
    const expected={kind:'jpeg-ycbcr-components',components:['Y','Cb','Cr'],codec:'JPEG',chromaSubsampling:[2,2],chromaUpsampling:'triangular fancy interpolation to the original pixel grid',previewComponent:'Y',scientificCalibration:false};
    if(!sameJson(value.sampleEncoding,expected)||value.bands.length!==3||value.bands.some(b=>b.dataType!=='UInt8'))fail('The visual JPEG component encoding is invalid.');
  }
  return value;
}

export function validateStacPixel(value, request) {
  if (!object(value) || value.jobId !== request.id || value.column !== request.column || value.row !== request.row || !hash.test(value.sha256)
    || !Array.isArray(value.values) || !value.values.length || value.values.length > 16 && !reviewedEnmapPixel(value) || value.values.some(v => !rawSample(v))
    || !Array.isArray(value.noData) || value.noData.length !== value.values.length || value.noData.some(v => typeof v !== 'boolean')) fail('The raster pixel response does not match the requested file and position.');
  return value;
}

const pinKey = value => JSON.stringify([value.snapshotId, value.assetKey]);
function sameJson(a, b) {
  if (a === b) return true;
  if (Array.isArray(a) || Array.isArray(b)) return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((value, index) => sameJson(value, b[index]));
  if (!object(a) || !object(b)) return false;
  const keys = Object.keys(a).sort(), other = Object.keys(b).sort();
  return keys.length === other.length && keys.every((key, index) => key === other[index] && sameJson(a[key], b[key]));
}
function sameOriginal(a, b, assetKey) {
  return a.connectionId === b.connectionId && new URL(a.provenance.documentUrl).origin === new URL(b.provenance.documentUrl).origin
    && a.itemId === b.itemId && a.collectionId === b.collectionId
    && sameJson(a.assets.find(asset => asset.key === assetKey), b.assets.find(asset => asset.key === assetKey))
    && ['properties', 'geometry', 'bbox'].every(key => sameJson(a[key], b[key])) && sameJson(a.provenance.collection, b.provenance.collection);
}
async function savedSelections(selections, entries, signal, message) {
  const snapshots = new Map(), resolved = [];
  const snapshot = id => { if (!snapshots.has(id)) snapshots.set(id, stacRequest('snapshot', { id }, signal)); return snapshots.get(id); };
  for (const selection of selections) {
    const exact = entries.find(item => pinKey(item) === pinKey(selection));
    if (exact) { resolved.push({ snapshotId: exact.snapshotId, assetKey: exact.assetKey }); continue; }
    if (!entries.some(item => item.assetKey === selection.assetKey)) fail(message);
    // A refreshed search has a new receipt. Reuse a saved pin only when its complete original declarations agree.
    const original = await snapshot(selection.snapshotId), asset = original.assets.find(item => item.key === selection.assetKey);
    if (!asset?.eligible) fail(message);
    const matches = [];
    for (const item of entries.filter(item => item.assetKey === selection.assetKey && item.itemId === original.itemId && item.href === asset.href
      && (item.collectionId === undefined || item.collectionId === original.collectionId))) {
      const saved = await snapshot(item.snapshotId), savedAsset = saved.assets.find(value => value.key === item.assetKey);
      if (savedAsset?.eligible && item.itemId === saved.itemId && item.href === savedAsset.href && item.mediaType === savedAsset.mediaType
        && sameOriginal(original, saved, selection.assetKey)) matches.push(item);
    }
    if (matches.length !== 1) fail(message);
    resolved.push({ snapshotId: matches[0].snapshotId, assetKey: matches[0].assetKey });
  }
  return [...new Map(resolved.map(item => [pinKey(item), item])).values()];
}
async function validateSavedProject(value, request, signal) {
  if (!object(value) || !uuid.test(value.id) || !text(value.name, 120) || !validQueryBounds(value.bounds)
    || !Array.isArray(value.scenes) || !Array.isArray(value.stacItems) || value.scenes.length + value.stacItems.length > 32
    || value.stacItems.some(item => !object(item) || !hash.test(item.snapshotId) || !text(item.assetKey, 512) || !item.assetKey
      || !text(item.title) || !text(item.itemId, 512) || !text(item.serviceName, 80) || !validStacUrl(item.href) || !text(item.mediaType, 256))
    || new Set(value.stacItems.map(pinKey)).size !== value.stacItems.length
    || request.projectId && value.id !== request.projectId) fail('The saved project does not contain the selected original assets.');
  const canonicalSelections = await savedSelections(request.selections, value.stacItems, signal, 'The saved project does not contain the selected original assets.');
  // This client-only field keeps current search checkboxes separate from the project's immutable download pins.
  return { ...value, canonicalSelections };
}

export async function stacRequest(operation, payload = {}, signal) {
  const commands = { list: 'list_stac_connections', connect: 'connect_stac', forget: 'forget_stac_connection', search: 'search_stac', snapshot: 'stac_snapshot', inspect: 'inspect_stac_asset', pixel: 'sample_stac_asset', project: 'save_stac_project', downloads: 'download_stac_project' };
  if (!commands[operation]) fail('Unknown raster source operation.');
  if (['forget', 'inspect', 'pixel'].includes(operation) && !uuid.test(payload.id) || operation === 'snapshot' && !hash.test(payload.id)) fail('Invalid saved raster identifier.');
  if (operation === 'connect' && (!validStacUrl(payload.url) || !payload.name?.trim() || !['api', 'item', 'raster', 'catalog'].includes(payload.kind))) fail('Enter a source name and a public HTTPS URL.');
  if (operation === 'search' && (!uuid.test(payload.connectionId) || !text(payload.collectionId, 512) || !payload.collectionId || !validQueryBounds(payload.bounds))) fail('Choose a collection and a valid search region.');
  if (operation === 'pixel' && (!integer(payload.column) || !integer(payload.row))) fail('Enter a nonnegative pixel column and row.');
  if (operation === 'inspect' && payload.displayBand!=null && (!integer(payload.displayBand,1)||payload.displayBand>224)) fail('Choose a spectral band from 1 to 224.');
  if (operation === 'project' && (!validQueryBounds(payload.bounds) || !Array.isArray(payload.selections) || !payload.selections.length || payload.selections.length > 32)) fail('Select 1 to 32 raster assets and a valid project region.');
  if (['project', 'downloads'].includes(operation) && payload.selections?.some(item => !hash.test(item.snapshotId) || !text(item.assetKey, 1024) || !item.assetKey)) fail('Invalid raster selection.');
  const check = () => { if (signal?.aborted) throw new DOMException('Raster request cancelled', 'AbortError'); }; check();
  let value;
  if (desktopAvailable()) {
    try { value = await window.__TAURI__.core.invoke(commands[operation], ['connect', 'search', 'project', 'downloads'].includes(operation) ? { request: payload } : payload); }
    catch (error) { throw error instanceof Error ? error : new Error(String(error)); }
  } else {
    const paths = { list: '/stac/connections', connect: '/stac/connections', forget: `/stac/connections/${payload.id}/forget`, search: '/stac/search', snapshot: `/stac/snapshots/${payload.id}`, inspect: `/stac/jobs/${payload.id}/inspect`, pixel: `/stac/jobs/${payload.id}/pixel?${new URLSearchParams({ column: payload.column, row: payload.row })}`, project: '/stac/project', downloads: '/stac/downloads' };
    if(operation==='inspect'&&payload.displayBand!=null) paths.inspect+=`?displayBand=${payload.displayBand}`;
    const mutation = ['connect', 'forget', 'search', 'project', 'downloads'].includes(operation);
    const response = await fetch('http://127.0.0.1:4318' + paths[operation], { method: mutation ? 'POST' : 'GET', signal,
      headers: mutation ? { 'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global' } : undefined, body: mutation ? JSON.stringify(payload) : undefined });
    value = await response.json();
    if (!response.ok) fail(value.error || `Raster source returned HTTP ${response.status}`);
  }
  check();
  if (operation === 'list') { if (!Array.isArray(value) || value.length > 128) fail('Invalid saved raster sources.'); value.forEach(validateStacConnection); }
  if (operation === 'connect') validateStacConnection(value);
  if (operation === 'snapshot') { validateStacSnapshot(value); if (value.id !== payload.id) fail('The saved item does not match the requested snapshot.'); }
  if (operation === 'search') validateStacSearchPage(value, payload);
  if (operation === 'inspect') validateStacInspection(value, payload.id);
  if (operation === 'inspect' && payload.displayBand!=null && value.displayBand!==payload.displayBand) fail('The preview does not match the selected spectral band.');
  if (operation === 'pixel') validateStacPixel(value, payload);
  if (operation === 'project') value = await validateSavedProject(value, payload, signal);
  if (operation === 'downloads' && (!object(value) || value.projectId !== payload.projectId || value.assetKey !== 'stac_asset' || !Array.isArray(value.jobs)
    || value.jobs.length > 32 || value.jobs.some(job => !object(job) || !uuid.test(job.id) || job.kind !== 'download' || job.assetKey !== 'stac_asset' || !hash.test(job.stacSource?.snapshotId) || !text(job.stacSource?.assetKey, 512))
    || new Set(value.jobs.map(job => job.id)).size !== value.jobs.length
    || new Set(value.jobs.map(job => pinKey(job.stacSource))).size !== value.jobs.length)) fail('The download queue does not match the saved raster selection.');
  if (operation === 'downloads' && payload.selections) {
    const entries = value.jobs.map(job => ({ ...job.stacSource, itemId: job.itemId, href: job.href, mediaType: job.mediaType }));
    const canonical = await savedSelections(payload.selections, entries, signal, 'The download queue does not match the saved raster selection.');
    if (canonical.length !== payload.selections.length || canonical.length !== entries.length) fail('The download queue does not match the saved raster selection.');
  }
  check();
  return value;
}
