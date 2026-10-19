import {usageRequest} from './usage-events.js';
import {desktopAvailable} from './runtime-client.js';
import {validQueryBounds} from './features-client.js';
import {validateStacInspection,validateStacPixel} from './stac-client.js';

const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const hash=/^[0-9a-f]{64}$/;
const max=32*1024*1024;
const kind='service-generated-regional-numerical-subset';
const fail=message=>{throw new Error(message);};
const sameBounds=(a,b)=>Array.isArray(a)&&Array.isArray(b)&&a.length===4&&b.length===4&&a.every((v,i)=>v===b[i]);
export function validateGebcoRequest(value) {
  if(!value||Object.keys(value).sort().join(',')!=='bounds,name'||typeof value.name!=='string'||!value.name.trim()||value.name!==value.name.trim()||value.name.length>120||/[\u0000-\u001f]/.test(value.name)||!validQueryBounds(value.bounds))fail('Enter a subset name and choose a valid region.');
  const [w,s,e,n]=value.bounds,cols=Math.ceil((e-w)*240),rows=Math.ceil((n-s)*240);
  if(cols<1||rows<1||cols>10000||rows>10000||(cols+2)*(rows+2)*2>max-1024*1024)fail('Choose a smaller GEBCO region.');
  return value;
}
export function validateGebcoRecord(r) {
  const phases=['official_metadata_verified','official_queue_submitted','official_new','official_processing','official_finished','whole_zip_downloaded','whole_zip_crc_and_numerical_tiff_verified'];
  if(!r||!uuid.test(r.id)||typeof r.name!=='string'||r.name.length>120||!Number.isFinite(Date.parse(r.createdAt))||!validQueryBounds(r.bounds)
    ||r.gridId!==1||r.gridName!=='gebco_2026_global'||r.dataSourceId!==1||r.dataSourceName!=='gebco_2026'||r.formatId!==2||r.resolutionArcSeconds!==15
    ||r.provenanceKind!==kind||r.publisherStaticWholeOriginal!==false||r.generationStatus!=='finished'||r.verificationStatus!=='verified'
    ||!/^GEBCO_[A-Za-z0-9_]{14,94}$/.test(r.basketId)||!/^gebco_2026_n[^_]+_s[^_]+_w[^_]+_e[^_]+_geotiff\.tif$/.test(r.memberName)
    ||[r.bytes,r.archiveBytes].some(v=>!Number.isSafeInteger(v)||v<1||v>max)||[r.sha256,r.archiveSha256,r.requestSha256,r.gridsSha256,r.formatsSha256,r.officialRequestSha256,r.queueResponseSha256].some(v=>!hash.test(v))
    ||typeof r.attribution!=='string'||!Array.isArray(r.limitations)||r.limitations.length>32||r.limitations.some(v=>typeof v!=='string'||v.length>4096)
    ||!Array.isArray(r.stages)||!r.stages.length||r.stages.length>40||r.stages.some(v=>!phases.includes(v.stage)||!Number.isFinite(Date.parse(v.at))))fail('Invalid GEBCO numerical subset record.');
  validateGebcoRequest({name:r.name,bounds:r.bounds});
  const match=r.memberName.match(/^gebco_2026_n([^_]+)_s([^_]+)_w([^_]+)_e([^_]+)_geotiff\.tif$/);
  if(!sameBounds([Number(match[3]),Number(match[2]),Number(match[4]),Number(match[1])],r.bounds))fail('The GEBCO file does not match its requested region.');
  return r;
}
function inspection(value,id,sha) {
  validateStacInspection(value,id);
  const t=value.transform;
  if(value.crs!=='EPSG:4326'||value.width>10000||value.height>10000||value.bands.length!==1||value.bands[0].dataType!=='Int16'||value.bands[0].nodata!==-32767||value.pixelInterpretation!=='area'
    ||!t||Math.abs(t[0]-1/240)>1e-10||Math.abs(t[4]+1/240)>1e-10||t[1]!==0||t[3]!==0||sha&&value.sha256!==sha)fail('The GEBCO numerical grid or checksum is invalid.');
  return value;
}
export async function gebcoRequest(operation,payload={},signal) {
  return usageRequest('gebco', operation, payload, () => performGebcoRequest(operation, payload, signal));
}
async function performGebcoRequest(operation,payload={},signal) {
  const commands={grids:'gebco_grids',list:'gebco_list',get:'gebco_get',create:'gebco_create_subset',inspect:'gebco_inspect',pixel:'gebco_pixel'};
  if(!commands[operation])fail('Unknown GEBCO operation.');
  if(operation==='create')validateGebcoRequest(payload);
  if(['get','inspect','pixel'].includes(operation)&&!uuid.test(payload.id))fail('Invalid GEBCO subset identifier.');
  if(operation==='pixel'&&(!Number.isSafeInteger(payload.column)||payload.column<0||!Number.isSafeInteger(payload.row)||payload.row<0))fail('Enter a nonnegative pixel column and row.');
  const check=()=>{if(signal?.aborted)throw new DOMException('GEBCO request cancelled','AbortError');};check();
  let value;
  if(desktopAvailable()) {
    try{value=await window.__TAURI__.core.invoke(commands[operation],['create','pixel'].includes(operation)?{request:payload}:payload);}
    catch(e){throw new Error(String(e?.message||e));}
  }else {
    const paths={grids:'/gebco/grids',list:'/gebco/subsets',create:'/gebco/subsets',get:`/gebco/subsets/${payload.id}`,inspect:`/gebco/subsets/${payload.id}/inspect`,pixel:`/gebco/subsets/${payload.id}/pixel`};
    const post=['create','pixel'].includes(operation);
    const body=operation==='pixel'?{column:payload.column,row:payload.row}:payload;
    const r=await fetch('http://127.0.0.1:4318'+paths[operation],{method:post?'POST':'GET',signal,headers:post?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:post?JSON.stringify(body):undefined});
    value=await r.json();if(!r.ok)fail(value.error||'GEBCO request failed.');
  }
  check();
  if(operation==='grids') {
    const g=value?.grid,f=value?.format;
    if(!Number.isFinite(Date.parse(value?.fetchedAt))||g?.id!==1||g.name!=='gebco_2026_global'||g.projection!=='EPSG:4326'||!g.data_sources?.some(s=>s.id===1&&s.name==='gebco_2026'&&s.z_name==='elevation'&&s.display_name==='Bathymetry')||f?.id!==2||f.name!=='geotiff'||f.category!=='Data')fail('The official GEBCO 2026 numerical grid is unavailable.');
  }
  if(operation==='list'){if(!Array.isArray(value)||value.length>1000||new Set(value.map(v=>v.id)).size!==value.length)fail('Invalid GEBCO numerical subset library.');value.forEach(validateGebcoRecord);}
  if(operation==='get'){validateGebcoRecord(value);if(value.id!==payload.id)fail('The GEBCO record does not match the requested subset.');}
  if(operation==='create'){validateGebcoRecord(value?.record);if(value.record.name!==payload.name||!sameBounds(value.record.bounds,payload.bounds))fail('The GEBCO response belongs to a different region.');inspection(value.raster,value.record.id,value.record.sha256);}
  if(operation==='inspect')inspection(value,payload.id);
  if(operation==='pixel'){validateStacPixel(value,payload);if(value.values.length!==1||typeof value.values[0]!=='number'||!Number.isInteger(value.values[0])||value.values[0]<-32768||value.values[0]>32767)fail('Invalid GEBCO signed elevation sample.');}
  return value;
}
export async function saveGebcoFile(id,archive=false) {
  if(!uuid.test(id))fail('Invalid GEBCO subset identifier.');
  if(desktopAvailable())return window.__TAURI__.core.invoke(archive?'gebco_export_archive':'gebco_export_source',{id});
  const record=await gebcoRequest('get',{id});
  const r=await fetch(`http://127.0.0.1:4318/gebco/subsets/${id}/${archive?'archive':'source'}`);
  if(!r.ok)fail('Could not read the verified GEBCO subset.');
  const blob=await r.blob();if(!blob.size||blob.size>max||blob.size!==(archive?record.archiveBytes:record.bytes))fail('Invalid GEBCO subset file size.');
  const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',await blob.arrayBuffer()))].map(v=>v.toString(16).padStart(2,'0')).join('');
  if(digest!==(archive?record.archiveSha256:record.sha256))fail('The GEBCO export checksum changed.');
  const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=archive?'gebco-2026-official-subset.zip':'gebco-2026-numerical-subset.tif';a.click();setTimeout(()=>URL.revokeObjectURL(url),60000);return true;
}
