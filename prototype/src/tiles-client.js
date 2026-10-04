import {desktopAvailable} from './runtime-client.js';
import {validQueryBounds,validServiceUrl} from './features-client.js';
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const digest=/^[a-f0-9]{64}$/;
const integer=(n,max=Number.MAX_SAFE_INTEGER)=>Number.isSafeInteger(n)&&n>=0&&n<=max;
const name=(n,max)=>typeof n==='string'&&n.trim()===n&&n.length>0&&[...n].length<=max&&!/[\x00-\x1f\x7f]/.test(n);
const coord=c=>c&&integer(c.z,24)&&integer(c.x,2**c.z-1)&&integer(c.y,2**c.z-1);
const key=c=>`${c.z}/${c.x}/${c.y}`;
const ranges=(rs,total)=>Array.isArray(rs)&&rs.length<=2048&&rs.every(r=>integer(r.offset)&&integer(r.bytes,8*1024*1024)&&r.bytes>0&&r.offset+r.bytes<=total&&digest.test(r.sha256))&&new Set(rs.map(r=>`${r.offset}/${r.bytes}`)).size===rs.length;
const layers=ls=>Array.isArray(ls)&&ls.length<=256&&new Set(ls.map(l=>l?.name)).size===ls.length&&ls.every(l=>name(l.name,256)&&integer(l.extent,1048576)&&l.extent>0&&[1,2].includes(l.version)&&integer(l.features,50000));
export const pmtilesExample={name:'US postal areas · Protomaps sample',url:'https://r2-public.protomaps.com/protomaps-sample-datasets/cb_2018_us_zcta510_500k.pmtiles'};
export function tileAttribution(metadata) {
  if(typeof metadata?.attribution!=='string')return '';
  return metadata.attribution.replace(/<[^>]*>/g,'').replace(/&(copy|amp|lt|gt|quot|apos|nbsp);/g,(_,entity)=>({copy:'©',amp:'&',lt:'<',gt:'>',quot:'"',apos:"'",nbsp:' '}[entity]));
}
export function tileSelection(bounds,minZoom,maxZoom) {
  if(!validQueryBounds(bounds)||bounds[1]<-85.0511287798066||bounds[3]>85.0511287798066||!integer(minZoom,24)||!integer(maxZoom,24)||minZoom>maxZoom)throw new Error('Choose a Web Mercator region and valid tile levels');
  const tiles=[];
  for(let z=minZoom;z<=maxZoom;z++){
    const n=2**z,x=lon=>(lon+180)/360*n,y=lat=>(1-Math.asinh(Math.tan(lat*Math.PI/180))/Math.PI)/2*n,clamp=v=>Math.max(0,Math.min(n-1,v));
    const west=clamp(Math.floor(x(bounds[0]))),east=Math.max(west,clamp(Math.ceil(x(bounds[2]))-1)),north=clamp(Math.floor(y(bounds[3]))),south=Math.max(north,clamp(Math.ceil(y(bounds[1]))-1));
    if(tiles.length+(east-west+1)*(south-north+1)>512)throw new Error('Choose a smaller area or fewer tile levels; the extraction exceeds 512 tiles');
    for(let x=west;x<=east;x++)for(let y=north;y<=south;y++)tiles.push({z,x,y});
  }return tiles;
}
export const tileFormat=p=>p.source.mbtiles?.format||"pbf";
function validateMbtilesSource(s) {
  const h=s.mbtiles,local=s.local;
  if(!uuid.test(s.id)||!name(s.name,80)||!Number.isFinite(Date.parse(s.connectedAt))||!digest.test(s.discoverySha256)||!local||!name(local.fileName,240)||/[\\/:]/.test(local.fileName)||!local.fileName.toLowerCase().endsWith('.mbtiles')||!digest.test(local.sha256)||s.header!==undefined||s.url!==''||s.etag!==''||!integer(s.totalBytes,128*1024*1024)||s.totalBytes<100||!Array.isArray(s.ranges)||s.ranges.length||!['pbf','png','jpg'].includes(h.format)||h.scheme!=='tms'||!integer(h.minZoom,24)||!integer(h.maxZoom,24)||h.minZoom>h.maxZoom||!validQueryBounds(h.bounds)||h.bounds[1]<-85.0511287798066||h.bounds[3]>85.0511287798066||!integer(h.addressedTiles,512)||!h.addressedTiles||(h.format==='pbf'?h.tileSize!==undefined:!integer(h.tileSize,1024)||!h.tileSize))throw new Error('Invalid saved MBTiles source');
  const meta=s.metadata?.mbtiles,ls=s.metadata?.vector_layers;
  if(!meta||Array.isArray(meta)||typeof meta!=='object'||Object.keys(meta).length>256||Object.entries(meta).some(([k,v])=>!name(k,80)||typeof v!=='string')||!name(meta.name,1024)||meta.format!==h.format||meta.scheme!==undefined&&meta.scheme!=='tms'||!Array.isArray(ls)||ls.length>256||(h.format==='pbf'?!ls.length:ls.length!==0)||new Set(ls.map(l=>l?.id)).size!==ls.length||ls.some(l=>!name(l.id,256)||!l.fields||Array.isArray(l.fields)||typeof l.fields!=='object'||Object.keys(l.fields).length>512||Object.entries(l.fields).some(([k,v])=>!name(k,256)||!['String','Number','Boolean'].includes(v))))throw new Error('Invalid MBTiles layer metadata');
  return s;
}
function validateMbtilesPackage(p) {
  validateMbtilesSource(p.source);const h=p.source.mbtiles;
  if(!uuid.test(p.id)||!name(p.name,120)||p.bytes!==p.source.totalBytes||p.sha256!==p.source.local.sha256||!digest.test(p.sha256)||!Number.isFinite(Date.parse(p.createdAt))||!validQueryBounds(p.tileCoverageBounds)||!validQueryBounds(p.requestedBounds)||p.requestedBounds.some((n,i)=>n!==h.bounds[i])||p.minZoom!==h.minZoom||p.maxZoom!==h.maxZoom||p.selection!=='imported-archive'||!Array.isArray(p.tiles)||p.tiles.length!==h.addressedTiles||!Array.isArray(p.absent)||p.absent.length||!Array.isArray(p.ranges)||p.ranges.length)throw new Error('Invalid saved MBTiles package');
  const seen=new Set(),declared=new Set(p.source.metadata.vector_layers.map(l=>l.id));
  for(const t of p.tiles){const c=t.coordinate;if(!coord(c)||c.z<h.minZoom||c.z>h.maxZoom||seen.has(key(c))||t.sourceOffset!==undefined||t.packageOffset!==undefined||!integer(t.bytes,8*1024*1024)||!t.bytes||!digest.test(t.sha256)||!layers(t.layers))throw new Error('Invalid saved MBTiles tile receipt');seen.add(key(c));
    if(h.format==='pbf'?(t.image!==undefined||t.layers.some(l=>!declared.has(l.name))):(t.layers.length||!t.image||t.image.width!==h.tileSize||t.image.height!==h.tileSize||t.image.contentType!==(h.format==='png'?'image/png':'image/jpeg')))throw new Error('Invalid MBTiles tile content receipt');
  }return p;
}
export function validateTileSource(s) {
  if(s?.mbtiles)return validateMbtilesSource(s);
  const h=s?.header;
  const local=s?.local;
  if(local){if(!name(local.fileName,240)||/[\\/:]/.test(local.fileName)||!local.fileName.toLowerCase().endsWith('.pmtiles')||!digest.test(local.sha256)||s.url!==''||s.etag!==''||!integer(s.totalBytes,128*1024*1024))throw new Error('Invalid local PMTiles source');}
  else if(!s||!validServiceUrl(s.url)||new URL(s.url).search||!new URL(s.url).pathname.endsWith('.pmtiles')||!/^"[\x21-\x7e]*"$/.test(s.etag)||s.etag.slice(1,-1).includes('"')||s.etag.length>256)throw new Error('Invalid saved PMTiles source');
  if(!s||!uuid.test(s.id)||!name(s.name,80)||!integer(s.totalBytes)||!Number.isFinite(Date.parse(s.connectedAt))||!digest.test(s.discoverySha256)||!ranges(s.ranges,s.totalBytes)||s.ranges.length!==3||!h)throw new Error('Invalid saved PMTiles source');
  if(!['rootOffset','rootLength','metadataOffset','metadataLength','leafOffset','leafLength','tileOffset','tileLength','addressedTiles','tileEntries','tileContents'].every(k=>integer(h[k]))||!h.rootLength||h.rootOffset<127||h.rootOffset+h.rootLength>16384||!h.metadataLength||h.metadataLength>2*1024*1024||!h.tileLength||![h.internalCompression,h.tileCompression].every(n=>[1,2].includes(n))||typeof h.clustered!=='boolean'||!integer(h.minZoom,24)||!integer(h.maxZoom,24)||h.minZoom>h.maxZoom||!integer(h.centerZoom,24)||!validQueryBounds(h.bounds)||!Array.isArray(h.center)||h.center.length!==2||!h.center.every(Number.isFinite)||Math.abs(h.center[0])>180||Math.abs(h.center[1])>90)throw new Error('Invalid saved PMTiles header');
  const spans=['root','metadata','leaf','tile'].filter(k=>h[k+'Length']).map(k=>[h[k+'Offset'],h[k+'Offset']+h[k+'Length']]).sort((a,b)=>a[0]-b[0]);
  if(spans.some((span,i)=>span[0]<127||span[1]>s.totalBytes||span[1]>Number.MAX_SAFE_INTEGER||(i&&spans[i-1][1]>span[0])))throw new Error('PMTiles archive sections overlap');
  for(const [offset,bytes] of [[0,127],[h.rootOffset,h.rootLength],[h.metadataOffset,h.metadataLength]])if(!s.ranges.some(r=>r.offset===offset&&r.bytes===bytes))throw new Error('PMTiles discovery receipts do not match its header');
  const ls=s.metadata?.vector_layers;
  if(!Array.isArray(ls)||!ls.length||ls.length>256||new Set(ls.map(l=>l?.id)).size!==ls.length||ls.some(l=>!name(l.id,256)||!l.fields||Array.isArray(l.fields)||typeof l.fields!=='object'||Object.keys(l.fields).length>512))throw new Error('Invalid PMTiles layer metadata');return s;
}
export function validateTilePackage(p) {
  if(p?.source?.mbtiles)return validateMbtilesPackage(p);
  validateTileSource(p?.source);
  const local=p.source.local,h=p.source.header;
  if(!uuid.test(p.id)||!name(p.name,120)||!integer(p.bytes,128*1024*1024)||!p.bytes||!digest.test(p.sha256)||!Number.isFinite(Date.parse(p.createdAt))||!validQueryBounds(p.tileCoverageBounds)||!validQueryBounds(p.requestedBounds)||p.selection!==(local?'imported-archive':'tile-aligned-pyramid')||!integer(p.minZoom,24)||!integer(p.maxZoom,24)||p.minZoom>p.maxZoom||p.minZoom<h.minZoom||p.maxZoom>h.maxZoom||!Array.isArray(p.tiles)||!p.tiles.length||!Array.isArray(p.absent)||p.tiles.length+p.absent.length>512||!ranges(p.ranges,p.source.totalBytes))throw new Error('Invalid saved PMTiles package');
  if(local&&(p.bytes!==p.source.totalBytes||p.sha256!==local.sha256||p.minZoom!==h.minZoom||p.maxZoom!==h.maxZoom||p.requestedBounds.some((v,i)=>v!==h.bounds[i])||p.absent.length||(h.addressedTiles&&h.addressedTiles!==p.tiles.length)))throw new Error('Local PMTiles import receipt changed');
  const selected=local?null:new Set(tileSelection(p.requestedBounds,p.minZoom,p.maxZoom).map(key)),seen=new Set();
  for(const c of [...p.tiles.map(t=>t.coordinate),...p.absent]){if(!coord(c)||c.z<p.minZoom||c.z>p.maxZoom||selected&&!selected.has(key(c))||seen.has(key(c)))throw new Error('PMTiles extraction membership changed');seen.add(key(c));}
  if(selected&&seen.size!==selected.size)throw new Error('PMTiles extraction membership changed');
  for(const t of p.tiles){if(!integer(t.sourceOffset)||t.sourceOffset<h.tileOffset||!integer(t.bytes,8*1024*1024)||!t.bytes||t.sourceOffset+t.bytes>h.tileOffset+h.tileLength||!integer(t.packageOffset)||t.packageOffset<127||t.packageOffset+t.bytes>p.bytes||local&&t.packageOffset!==t.sourceOffset||!digest.test(t.sha256)||!layers(t.layers)||!p.ranges.some(r=>r.offset===t.sourceOffset&&r.bytes===t.bytes&&r.sha256===t.sha256))throw new Error('Invalid PMTiles tile receipt');}return p;
}
const commands={sources:'list_tile_sources',connect:'connect_tiles',forget:'forget_tile_source',list:'list_tile_packages',extract:'extract_tiles',inspect:'inspect_tile_package',tile:'read_tile'};
export async function tileRequest(operation,payload={},signal) {
  if(!commands[operation])throw new Error('Unknown tile operation');
  if(['forget','inspect','tile'].includes(operation)&&!uuid.test(payload.id))throw new Error('Invalid tile package identifier');
  if(operation==='extract'){if(!uuid.test(payload.sourceId))throw new Error('Invalid tile source identifier');tileSelection(payload.bounds,payload.minZoom,payload.maxZoom);}
  if(operation==='tile'&&!coord(payload))throw new Error('Invalid PMTiles tile coordinate');
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');
  let value;
  if(desktopAvailable()){try{value=await window.__TAURI__.core.invoke(commands[operation],['connect','extract','tile'].includes(operation)?{request:payload}:payload);}catch(e){throw e instanceof Error?e:new Error(String(e));}}
  else{
    const path=operation==='connect'||operation==='sources'?'/tile-sources':operation==='forget'?`/tile-sources/${payload.id}/forget`:operation==='inspect'?`/tile-packages/${payload.id}`:operation==='tile'?`/tile-packages/${payload.id}/tiles/${payload.z}/${payload.x}/${payload.y}`:'/tile-packages';
    const mutation=['connect','forget','extract'].includes(operation),timeout=AbortSignal.timeout(operation==='extract'?190000:65000),requestSignal=signal?AbortSignal.any([signal,timeout]):timeout;
    const r=await fetch('http://127.0.0.1:4318'+path,{method:mutation?'POST':'GET',signal:requestSignal,headers:mutation?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,body:mutation?JSON.stringify(payload):undefined});value=await r.json();if(!r.ok)throw new Error(value.error||'Tile request failed');
  }
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');
  if(['sources','list'].includes(operation)){if(!Array.isArray(value)||value.length>(operation==='sources'?24:128))throw new Error('Invalid tile registry');value.forEach(operation==='sources'?validateTileSource:validateTilePackage);if(operation==='sources'&&value.some(s=>s.local))throw new Error('Invalid tile registry');}
  if(operation==='connect')validateTileSource(value);
  if(operation==='extract')validateTilePackage(value);
  if(operation==='inspect'){validateTilePackage(value?.asset);if(!value.metadata||typeof value.metadata!=='object')throw new Error('Invalid tile inspection');}
  if(operation==='tile'){
    if(value?.contentType!==undefined&&(!['image/png','image/jpeg'].includes(value.contentType)||value.layers?.length||value.dataBase64===null))throw new Error('Invalid local image tile response');
    if(!coord(value?.coordinate)||key(value.coordinate)!==key(payload)||!layers(value.layers)||(value.dataBase64===null?value.sha256!==null||value.layers.length:typeof value.dataBase64!=='string'||value.dataBase64.length>11200000||!digest.test(value.sha256)||!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value.dataBase64)))throw new Error('Invalid local tile response');
  }return value;
}
export async function openTilePackage(file) {
  if(desktopAvailable()){
    try{const p=await window.__TAURI__.core.invoke('open_tile_package');return p===null?null:validateTilePackage(p);}catch(e){throw e instanceof Error?e:new Error(String(e));}
  }
  if(!file||!name(file.name,240)||/[\\/:]/.test(file.name)||!['.pmtiles','.mbtiles'].some(ext=>file.name.toLowerCase().endsWith(ext)))throw new Error('Choose a local .pmtiles or .mbtiles file');
  const mbtiles=file.name.toLowerCase().endsWith('.mbtiles');
  if(!integer(file.size,128*1024*1024)||file.size<(mbtiles?100:127))throw new Error(file.size>128*1024*1024?'Local tile file exceeds 128 MiB':mbtiles?'Choose a valid MBTiles SQLite database, up to 128 MiB':'Choose a PMTiles version 3 archive');
  const r=await fetch(`http://127.0.0.1:4318/tile-packages/import?name=${encodeURIComponent(file.name)}`,{method:'POST',headers:{'Content-Type':'application/octet-stream','X-GeoD-Client':'geod-global'},body:file,signal:AbortSignal.timeout(190000)});
  const p=await r.json().catch(()=>({error:'Tile import failed'}));if(!r.ok)throw new Error(p.error||'Tile import failed');return validateTilePackage(p);
}
export async function exportTilePackage(id) {
  if(!uuid.test(id))throw new Error('Invalid tile package identifier');
  if(desktopAvailable()){try{return await window.__TAURI__.core.invoke('export_tile_package',{id});}catch(e){throw new Error(String(e));}}
  const r=await fetch(`http://127.0.0.1:4318/tile-packages/${id}/export`);if(!r.ok)throw new Error((await r.json()).error||'Tile export failed');if(r.headers.get('content-type')!=='application/zip')throw new Error('Tile export did not return a ZIP package');
  const b=await r.blob(),url=URL.createObjectURL(b),a=document.createElement('a');a.href=url;a.download=`geod-tiles-${id}.zip`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);return true;
}
