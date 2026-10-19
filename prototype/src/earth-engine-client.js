import { desktopAvailable } from './runtime-client.js';
import { validateStacInspection } from './stac-client.js';
import { validQueryBounds } from './features-client.js';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export function earthEngineProject(value) {
  try {
    const url = new URL(value.trim());
    if (url.origin !== 'https://earthengine.googleapis.com' || url.username || url.password || url.search || url.hash) return null;
    return url.pathname.match(/^\/v1\/projects\/([a-z0-9-]{6,63})\/thumbnails\/[a-zA-Z0-9_-]{16,512}:getPixels$/)?.[1] || null;
  } catch { return null; }
}
export function landsatEarthEngineId(itemId = '') {
  const match = itemId.match(/^(LC0[89])_L2\w{2}_(\d{6})_(\d{8})_\d{8}_02_T1$/);
  return match ? `LANDSAT/${match[1]}/C02/T1_L2/${match[1]}_${match[2]}_${match[3]}` : '';
}
export function earthEngineScript({ imageId, band, bounds, tag }) {
  if (!/^LANDSAT\/LC0[89]\/C02\/T1_L2\/LC0[89]_\d{6}_\d{8}$/.test(imageId)
      || !['SR_B2','SR_B3','SR_B4','SR_B5','QA_PIXEL','QA_RADSAT'].includes(band)
      || !validQueryBounds(bounds) || !/^[a-z0-9][a-z0-9_-]{1,61}[a-z0-9]$/.test(tag)) {
    throw new Error('Choose a Landsat 8/9 image, a band and valid WGS84 bounds.');
  }
  // Conservative request sizing; the server remains authoritative. This is a
  // 30 m generated subset, not an assurance of an unchanged original grid.
  const pixels = Math.ceil((bounds[2]-bounds[0])*111320/30) * Math.ceil((bounds[3]-bounds[1])*111320/30);
  if (pixels * 2 > 24*1024*1024) throw new Error('This region is too large for direct export. Choose a smaller region.');
  return `// GeoD Global: a generated subset, not the original scene file.\n// Uses the Cloud project selected in your Earth Engine editor.\nee.data.setWorkloadTag(${JSON.stringify(tag)});\nvar image = ee.Image(${JSON.stringify(imageId)}).select(${JSON.stringify(band)});\nvar region = ee.Geometry.Rectangle(${JSON.stringify(bounds)});\nprint('LANDSAT_PRODUCT_ID', image.get('LANDSAT_PRODUCT_ID'));\nprint('workload_tag', ${JSON.stringify(tag)});\nprint('GeoTIFF download', image.getDownloadURL({\n  name: 'geod_landsat_export', region: region, scale: 30, format: 'GEO_TIFF'\n}));\n`;
}
function validateRecord(value) {
  if (!value || !uuid.test(value.id) || typeof value.name !== 'string' || value.name.length > 120
      || !/^[a-z0-9-]{6,63}$/.test(value.projectId) || !/^[a-f0-9]{64}$/.test(value.sha256)
      || !Number.isSafeInteger(value.bytes) || value.bytes < 1 || value.bytes > 32*1024*1024
      || !Number.isFinite(value.downloadSeconds) || value.downloadSeconds < 0
      || !Number.isFinite(Date.parse(value.createdAt)) || value.eecuSeconds !== null || value.usageStatus !== 'not_measured') {
    throw new Error('Invalid GEE export record.');
  }
  return value;
}
export async function earthEngineRequest(operation, payload = {}) {
  const commands = { list:'list_earth_engine_exports', download:'download_earth_engine_export', inspect:'inspect_earth_engine_export' };
  if (!commands[operation]) throw new Error('Unknown GEE operation.');
  if (operation === 'download' && (!earthEngineProject(payload.url) || !payload.name?.trim())) throw new Error('Enter a name and a valid GEE export link.');
  if (operation === 'inspect' && !uuid.test(payload.id)) throw new Error('Invalid GEE export identifier.');
  let value;
  if (desktopAvailable()) {
    try { value = await window.__TAURI__.core.invoke(commands[operation], operation === 'download' ? {request:payload} : payload); }
    catch (error) { throw new Error(String(error?.message || error)); }
  } else {
    const url = 'http://127.0.0.1:4318/earth-engine/exports' + (operation === 'inspect' ? `/${payload.id}/inspect` : '');
    const response = await fetch(url, operation === 'download' ? {method:'POST',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:JSON.stringify(payload)} : {});
    value = await response.json();
    if (!response.ok) throw new Error(value.error || 'GEE request failed.');
  }
  if (operation === 'list') { if (!Array.isArray(value)) throw new Error('Invalid GEE export record.'); value.forEach(validateRecord); }
  if (operation === 'download') { validateRecord(value.export); validateStacInspection(value.raster, value.export.id); if(value.raster.sha256 !== value.export.sha256) throw new Error('Invalid GEE export record.'); }
  if (operation === 'inspect') validateStacInspection(value, payload.id);
  return value;
}
export async function saveEarthEngineFile(id) {
  if (!uuid.test(id)) throw new Error('Invalid GEE export identifier.');
  if (desktopAvailable()) return window.__TAURI__.core.invoke('export_earth_engine_file', {id});
  const response = await fetch(`http://127.0.0.1:4318/earth-engine/exports/${id}/file`);
  if (!response.ok) throw new Error('Could not read the saved GEE export.');
  const url = URL.createObjectURL(await response.blob());
  const a = document.createElement('a'); a.href=url; a.download='gee-export.tif'; a.click();
  setTimeout(()=>URL.revokeObjectURL(url), 60000);
  return true;
}
export async function openEarthEnginePage(url) {
  const parsed = new URL(url);
  if (parsed.protocol !== 'https:' || !['console.cloud.google.com','code.earthengine.google.com'].includes(parsed.hostname)) throw new Error('Invalid Google page.');
  if (desktopAvailable()) return window.__TAURI__.core.invoke('open_source', {url});
  window.open(url, '_blank', 'noopener,noreferrer');
}
