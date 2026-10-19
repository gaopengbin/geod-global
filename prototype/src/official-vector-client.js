import {usageRequest} from './usage-events.js';
import {desktopAvailable} from './runtime-client.js';import {validQueryBounds} from './features-client.js';
import ms from '../../crates/geod-runtime/src/vector/official_packages/ms-buildings-reviewed-index.json' with {type:'json'};
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b),hash=/^[a-f0-9]{64}$/;
const fail=m=>{throw new Error(m);};
const packageKeys=['datasetId','id','title','format','url','fileName','release','bounds','sourceIndexUrl','sourceIndexSha256','publisherSize','expectedSha256','documentationUrl','attribution','licenseUrl','dataTimestamp','warnings'];
const msAttribution='Microsoft Global ML Building Footprints; retain Microsoft, the selected country, LOD9 quadkey, original URL, pinned 2026-08-13 index and CDLA Permissive 2.0 notice.';
const geoAttribution='© OpenStreetMap contributors; Geofabrik GmbH regional free GIS-format extract. Retain ODbL 1.0, original bundle README, source timestamp and Geofabrik format documentation.';
const rows=new Map(ms.rows.map(row=>[`${row[0]}-${row[1]}`,row]));
const quadBounds=key=>{let x=0,y=0;for(let i=0;i<key.length;i++){const mask=1<<(key.length-i-1),digit=Number(key[i]);if(digit===1||digit===3)x|=mask;if(digit===2||digit===3)y|=mask;}const n=2**key.length,lat=v=>Math.atan(Math.sinh(Math.PI*(1-2*v/n)))*180/Math.PI;return[x/n*360-180,lat(y+1),(x+1)/n*360-180,lat(y)];};
export function validateOfficialVectorPackage(p,datasetId) {
  if(!p||Object.keys(p).length!==packageKeys.length||packageKeys.some(k=>!Object.hasOwn(p,k))||!['ms-buildings','geofabrik'].includes(p.datasetId)||datasetId&&datasetId!==p.datasetId||!validQueryBounds(p.bounds)||typeof p.title!=='string'||!p.title||!Array.isArray(p.warnings)||p.warnings.length>20||p.warnings.some(v=>typeof v!=='string'||v.length>16384))fail('Invalid official vector package.');
  if(p.datasetId==='ms-buildings'){
    const row=rows.get(p.id);if(!row)fail('Unknown reviewed Microsoft building partition.');const [country,key,part,size]=row;
    if(p.format!=='geojsonl-gzip'||p.title!==`Microsoft buildings · ${country} · ${key}`||p.attribution!==msAttribution||p.release!=='2026-08-13'||p.fileName!==`Microsoft-${country}-${key}-2026-08-13.csv.gz`
      ||p.url!==`https://bfppub.z5.web.core.windows.net/2026-08-13/global-buildings.geojsonl/RegionName=${country}/quadkey=${key}/${part}`
      ||p.sourceIndexUrl!=='https://bfppub.blob.core.windows.net/%24web/2026-08-13/dataset-links.csv'||p.sourceIndexSha256!==ms.sourceSha256
      ||p.publisherSize!==size||p.expectedSha256!==null||p.licenseUrl!=='https://cdla.dev/permissive-2-0/'||p.dataTimestamp!==null
      ||p.documentationUrl!=='https://github.com/microsoft/GlobalMLBuildingFootprints'||p.bounds.some((v,i)=>Math.abs(v-quadBounds(key)[i])>1e-9))fail('Microsoft package differs from its pinned publisher index.');
  }else if(p.id!=='monaco-261011-free-shp'||p.format!=='shapefile'||p.release!=='2026-10-11'||p.url!=='https://download.geofabrik.de/europe/monaco-261011-free.shp.zip'
    ||p.fileName!=='monaco-261011-free.shp.zip'||p.sourceIndexUrl!=='https://download.geofabrik.de/europe/monaco.html'||p.sourceIndexSha256!==null||p.publisherSize!=='746384 bytes'
    ||p.expectedSha256!=='33611b0f8fd1fbc6562ce6a2c93b35371d26da9e38f3015a889fb9e527e08ccd'||p.licenseUrl!=='https://opendatacommons.org/licenses/odbl/1-0/'
    ||p.title!=='Geofabrik Monaco · complete free GIS Shapefile bundle · 2026-10-11'||p.documentationUrl!=='https://download.geofabrik.de/osm-data-in-gis-formats-free.pdf'||p.attribution!==geoAttribution
    ||p.dataTimestamp!=='2026-10-11T20:21:06Z'||!same(p.bounds,[7.4016855,43.5165358,7.5329917,43.7543687]))fail('Geofabrik package differs from its pinned original bundle.');
  return p;
}
export function validateOfficialVectorProvenance(receipt,asset) {
  validateOfficialVectorPackage(receipt?.package);
  if(Object.keys(receipt).length!==7||!['conversion','package','requestedBounds','fetchedAt','bytes','sha256','selection'].every(k=>Object.hasOwn(receipt,k))||receipt.conversion!==1||!validQueryBounds(receipt.requestedBounds)||!Number.isFinite(Date.parse(receipt.fetchedAt))||!hash.test(receipt.sha256)
    ||receipt.bytes!==asset.bytes||receipt.sha256!==asset.sourceSha256||asset.format!==receipt.package.format||asset.storageMode!=='managed'
    ||receipt.package.expectedSha256&&receipt.sha256!==receipt.package.expectedSha256
    ||receipt.selection!=='complete publisher original; area selects a package, not a geometry clip'
    ||![0,1].every(i=>receipt.requestedBounds[i]<receipt.package.bounds[i+2]&&receipt.requestedBounds[i+2]>receipt.package.bounds[i]))fail('Official vector original differs from its retained receipt.');
  return receipt;
}
export async function officialVectorRequest(operation,payload={},signal) {
  return usageRequest('official-vector', operation, payload, () => performOfficialVectorRequest(operation, payload, signal));
}
async function performOfficialVectorRequest(operation,payload={},signal) {
  const commands={list:'list_official_vector_packages',search:'search_official_vector_packages',download:'download_official_vector_package'};
  if(!commands[operation])fail('Unknown official vector operation.');
  if(operation!=='list'&&(!['ms-buildings','geofabrik'].includes(payload.datasetId)||!validQueryBounds(payload.bounds)))fail('Choose a valid area for the official vector source.');
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');let value;
  if(desktopAvailable())value=await window.__TAURI__.core.invoke(commands[operation],operation==='list'?{}:{request:payload});
  else {const r=await fetch(`http://127.0.0.1:4318/official-vector-packages${operation==='list'?'':`/${operation}`}`,{method:operation==='list'?'GET':'POST',signal,headers:operation==='list'?undefined:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:operation==='list'?undefined:JSON.stringify(payload)});value=await r.json();if(!r.ok)fail(value.error||'Official vector request failed.');}
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');
  if(operation==='search'){if(value.datasetId!==payload.datasetId||!same(value.bounds,payload.bounds)||!Array.isArray(value.items)||value.items.length>8||value.complete!==true||value.reviewedIndexOnly!==true||value.liveAvailabilityChecked!==false)fail('Official package search receipt changed.');value.items.forEach(p=>validateOfficialVectorPackage(p,payload.datasetId));}
  if(operation==='download'){const {validateVectorAsset}=await import('./vector-client.js');validateVectorAsset(value);if(value.officialPackage?.package.id!==payload.packageId||!same(value.officialPackage.requestedBounds,payload.bounds))fail('The downloaded official vector package changed.');}
  return value;
}
