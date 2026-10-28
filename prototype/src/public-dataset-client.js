import {usageRequest} from './usage-events.js';
import {diagnosticError} from './usage-diagnostics.js';
import { desktopAvailable } from './runtime-client.js';
import { validQueryBounds } from './features-client.js';
import { validStacUrl, validateStacConnection, validateStacSearchPage, validateStacSnapshot } from './stac-client.js';
import { validateWcsConnection } from './wcs-client.js';
import { isNextPlanetaryProduct, NEXT_PC_PRODUCTS, nextPlanetaryRange, NEXT_DIRECT_PRODUCTS, nextDirectRange, nextOverviewPolicy } from './next-public-products.js';
import {reviewedArchiveSelection} from './original-backlog-client.js';

const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const text = (value, max = 16384) => typeof value === 'string' && value.length <= max;
const identifier = value => text(value, 80) && /^[a-z0-9][a-z0-9-]*$/.test(value);
const layerIdentifier = value => text(value, 80) && (/^[a-z0-9][a-z0-9_-]*$/.test(value) || ['Burn_Date','QA'].includes(value));
const strings = value => Array.isArray(value) && value.length <= 256 && value.every(item => text(item));
const fail = message => { throw new Error(message); };
const optional = (value, check) => value === undefined || value === null || check(value);
const monthNumber = value => Number.isInteger(value) && value >= 1 && value <= 12;
const sameBounds = (a, b) => a.every((coordinate, index) => coordinate === b[index]);
const intersects = (a, b) => a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1];
const sameJson = (a, b) => a === b || Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((v, i) => sameJson(v, b[i]))
  || object(a) && object(b) && Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key => sameJson(a[key], b[key]));
const options = (value, check = () => true) => Array.isArray(value) && value.length <= 256
  && value.every(option => object(option) && identifier(option.id) && text(option.title, 512) && option.title.trim() && check(option))
  && new Set(value.map(option => option.id)).size === value.length;
const validInterval = source => typeof source.startDatetime === 'string' && typeof source.endDatetime === 'string'
  && Number.isFinite(Date.parse(source.startDatetime)) && Number.isFinite(Date.parse(source.endDatetime)) && Date.parse(source.startDatetime) <= Date.parse(source.endDatetime);
const annualTileProducts = new Set(['deafrica-fractional-cover', 'deafrica-wofs-annual']);
const jaxaProducts = new Set(['jaxa-amsr2-soil-moisture', 'jaxa-sgli-lst', 'jaxa-modis-ndvi', 'jaxa-palsar-fnf', 'jaxa-sgli-chla', 'jaxa-gsmap-rainfall', 'jaxa-hrlulc-japan', 'jaxa-modis-swr', 'jaxa-sgli-sst',
  'jaxa-sgli-et','jaxa-sgli-et-index','jaxa-sgli-aerosol','jaxa-modis-aerosol','jaxa-amsr2-wind','jaxa-amsr2-sst','jaxa-gsmap-spi']);
const batchJaxaLayers = {
  'jaxa-sgli-et':['ET','et'], 'jaxa-sgli-et-index':['ETidx','et-index'],
  'jaxa-sgli-aerosol':['AROT','aod'], 'jaxa-modis-aerosol':['T500','aod'],
  'jaxa-amsr2-wind':['SSW','ssw'], 'jaxa-amsr2-sst':['SST','sst'], 'jaxa-gsmap-spi':['SPI','spi'],
};
// These are display ranges for separately reviewed products. Original DN,
// embedded TIFF NoData and publisher calibration remain unchanged.
const batchJaxaDisplay = {
  'jaxa-sgli-et':['uint16',0.0004615384677890688,-5,65535,'mm/day',2026,4],
  'jaxa-sgli-et-index':['uint16',0.00009999999747378752,0,65535,'N/A',2026,4],
  'jaxa-sgli-aerosol':['uint16',0.00009999999747378752,0,65535,'NA',2026,9,[0,20000]],
  'jaxa-modis-aerosol':['uint16',0.001,0,65535,null,2026,9,[0,1500]],
  'jaxa-amsr2-wind':['int16',0.01,0,-32768,'m/s',2026,9],
  'jaxa-amsr2-sst':['int16',0.01,0,-32768,'degC',2026,10,[-200,4000]],
  'jaxa-gsmap-spi':['float32',1,0,-999,'-',2026,9,[-4,4]],
};
const pointTimeProducts = new Set(['maxar-open-legacy','pc-hrea','vantor-open']);
const reviewedArchiveProducts = new Set(['fabdem','glc-fcs10','glc-fcs30d','sinolc-1','enmap','gebco-tid']);
const reviewedItemProducts = new Set(['worldcereal', 'cgls-landcover', 'wsf-settlements', 'gmw-mangroves', ...annualTileProducts, ...jaxaProducts, ...pointTimeProducts, ...Object.keys(NEXT_PC_PRODUCTS)]);
function validCurrentElevationQuery(href, request) {
  if (!validStacUrl(href)) return false;
  const url=new URL(href),query=url.searchParams;
  return url.origin === 'https://tnmaccess.nationalmap.gov' && url.pathname === '/api/v1/products'
    && [...query.keys()].length === 5 && new Set(query.keys()).size === 5
    && query.get('datasets') === (request.datasetId === 'usgs-3dep-1m'
      ? 'Digital Elevation Model (DEM) 1 meter' : 'National Elevation Dataset (NED) 1 arc-second Current')
    && query.get('bbox')?.split(',').map(Number).every((coordinate,index,values)=>values.length===4 && coordinate===request.bounds[index])
    && query.get('max') === '100' && query.get('offset') === '0' && query.get('outputFormat') === 'JSON';
}
function validCatalogRasterSource(source, request) {
  if (!['usgs-3dep','usgs-3dep-1m'].includes(request.datasetId) || ['year','month','country','season','region'].some(key=>request[key]!=null)
    || request.layer !== 'elevation' || source.stac.kind !== 'raster' || source.search != null || source.startDatetime != null || source.endDatetime != null) return false;
  if (source.catalogRaster == null) return source.id === `${request.datasetId}-catalog` && source.bounds == null && validCurrentElevationQuery(source.stac.url,request);
  const evidence=source.catalogRaster,record=evidence.record,bounds=record?.boundingBox;
  return object(evidence) && /^[a-f0-9]{64}$/.test(evidence.catalogSha256) && validCurrentElevationQuery(evidence.catalogUrl,request)
    && object(record) && record.sourceId === source.id && record.title === source.title && record.format === 'GeoTIFF'
    && Number.isSafeInteger(record.sizeInBytes) && record.sizeInBytes > 0 && record.sizeInBytes <= 512*1024*1024
    && validQueryBounds(source.bounds) && intersects(source.bounds,request.bounds) && object(bounds)
    && sameBounds(source.bounds,[bounds.minX,bounds.minY,bounds.maxX,bounds.maxY])
    && source.stac.url === record.downloadURL && record.urls?.TIFF === record.downloadURL
    && new URL(record.downloadURL).origin === 'https://prd-tnm.s3.amazonaws.com'
    && (request.datasetId === 'usgs-3dep-1m'
      ? record.sourceId === '6aac9d8b1ba49b6a17badefa' && record.sizeInBytes === 45328865
        && new URL(record.downloadURL).pathname === '/StagedProducts/Elevation/1m/Projects/NJ_NE6County_B23/TIFF/USGS_1M_18_x58y451_NJ_NE6County_B23.tif'
      : /^\/StagedProducts\/Elevation\/1\/TIFF\/historical\/[ns]\d{2}[ew]\d{3}\/USGS_1_[ns]\d{2}[ew]\d{3}_\d{8}\.tif$/.test(new URL(record.downloadURL).pathname));
}
function validItemSource(source, dataset, request) {
  if (!reviewedItemProducts.has(dataset.id) || source.stac.kind !== 'item' || source.search != null || !validQueryBounds(source.bounds)
    || !intersects(source.bounds, request.bounds) || !validInterval(source)) return false;
  if (dataset.regions?.length ? !dataset.regions.some(region => region.id === source.region) : !optional(source.region, value => text(value, 80) && value.trim())) return false;
  if (request.region != null && source.region !== request.region) return false;
  if ((source.season ?? null) !== (request.season ?? null)) return false;
  if (dataset.id === 'worldcereal' && (!dataset.regions?.length || !dataset.seasons?.length)) return false;
  return true;
}
function validProductSearch(value, request) {
  if (!object(value) || !text(value.collectionId, 512) || !value.collectionId || !text(value.datetime, 512)
    || !validQueryBounds(value.bounds) || !sameBounds(value.bounds, request.bounds) || value.limit !== 20
    || Object.keys(value).some(key => !['collectionId', 'datetime', 'bounds', 'limit'].includes(key))) return false;
  const parts = value.datetime.split('/');
  if (parts.length !== 2 || parts.some(part => !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(part) || !Number.isFinite(Date.parse(part)))) return false;
  const start = Date.parse(parts[0]), end = Date.parse(parts[1]);
  const firstMonth = request.month == null ? 0 : request.month - 1;
  return request.year != null && start === Date.UTC(request.year, firstMonth, 1)
    && end === Date.UTC(request.year, request.month == null ? 12 : request.month, 1) - 1;
}

export function validatePublicDataset(value) {
  if (!object(value) || !identifier(value.id) || !text(value.title, 512) || !value.title.trim()
    || !text(value.category, 80) || !text(value.description) || !text(value.version, 256)
    || !text(value.license, 2048) || !validStacUrl(value.licenseUrl) || !text(value.attribution, 4096)
    || !validStacUrl(value.documentationUrl) || !text(value.nominalResolution, 512)
    || !text(value.unit, 512) || !text(value.valueInterpretation) || !strings(value.warnings) || !strings(value.limits)
    || !Array.isArray(value.years) || value.years.length > 256 || value.years.some(year => !Number.isInteger(year) || year < 1900 || year > 2100)
    || new Set(value.years).size !== value.years.length || !optional(value.defaultYear, year => value.years.includes(year))
    || !optional(value.months, months => Array.isArray(months) && months.length <= 12 && months.every(monthNumber) && new Set(months).size === months.length)
    || !optional(value.defaultMonth, month => (value.months || []).includes(month))
    || !optional(value.discovery, discovery => ['stac-api', 'stac-items', 'catalog-raster'].includes(discovery))
    || !optional(value.regions, regions => options(regions, option => validQueryBounds(option.bounds)))
    || !optional(value.seasons, seasons => options(seasons))
    || !optional(value.defaultRegion, id => (value.regions || []).some(region => region.id === id))
    || !optional(value.defaultSeason, id => (value.seasons || []).some(season => season.id === id))
    || typeof value.requiresCountry !== 'boolean' || !Array.isArray(value.layers) || !value.layers.length || value.layers.length > 64
    || value.layers.some(layer => !object(layer) || !layerIdentifier(layer.id) || !text(layer.title, 512) || !layer.title.trim()
      || !text(layer.unit, 512) || !text(layer.valueInterpretation)
      || !optional(layer.seasons, seasons => Array.isArray(seasons) && seasons.length <= 256 && seasons.every(id => (value.seasons || []).some(season => season.id === id)) && new Set(seasons).size === seasons.length)
      || !['scale', 'offset', 'nodata'].every(key => optional(layer[key], Number.isFinite)))
    || new Set(value.layers.map(layer => layer.id)).size !== value.layers.length
    || !optional(value.defaultLayer, id => value.layers.some(layer => layer.id === id))) fail('Invalid public dataset metadata.');
  return value;
}

export function validateDatasetRequest(value) {
  if (!object(value) || !identifier(value.datasetId) || !validQueryBounds(value.bounds)
    || !optional(value.year, year => Number.isInteger(year) && year >= 1900 && year <= 2100)
    || !optional(value.month, monthNumber)
    || !optional(value.country, country => /^[A-Z]{3}$/.test(country)) || !optional(value.layer, layerIdentifier)
    || !optional(value.region, identifier) || !optional(value.season, identifier)
    || Object.keys(value).some(key => !['datasetId', 'bounds', 'year', 'month', 'country', 'layer', 'region', 'season'].includes(key))) fail('Choose a dataset and valid request parameters.');
  return value;
}

export function validatePublicDatasetPlan(value, request) {
  if (!object(value)) fail('Invalid public dataset plan.');
  const dataset = validatePublicDataset(value.dataset);
  validateDatasetRequest(value.request);
  if (dataset.id !== value.request.datasetId || request && (dataset.id !== request.datasetId
    || request.bounds.some((coordinate, index) => coordinate !== value.request.bounds[index])
    || ['year', 'month', 'country', 'layer', 'region', 'season'].some(key => (request[key] ?? null) !== (value.request[key] ?? null)))
    || value.request.year != null && !dataset.years.includes(value.request.year)
    || value.request.month != null && !(dataset.months || []).includes(value.request.month)
    || dataset.months?.length && value.request.month == null
    || dataset.requiresCountry && !value.request.country
    || value.request.layer != null && !dataset.layers.some(layer => layer.id === value.request.layer)
    || value.request.region != null && !(dataset.regions || []).some(region => region.id === value.request.region)
    || value.request.season != null && !(dataset.seasons || []).some(season => season.id === value.request.season)
    || dataset.layers.find(layer => layer.id === value.request.layer)?.seasons?.length && !dataset.layers.find(layer => layer.id === value.request.layer).seasons.includes(value.request.season)
    || !strings(value.warnings) || !Array.isArray(value.sources) || value.sources.length > 32
    || value.sources.some(source => !object(source) || !text(source.id, 512) || !source.id || !text(source.title, 512)
      || !optional(source.bounds, validQueryBounds) || Boolean(source.stac) === Boolean(source.wcs)
      || dataset.discovery != null && !source.stac
      || source.stac && (!object(source.stac) || !text(source.stac.name, 80) || !source.stac.name.trim() || !validStacUrl(source.stac.url)
        || (dataset.discovery === 'stac-api' ? source.stac.kind !== 'api' || source.bounds != null || !validProductSearch(source.search, value.request)
          : dataset.discovery === 'stac-items' ? !validItemSource(source, dataset, value.request)
          : dataset.discovery === 'catalog-raster' ? !validCatalogRasterSource(source, value.request)
          : source.stac.kind !== 'raster' || source.search != null))
      || source.wcs && (!object(source.wcs) || !text(source.wcs.name, 80) || !source.wcs.name.trim() || !validStacUrl(source.wcs.url) || !text(source.coverageId, 512) || !source.coverageId))
    || new Set(value.sources.map(source => source.id)).size !== value.sources.length) fail('The public dataset plan does not match the requested product and area.');
  return value;
}

export function validatePublicDatasetConnection(value, request) {
  if (!object(value)) fail('Invalid public dataset connection.');
  validatePublicDatasetPlan(value.plan, request);
  if (!Array.isArray(value.failures) || value.failures.length > 32 || typeof value.complete !== 'boolean'
    || value.complete !== (value.failures.length === 0)
    || value.failures.some(failure => !object(failure) || !text(failure.sourceId, 512) || !text(failure.error, 16384) || !failure.error.trim()
      || !value.plan.sources.some(source => source.id === failure.sourceId))
    || new Set(value.failures.map(failure => failure.sourceId)).size !== value.failures.length) fail('Dataset source check failures are invalid.');
  for (const [key, protocol, validate] of [['stacConnections', 'stac', validateStacConnection], ['wcsConnections', 'wcs', validateWcsConnection]]) {
    const expected = value.plan.sources.filter(source => source[protocol]).map(source => source[protocol].url);
    // Reviewed TIFF members can share one unchanged original ZIP and its saved
    // connection identity. Native receipts still select one immutable snapshot
    // per source; the member's proof and both hashes are checked when it opens.
    const archiveReceipts = protocol === 'stac' && value.plan.dataset.discovery == null && reviewedArchiveProducts.has(value.plan.dataset.id);
    const sourceFor = connection => value.plan.sources.find(source => source[protocol]?.url === connection.url
      && (!archiveReceipts || source.stac.name === connection.name));
    if (!Array.isArray(value[key]) || value[key].length > (archiveReceipts ? expected.length : new Set(expected).size)
      || !archiveReceipts && (new Set(value[key].map(connection => connection.id)).size !== value[key].length
        || new Set(value[key].map(connection => connection.url)).size !== value[key].length)) fail('Dataset connections do not match the reviewed sources.');
    value[key].forEach(connection => {
      validate(connection);
      const source = sourceFor(connection);
      if (!source || protocol === 'stac' && (connection.kind !== source.stac.kind || ['raster', 'item'].includes(connection.kind) && !connection.snapshotIds.length
        || source.stac.kind === 'item' && connection.snapshotIds.length !== 1)) fail('Dataset connections do not match the reviewed sources.');
    });
    if (archiveReceipts && value[key].some((connection,index) => connection.snapshotIds.length !== 1
      || !Array.isArray(connection.metadataSha256) || connection.metadataSha256.length !== 1 || !/^[a-f0-9]{64}$/.test(connection.metadataSha256[0])
      || value[key].slice(0,index).some(previous => previous.url === connection.url && previous.name === connection.name
        || previous.snapshotIds.includes(connection.snapshotIds[0])
        || (previous.id === connection.id) !== (previous.url === connection.url)))) fail('Dataset connections do not match the reviewed sources.');
    for (const source of value.plan.sources.filter(source => source[protocol])) {
      const found = value[key].some(connection => sourceFor(connection)?.id === source.id);
      const failed = value.failures.some(failure => failure.sourceId === source.id);
      if (found === failed) fail('Dataset connections do not account for each reviewed source exactly once.');
    }
  }
  const searches = value.searches || [];
  if (!Array.isArray(searches) || searches.length > 32 || new Set(searches.map(search => search.sourceId)).size !== searches.length) fail('Dataset catalog searches do not match the reviewed sources.');
  for (const search of searches) {
    const source = value.plan.sources.find(source => source.id === search.sourceId && source.search);
    const connection = value.stacConnections.find(connection => connection.id === search.connectionId && connection.url === source?.stac.url);
    if (!source || !connection || connection.kind !== 'api') fail('Dataset catalog searches do not match the reviewed sources.');
    validateStacSearchPage(search.page, { connectionId: connection.id, ...source.search }, true);
    if (connection.snapshotIds.length !== search.page.items.length || connection.snapshotIds.some(id => !search.page.items.some(item => item.id === id))) fail('Dataset catalog results contain snapshots from another search.');
  }
  for (const source of value.plan.sources.filter(source => source.search)) {
    if (value.failures.some(failure => failure.sourceId === source.id) ? searches.some(search => search.sourceId === source.id) : !searches.some(search => search.sourceId === source.id)) fail('Dataset catalog searches do not account for each reviewed source.');
  }
  return value;
}

export function validatePublicDatasetItem(value, plan, connections) {
  validateStacSnapshot(value);
  if (plan.dataset.discovery == null) {
    const connection = connections.find(connection => connection.id === value.connectionId && connection.snapshotIds.includes(value.id));
    const proof = value.properties.datasetPreset;
    const source = plan.sources.find(source => source.stac?.url === connection?.url
      && (!reviewedArchiveProducts.has(plan.dataset.id) || source.id === proof?.sourceId && source.stac.name === connection?.name));
    if (!source || source.stac.kind !== 'raster' || !object(proof) || proof.schemaVersion !== 2
      || !sameJson(proof.dataset,plan.dataset) || !sameJson(proof.request,plan.request)
      || !sameJson(proof.layer,plan.dataset.layers.find(layer=>layer.id===plan.request.layer))
      || proof.sourceId !== source.id || proof.sourceTitle !== source.title || !sameJson(proof.sourceBounds,source.bounds??null)
      || proof.declarationKind !== 'reviewed-client-preset; publisher file bytes and embedded metadata verified separately' || proof.catalogRaster != null
      || value.provenance.documentUrl !== source.stac.url || !sameJson(value.bbox,source.bounds??null)
      || value.assets.length !== 1 || value.assets[0].key !== 'raster' || value.assets[0].href !== source.stac.url
      || reviewedArchiveProducts.has(plan.dataset.id) && (connection?.snapshotIds.length !== 1
        || connection.metadataSha256?.length !== 1 || connection.metadataSha256[0] !== value.documentSha256)
      || !(reviewedArchiveProducts.has(plan.dataset.id) ? reviewedArchiveSelection(value.assets[0]) : value.assets[0].metadata.kind === 'direct-raster') || !sameJson(value.assets[0].metadata.datasetPreset,proof)) fail('The original raster does not match the reviewed product and area.');
    return value;
  }
  if (plan.dataset.discovery === 'catalog-raster') {
    const connection = connections.find(connection => connection.id === value.connectionId && connection.snapshotIds.includes(value.id));
    const source = plan.sources.find(source => source.stac?.url === connection?.url);
    const proof = value.properties.datasetPreset;
    const catalog = source?.catalogRaster;
    if (!['usgs-3dep','usgs-3dep-1m'].includes(plan.dataset.id) || !source || !object(catalog) || !object(catalog.record)
      || !sameJson(proof?.dataset, plan.dataset) || !sameJson(proof?.request, plan.request) || !sameJson(proof?.catalogRaster, catalog)
      || proof.sourceId !== source.id || proof.sourceTitle !== source.title || !sameJson(proof.sourceBounds, source.bounds)
      || !sameJson(value.bbox, source.bounds) || value.datetime !== null || value.startDatetime !== null || value.endDatetime !== null
      || value.assets.length !== 1 || value.assets[0].href !== catalog.record.downloadURL
      || value.assets[0].metadata['file:size'] !== catalog.record.sizeInBytes
      || !value.provenance.metadataDocuments.some(receipt => receipt.url === catalog.catalogUrl && receipt.sha256 === catalog.catalogSha256)) fail('The original raster does not match its archived publisher catalog record.');
    return value;
  }
  if (plan.dataset.discovery !== 'stac-items') return value;
  const connection = connections.find(connection => connection.id === value.connectionId && connection.snapshotIds.includes(value.id));
  const source = plan.sources.find(source => source.stac?.url === connection?.url);
  const proof = value.provenance.reviewedDataset;
  const dynamicTile = annualTileProducts.has(plan.dataset.id);
  const boundsMatch = source && (dynamicTile ? validQueryBounds(value.bbox) && value.bbox.every((coordinate, index) => Math.abs(coordinate - source.bounds[index]) <= 1e-8) : sameJson(value.bbox, source.bounds));
  const originalStart = value.properties.start_datetime ?? (dynamicTile ? value.properties['dtr:start_datetime'] : pointTimeProducts.has(plan.dataset.id) ? value.properties.datetime : undefined);
  const originalEnd = value.properties.end_datetime ?? (dynamicTile ? value.properties['dtr:end_datetime'] : pointTimeProducts.has(plan.dataset.id) ? value.properties.datetime : undefined);
  const timeMatches = pointTimeProducts.has(plan.dataset.id)
    ? value.datetime === value.properties.datetime && value.startDatetime == null && value.endDatetime == null
      && source?.startDatetime === value.datetime && source?.endDatetime === value.datetime
    : value.startDatetime === source?.startDatetime && value.endDatetime === source?.endDatetime
      && originalStart === value.startDatetime && originalEnd === value.endDatetime;
  if (!source || !reviewedItemProducts.has(plan.dataset.id) || value.provenance.documentUrl !== source.stac.url || !dynamicTile && !jaxaProducts.has(plan.dataset.id) && value.itemId !== source.id
    || !boundsMatch || !timeMatches
    || source.region != null && !jaxaProducts.has(plan.dataset.id) && value.properties['odc:region_code'] !== source.region
    || !object(proof) || proof.schemaVersion !== 2 || !sameJson(proof.dataset, plan.dataset) || !sameJson(proof.request, plan.request)
    || !sameJson(proof.layer, plan.dataset.layers.find(layer => layer.id === plan.request.layer))
    || proof.sourceId !== source.id || proof.sourceTitle !== source.title || !sameJson(proof.sourceBounds, source.bounds)
    || proof.declarationKind !== 'reviewed-client-preset; publisher file bytes and embedded metadata verified separately') fail('The published item does not match the reviewed region, season and product.');
  return value;
}

export function publicDatasetAssetLayer(dataset, request, assetKey) {
  if(isNextPlanetaryProduct(dataset.id)) return request?.layer===assetKey && NEXT_PC_PRODUCTS[dataset.id].keys.includes(assetKey)
    ? dataset.layers.find(layer=>layer.id===assetKey) || null : null;
  const batchLayer=batchJaxaLayers[dataset.id];
  if(batchLayer) return assetKey===batchLayer[0] && request?.layer===batchLayer[1]
    ? dataset.layers.find(layer=>layer.id===batchLayer[1]) || null : null;
  if (dataset.id==='cbers-wpm') return ['BAND1','BAND2','BAND3','BAND4'].includes(assetKey)
    && request?.layer==='wpm-ms' ? dataset.layers.find(layer=>layer.id==='wpm-ms') || null : null;
  if (dataset.id==='amazonia-wfi-dn') return ['BAND1','BAND2','BAND3','BAND4'].includes(assetKey)
    && request?.layer==='wfi-ms' ? dataset.layers.find(layer=>layer.id==='wfi-ms') || null : null;
  if (pointTimeProducts.has(dataset.id) && assetKey !== request.layer) return null;
  const exact = dataset.layers.find(layer => layer.id === assetKey);
  if (exact) return exact;
  const mapped = dataset.id === 'cgls-landcover' && assetKey === 'permanentwater_cover_fraction' && request.layer === 'permanentwater'
    || dataset.id === 'wsf-settlements' && assetKey === `wsf${request.year}` && request.layer === 'settlement'
    || dataset.id === 'worldcereal' && assetKey === 'classification';
  return mapped || assetKey === 'raster' || jaxaProducts.has(dataset.id) && assetKey.toLowerCase() === request.layer ? dataset.layers.find(layer => layer.id === request.layer) || null : null;
}

// Display stretch only. The immutable TIFF and native class values stay intact.
// Bind the narrow class range to the reviewed product and actual publisher asset.
export function publicDatasetPreviewRange(dataset, asset, item) {
  if(isNextPlanetaryProduct(dataset?.id)) return nextPlanetaryRange(dataset,asset,item);
  if(NEXT_DIRECT_PRODUCTS.has(dataset?.id)) return nextDirectRange(dataset,asset);
  const batch=batchJaxaDisplay[dataset?.id], mapping=batchJaxaLayers[dataset?.id];
  if(batch) {
    const proof=item?.provenance?.reviewedDataset,raster=asset?.metadata?.['je:rasters'],dn=raster?.dn;
    if(!mapping || asset?.key!==mapping[0] || proof?.dataset?.id!==dataset.id
      || proof.request?.datasetId!==dataset.id || proof.request.layer!==mapping[1]
      || proof.request.year!==batch[5] || proof.request.month!==batch[6]
      || dn?.data_type!==batch[0] || raster.dn2value?.slope!==batch[1]
      || raster.dn2value.offset!==batch[2] || dn.nodata!==batch[3]
      || raster.value?.unit!==batch[4]) return undefined;
    if(batch[7] && ['jaxa-amsr2-sst','jaxa-gsmap-spi'].includes(dataset.id))
      return dn.min===null && dn.max===null ? [...batch[7]] : undefined;
    if(batch[7]) return Number.isFinite(dn.min) && Number.isFinite(dn.max)
      && dn.min>=0 && dn.min<dn.max && dn.max<65535 ? [...batch[7]] : undefined;
    return Number.isFinite(dn.min) && Number.isFinite(dn.max) && dn.min>=0
      && dn.min<dn.max && dn.max<(batch[0]==='uint16'?65535:32767)
      ? [dn.min,dn.max] : undefined;
  }
  const preset=asset?.metadata?.datasetPreset;
  if(['srtm-gl3','gedi-l3-canopy','srtm15plus-bathymetry'].includes(dataset?.id)
    && asset.key==='raster' && preset?.schemaVersion===2 && preset.dataset?.id===dataset.id
    && preset.request?.datasetId===dataset.id) {
    const ranges={'srtm-gl3':['elevation',1,0,-32768,[-50,3000]],
      'gedi-l3-canopy':['rh100-mean',1,0,-9999,[0,80]],
      'srtm15plus-bathymetry':['surface',null,null,null,[-8000,4000]]};
    const [layer,scale,offset,nodata,range]=ranges[dataset.id];
    return preset.request.layer===layer && preset.layer?.id===layer
      && (preset.layer.scale??null)===scale && (preset.layer.offset??null)===offset
      && (preset.layer.nodata??null)===nodata ? [...range] : undefined;
  }
  if(dataset?.id==='openlandmap-nightlights' && asset.key==='raster'
    && preset?.schemaVersion===2 && preset.dataset?.id===dataset.id
    && preset.request?.year===2024 && preset.layer?.id==='annual-dn'
    && preset.layer.scale===10 && preset.layer.offset===0 && preset.layer.nodata===-32768) return [0,200];
  // Fixed raw-DN contrast only; the original TIFF supplies the actual NoData.
  // Keep the publisher's conflicting radiometric scale out of this display.
  const wfiBand=asset?.metadata?.['eo:bands'], wfiCollection='AMZ1-WFI-L4-DN-1';
  const wfiNames={BAND1:'blue',BAND2:'green',BAND3:'red',BAND4:'nir'};
  if(dataset?.id==='amazonia-wfi-dn' && dataset.layers?.some(layer=>layer.id==='wfi-ms')
    && item?.collectionId===wfiCollection && item.provenance?.search?.collectionId===wfiCollection
    && item.provenance?.collection?.id===wfiCollection
    && /^AMAZONIA_1_WFI_\d{8}_\d{3}_\d{3}_L4$/.test(item.itemId || '')
    && wfiNames[asset?.key] && wfiBand?.length===1 && wfiBand[0].name===asset.key
    && wfiBand[0].common_name===wfiNames[asset.key] && wfiBand[0].data_type==='int16'
    && wfiBand[0].resolution_x===64 && wfiBand[0].resolution_y===64
    && asset.href===asset.metadata.href && asset.roles?.includes('data')
    && asset.href.startsWith('https://data.inpe.br/bdc/data/AMZ1-WFI-L4-DN/')
    && asset.href.endsWith(`/${item.itemId}_${asset.key}.tif`)) return [0,1023];
  // WPM's declared 10-bit DN range is a display stretch only. The publisher's
  // contradictory uint8 label must never replace the TIFF's actual Int16 type.
  const wpmBand=asset?.metadata?.['eo:bands'];
  if (dataset?.id==='cbers-wpm' && /^BAND[1-4]$/.test(asset?.key || '')
    && wpmBand?.length===1 && wpmBand[0].name===asset.key
    && wpmBand[0].min===0 && wpmBand[0].max===1024 && wpmBand[0].nodata===0
    && wpmBand[0].scale===1 && [null,0].includes(wpmBand[0].scale_add)
    && wpmBand[0].resolution_x===8 && wpmBand[0].resolution_y===8) return [0,1024];
  const raster=asset?.metadata?.['je:rasters'], dn=raster?.dn;
  if (dataset?.id==='jaxa-palsar-fnf' && asset.key==='FNF'
    && dn?.data_type==='uint8' && dn.min===1 && dn.max===4 && dn.nodata===0) return [1,4];
  if (dataset?.id==='jaxa-gsmap-rainfall' && asset.key==='PRECIP'
    && dn?.data_type==='float32' && dn.min===0 && Number.isFinite(dn.max) && dn.max>0
    && dn.nodata===-999.9 && raster.dn2value?.slope===1 && raster.dn2value.offset===0
    && raster.value?.unit==='mm/hr') return [dn.min,dn.max];
  if (dataset?.id==='jaxa-hrlulc-japan' && asset.key==='HRLULC'
    && dn?.data_type==='uint8' && dn.nodata===0
    && raster.dn2value?.slope===1 && raster.dn2value.offset===0
    && asset.metadata['classification:classes']?.length===16
    && asset.metadata['classification:classes'].every((entry,index)=>entry.value===index
      && (index!==0 || entry.nodata===true))) return [1,15];
  if (dataset?.id==='jaxa-modis-swr' && asset.key==='swr'
    && dn?.data_type==='uint16' && dn.min===0 && Number.isFinite(dn.max) && dn.max>0 && dn.max<65535
    && dn.nodata===65535 && raster.dn2value?.slope===0.01 && raster.dn2value.offset===0
    && raster.value?.unit==='W/m^2') return [dn.min,dn.max];
  if (dataset?.id==='jaxa-sgli-sst' && asset.key==='SST'
    && dn?.data_type==='uint16' && Number.isFinite(dn.min) && Number.isFinite(dn.max)
    && dn.min>=0 && dn.min<dn.max && dn.max<65532 && dn.nodata===65535
    && raster.dn2value?.slope===0.0012000000569969416 && raster.dn2value.offset===-10
    && raster.value?.unit==='degreeC') return [dn.min,dn.max];
  return undefined;
}

export function publicDatasetPreviewPolicy(dataset, asset, item) {
  if(dataset?.id==='modis-more') return nextOverviewPolicy(dataset,asset,item);
  const proof=asset?.metadata?.datasetPreset;
  return dataset?.id==='srtm15plus-bathymetry' && asset?.key==='raster'
    && asset.href==='https://opentopography.s3.sdsc.edu/raster/SRTM15Plus/SRTM15Plus_srtm/SRTM15_V2.5.5_02_10.tif'
    && asset.metadata.href===asset.href && proof?.schemaVersion===2
    && proof.dataset?.id===dataset.id && proof.request?.datasetId===dataset.id
    && proof.request.layer==='surface' && proof.layer?.id==='surface'
    && proof.sourceId==='SRTM15_V2.5.5_02_10' ? 'srtm15plus-02-10-v255' : undefined;
}

export async function publicDatasetRequest(operation, payload = {}, signal) {
  return usageRequest('public-dataset', operation, payload, () => performPublicDatasetRequest(operation, payload, signal));
}
async function performPublicDatasetRequest(operation, payload = {}, signal) {
  const commands = { list: 'list_public_datasets', resolve: 'resolve_public_dataset', connect: 'connect_public_dataset' };
  if (!commands[operation]) fail('Unknown public dataset operation.');
  if (operation !== 'list') validateDatasetRequest(payload);
  const check = () => { if (signal?.aborted) throw new DOMException('Dataset request cancelled', 'AbortError'); };
  check();
  let value;
  if (desktopAvailable()) {
    try { value = await window.__TAURI__.core.invoke(commands[operation], operation === 'list' ? {} : { request: payload }); }
    catch (cause) { throw diagnosticError(cause instanceof Error ? cause : new Error(String(cause)), {stage:'service_request',diagnosticOrigin:'explicit'}); }
  } else {
    let response;
    try { response = await fetch(`http://127.0.0.1:4318/datasets${operation === 'list' ? '' : `/${operation}`}`, {
      method: operation === 'list' ? 'GET' : 'POST', signal,
      headers: operation === 'list' ? undefined : { 'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global' },
      body: operation === 'list' ? undefined : JSON.stringify(payload),
    }); } catch(cause) { throw diagnosticError(cause,{stage:'service_request',reason:cause?.name==='AbortError'?'cancelled':cause?.name==='TimeoutError'?'timeout':'network',diagnosticOrigin:'explicit'}); }
    try { value = await response.json(); }
    catch(cause) { throw diagnosticError(cause,{stage:'response_decode',httpStatus:response.status,diagnosticOrigin:'explicit'}); }
    if (!response.ok) throw diagnosticError(new Error(value?.error || `Dataset service returned HTTP ${response.status}`),{stage:'service_request',httpStatus:response.status,diagnosticOrigin:'explicit'});
  }
  check();
  try {
    if (operation === 'list') {
      if (!Array.isArray(value) || value.length > 128 || new Set(value.map(dataset => dataset.id)).size !== value.length) fail('Invalid public dataset catalog.');
      value.forEach(validatePublicDataset);
    } else if (operation === 'resolve') validatePublicDatasetPlan(value, payload);
    else validatePublicDatasetConnection(value, payload);
  } catch(cause) { throw diagnosticError(cause,{stage:'response_validation',reason:'validation',diagnosticOrigin:'explicit'}); }
  return value;
}
