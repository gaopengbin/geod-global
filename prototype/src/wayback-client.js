import {validQueryBounds} from './features-client.js';

export const WAYBACK_CATALOG_URL='https://s3-us-west-2.amazonaws.com/config.maptiles.arcgis.com/waybackconfig.json';
export const WAYBACK_APP_URL='https://livingatlas.arcgis.com/wayback/';
export const WAYBACK_ATTRIBUTION='Esri, Vantor, Earthstar Geographics, and the GIS User Community';
export const WAYBACK_TERMS_URL='https://goto.arcgis.com/termsofuse/viewtermsofuse';
export const WAYBACK_EXPORT_NOTE='The public Wayback layer is not intended for offline tile export. Authorized tile packages require an ArcGIS organizational account and are intended for use within ArcGIS.';
const itemId=/^[a-f0-9]{32}$/;
const fail=message=>{throw new Error(message);};
const safeText=(value,max)=>typeof value==='string'&&value.length<=max&&!/[\u0000-\u001f\u007f]/.test(value);
const publicationDate=value=>{const date=/^World Imagery \(Wayback (\d{4}-\d{2}-\d{2})\)$/.exec(value)?.[1];if(!date||!Number.isFinite(Date.parse(date))||new Date(date).toISOString().slice(0,10)!==date)fail('Invalid Wayback publication date');return date;};

// Keep the publisher record unchanged. Publication is a basemap version date,
// and cannot establish the acquisition date at any particular map location.
export function parseWaybackCatalog(document) {
  if(!document||Array.isArray(document)||typeof document!=='object')fail('Invalid Wayback version directory');
  const entries=Object.entries(document);
  if(!entries.length||entries.length>2048)fail('Invalid Wayback version count');
  return entries.map(([release,record])=>{
    if(!/^[1-9]\d{0,9}$/.test(release)||Number(release)>0xffffffff||!record||!itemId.test(record.itemID)||!itemId.test(record.metadataLayerItemID)||!safeText(record.itemTitle,100)||!/^WB_\d{4}_R\d{2}$/.test(record.layerIdentifier))fail('Invalid Wayback version identity');
    const publication=publicationDate(record.itemTitle);
    const expected=`https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/${release}/{level}/{row}/{col}`;
    if(record.itemURL!==expected||!/^https:\/\/metadata\.maptiles\.arcgis\.com\/arcgis\/rest\/services\/World_Imagery_Metadata_\d{4}_r\d{2}\/MapServer$/.test(record.metadataLayerUrl))fail('Wayback version endpoints do not match the publisher directory');
    return {id:`wayback-${release}`,releaseNumber:Number(release),title:record.itemTitle,publicationDate:publication,acquisitionDate:null,layerIdentifier:record.layerIdentifier,itemId:record.itemID,metadataItemId:record.metadataLayerItemID,tileTemplate:record.itemURL.replace('{level}','{z}').replace('{row}','{y}').replace('{col}','{x}'),metadataUrl:record.metadataLayerUrl,itemUrl:`https://www.arcgis.com/home/item.html?id=${record.itemID}`,publisherRecord:record,rendered:true,offlineExportAllowed:false};
  }).sort((a,b)=>b.publicationDate.localeCompare(a.publicationDate)||b.releaseNumber-a.releaseNumber);
}

async function readPublicJson(url,signal,maxBytes) {
  const response=await fetch(url,{signal,credentials:'omit',cache:'no-store'});
  if(!response.ok)fail(`Wayback returned HTTP ${response.status}`);
  if(response.url&&new URL(response.url).origin!==new URL(url).origin)fail('Wayback response changed origin');
  if(!/^application\/json(?:\s*;|$)/i.test(response.headers.get('content-type')||''))fail('Wayback did not return JSON metadata');
  const declared=Number(response.headers.get('content-length'));
  if(Number.isFinite(declared)&&declared>maxBytes)fail('Wayback metadata exceeds the response limit');
  const raw=await response.text();
  if(new TextEncoder().encode(raw).length>maxBytes)fail('Wayback metadata exceeds the response limit');
  if(signal?.aborted)throw new DOMException('Aborted','AbortError');
  let document;try{document=JSON.parse(raw);}catch{fail('Invalid Wayback metadata JSON');}
  if(document?.error)fail(document.error.message||'Wayback metadata request failed');
  return {document,raw};
}

export async function fetchWaybackCatalog(signal) {
  const result=await readPublicJson(WAYBACK_CATALOG_URL,signal,2*1024*1024);
  return {...result,versions:parseWaybackCatalog(result.document),url:WAYBACK_CATALOG_URL};
}

export function waybackMetadataRequest(version,point,zoom) {
  if(!version?.publisherRecord)fail('Choose a verified Wayback version');
  const checked=parseWaybackCatalog({[version.releaseNumber]:version.publisherRecord})[0];
  if(checked.id!==version.id||checked.metadataUrl!==version.metadataUrl)fail('Wayback metadata version identity changed');
  if(!Array.isArray(point)||point.length!==2||!point.every(Number.isFinite)||point[0]<-180||point[0]>180||point[1]<-90||point[1]>90||!Number.isInteger(zoom)||zoom<10||zoom>23)fail('Choose a map point and a metadata level from 10 to 23');
  const url=new URL(`${version.metadataUrl}/${23-zoom}/query`);
  url.search=new URLSearchParams({f:'json',where:'1=1',outFields:'SRC_DATE2,NICE_DESC,SRC_DESC,SAMP_RES,SRC_ACC',geometry:JSON.stringify({spatialReference:{wkid:4326},x:point[0],y:point[1]}),returnGeometry:'false',geometryType:'esriGeometryPoint',spatialRel:'esriSpatialRelIntersects'}).toString();
  return url.href;
}

export function parseWaybackPointMetadata(document) {
  if(!document||document.error||document.exceededTransferLimit||!Array.isArray(document.features)||document.features.length>128)fail('Invalid or incomplete Wayback point metadata');
  // Multiple intersecting contributors are preserved, rather than silently
  // assigning the first source acquisition time to the whole query region.
  return document.features.map(feature=>{
    const attributes=feature?.attributes;
    if(!attributes||typeof attributes!=='object'||Array.isArray(attributes))fail('Invalid Wayback source attributes');
    const rawDate=attributes.SRC_DATE2;
    const acquisitionDate=typeof rawDate==='number'&&Number.isFinite(rawDate)&&rawDate>0?new Date(rawDate).toISOString():null;
    return {acquisitionDate,acquisitionDateRaw:rawDate??null,provider:attributes.NICE_DESC??null,source:attributes.SRC_DESC??null,resolutionMeters:attributes.SAMP_RES??null,accuracyMeters:attributes.SRC_ACC??null,attributes};
  });
}

export async function fetchWaybackPointMetadata(version,point,zoom,signal) {
  const url=waybackMetadataRequest(version,point,zoom),result=await readPublicJson(url,signal,512*1024);
  return {...result,records:parseWaybackPointMetadata(result.document),url,point:[...point],zoom,versionId:version.id};
}

export function waybackReference(version,bounds,catalog,metadata=null) {
  if(!validQueryBounds(bounds)||catalog?.url!==WAYBACK_CATALOG_URL||typeof catalog.raw!=='string')fail('Choose a region and the real Wayback version directory');
  const matched=parseWaybackCatalog(JSON.parse(catalog.raw)).find(value=>value.id===version?.id);
  if(!matched||JSON.stringify(matched.publisherRecord)!==JSON.stringify(version.publisherRecord))fail('Selected Wayback version is not in this directory');
  if(metadata&&metadata.versionId!==version.id)fail('Wayback point metadata belongs to another version');
  return {schemaVersion:1,type:'geod-wayback-reference',datasetId:'wayback',bounds:[...bounds],version:matched,catalog:{url:catalog.url,raw:catalog.raw},pointMetadata:metadata,attribution:WAYBACK_ATTRIBUTION,termsUrl:WAYBACK_TERMS_URL,exportNote:WAYBACK_EXPORT_NOTE,containsImagery:false};
}

export function waybackOfficialUrl(version,bounds) {
  if(!version?.publisherRecord||!validQueryBounds(bounds))fail('Choose a Wayback version and region');
  parseWaybackCatalog({[version.releaseNumber]:version.publisherRecord});
  const url=new URL(WAYBACK_APP_URL);
  // The publisher app reads its version and extent from URL hash parameters.
  url.hash=new URLSearchParams({ext:bounds.join(','),active:String(version.releaseNumber)}).toString();
  return url.href;
}
