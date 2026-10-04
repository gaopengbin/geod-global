import { desktopAvailable } from './runtime-client.js';
import { validQueryBounds } from './features-client.js';
import { validStacUrl, validateStacInspection as validateFileInspection, validateStacPixel as validateFilePixel } from './stac-client.js';
const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const hash = /^[a-f0-9]{64}$/;
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const text = (value, max = 16384) => typeof value === 'string' && value.length <= max;
const strings = (value, max, length = 2048) => Array.isArray(value) && value.length <= max && value.every(item => text(item, length));
const array = (value, length) => Array.isArray(value) && value.length === length && value.every(Number.isFinite);
const bounds = value => array(value, 4) && value[0] < value[2] && value[1] < value[3];
const date = value => text(value, 80) && Number.isFinite(Date.parse(value));
const integer = (value, max = Number.MAX_SAFE_INTEGER) => Number.isSafeInteger(value) && value > 0 && value <= max;
const fail = message => { throw new Error(message); };
const wcsCrs = value => value === 'EPSG:4326' || value === 'EPSG:3857' || /^EPSG:(?:326|327)(?:0[1-9]|[1-5]\d|60)$/.test(value);
export function validateWcsConnection(value) {
  if (!object(value) || !uuid.test(value.id) || !text(value.name, 512) || !value.name.trim() || !text(value.title, 512) || !validStacUrl(value.url)
    || value.version !== '2.0.1' || !date(value.connectedAt) || !hash.test(value.capabilitiesSha256)
    || !validStacUrl(value.describeUrl) || !validStacUrl(value.coverageUrl) || !strings(value.formats, 256) || !strings(value.profiles, 256)
    || !text(value.accessConstraints) || !text(value.fees) || !(value.attribution === null || text(value.attribution))
    || !Array.isArray(value.coverages) || value.coverages.length > 4096 || value.coverages.some(item => !object(item) || !text(item.id, 512) || !item.id || !text(item.title, 512) || !text(item.subtype, 512))
    || new Set(value.coverages.map(item => item.id)).size !== value.coverages.length) fail('The coverage service returned invalid connection metadata.');
  return value;
}
function validGrid(value) {
  if (!integer(value.width) || !integer(value.height) || !array(value.transform, 6) || !bounds(value.nativeBounds)
    || value.transform[0] <= 0 || value.transform[4] >= 0 || value.transform[1] !== 0 || value.transform[3] !== 0) return false;
  const [dx, , x, , dy, y] = value.transform;
  return [x, y + value.height * dy, x + value.width * dx, y].every((expected, index) => Math.abs(expected - value.nativeBounds[index]) <= Math.max(1, Math.abs(expected)) * 1e-8);
}
export function validateWcsDescription(value) {
  if (!object(value) || !hash.test(value.id) || !uuid.test(value.connectionId) || !text(value.coverageId, 512) || !value.coverageId
    || !text(value.title, 512) || !text(value.serviceName, 512) || !wcsCrs(value.crs) || !text(value.declaredCrs, 2048)
    || !strings(value.axisLabels, 2, 512) || value.axisLabels.length !== 2 || !strings(value.gridAxisLabels, 2, 512) || value.gridAxisLabels.length !== 2
    || !validGrid(value) || !validQueryBounds(value.bounds) || !hash.test(value.capabilitiesSha256) || !hash.test(value.descriptionSha256)
    || !validStacUrl(value.descriptionUrl) || !date(value.retrievedAt) || !strings(value.warnings, 256)
    || !Array.isArray(value.fields) || !value.fields.length || value.fields.length > 16 || value.fields.some(field => !object(field) || !text(field.name, 512) || !field.name
      || !text(field.description) || !(field.unit === null || text(field.unit, 256)) || !Array.isArray(field.nilValues) || field.nilValues.length > 256
      || field.nilValues.some(nil => !object(nil) || !text(nil.value, 256) || !(nil.reason === null || text(nil.reason, 2048))))
    || !strings(value.metadataLinks, 256, 8192)) fail('The coverage description has an invalid grid or range metadata.');
  return value;
}
export function validateWcsPlan(value) {
  if (!object(value)) fail('Invalid coverage subset plan.');
  validateWcsDescription(value.description);
  if (!hash.test(value.id) || !validQueryBounds(value.requestedBounds) || !validQueryBounds(value.bounds) || !validGrid(value)
    || !integer(value.width, 65536) || !integer(value.height, 65536) || value.width * value.height * value.description.fields.length > 536870912
    || !validStacUrl(value.requestUrl) || !text(value.format, 256) || !['image/tiff', 'image/tiff;application=geotiff'].includes(value.format.replace(/;\s+/g, ';'))
    || value.selection !== 'bbox-native-grid' || !strings(value.warnings, 256)) fail('The coverage subset plan exceeds its limits or has an invalid predicted grid.');
  return value;
}
function selections(value) { return Array.isArray(value) && value.length > 0 && value.length <= 32 && value.every(item => object(item) && hash.test(item.planId)) && new Set(value.map(item => item.planId)).size === value.length; }
export async function wcsRequest(operation, payload = {}, signal) {
  const commands = { list: 'list_wcs_connections', connect: 'connect_wcs', forget: 'forget_wcs_connection', describe: 'describe_wcs', plan: 'plan_wcs', savedPlan: 'wcs_plan', project: 'save_wcs_project', downloads: 'download_wcs_project', inspect: 'inspect_wcs_asset', pixel: 'sample_wcs_asset' };
  if (!commands[operation]) fail('Unknown coverage service operation.');
  if (operation === 'connect' && (!payload.name?.trim() || !validStacUrl(payload.url))) fail('Enter a coverage service name and public HTTPS URL.');
  if (['forget', 'inspect', 'pixel'].includes(operation) && !uuid.test(payload.id) || operation === 'savedPlan' && !hash.test(payload.id)) fail('Invalid saved coverage identifier.');
  if (operation === 'describe' && (!uuid.test(payload.connectionId) || !text(payload.coverageId, 512) || !payload.coverageId)) fail('Choose a coverage from a saved service.');
  if (operation === 'plan' && (!hash.test(payload.descriptionId) || !validQueryBounds(payload.bounds))) fail('Choose a described coverage and valid region.');
  if (operation === 'project' && (!validQueryBounds(payload.bounds) || !selections(payload.selections))) fail('Choose valid coverage plans and a project region.');
  if (operation === 'downloads' && (!uuid.test(payload.projectId) || payload.selections !== undefined && !selections(payload.selections))) fail('Choose saved coverage plans to download.');
  if (operation === 'pixel' && ['column', 'row'].some(key => !Number.isSafeInteger(payload[key]) || payload[key] < 0)) fail('Enter a nonnegative pixel column and row.');
  const check = () => { if (signal?.aborted) throw new DOMException('Coverage request cancelled', 'AbortError'); }; check();
  let value;
  const mutation = ['connect', 'forget', 'describe', 'plan', 'project', 'downloads'].includes(operation);
  if (desktopAvailable()) {
    try { value = await window.__TAURI__.core.invoke(commands[operation], mutation && operation !== 'forget' ? { request: payload } : payload); }
    catch (error) { throw error instanceof Error ? error : new Error(String(error)); }
  } else {
    const paths = { list: '/wcs/connections', connect: '/wcs/connections', forget: `/wcs/connections/${payload.id}/forget`, describe: '/wcs/describe', plan: '/wcs/plan', savedPlan: `/wcs/plans/${payload.id}`, project: '/wcs/project', downloads: '/wcs/downloads', inspect: `/wcs/jobs/${payload.id}/inspect`, pixel: `/wcs/jobs/${payload.id}/pixel?${new URLSearchParams({ column: payload.column, row: payload.row })}` };
    const response = await fetch('http://127.0.0.1:4318' + paths[operation], { method: mutation ? 'POST' : 'GET', signal, headers: mutation ? { 'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global' } : undefined, body: mutation ? JSON.stringify(payload) : undefined });
    value = await response.json(); if (!response.ok) fail(value.error || `Coverage service returned HTTP ${response.status}`);
  }
  check();
  if (operation === 'list') { if (!Array.isArray(value) || value.length > 24) fail('Invalid coverage connection registry.'); value.forEach(validateWcsConnection); }
  if (operation === 'connect') validateWcsConnection(value);
  if (operation === 'describe') { validateWcsDescription(value); if (value.connectionId !== payload.connectionId || value.coverageId !== payload.coverageId) fail('The returned description belongs to a different coverage.'); }
  if (['plan', 'savedPlan'].includes(operation)) { validateWcsPlan(value); if (operation === 'savedPlan' ? value.id !== payload.id : value.description.id !== payload.descriptionId || value.requestedBounds.some((v, i) => v !== payload.bounds[i])) fail('The returned plan does not match the selected coverage and region.'); }
  if (operation === 'inspect') validateFileInspection(value, payload.id);
  if (operation === 'pixel') validateFilePixel(value, payload);
  if (operation === 'project' && (!object(value) || !uuid.test(value.id) || !Array.isArray(value.wcsItems) || !payload.selections.every(pin => value.wcsItems.some(item => item.planId === pin.planId)) || payload.projectId && value.id !== payload.projectId)) fail('The saved project does not contain the selected coverage plans.');
  if (operation === 'downloads' && (!object(value) || value.projectId !== payload.projectId || value.assetKey !== 'wcs_coverage' || !Array.isArray(value.jobs) || value.jobs.some(job => job.kind !== 'download' || !uuid.test(job.id) || job.assetKey !== 'wcs_coverage' || !hash.test(job.wcsSource?.planId) || payload.selections && !payload.selections.some(pin => pin.planId === job.wcsSource.planId)))) fail('The download queue does not match the saved coverage selection.');
  return value;
}
