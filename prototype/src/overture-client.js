import {diagnosticError} from './usage-diagnostics.js';
import {usageRequest} from './usage-events.js';
import {desktopAvailable} from './runtime-client.js';
const validQueryBounds=b=>Array.isArray(b)&&b.length===4&&b.every(Number.isFinite)&&b[0]>=-180&&b[2]<=180&&b[1]>=-90&&b[3]<=90&&b[0]<b[2]&&b[1]<b[3];

export const OVERTURE_PACKAGE_ID='base-bathymetry-2026-09-23.1-part-00000';
export const OVERTURE_ORIGINAL_BYTES=52018294;
export const OVERTURE_ORIGINAL_ROWS=407277;
export const OVERTURE_ORIGINAL_SHA256='cfaf02a15f70713a52ed2f4367858d1b3d3be0dadba2a9aa4ee7faf0267ea23d';
export const OVERTURE_SOURCE_URL='https://overturemaps-us-west-2.s3.us-west-2.amazonaws.com/release/2026-09-23.1/theme=base/type=bathymetry/part-00000-304001d2-9e92-5aeb-8db5-60be5bd09d96-c000.zstd.parquet';
export const OVERTURE_LICENSE_URL='https://opendatacommons.org/licenses/odbl/1-0/';
export const OVERTURE_SELECTION='GeoJSON preview of complete features whose publisher per-row bbox intersects the AOI; no geometry clipping; full GeoParquet original retained separately';
const hash=/^[a-f0-9]{64}$/;
const uuid=/^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/;
const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const integer=(v,min,max)=>Number.isSafeInteger(v)&&v>=min&&v<=max;
const timestamp=v=>typeof v==='string'&&/^\d{4}-\d{2}-\d{2}T/.test(v)&&Number.isFinite(Date.parse(v));
const canonical=v=>Array.isArray(v)?v.map(canonical):plain(v)?Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])])):v;
const same=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
const expectedGeo={version:'1.1.0',primary_column:'geometry',columns:{geometry:{encoding:'WKB',geometry_types:['MultiPolygon','Polygon'],bbox:[-180,-90,180.00001525878906,90],covering:{bbox:{xmin:['bbox','xmin'],ymin:['bbox','ymin'],xmax:['bbox','xmax'],ymax:['bbox','ymax']}}}}};
const provenanceKeys=['conversion','datasetId','packageId','originalUrl','originalSha256','originalBytes','originalRows','rowGroups','geoMetadata','schema','maxRowGroupUncompressedBytes','requestedBounds','previewFeatureCount','selection','fetchedAt','attribution','licenseUrl'];
export function validateOvertureProvenance(p,asset) {
  if(!plain(p)||Object.keys(p).some(k=>!provenanceKeys.includes(k))||provenanceKeys.some(k=>!Object.hasOwn(p,k))
    ||p.conversion!==1||p.datasetId!=='overture-maps'||p.packageId!==OVERTURE_PACKAGE_ID||p.originalUrl!==OVERTURE_SOURCE_URL
    ||p.originalSha256!==OVERTURE_ORIGINAL_SHA256||p.originalBytes!==OVERTURE_ORIGINAL_BYTES||p.originalRows!==OVERTURE_ORIGINAL_ROWS
    ||p.rowGroups!==16||p.maxRowGroupUncompressedBytes!==9942868||!same(p.geoMetadata,expectedGeo)
    ||typeof p.schema!=='string'||!p.schema.length||p.schema.length>65536||!validQueryBounds(p.requestedBounds)
    ||!integer(p.previewFeatureCount,0,50000)||p.selection!==OVERTURE_SELECTION||!timestamp(p.fetchedAt)
    ||typeof p.attribution!=='string'||!p.attribution.includes('Overture Maps Foundation')||!p.attribution.includes('ETOPO/GLOBathy')
    ||p.licenseUrl!==OVERTURE_LICENSE_URL)throw new Error('Invalid retained Overture GeoParquet original provenance.');
  if(asset&&(!uuid.test(asset.id)||asset.format!=='geoparquet-preview'||asset.storageMode!=='managed'||asset.crs!=='EPSG:4326'
    ||asset.featureCount!==p.previewFeatureCount||!integer(asset.bytes,1,20*1024*1024)||!integer(asset.coordinateCount,0,500000)
    ||!hash.test(asset.sourceSha256)||!hash.test(asset.geojsonSha256)||asset.licenseUrl!==p.licenseUrl
    ||asset.attribution!==p.attribution||asset.dataTimestamp!==null||asset.officialPackage||asset.remoteSource||asset.osmSource
    ||asset.osmConversion||asset.localOsm||asset.shapefile||asset.geoPackage))throw new Error('Overture preview is not a complete original GeoJSON file.');
  return p;
}
export function validateOvertureInspection(data) {
  if(!plain(data)||!plain(data.asset)||!plain(data.geojson)||data.geojson.type!=='FeatureCollection'||!Array.isArray(data.geojson.features))throw new Error('Invalid Overture preview inspection.');
  const p=validateOvertureProvenance(data.asset.geoParquet,data.asset);
  if(data.geojson.features.length!==p.previewFeatureCount)throw new Error('Overture selected feature count changed.');
  let previous=-1;
  for(const f of data.geojson.features) {
    if(f.type!=='Feature'||!plain(f.properties)||f.properties.id!==f.id||!integer(f.geodOriginalRow,0,p.originalRows-1)
      ||f.geodOriginalRow<=previous||!hash.test(f.geodOriginalWkbSha256))throw new Error('Overture original row or geometry identity changed.');
    const b=f.properties.bbox,a=p.requestedBounds;
    if(!plain(b)||!['xmin','xmax','ymin','ymax'].every(k=>Number.isFinite(b[k]))||b.xmin>b.xmax||b.ymin>b.ymax
      ||b.xmin>a[2]||b.xmax<a[0]||b.ymin>a[3]||b.ymax<a[1])throw new Error('Overture preview does not match the selected map area.');
    previous=f.geodOriginalRow;
  }
  return data;
}
function validateRequest(operation,payload) {
  if(['search','download'].includes(operation)&&(!plain(payload)||Object.keys(payload).some(k=>!['datasetId','packageId','bounds'].includes(k))
    ||payload.datasetId!=='overture-maps'||!validQueryBounds(payload.bounds)||payload.packageId!==undefined&&payload.packageId!==OVERTURE_PACKAGE_ID
    ||operation==='download'&&payload.packageId!==OVERTURE_PACKAGE_ID))throw new Error('Choose the reviewed Overture original and the current Explore area.');
  if(['metadata','rows'].includes(operation)&&(!uuid.test(payload.id)||operation==='rows'&&(!integer(payload.offset??0,0,OVERTURE_ORIGINAL_ROWS)||!integer(payload.limit??5,1,20))))throw new Error('Invalid original Overture row request.');
}
export async function overtureRequest(operation,payload={},signal) {
  return usageRequest('overture', operation, payload, () => performOvertureRequest(operation, payload, signal));
}
async function performOvertureRequest(operation,payload={},signal) {
  const commands={providers:'list_official_parquet',search:'search_official_parquet',download:'download_official_parquet',metadata:'parquet_original_metadata',rows:'parquet_original_rows'};
  if(!commands[operation])throw new Error('Unknown Overture operation.');
  validateRequest(operation,payload);if(signal?.aborted)throw new DOMException('Overture request aborted','AbortError');
  let value;
  if(desktopAvailable()) {
    const args=['search','download'].includes(operation)?{request:payload}:operation==='rows'?{id:payload.id,args:{offset:payload.offset??0,limit:payload.limit??5}}:payload;
    try {value=await window.__TAURI__.core.invoke(commands[operation],args);}catch(e){throw e instanceof Error?e:new Error(String(e));}
  }else {
    const path=['metadata','rows'].includes(operation)?`/vectors/${payload.id}/parquet/${operation}${operation==='rows'?`?offset=${payload.offset??0}&limit=${payload.limit??5}`:''}`:`/official-parquet${operation==='providers'?'':'/'+operation}`;
    const mutation=['search','download'].includes(operation);
    const response=await fetch('http://127.0.0.1:4318'+path,{method:mutation?'POST':'GET',signal,
      headers:mutation?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:mutation?JSON.stringify(payload):undefined});
    value=await response.json();if(!response.ok)throw diagnosticError(new Error(value.error||`Overture service returned HTTP ${response.status}`),{stage:'service_request',httpStatus:String(response.status),diagnosticOrigin:'explicit'});
  }
  if(signal?.aborted)throw new DOMException('Overture request aborted','AbortError');
  if(operation==='search') {
    if(value.datasetId!=='overture-maps'||!same(value.bounds,payload.bounds)||value.complete!==true||!Array.isArray(value.items)||value.items.length!==1
      ||value.items[0].id!==OVERTURE_PACKAGE_ID||value.items[0].url!==OVERTURE_SOURCE_URL||value.items[0].expectedSha256!==OVERTURE_ORIGINAL_SHA256
      ||value.items[0].originalRows!==OVERTURE_ORIGINAL_ROWS||value.items[0].originalBytes!==OVERTURE_ORIGINAL_BYTES)throw new Error('Overture original package identity changed.');
  }
  if(operation==='download') {validateOvertureProvenance(value.geoParquet,value);if(!same(value.geoParquet.requestedBounds,payload.bounds))throw new Error('Overture download returned a different preview area.');}
  if(operation==='metadata') {
    if(value.assetId!==payload.id||value.originalVerified!==true||value.previewSubset!==true)throw new Error('Original GeoParquet metadata is not verified.');
    validateOvertureProvenance(value.original);
  }
  if(operation==='rows') {
    if(value.assetId!==payload.id||value.originalVerified!==true||value.originalSha256!==OVERTURE_ORIGINAL_SHA256||value.originalRows!==OVERTURE_ORIGINAL_ROWS
      ||value.offset!==(payload.offset??0)||!Array.isArray(value.rows)||value.rows.length>(payload.limit??5)
      ||value.complete!==(value.offset+value.rows.length>=OVERTURE_ORIGINAL_ROWS)
      ||value.nextOffset!==(value.complete?null:value.offset+value.rows.length)
      ||new TextEncoder().encode(JSON.stringify(value)).length>24*1024
      ||value.rows.some((r,i)=>r.originalRow!==value.offset+i||!plain(r.properties)||r.geometryEncoding!=='WKB'||r.geometryOmitted!==true
        ||!integer(r.geometryBytes,5,32*1024*1024)||!hash.test(r.geometryWkbSha256)))throw new Error('Invalid bounded original Overture property page.');
  }
  return value;
}
export async function exportOvertureOriginal(asset) {
  validateOvertureProvenance(asset.geoParquet,asset);
  if(desktopAvailable())return window.__TAURI__.core.invoke('export_vector',{id:asset.id,original:true});
  const response=await fetch(`http://127.0.0.1:4318/vectors/${asset.id}/parquet/original`,{signal:AbortSignal.timeout(300000)});
  if(!response.ok)throw diagnosticError(new Error((await response.json()).error||'Original GeoParquet export failed.'),{stage:'service_request',httpStatus:String(response.status),diagnosticOrigin:'explicit'});
  if(response.headers.get('content-type')!=='application/vnd.apache.parquet')throw new Error('Original GeoParquet file format changed.');
  const bytes=new Uint8Array(await response.arrayBuffer());
  const sha=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
  if(bytes.length!==OVERTURE_ORIGINAL_BYTES||sha!==OVERTURE_ORIGINAL_SHA256)throw new Error('Complete original GeoParquet export checksum changed.');
  const url=URL.createObjectURL(new Blob([bytes],{type:'application/vnd.apache.parquet'})),a=document.createElement('a');
  a.href=url;a.download='Overture-2026-09-23.1-base-bathymetry-part-00000.parquet';document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);return true;
}
