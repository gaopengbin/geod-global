import {validServiceUrl} from './features-client.js';
import {wmtsPlan} from './wmts-client.js';
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/,sha=/^[a-f0-9]{64}$/;
const text=(s,max)=>typeof s==='string'&&s.trim()===s&&s.length>0&&[...s].length<=max&&!/[\x00-\x1f\x7f]/.test(s);
const same=(a,b)=>Array.isArray(a)&&a.length===b.length&&a.every((v,i)=>Number.isFinite(v)&&Math.abs(v-b[i])<=Math.max(1,Math.abs(b[i]))*1e-12);
const fail=s=>{throw new Error(s);};
export const defaultTileConfig=()=>({tileSize:256,minZoom:0,maxZoom:18,zoomOffset:0,format:'image/png',attribution:'',accessConstraints:''});
export const gibsXyzExample={name:'NASA GIBS · MODIS Terra · 2025-06-27',url:'https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/MODIS_Terra_CorrectedReflectance_TrueColor/default/2025-06-27/GoogleMapsCompatible_Level9/{z}/{y}/{x}.jpeg',tileConfig:{tileSize:256,minZoom:0,maxZoom:9,zoomOffset:0,format:'image/jpeg',attribution:'NASA GIBS / MODIS Terra',accessConstraints:'NASA GIBS visualization; dataset reuse terms apply. https://nasa-gibs.github.io/gibs-api-docs/'}};
export const dlrTmsExample={name:'DLR EOC Basemap',url:'https://tiles.geoservice.dlr.de/service/tms/1.0.0/eoc%3Abasemap@EPSG%3A3857@png/{z}/{x}/{y}.png',tileConfig:{tileSize:256,minZoom:0,maxZoom:16,zoomOffset:0,format:'image/png',attribution:'Data © OpenStreetMap contributors and others; rendering © DLR/EOC',accessConstraints:'Rendered basemap; source use policy: https://geoservice.dlr.de/web/about'}};
function validTemplatePath(path){
 return path.split('/').every(segment=>{
  try{
   if(/%(?:2f|5c|25|3f|23|7b|7d)/i.test(segment)||segment.includes('\\'))return false;
   const decoded=decodeURIComponent(segment);
   return decoded!=='.'&&decoded!=='..'&&!/[\u0000-\u001f\u007f-\u009f]/u.test(decoded);
  }catch{return false;}
 });
}
export function validateXyzConfiguration(c){
 const g=c?.grid,raw=c?.urlTemplate;
 if(!['XYZ','TMS'].includes(c?.scheme)||!text(raw,2048)||['{z}','{x}','{y}'].some(t=>raw.split(t).length!==2))fail('Use one {z}, {x} and {y} in the tile URL path');
 const rendered=raw.replace('{z}','0').replace('{x}','0').replace('{y}','0'),path=raw.split('://')[1]?.slice(raw.split('://')[1].indexOf('/')+1);
 if(!validServiceUrl(rendered)||/[{}]/.test(rendered)||!path||['{z}','{x}','{y}'].some(t=>!path.includes(t))||!validTemplatePath(path)||new URL(rendered).search||new URL(rendered).hash)fail('Tile templates require a public HTTPS path without query parameters');
 const host=new URL(rendered).hostname;if(host==='tile.openstreetmap.org'||host.endsWith('.tile.openstreetmap.org'))fail('The standard OpenStreetMap tile server does not permit offline downloads');
 if(!g||![256,512].includes(g.tileSize)||![g.minZoom,g.maxZoom,g.zoomOffset].every(Number.isInteger)||g.minZoom<0||g.minZoom>g.maxZoom||g.maxZoom>24||g.zoomOffset < -2||g.zoomOffset>2||g.minZoom+g.zoomOffset<0||!['image/png','image/jpeg'].includes(g.format)||typeof g.attribution!=='string'||typeof g.accessConstraints!=='string'||(g.attribution&&!text(g.attribution,1024))||(g.accessConstraints&&!text(g.accessConstraints,16384)))fail('Invalid XYZ/TMS grid configuration');
 return c;
}
export function xyzMatrix(c,zoom){
 validateXyzConfiguration(c);if(!Number.isInteger(zoom)||zoom<c.grid.minZoom||zoom>c.grid.maxZoom)fail('Choose a configured XYZ/TMS tile level');
 const half=Math.PI*6378137,count=2**zoom;
 return {id:String(zoom),scaleDenominator:(2*half/c.grid.tileSize/count)/.00028,topLeft:[-half,half],tileWidth:c.grid.tileSize,tileHeight:c.grid.tileSize,matrixWidth:count,matrixHeight:count};
}
export function xyzPlan(bounds,c,zoom){try{return wmtsPlan(bounds,'EPSG:3857',xyzMatrix(c,zoom));}catch(e){fail(e.message.replaceAll('WMTS','XYZ/TMS'));}}
export function xyzTileUrl(c,zoom,row,col){
 const m=xyzMatrix(c,zoom);if(![row,col].every(n=>Number.isInteger(n)&&n>=0&&n<m.matrixWidth))fail('XYZ/TMS tile lies outside the configured grid');
 return new URL(c.urlTemplate.replace('{z}',String(zoom+c.grid.zoomOffset)).replace('{x}',String(col)).replace('{y}',String(c.scheme==='TMS'?m.matrixHeight-1-row:row))).href;
}
export function validateXyzService(s){
 const c=validateXyzConfiguration(s?.xyz),l=s?.layers?.[0],canonical=new URL(c.urlTemplate).href;
 if(!uuid.test(s.id)||!text(s.name,80)||s.title!==s.name||s.url!==canonical||s.mapUrl!==canonical||s.version!=='xyz-1'||s.wmts||s.arcgis||!sha.test(s.capabilitiesSha256)||!Number.isFinite(Date.parse(s.connectedAt))||s.maxWidth!==2048||s.maxHeight!==2048||!Array.isArray(s.layers)||s.layers.length!==1||l.name!=='tiles'||l.title!==s.name||l.crs!=='EPSG:3857'||l.wmts||l.time||l.bounds||!Array.isArray(l.styles)||l.styles.length||s.accessConstraints!==c.grid.accessConstraints||(l.attribution??null)!==(c.grid.attribution||null))fail('XYZ/TMS service does not match its configured grid');
 return s;
}
export function validateXyzImage(a){
 const s=a?.source,w=s?.xyz,c=validateXyzConfiguration(w?.configuration);
 if(!a||!uuid.test(a.id)||!text(a.name,120)||!Number.isInteger(a.bytes)||a.bytes<=0||a.bytes>16*1024*1024||!sha.test(a.sha256)||!s||s.wmts||s.arcgis||!text(s.serviceName,80)||s.serviceTitle!==s.serviceName||s.serviceUrl!==new URL(c.urlTemplate).href||s.mapEndpoint!==s.serviceUrl||s.version!=='xyz-1'||s.layerName!=='tiles'||s.layerTitle!==s.serviceName||s.style!==''||s.time!=null||s.requestCrs!=='EPSG:3857'||a.crs!==s.requestCrs||!sha.test(s.capabilitiesSha256)||!Number.isFinite(Date.parse(s.requestedAt))||s.selection!=='pixel-window-rendered-tiles'||s.accessConstraints!==c.grid.accessConstraints||(s.attribution??null)!==(c.grid.attribution||null)||!Number.isInteger(w.archiveBytes)||w.archiveBytes<=0||w.archiveBytes>64*1024*1024||!sha.test(w.archiveSha256))fail('Invalid saved XYZ/TMS image metadata');
 const p=xyzPlan(w.requestedBounds,c,w.logicalZoom);
 const m=xyzMatrix(c,w.logicalZoom),saved=w.matrix;
 if(w.matrixSet!=='WebMercator'||!saved||saved.id!==m.id||!same(saved.topLeft,m.topLeft)||!Number.isFinite(saved.scaleDenominator)||Math.abs(saved.scaleDenominator-m.scaleDenominator)>m.scaleDenominator*1e-12||['tileWidth','tileHeight','matrixWidth','matrixHeight'].some(k=>saved[k]!==m[k]))fail('XYZ/TMS matrix differs from the configured global grid');
 if(a.width!==p.width||a.height!==p.height||!same(a.bounds,p.bounds)||!same(a.imageExtent,p.extent)||!same(w.pixelWindow,p.window)||!Array.isArray(w.tiles)||w.tiles.length!==p.tiles.length)fail('XYZ/TMS output does not match its configured pixel grid');
 for(let i=0;i<p.tiles.length;i++){const t=w.tiles[i],v=p.tiles[i];if(!t||t.row!==v.row||t.col!==v.col||!Number.isInteger(t.bytes)||t.bytes<=0||t.bytes>4*1024*1024||!sha.test(t.sha256)||t.requestUrl!==xyzTileUrl(c,w.logicalZoom,t.row,t.col))fail('XYZ/TMS tile receipts do not match the saved grid');}
 if(s.requestUrl!==w.tiles[0].requestUrl)fail('XYZ/TMS first tile receipt changed');return a;
}
