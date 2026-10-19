import {usageRequest} from './usage-events.js';
import {desktopAvailable} from './runtime-client.js';
import {validateStacInspection,validateStacPixel} from './stac-client.js';
export const LOCAL_RASTER_MAX_BYTES=32*1024*1024;
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const hash=/^[a-f0-9]{64}$/;
const fail=m=>{throw new Error(m);};
export function validateLocalRaster(asset) {
  if(!asset||!uuid.test(asset.id)||typeof asset.name!=='string'||!asset.name||asset.name!==asset.name.trim()||asset.name.length>240
    ||/[\u0000-\u001f/\\:]/.test(asset.name)||! /\.tiff?$/i.test(asset.name)||!hash.test(asset.sha256)
    ||!Number.isSafeInteger(asset.bytes)||asset.bytes<=0||asset.bytes>LOCAL_RASTER_MAX_BYTES||!Number.isFinite(Date.parse(asset.createdAt)))fail('Invalid local raster original record.');
  return asset;
}
export function validateLocalRasterInspection(data,asset) {
  validateLocalRaster(asset);validateStacInspection(data,asset.id);
  if(data.sha256!==asset.sha256||data.width>10000||data.height>10000||!data.crs||!data.transform||data.archiveOriginal!=null)fail('The local raster does not match its saved original.');
  return data;
}
const check=signal=>{if(signal?.aborted)throw new DOMException('Request aborted','AbortError');};
export async function localRasterRequest(operation,payload={},signal) {
  return usageRequest('local', operation, payload, () => performLocalRasterRequest(operation, payload, signal));
}
async function performLocalRasterRequest(operation,payload={},signal) {
  const commands={list:'list_local_rasters',open:'open_local_raster',inspect:'inspect_local_raster',pixel:'sample_local_raster',export:'export_local_raster'};
  if(!commands[operation])fail('Unknown local raster operation.');
  if(['inspect','pixel','export'].includes(operation)&&!uuid.test(payload.id))fail('Invalid local raster identifier.');
  check(signal);let value;
  if(desktopAvailable())value=await window.__TAURI__.core.invoke(commands[operation],payload);
  else {
    if(['open','export'].includes(operation))fail('Native file selection requires the desktop application.');
    const suffix=operation==='list'?'':`/${payload.id}/${operation==='pixel'?`pixel?${new URLSearchParams({column:payload.column,row:payload.row})}`:'inspect'}`;
    const response=await fetch(`http://127.0.0.1:4318/local-rasters${suffix}`,{signal});value=await response.json();if(!response.ok)fail(value.error||'Local raster request failed.');
  }
  check(signal);
  if(operation==='list'){if(!Array.isArray(value)||value.length>1000||new Set(value.map(v=>v.id)).size!==value.length)fail('Invalid local raster library.');value.forEach(validateLocalRaster);}
  if(operation==='inspect')validateStacInspection(value,payload.id);
  if(operation==='pixel')validateStacPixel(value,payload);
  if(operation==='open'&&value!==null){validateLocalRaster(value.asset);validateLocalRasterInspection(value.raster,value.asset);}
  return value;
}
export async function importLocalRasterFile(file,signal) {
  if(!file||! /\.tiff?$/i.test(file.name)||file.size<1||file.size>LOCAL_RASTER_MAX_BYTES)fail('Choose a local GeoTIFF or COG file up to 32 MiB.');
  const bytes=await file.arrayBuffer();check(signal);
  const sha=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
  const response=await fetch(`http://127.0.0.1:4318/local-rasters/import-file?name=${encodeURIComponent(file.name)}`,{method:'POST',signal,headers:{'Content-Type':'application/octet-stream','X-GeoD-Client':'geod-global'},body:bytes});
  const value=await response.json();if(!response.ok)fail(value.error||'Local raster import failed.');check(signal);validateLocalRasterInspection(value.raster,value.asset);
  if(value.asset.sha256!==sha||value.asset.bytes!==file.size)fail('The imported local raster original bytes changed.');return value;
}
export async function exportLocalRaster(asset,signal) {
  validateLocalRaster(asset);
  if(desktopAvailable())return localRasterRequest('export',{id:asset.id},signal);
  const response=await fetch(`http://127.0.0.1:4318/local-rasters/${asset.id}/source`,{signal});
  if(!response.ok)fail((await response.json()).error||'Local raster original export failed.');
  if(response.headers.get('content-type')?.split(';')[0]!=='image/tiff')fail('Local raster original format changed.');
  const bytes=await response.arrayBuffer(),sha=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
  if(bytes.byteLength!==asset.bytes||sha!==asset.sha256)fail('Local raster original checksum changed.');
  check(signal);const url=URL.createObjectURL(new Blob([bytes],{type:'image/tiff'})),link=document.createElement('a');link.href=url;link.download=asset.name;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);return true;
}
