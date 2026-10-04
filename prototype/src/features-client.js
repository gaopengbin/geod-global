import {desktopAvailable} from './runtime-client.js';
import {validateVectorAsset,validateArcgisLayer,arcgisServiceRoot,validArcgisLayerId,validPublicFeatureUrl,validOverpassBounds,validOverpassEndpoint,validOsmCopyright,validateWfsLayer,validWfsQName,validWfsText,OVERPASS_PRESETS,OVERPASS_DESCRIPTION,OSM_LICENSE_URL} from './vector-client.js';
export {validOverpassBounds};
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const digest=/^[a-f0-9]{64}$/;
const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const clean=(v,max)=>typeof v==='string'&&v.length>0&&v===v.trim()&&Array.from(v).length<=max&&!/[\u0000-\u001f\u007f-\u009f]/.test(v);
const bytes=(v,max)=>typeof v==='string'&&new TextEncoder().encode(v).length<=max;
export function validServiceUrl(raw) {
  return validPublicFeatureUrl(raw);
}
export function validQueryBounds(bounds) {
  return Array.isArray(bounds)&&bounds.length===4&&bounds.every(Number.isFinite)&&bounds[0]>=-180&&bounds[2]<=180&&bounds[1]>=-90&&bounds[3]<=90&&bounds[0]<bounds[2]&&bounds[1]<bounds[3];
}
export function validateFeatureService(service) {
  if(!plain(service)||!uuid.test(service.id)||!clean(service.name,80)||!validServiceUrl(service.url)||new URL(service.url).href.includes('?')
    ||!clean(service.title,240)||!Number.isFinite(Date.parse(service.connectedAt))||!Array.isArray(service.collections)||service.collections.length>512)throw new Error('Invalid saved data service.');
  const ids=new Set();
  const hasOverpass=service.overpass!==undefined&&service.overpass!==null;
  const hasWfs=service.wfs!==undefined&&service.wfs!==null;
  if(['arcgis','overpass','wfs'].filter(key=>service[key]!==undefined&&service[key]!==null).length>1)throw new Error('Ambiguous data service protocol.');
  if(hasWfs) {
    const meta=service.wfs;
    if(!plain(meta)||!validOverpassEndpoint(service.url)||meta.version!=='2.0.0'||!digest.test(meta.capabilitiesSha256)||!validWfsText(meta.fees)||!validWfsText(meta.accessConstraints)
      ||typeof meta.pagingSupported!=='boolean'||!Array.isArray(meta.excludedLayers)||service.collections.length+meta.excludedLayers.length>512)throw new Error('Invalid saved WFS connection.');
  }
  if(hasOverpass) {
    const meta=service.overpass;
    if(service.arcgis!==undefined&&service.arcgis!==null||!plain(meta)||!validOverpassEndpoint(service.url)||!clean(meta.generator,256)||!meta.generator.startsWith('Overpass API')||meta.apiVersion!==0.6||!digest.test(meta.metadataSha256)||!validOsmCopyright(meta.copyrightText)
      ||service.collections.length!==Object.keys(OVERPASS_PRESETS).length)throw new Error('Invalid saved Overpass connection.');
  }
  let arcgisRoot;
  if(service.arcgis!==undefined&&service.arcgis!==null) {
    const a=service.arcgis;
    arcgisRoot=arcgisServiceRoot(service.url);
    if(!plain(a)||arcgisRoot!==service.url||!Number.isFinite(a.currentVersion)||a.currentVersion<10||!bytes(a.copyrightText,16384)||!digest.test(a.metadataSha256)||!Array.isArray(a.excludedLayers)||service.collections.length+a.excludedLayers.length>512)throw new Error('Invalid ArcGIS service metadata.');
  }
  for(const c of service.collections) {
    if(!plain(c)||!clean(c.id,160)||ids.has(c.id)||!clean(c.title,240)||!bytes(c.description,16384)||!validServiceUrl(c.itemsUrl)||new URL(c.itemsUrl).origin!==new URL(service.url).origin||!Array.isArray(c.licenseLinks)||c.licenseLinks.length>16||c.licenseLinks.some(raw=>{try{const u=new URL(raw);return !bytes(raw,2048)||!['http:','https:'].includes(u.protocol)||u.username||u.password;}catch{return true;}}))throw new Error('Invalid saved feature collection.');
    ids.add(c.id);
    if(!hasWfs&&c.wfs!==undefined&&c.wfs!==null)throw new Error('Unexpected WFS feature type metadata.');
    if(hasOverpass) {
      if(c.id!==Object.keys(OVERPASS_PRESETS)[ids.size-1]||c.title!==OVERPASS_PRESETS[c.id]||c.description!==OVERPASS_DESCRIPTION||c.itemsUrl!==service.url||c.licenseLinks.length!==1||c.licenseLinks[0]!==OSM_LICENSE_URL||c.arcgis!==undefined&&c.arcgis!==null)throw new Error('Invalid saved OSM category.');
    }else if(hasWfs) {
      validateWfsLayer(c.wfs);
      if(c.id!==c.wfs.typeName||c.itemsUrl!==service.url||c.licenseLinks.length||c.arcgis!==undefined&&c.arcgis!==null||!validWfsText(c.description))throw new Error('Invalid saved WFS feature type.');
    }else if(service.arcgis) {
      if(!validArcgisLayerId(c.id)||c.itemsUrl!==`${arcgisRoot}/${c.id}/query`||c.licenseLinks.length)throw new Error('Invalid ArcGIS query endpoint.');
      validateArcgisLayer(c.arcgis);
    }else if(c.arcgis!==undefined&&c.arcgis!==null)throw new Error('Unexpected ArcGIS layer metadata.');
  }
  for(const excluded of service.arcgis?.excludedLayers??[]) {
    if(!plain(excluded)||!validArcgisLayerId(excluded.id)||ids.has(excluded.id)||!bytes(excluded.name,960)||!clean(excluded.reason,512))throw new Error('Invalid ArcGIS discovery exclusion.');
    ids.add(excluded.id);
  }
  for(const excluded of service.wfs?.excludedLayers??[]) {
    if(!plain(excluded)||!validWfsQName(excluded.id)||ids.has(excluded.id)||!clean(excluded.title,240)||!clean(excluded.reason,500))throw new Error('Invalid WFS discovery exclusion.');
    ids.add(excluded.id);
  }
  return service;
}
export async function featureRequest(operation,payload={},signal) {
  const commands={list:'list_feature_services',connect:'connect_feature_service',forget:'forget_feature_service',query:'query_features'};
  if(!commands[operation])throw new Error('Unknown data service operation.');
  if(operation==='forget'&&!uuid.test(payload.id))throw new Error('Invalid data service identifier.');
  if(operation==='query'&&(!uuid.test(payload.serviceId)||typeof payload.collectionId!=='string'||!validQueryBounds(payload.bounds)))throw new Error('Choose a valid query region and collection.');
  const check=()=>{if(signal?.aborted)throw new DOMException('Data service request aborted','AbortError');};check();
  let value;
  if(desktopAvailable()) {
    try {value=await window.__TAURI__.core.invoke(commands[operation],['connect','query'].includes(operation)?{request:payload}:payload);}
    catch(error) {throw error instanceof Error?error:new Error(typeof error==='string'?error:'Data service request failed.');}
  }
  else {
    const path=operation==='forget'?`/feature-services/${payload.id}/forget`:operation==='query'?'/feature-services/query':'/feature-services';
    const mutation=operation!=='list';
    const response=await fetch('http://127.0.0.1:4318'+path,{method:mutation?'POST':'GET',signal,headers:mutation?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:mutation?JSON.stringify(payload):undefined});
    value=await response.json();if(!response.ok)throw new Error(value.error||`Data service returned HTTP ${response.status}`);
  }
  check();
  if(operation==='list'){if(!Array.isArray(value)||value.length>24)throw new Error('Invalid data service registry.');value.forEach(validateFeatureService);}
  if(operation==='connect')validateFeatureService(value);
  if(operation==='query') {validateVectorAsset(value);if(!value.osmSource&&(!value.remoteSource||value.remoteSource.selection!==(value.remoteSource.wfs?'wfs-bbox-full-features':'bbox-full-features')||value.remoteSource.featureCount!==value.featureCount))throw new Error('Feature extraction has no complete query provenance.');}
  return value;
}
