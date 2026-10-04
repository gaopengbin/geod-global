import {validQueryBounds,validServiceUrl} from './features-client.js';
import {wmtsRestTemplate,wmtsRestTileUrl} from './wmts-rest-client.js';
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const sha=/^[a-f0-9]{64}$/,metersPerDegree=Math.PI*6378137/180;
const endpoint=u=>validServiceUrl(u)&&!new URL(u).search;
const positive=(n,max)=>Number.isInteger(n)&&n>0&&n<=max;
const text=(s,max)=>typeof s==='string'&&s.trim()===s&&s.length>0&&s.length<=max&&!/[\x00-\x1f\x7f]/.test(s);
const same=(a,b)=>Array.isArray(a)&&a.length===b.length&&a.every((n,i)=>Number.isFinite(n)&&Math.abs(n-b[i])<=Math.max(1,Math.abs(b[i]))*1e-12);
export function wmtsCrs(raw) {
  if(['CRS:84','urn:ogc:def:crs:OGC:1.3:CRS84','http://www.opengis.net/def/crs/OGC/1.3/CRS84'].includes(raw))return 'EPSG:4326';
  for(const n of ['4326','3857'])if(raw===`EPSG:${n}`||raw===`http://www.opengis.net/def/crs/EPSG/0/${n}`||(raw.startsWith('urn:ogc:def:crs:EPSG:')&&raw.endsWith(`:${n}`)))return `EPSG:${n}`;
  throw new Error('Unsupported WMTS projection; use WGS84 or Web Mercator');
}
export function validateWmtsMatrix(m) {
  if(!m||!text(m.id,256)||!Number.isFinite(m.scaleDenominator)||m.scaleDenominator<=0||m.scaleDenominator>1e12||!Array.isArray(m.topLeft)||m.topLeft.length!==2||m.topLeft.some(n=>!Number.isFinite(n)||Math.abs(n)>1e9)||![m.tileWidth,m.tileHeight].every(n=>positive(n,1024))||![m.matrixWidth,m.matrixHeight].every(n=>positive(n,2**24)))throw new Error('Invalid WMTS tile matrix.');
  return m;
}
export function wmtsPlan(bounds,crs,m,limits=[]) {
  validateWmtsMatrix(m);if(!validQueryBounds(bounds))throw new Error('Select a query region on Explore first.');
  let b=bounds;
  if(crs==='EPSG:3857') {
    if(b[1]<-85.0511287798066||b[3]>85.0511287798066)throw new Error('The selected region is outside Web Mercator');
    const y=lat=>6378137*Math.log(Math.tan(Math.PI/4+lat*Math.PI/360));b=[b[0]*metersPerDegree,y(b[1]),b[2]*metersPerDegree,y(b[3])];
  }else if(crs!=='EPSG:4326')throw new Error('Unsupported WMTS projection; use WGS84 or Web Mercator');
  const resolution=m.scaleDenominator*0.00028/(crs==='EPSG:3857'?1:metersPerDegree);
  const snap=n=>Math.abs(n-Math.round(n))<1e-7?Math.round(n):n;
  const x=Math.floor(snap((b[0]-m.topLeft[0])/resolution)),y=Math.floor(snap((m.topLeft[1]-b[3])/resolution));
  const right=Math.ceil(snap((b[2]-m.topLeft[0])/resolution)),bottom=Math.ceil(snap((m.topLeft[1]-b[1])/resolution));
  if([x,y,right,bottom].some(n=>!Number.isSafeInteger(n)||n<0)||right<=x||bottom<=y||right>m.matrixWidth*m.tileWidth||bottom>m.matrixHeight*m.tileHeight)throw new Error('The selected region is outside the WMTS tile grid');
  const width=right-x,height=bottom-y;if(width>2048||height>2048)throw new Error('Choose a coarser WMTS level or a smaller area (2048 pixels per edge)');
  const minRow=Math.floor(y/m.tileHeight),maxRow=Math.floor((bottom-1)/m.tileHeight),minCol=Math.floor(x/m.tileWidth),maxCol=Math.floor((right-1)/m.tileWidth);
  if((maxRow-minRow+1)*(maxCol-minCol+1)>16)throw new Error('Choose a coarser WMTS level or a smaller area (16 tiles per image)');
  const tiles=[];for(let row=minRow;row<=maxRow;row++)for(let col=minCol;col<=maxCol;col++) {
    if(limits.length&&!limits.some(l=>row>=l.minRow&&row<=l.maxRow&&col>=l.minCol&&col<=l.maxCol))throw new Error('The selected region exceeds the layer tile limits');
    tiles.push({row,col});
  }
  const extent=[m.topLeft[0]+x*resolution,m.topLeft[1]-bottom*resolution,m.topLeft[0]+right*resolution,m.topLeft[1]-y*resolution];
  const latitude=n=>(2*Math.atan(Math.exp(n/6378137))-Math.PI/2)*180/Math.PI;
  const geographic=(crs==='EPSG:4326'?[...extent]:[extent[0]/metersPerDegree,latitude(extent[1]),extent[2]/metersPerDegree,latitude(extent[3])]).map((n,i)=>{
    const limit=i%2===0?180:90;
    return Math.abs(n)>limit&&Math.abs(n)-limit<=1e-9?Math.sign(n)*limit:n;
  });
  if(!validQueryBounds(geographic))throw new Error('The output pixel window is outside WGS84');
  return {width,height,window:[x,y,width,height],extent,bounds:geographic,tiles,resolution};
}
function baseSource(s) {
  if(!s||!endpoint(s.serviceUrl)||!endpoint(s.mapEndpoint)||new URL(s.serviceUrl).origin!==new URL(s.mapEndpoint).origin||s.version!=='1.0.0'||!sha.test(s.capabilitiesSha256)||!text(s.layerName,256)||!text(s.layerTitle,1024)||!text(s.style,256)||!Number.isFinite(Date.parse(s.requestedAt)))throw new Error('Invalid saved WMTS source.');
}
export function validateWmtsService(s) {
  if(!s||!uuid.test(s.id)||!text(s.name,80)||!endpoint(s.url)||!endpoint(s.mapUrl)||new URL(s.url).origin!==new URL(s.mapUrl).origin||s.version!=='1.0.0'||!sha.test(s.capabilitiesSha256)||!Number.isFinite(Date.parse(s.connectedAt))||s.maxWidth!==2048||s.maxHeight!==2048||!Array.isArray(s.layers)||!s.layers.length||s.layers.length>4096||!Array.isArray(s.wmts?.matrixSets)||!s.wmts.matrixSets.length||s.wmts.matrixSets.length>128)throw new Error('Invalid saved WMTS service.');
  if(['restOnly','capabilitiesDocument'].some(k=>s.wmts[k]!=null&&typeof s.wmts[k]!=='boolean')||(s.wmts.restOnly&&s.mapUrl!==s.url))throw new Error('Invalid saved WMTS service.');
  const excluded=s.wmts.excludedLayers??[];
  if(!Array.isArray(excluded)||excluded.length>4096||excluded.some(l=>!l||!text(l.name,256)||!text(l.reason,1024)))throw new Error('Invalid WMTS discovery exclusions.');
  const sets=new Map();for(const set of s.wmts.matrixSets) {
    if(!text(set.id,256)||sets.has(set.id)||wmtsCrs(set.declaredCrs)!==set.crs||!Array.isArray(set.matrices)||!set.matrices.length||set.matrices.length>64)throw new Error('Invalid WMTS matrix set.');
    const ids=new Set();for(const m of set.matrices){validateWmtsMatrix(m);if(ids.has(m.id))throw new Error('Duplicate WMTS tile matrix.');ids.add(m.id);}sets.set(set.id,set);
  }
  const names=new Set();for(const l of s.layers) {
    const w=l.wmts;
    if(!text(l.name,256)||names.has(l.name)||!text(l.title,1024)||!w||!['image/png','image/jpeg'].includes(w.format)||!Array.isArray(l.styles)||!l.styles.length||l.styles.some(v=>!text(v,256))||!l.styles.includes(w.defaultStyle)||!Array.isArray(w.links)||!w.links.length||w.links.length>128||Boolean(l.time)!==Boolean(w.timeIdentifier)||(w.timeIdentifier&&!/^time$/i.test(w.timeIdentifier))||(l.time&&(typeof l.time.values!=='string'||!l.time.values))||(l.bounds&&!validQueryBounds(l.bounds)))throw new Error('Invalid WMTS layer.');
    if(w.resourceUrl!=null){if(wmtsRestTemplate(w.resourceUrl,s.url,w.timeIdentifier)!==w.resourceUrl||(l.styles.length>1&&!w.resourceUrl.includes('{Style}'))||(w.links.length>1&&!w.resourceUrl.includes('{TileMatrixSet}')))throw new Error('WMTS REST template does not match the layer dimensions');}
    else if(s.wmts.restOnly)throw new Error('REST-only WMTS layer has no tile template');
    names.add(l.name);const links=new Set();for(const link of w.links) {
      const set=sets.get(link.matrixSet);if(!set||links.has(link.matrixSet)||!Array.isArray(link.limits)||link.limits.length>64)throw new Error('Invalid WMTS matrix set link.');links.add(link.matrixSet);
      const limits=new Set();for(const v of link.limits) {
        const m=set.matrices.find(m=>m.id===v.matrix);if(!m||limits.has(JSON.stringify(v))||![v.minRow,v.maxRow,v.minCol,v.maxCol].every(n=>Number.isInteger(n)&&n>=0)||v.minRow>v.maxRow||v.minCol>v.maxCol||v.maxRow>=m.matrixHeight||v.maxCol>=m.matrixWidth)throw new Error('Invalid WMTS tile limits.');limits.add(JSON.stringify(v));
      }
    }
    if(l.crs!==sets.get(w.links[0].matrixSet).crs)throw new Error('WMTS layer CRS does not match its matrix set.');
  }return s;
}
export function validateWmtsImage(a) {
  const s=a?.source,w=s?.wmts;baseSource(s);
  if(!uuid.test(a.id)||!text(a.name,120)||!validQueryBounds(a.bounds)||!sha.test(a.sha256)||!positive(a.bytes,16*1024*1024)||!w||!text(w.matrixSet,256)||wmtsCrs(w.declaredCrs)!==a.crs||s.requestCrs!==a.crs||s.selection!=='pixel-window-rendered-tiles'||!['image/png','image/jpeg'].includes(w.format)||Boolean(s.time)!==Boolean(w.timeIdentifier)||(w.timeIdentifier&&!/^time$/i.test(w.timeIdentifier))||!sha.test(w.archiveSha256)||!positive(w.archiveBytes,64*1024*1024)||!Array.isArray(w.tiles))throw new Error('Invalid saved WMTS map image.');
  if(w.resourceUrl!=null&&wmtsRestTemplate(w.resourceUrl,s.serviceUrl,w.timeIdentifier)!==w.resourceUrl)throw new Error("Invalid saved WMTS REST template");
  const p=wmtsPlan(w.requestedBounds,a.crs,w.matrix);
  if(!same(w.pixelWindow,p.window)||!same(a.imageExtent,p.extent)||!same(a.bounds,p.bounds)||a.width!==p.width||a.height!==p.height||w.tiles.length!==p.tiles.length)throw new Error('WMTS output does not match its declared tile grid.');
  for(let i=0;i<w.tiles.length;i++) {
    const receipt=w.tiles[i],tile=p.tiles[i],expected={SERVICE:'WMTS',REQUEST:'GetTile',VERSION:'1.0.0',LAYER:s.layerName,STYLE:s.style,FORMAT:w.format,TILEMATRIXSET:w.matrixSet,TILEMATRIX:w.matrix.id,TILEROW:String(tile.row),TILECOL:String(tile.col)};
    if(w.resourceUrl!=null) {
      if(receipt.row!==tile.row||receipt.col!==tile.col||!positive(receipt.bytes,4*1024*1024)||!sha.test(receipt.sha256)||receipt.requestUrl!==wmtsRestTileUrl(s,w,tile.row,tile.col))throw new Error('WMTS tile receipts do not match the saved grid.');
      continue;
    }
    if(w.timeIdentifier)expected[w.timeIdentifier]=s.time;
    const u=new URL(receipt.requestUrl),endpoint=new URL(s.mapEndpoint),entries=[...u.searchParams];
    if(receipt.row!==tile.row||receipt.col!==tile.col||!positive(receipt.bytes,4*1024*1024)||!sha.test(receipt.sha256)||u.origin!==endpoint.origin||u.pathname!==endpoint.pathname||u.hash||u.username||u.password||entries.length!==Object.keys(expected).length||entries.some(([k,v])=>expected[k]!==v)||new Set(entries.map(([k])=>k)).size!==entries.length)throw new Error('WMTS tile receipts do not match the saved grid.');
  }
  if(s.requestUrl!==w.tiles[0].requestUrl)throw new Error('WMTS first tile receipt changed.');return a;
}
