import { snapshotId, stacConnection, stacSnapshot, stacProject, stacJob } from './stac-fixtures.js';

// Transport-only fixtures. These are never included in the product catalog.
export const publicDataset = {
  id: 'worldcover', title: 'ESA WorldCover', category: 'land-cover', description: 'Published categorical land-cover maps.',
  version: '2020 v100 / 2021 v200', license: 'CC-BY-4.0', licenseUrl: 'https://creativecommons.org/licenses/by/4.0/',
  attribution: 'ESA WorldCover / VITO', documentationUrl: 'https://esa-worldcover.org/en/data-access',
  nominalResolution: '10 m', unit: 'class code', valueInterpretation: 'Land-cover classes, not optical reflectance.',
  years: [2020, 2021], defaultYear: 2021, requiresCountry: false,
  layers: [{ id: 'map', title: 'Land cover', unit: 'class code', valueInterpretation: 'Class labels with provider NoData.' }],
  defaultLayer: 'map', warnings: ['Publication years are not current observation dates.'], limits: ['Up to 32 selected files per project request.'],
};
export const datasetPayload = { datasetId: publicDataset.id, bounds: [10,20,11,21], year: 2021, layer: 'map' };
export const datasetPlan = { dataset: publicDataset, request: datasetPayload, sources: [
  { id: 'tile-10-20', title: 'Published WorldCover tile', bounds: [9,18,12,21], stac: { name: 'Published WorldCover', url: 'https://example.com/worldcover.tif', kind: 'raster' } },
], warnings: ['Downloads keep the complete tile.'] };
export const datasetStacConnection = { ...stacConnection, name: 'Published WorldCover', url: datasetPlan.sources[0].stac.url, kind: 'raster', collections: [], snapshotIds: [snapshotId], capabilities: { searchGet: false, searchPost: false } };
export const datasetSnapshot = { ...stacSnapshot, title: 'Published WorldCover tile', properties: { dataset: publicDataset.id, classification: 'provider labels' },
  assets: [{ ...stacSnapshot.assets[0], title: 'Land-cover raster', href: datasetPlan.sources[0].stac.url, metadata: { kind: 'direct-raster', dataset: publicDataset.id, units: 'class code' } }, stacSnapshot.assets[1]] };
export const datasetConnected = { plan: datasetPlan, stacConnections: [datasetStacConnection], wcsConnections: [], failures: [], complete: true };
export const datasetProject = { ...stacProject, name: publicDataset.title, stacItems: [{ ...stacProject.stacItems[0], title: 'Land-cover raster', href: datasetPlan.sources[0].stac.url }], canonicalSelections: [{ snapshotId, assetKey: 'surface_temp' }] };
export const datasetJob = { ...stacJob, title: 'Land-cover raster', href: datasetPlan.sources[0].stac.url, status: 'queued' };

export const catalogDataset = { ...publicDataset, id: 'cbers-sr', title: 'INPE CBERS surface reflectance scenes', category: 'imagery', discovery: 'stac-api',
  description: 'Individual published MUX surface-reflectance scenes.', years: [2026], defaultYear: 2026, months: [8,9,10], defaultMonth: 9,
  layers: [{ id: 'cbers4a-mux', title: 'CBERS-4A MUX', unit: 'band-specific', valueInterpretation: 'Individual raw numerical bands, not RGB quicklooks.' }], defaultLayer: 'cbers4a-mux' };
export const catalogPayload = { datasetId: catalogDataset.id, bounds: datasetPayload.bounds, year: 2026, month: 9, layer: 'cbers4a-mux' };
export const catalogQuery = { collectionId: 'CB4A-MUX-L4-SR-1', datetime: '2026-09-01T00:00:00Z/2026-09-30T23:59:59.999999Z', bounds: datasetPayload.bounds, limit: 20 };
export const catalogPlan = { dataset: catalogDataset, request: catalogPayload, sources: [{ id: catalogQuery.collectionId, title: 'CBERS observations 2026-09',
  stac: { name: 'INPE STAC scene catalog', url: 'https://example.com/stac/', kind: 'api' }, search: catalogQuery }], warnings: ['An observation month is not a monthly composite.'] };
export const catalogSnapshot = { ...stacSnapshot, collectionId: catalogQuery.collectionId, itemId: 'CBERS-test-observation-1', title: 'CBERS MUX scene',
  datetime: '2026-09-02T12:00:00Z', startDatetime: null, endDatetime: null, temporalStatus: 'instant',
  provenance: { ...stacSnapshot.provenance, search: { connectionId: stacConnection.id, ...catalogQuery, cursor: null } },
  assets: [{ ...stacSnapshot.assets[0], key: 'BAND5', title: 'Blue surface reflectance band', metadata: {'raster:bands':[{data_type:'int16',scale:0.0001,nodata:-9999}]} }] };
export const catalogConnection = { ...stacConnection, name: catalogPlan.sources[0].stac.name, url: catalogPlan.sources[0].stac.url,
  collections: [{id:catalogQuery.collectionId,title:'CBERS MUX SR',description:'Original scene archive',license:'CC-BY-4.0'}], snapshotIds:[catalogSnapshot.id] };
export const catalogPage = { items:[catalogSnapshot], nextCursor:'test-next-page', complete:false, limitReached:false };
export const catalogConnected = { plan:catalogPlan, stacConnections:[catalogConnection], wcsConnections:[], failures:[], complete:true,
  searches:[{sourceId:catalogPlan.sources[0].id,connectionId:catalogConnection.id,page:catalogPage}] };
