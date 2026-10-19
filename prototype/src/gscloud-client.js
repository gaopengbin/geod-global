import {desktopAvailable} from './runtime-client.js';
import {validQueryBounds} from './features-client.js';

const identifier=id=>typeof id==='string'&&/^[\w()+.\-]{1,180}$/.test(id);
const fail=message=>{throw new Error(message);};
const date=value=>typeof value==='string'&&/^\d{4}-\d{2}-\d{2}$/.test(value)&&Number.isFinite(Date.parse(value))&&new Date(value).toISOString().slice(0,10)===value;
export function validateGSCloudSearch(request) {
  if(!request||Object.keys(request).some(k=>!['productId','bounds','start','end','cloudMax','presence','page','pageSize'].includes(k))
    ||!identifier(request.productId)||!validQueryBounds(request.bounds)||!date(request.start)||!date(request.end)||request.start>request.end
    ||!Number.isInteger(request.page)||request.page<1||request.page>10000||!Number.isInteger(request.pageSize)||request.pageSize<1||request.pageSize>20
    ||!['all','has','none'].includes(request.presence)||request.cloudMax!=null&&(!Number.isFinite(request.cloudMax)||request.cloudMax<0||request.cloudMax>100))fail('Choose a GSCloud product, valid area, dates and page.');
  return request;
}
function item(value,productId) {
  if(!identifier(value?.id)||value.productId!==productId||!['available','reservation','unknown'].includes(value.presence)
    ||value.downloadStatus!==(value.presence==='reservation'?'reservation-required':'account-required')
    ||value.datetime!=null&&!Number.isFinite(Date.parse(value.datetime))
    ||value.cloudCover!=null&&(!Number.isFinite(value.cloudCover)||value.cloudCover<0||value.cloudCover>100)
    ||value.bytes!=null&&(!Number.isFinite(value.bytes)||value.bytes<=0))fail('Invalid GSCloud item metadata.');
  if(value.geometry!=null||value.bbox!=null){
    if(!validQueryBounds(value.bbox)||!['Polygon','MultiPolygon'].includes(value.geometry?.type)||!Array.isArray(value.geometry.coordinates))fail('Invalid GSCloud footprint.');
  }
  return value;
}
export function validateGSCloudResponse(operation,value,payload) {
  if(!value||value.downloadsStarted!==false||!Number.isFinite(Date.parse(value.retrievedAt))||!/^[a-f0-9]{64}$/.test(value.metadataSha256))fail('Invalid GSCloud metadata receipt.');
  if(operation==='products'){
    if(value.source!=='https://www.gscloud.cn/geoagent'||value.downloadCapability!=='official-account-required'||!Array.isArray(value.products)||!value.products.length||value.products.length>500||new Set(value.products.map(p=>p.id)).size!==value.products.length
      ||value.products.some(p=>!identifier(p.id)||typeof p.title!=='string'||!p.title||typeof p.seriesId!=='string'||!p.seriesId))fail('Invalid GSCloud product directory.');
  }else if(operation==='search'){
    const r=value.request;
    if(!r||['productId','start','end','cloudMax','page','pageSize','presence'].some(k=>(r[k]??null)!==(payload[k]??null))||r.bounds?.join(',')!==payload.bounds.join(',')
      ||value.product?.id!==payload.productId||!Number.isSafeInteger(value.total)||value.total<0||!Number.isSafeInteger(value.totalPages)||value.totalPages<0
      ||value.hasMore!==(payload.page<value.totalPages)||!Array.isArray(value.items)||value.items.length>payload.pageSize||value.total<value.items.length
      ||new Set(value.items.map(v=>v.id)).size!==value.items.length)fail('GSCloud returned a different result page.');
    value.items.forEach(v=>item(v,payload.productId));
  }else if(operation==='detail'){
    item(value.item,payload.productId);if(value.item.id!==payload.itemId)fail('GSCloud returned a different item.');
  }
  return value;
}
export async function gscloudRequest(operation,payload={},signal) {
  const commands={products:'gscloud_products',search:'gscloud_search',detail:'gscloud_detail'};
  if(!commands[operation])fail('Unknown GSCloud operation.');
  if(operation==='search')validateGSCloudSearch(payload);
  if(operation==='detail'&&(!identifier(payload.productId)||!identifier(payload.itemId)||Object.keys(payload).some(k=>!['productId','itemId'].includes(k))))fail('Invalid GSCloud item identifier.');
  const check=()=>{if(signal?.aborted)throw new DOMException('GSCloud request cancelled','AbortError');};check();
  let value;
  if(desktopAvailable()) {
    try{value=await window.__TAURI__.core.invoke(commands[operation],operation==='products'?{}:{request:payload});}catch(e){fail(String(e?.message||e));}
  }else{
    const post=operation!=='products';const response=await fetch(`http://127.0.0.1:4318/gscloud/${operation}`,{method:post?'POST':'GET',signal,
      headers:post?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:post?JSON.stringify(payload):undefined});
    try{value=await response.json();}catch{fail('GSCloud local runtime needs an update. Restart the desktop app or development runtime.');}
    if(!response.ok)fail(value.error||'GSCloud metadata request failed.');
  }
  check();return validateGSCloudResponse(operation,value,payload);
}
// The official website handles credentials. Native commands open the website
// and verify its session; no frontend cookie/password payload is accepted.
export async function gscloudAuth(operation,payload={}) {
  if(!desktopAvailable())fail('Connect GSCloud accounts in the desktop app. Public search remains available here.');
  const commands={status:'gscloud_auth_status',begin:'gscloud_login_begin',finish:'gscloud_login_finish',cancel:'gscloud_login_cancel',logout:'gscloud_logout'};
  if(!commands[operation])fail('Unknown GSCloud account operation.');
  if(!payload||Object.keys(payload).length)fail('GSCloud website sign-in does not accept credentials from the application.');
  let value;try{value=await window.__TAURI__.core.invoke(commands[operation],{});}catch(e){fail(String(e?.message||e));}
  const states={begin:['pending'],cancel:['cancelled'],finish:['connected'],logout:['disconnected'],status:['saved','disconnected']};
  if(!states[operation].includes(value?.status)||value.verified!==(operation==='finish')||value.downloadVerified!==false)fail('Invalid GSCloud account status.');
  return value;
}

export const GSCloudOriginalProducts=Object.freeze({
  '305':{key:'gscloud_srtm90',prefix:'srtm_',maxBytes:64*1024*1024,format:'IMG'},
  '307':{key:'gscloud_slope90',prefix:'slope_',maxBytes:128*1024*1024,format:'IMG'},
  '308':{key:'gscloud_tpi90',prefix:'tpi_',maxBytes:64*1024*1024,format:'IMG'},
  '310':{key:'gscloud_aster30v1',prefix:'ASTGTM_',maxBytes:64*1024*1024,format:'IMG'},
  '421':{key:'gscloud_aster30v2',prefix:'ASTGTM2_',maxBytes:64*1024*1024,format:'GeoTIFF'},
  'aeab8000652a45b38afbb7ff023ddabb':{key:'gscloud_aster30v3',prefix:'ASTGTMV003_',maxBytes:64*1024*1024,format:'GeoTIFF'},
});
export function originalTile(productId,id) {
  const p=GSCloudOriginalProducts[productId];if(!p||typeof id!=='string'||!id.startsWith(p.prefix))return false;
  const tile=id.slice(p.prefix.length);
  if(p.prefix.startsWith('ASTGTM')){
    if(!/^[NS]\d{2}[EW]\d{3}$/.test(tile))return false;
    const lat=Number(tile.slice(1,3))*(tile[0]==='S'?-1:1),lon=Number(tile.slice(4))*(tile[3]==='W'?-1:1);
    return lat>=-90&&lat<90&&lon>=-180&&lon<180&&!(tile[0]==='S'&&lat===0)&&!(tile[3]==='W'&&lon===0);
  }
  if(!/^\d{2}_\d{2}$/.test(tile))return false;
  const [x,y]=tile.split('_').map(Number);return x>=1&&x<=72&&y>=1&&y<=24;
}
export function validateGSCloudOriginalJob(job) {
  const productId=Object.keys(GSCloudOriginalProducts).find(id=>GSCloudOriginalProducts[id].key===job?.assetKey),product=GSCloudOriginalProducts[productId];
  if(!job||!product||!/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(job.id)||job.kind!=='download'||!originalTile(productId,job.itemId)
    ||job.href!==`https://bjdl.gscloud.cn/sources/download/${productId}/${job.itemId}`||job.mediaType!=='application/zip'
    ||!['queued','running','succeeded','failed','cancelled','interrupted'].includes(job.status)
    ||!Number.isSafeInteger(job.bytesDownloaded)||job.bytesDownloaded<0||job.bytesDownloaded>product.maxBytes
    ||job.totalBytes!=null&&(!Number.isSafeInteger(job.totalBytes)||job.totalBytes<=0||job.totalBytes>product.maxBytes||job.bytesDownloaded>job.totalBytes)
    ||job.status==='succeeded'&&(!/^[a-f0-9]{64}$/.test(job.sha256)||job.bytesDownloaded!==job.totalBytes))fail('Invalid GSCloud original task.');
  return job;
}
export async function downloadGSCloudOriginal(request) {
  if(!desktopAvailable())fail('Connect GSCloud accounts in the desktop app. Public search remains available here.');
  if(!request||Object.keys(request).some(k=>!['productId','itemId'].includes(k))||!originalTile(request.productId,request.itemId))fail('Select an available original from a supported GSCloud product.');
  try{return validateGSCloudOriginalJob(await window.__TAURI__.core.invoke('gscloud_download_original',{request}));}
  catch(e){fail(String(e?.message||e));}
}

export function validateGSCloudArchive(value,id) {
  const product=GSCloudOriginalProducts[value?.productId];
  if(!product||value?.jobId!==id||value.kind!=='archive'||value.verified!==true||value.format!=='Original ZIP'||value.rasterPreview!==false
    ||!originalTile(value.productId,value.itemId)||!/^[a-f0-9]{64}$/.test(value.sha256??'')
    ||!Number.isSafeInteger(value.bytes)||value.bytes<=0||value.bytes>product.maxBytes
    ||!Array.isArray(value.members)||value.members.length<1||value.members.length>3
    ||new Set(value.members.map(member=>member.name)).size!==value.members.length
    ||value.members.some(member=>typeof member.name!=='string'||!/^[-A-Za-z0-9_.]+$/.test(member.name)||member.name.includes('..')
      ||!Number.isSafeInteger(member.bytes)||member.bytes<=0||member.bytes>256*1024*1024||!/^[a-f0-9]{8}$/.test(member.crc32??'')))fail('Invalid GSCloud archive verification.');
  return value;
}
export async function inspectGSCloudOriginal(id) {
  if(!desktopAvailable())fail('Open original archive results in the desktop app.');
  if(!/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(id))fail('Invalid GSCloud original task.');
  try{return validateGSCloudArchive(await window.__TAURI__.core.invoke('gscloud_inspect_original',{id}),id);}
  catch(error){fail(String(error?.message||error));}
}
