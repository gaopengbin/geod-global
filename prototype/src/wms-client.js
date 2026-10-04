import {desktopAvailable} from './runtime-client.js';
import {validQueryBounds,validServiceUrl} from './features-client.js';
import {validateWmtsService,validateWmtsImage} from './wmts-client.js';
import {validateArcgisMapService,validateArcgisMapImage} from './arcgis-map-client.js';
import {validateXyzService,validateXyzImage} from './xyz-client.js';
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const digest=/^[a-f0-9]{64}$/;
const endpoint=u=>validServiceUrl(u)&&!new URL(u).search;
export function validateMapService(s) {
  if([s?.xyz,s?.arcgis,s?.wmts].filter(Boolean).length>1)throw new Error('Saved map service contains conflicting protocols');
  if(s?.xyz)return validateXyzService(s);
  if(s?.arcgis)return validateArcgisMapService(s);
  if(s?.wmts)return validateWmtsService(s);
  if(!s||!uuid.test(s.id)||typeof s.name!=='string'||!s.name.trim()||!endpoint(s.url)||!endpoint(s.mapUrl)||new URL(s.url).origin!==new URL(s.mapUrl).origin||!['1.3.0','1.1.1'].includes(s.version)||!digest.test(s.capabilitiesSha256)||!Array.isArray(s.layers)||!s.layers.length||s.layers.length>4096||![s.maxWidth,s.maxHeight].every(n=>Number.isInteger(n)&&n>0&&n<=2048)||!Number.isFinite(Date.parse(s.connectedAt)))throw new Error('Invalid saved WMS service.');
  const seen=new Set();for(const l of s.layers){if(!l||typeof l.name!=='string'||!l.name||seen.has(l.name)||typeof l.title!=='string'||!['EPSG:4326','CRS:84'].includes(l.crs)||!Array.isArray(l.styles)||l.styles.some(v=>typeof v!=='string')||(l.bounds&&!validQueryBounds(l.bounds))||(l.time&&(typeof l.time.values!=='string'||!l.time.values)))throw new Error('Invalid WMS layer metadata.');seen.add(l.name);}return s;
}
export function validateMapImage(a) {
  if([a?.source?.xyz,a?.source?.arcgis,a?.source?.wmts].filter(Boolean).length>1)throw new Error('Saved map image contains conflicting protocols');
  if(a?.source?.xyz)return validateXyzImage(a);
  if(a?.source?.arcgis)return validateArcgisMapImage(a);
  if(a?.source?.wmts)return validateWmtsImage(a);
  if(a?.imageExtent)throw new Error('Unexpected WMS image extent.');
  const s=a?.source;
  if(!a||!uuid.test(a.id)||typeof a.name!=='string'||!validQueryBounds(a.bounds)||a.crs!=='EPSG:4326'||![a.width,a.height].every(n=>Number.isInteger(n)&&n>0&&n<=2048)||!Number.isInteger(a.bytes)||a.bytes<=0||a.bytes>16*1024*1024||!digest.test(a.sha256)||!s||!endpoint(s.serviceUrl)||!endpoint(s.mapEndpoint)||new URL(s.serviceUrl).origin!==new URL(s.mapEndpoint).origin||!['1.3.0','1.1.1'].includes(s.version)||!['EPSG:4326','CRS:84'].includes(s.requestCrs)||s.selection!=='bbox-rendered-map'||!digest.test(s.capabilitiesSha256)||typeof s.layerName!=='string'||typeof s.layerTitle!=='string'||typeof s.style!=='string'||!Number.isFinite(Date.parse(s.requestedAt)))throw new Error('Invalid saved map image.');
  const u=new URL(s.mapEndpoint),q=u.searchParams;
  for(const [k,v] of Object.entries({SERVICE:'WMS',VERSION:s.version,REQUEST:'GetMap',LAYERS:s.layerName,STYLES:s.style,[s.version==='1.3.0'?'CRS':'SRS']:s.requestCrs,BBOX:(s.version==='1.3.0'&&s.requestCrs==='EPSG:4326'?[a.bounds[1],a.bounds[0],a.bounds[3],a.bounds[2]]:a.bounds).join(','),WIDTH:String(a.width),HEIGHT:String(a.height),FORMAT:'image/png',TRANSPARENT:'TRUE'}))q.set(k,v);
  if(s.time!==null&&s.time!==undefined)q.set('TIME',s.time);
  const actual=new URL(s.requestUrl),pairs=[...actual.searchParams],expected=[...q];
  const sameValue=(key,value)=>key==='BBOX'?value.split(',').length===4&&value.split(',').every((n,i)=>Number(n)===Number(q.get(key).split(',')[i])):value===q.get(key);
  if(actual.origin!==u.origin||actual.pathname!==u.pathname||actual.hash||actual.username||actual.password||pairs.length!==expected.length||new Set(pairs.map(([k])=>k)).size!==pairs.length||pairs.some(([k,v])=>!q.has(k)||!sameValue(k,v)))throw new Error('Map request does not match the saved image grid.');return a;
}
export async function mapRequest(operation,payload={},signal) {
  const commands={services:'list_map_services',connect:'connect_map_service',forget:'forget_map_service',list:'list_map_images',get:'get_map_image',inspect:'inspect_map_image'};
  if(!commands[operation])throw new Error('Unknown map service operation.');
  if(['forget','inspect'].includes(operation)&&!uuid.test(payload.id))throw new Error('Invalid map identifier.');
  if(operation==='get'&&(!uuid.test(payload.serviceId)||!validQueryBounds(payload.bounds)))throw new Error('Choose a valid map service and region.');
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');let value;
  if(desktopAvailable()) {try{value=await window.__TAURI__.core.invoke(commands[operation],['connect','get'].includes(operation)?{request:payload}:payload);}catch(e){throw e instanceof Error?e:new Error(typeof e==='string'?e:'Map request failed.');}}
  else {const path=operation==='services'||operation==='connect'?'/map-services':operation==='forget'?`/map-services/${payload.id}/forget`:operation==='inspect'?`/map-images/${payload.id}`:'/map-images';const mutation=['connect','forget','get'].includes(operation);const r=await fetch('http://127.0.0.1:4318'+path,{method:mutation?'POST':'GET',signal,headers:mutation?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:mutation?JSON.stringify(payload):undefined});value=await r.json();if(!r.ok)throw new Error(value.error||`Map service returned HTTP ${r.status}`);}
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');
  if(operation==='services'){if(!Array.isArray(value)||value.length>24)throw new Error('Invalid map service registry.');value.forEach(validateMapService);}
  if(operation==='connect')validateMapService(value);
  if(operation==='list'){if(!Array.isArray(value)||value.length>512)throw new Error('Invalid map image registry.');value.forEach(validateMapImage);}
  if(operation==='get')validateMapImage(value);
  if(operation==='inspect'){validateMapImage(value?.asset);if(typeof value.imageUrl!=='string'||!value.imageUrl.startsWith('data:image/png;base64,')||value.imageUrl.length>23*1024*1024)throw new Error('Invalid local map image pixels.');}return value;
}
export async function exportMapImage(id) {
  if(!uuid.test(id))throw new Error('Invalid map identifier.');
  if(desktopAvailable()){try{return await window.__TAURI__.core.invoke('export_map_image',{id});}catch(e){throw e instanceof Error?e:new Error(String(e));}}
  const response=await fetch(`http://127.0.0.1:4318/map-images/${id}/export`);if(!response.ok){const value=await response.json();throw new Error(value.error||'Map export failed.');}
  if(response.headers.get('content-type')!=='application/zip')throw new Error('Map export did not return a ZIP package.');
  const blob=await response.blob(),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=`geod-map-${id}.zip`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);return true;
}
export function mapSize(bounds,edge,maxWidth=2048,maxHeight=2048) {
  if(!validQueryBounds(bounds))return null;const dx=bounds[2]-bounds[0],dy=bounds[3]-bounds[1];
  const scale=Math.min(edge/Math.max(dx,dy),maxWidth/dx,maxHeight/dy);return [Math.max(1,Math.round(dx*scale)),Math.max(1,Math.round(dy*scale))];
}
