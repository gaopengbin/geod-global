import {desktopAvailable} from './runtime-client.js';
import {validateGeoPackage} from './geopackage-client.js';
import {validateShapefile} from './shapefile-client.js';
import {validateLocalOsm,validateLocalOsmFeatures} from './osm-local-client.js';
export const VECTOR_MAX_BYTES=20*1024*1024;
const idPattern=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const hashPattern=/^[a-f0-9]{64}$/;
const canonical=value=>value&&typeof value==='object'?(Array.isArray(value)?value.map(canonical):Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])]))):value;
const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const clean=(v,max)=>typeof v==='string'&&v.length>0&&v===v.trim()&&Array.from(v).length<=max&&!/[\u0000-\u001f\u007f-\u009f]/.test(v);
const utf8Length=v=>new TextEncoder().encode(v).length;
const arcgisFieldTypes=new Set(['esriFieldTypeOID','esriFieldTypeSmallInteger','esriFieldTypeInteger','esriFieldTypeSingle','esriFieldTypeDouble','esriFieldTypeString','esriFieldTypeDate','esriFieldTypeGUID','esriFieldTypeGlobalID','esriFieldTypeBigInteger','esriFieldTypeDateOnly','esriFieldTypeTimeOnly','esriFieldTypeTimestampOffset']);
export const OSM_LICENSE_URL='https://www.openstreetmap.org/copyright';
export const OVERPASS_PRESETS=Object.freeze({buildings:'Buildings',roads:'Roads',water:'Water',landuse:'Land use',pois:'Points of interest'});
export const OVERPASS_DESCRIPTION='OSM bounding-box selection with complete geometry and recursive members; not an exact polygon clip.';
const overpassFilters={buildings:['[building][building!=no]','["building:part"]["building:part"!=no]'],roads:['[highway][highway!=no]'],water:['[natural=water]','[waterway][waterway!=no]','[landuse=reservoir]','[landuse=basin]'],landuse:['[landuse][landuse!=no]'],pois:['[amenity][amenity!=no]','[shop][shop!=no]','[tourism][tourism!=no]','[leisure][leisure!=no]']};
const hasPreset=preset=>typeof preset==='string'&&Object.hasOwn(OVERPASS_PRESETS,preset);
export function validOverpassBounds(b) {
  if(!Array.isArray(b)||b.length!==4||!b.every(Number.isFinite)||b[0]<-180||b[2]>180||b[1]<-90||b[3]>90||b[0]>=b[2]||b[1]>=b[3]||b[2]-b[0]>1||b[3]-b[1]>1)return false;
  const radians=degrees=>degrees*Math.PI/180;
  return 6371.0088**2*radians(b[2]-b[0])*(Math.sin(radians(b[3]))-Math.sin(radians(b[1])))<=100;
}
export function validPublicFeatureUrl(raw) {
  if(typeof raw!=='string'||utf8Length(raw)>2048)return false;
  try {
    const u=new URL(raw),host=u.hostname;
    if(u.protocol!=='https:'||(u.port&&u.port!=='443')||u.username||u.password||u.href.includes('#')||!host.includes('.')||host==='localhost'||host.endsWith('.local')||host.endsWith('.localhost'))return false;
    if(/^\d+\.\d+\.\d+\.\d+$/.test(host)) {
      const [a,b,c]=host.split('.').map(Number);
      if(a===0||a===10||a===127||a>=224||a===169&&b===254||a===172&&b>=16&&b<=31||a===192&&(b===168||b===0&&(c===0||c===2))||a===100&&b>=64&&b<=127||a===198&&(b===18||b===19||b===51&&c===100)||a===203&&b===0&&c===113)return false;
    }
    return true;
  }catch{return false;}
}
export function validOverpassEndpoint(raw) {
  return validPublicFeatureUrl(raw)&&!new URL(raw).href.includes('?')&&new URL(raw).href===raw;
}
export function validOsmCopyright(text) {
  return typeof text==='string'&&utf8Length(text)<=16384&&text.includes('openstreetmap.org')&&text.includes('ODbL');
}
const overpassQuery=(preset,bbox)=>`[out:json][timeout:25][maxsize:16777216];(${overpassFilters[preset].map(filter=>`nwr${filter}(${bbox});`).join('')})->.selected;.selected out meta geom;.selected out count;.selected >> ->.dependencies;(.dependencies; - .selected;)->.dependencies;.dependencies out meta geom;.dependencies out count;`;
export function buildOverpassQuery(preset,bounds) {
  if(!hasPreset(preset)||!validOverpassBounds(bounds))throw new Error('Invalid OSM preset or query region.');
  return overpassQuery(preset,[bounds[1],bounds[0],bounds[3],bounds[2]].join(','));
}
function validOverpassQuery(query,preset,bounds) {
  if(typeof query!=='string'||utf8Length(query)>4096)return false;
  const decimal=/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i,expected=[bounds[1],bounds[0],bounds[3],bounds[2]];
  let matched=0;
  const normalized=query.replace(/\(([^()]+)\);/g,(full,value)=>{
    const coordinates=value.split(',');
    if(coordinates.length!==4||coordinates.some((v,i)=>!decimal.test(v)||Number(v)!==expected[i]))return full;
    matched++;return '(__bbox__);';
  });
  return matched===overpassFilters[preset].length&&normalized===overpassQuery(preset,'__bbox__');
}
function validRecordedArea(area,bounds) {
  if(area===undefined||area===null)return true;
  if(!plain(area)||Object.keys(area).length!==2||!['Polygon','MultiPolygon'].includes(area.type))return false;
  const polygons=area.type==='Polygon'?[area.coordinates]:area.coordinates;let count=0;
  if(!Array.isArray(polygons)||!polygons.length||polygons.length>500)return false;
  return polygons.every(polygon=>Array.isArray(polygon)&&polygon.length>0&&polygon.length<=1000&&polygon.every(ring=>{
    if(!Array.isArray(ring)||ring.length<4||(count+=ring.length)>30000||ring.some(p=>!Array.isArray(p)||p.length!==2||!p.every(Number.isFinite)||p[0]<bounds[0]||p[0]>bounds[2]||p[1]<bounds[1]||p[1]>bounds[3])||ring[0].some((n,i)=>n!==ring.at(-1)[i]))return false;
    return Math.abs(ring.slice(1).reduce((sum,p,i)=>sum+ring[i][0]*p[1]-p[0]*ring[i][1],0))>=1e-12;
  }));
}
export function validateOsmProvenance(source,count) {
  const counts=value=>plain(value)&&Object.keys(value).length===4&&['nodes','ways','relations','total'].every(k=>Number.isSafeInteger(value[k])&&value[k]>=0&&value[k]<=50000)&&value.nodes+value.ways+value.relations===value.total;
  const timestamp=value=>typeof value==='string'&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value)&&Number.isFinite(Date.parse(value));
  if(!plain(source)||!validOverpassEndpoint(source.serviceUrl)||!clean(source.serviceName,80)||!hasPreset(source.preset)||source.presetTitle!==OVERPASS_PRESETS[source.preset]
    ||!validOverpassBounds(source.requestedBounds)||!validRecordedArea(source.areaGeometry,source.requestedBounds)||!timestamp(source.requestedAt)||!timestamp(source.dataTimestamp)
    ||!validOverpassQuery(source.query,source.preset,source.requestedBounds)||!hashPattern.test(source.responseSha256)||!Number.isSafeInteger(source.bytes)||source.bytes<=0||source.bytes>VECTOR_MAX_BYTES
    ||!counts(source.elementCounts)||!counts(source.dependencyCounts)||source.elementCounts.total!==count||source.elementCounts.total+source.dependencyCounts.total>50000
    ||!clean(source.generator,256)||!source.generator.startsWith('Overpass API')||source.apiVersion!==0.6||!validOsmCopyright(source.copyrightText)||source.selection!=='overpass-bbox-full-geometry')throw new Error('Invalid OSM query provenance.');
  return source;
}
export const validArcgisLayerId=v=>typeof v==='string'&&/^(0|[1-9]\d*)$/.test(v)&&Number(v)<=4294967295;
export function arcgisServiceRoot(raw) {
  if(!validPublicFeatureUrl(raw))throw new Error('Invalid ArcGIS service URL.');
  const u=new URL(raw);
  u.pathname=u.pathname.replace(/\/+$/,'');
  if(u.href.includes('?')||!u.pathname.endsWith('/FeatureServer'))throw new Error('Invalid ArcGIS service URL.');
  return u.href;
}
export function validateArcgisLayer(layer) {
  if(!plain(layer)||!clean(layer.objectIdField,256)||!['esriGeometryPoint','esriGeometryMultipoint','esriGeometryPolyline','esriGeometryPolygon'].includes(layer.geometryType)
    ||!plain(layer.spatialReference)||!Object.keys(layer.spatialReference).length||utf8Length(JSON.stringify(layer.spatialReference))>65536
    ||!Array.isArray(layer.fields)||!layer.fields.length||layer.fields.length>512
    ||layer.fields.some(f=>!plain(f)||!clean(f.name,256)||typeof f.alias!=='string'||Array.from(f.alias).length>256||!arcgisFieldTypes.has(f.fieldType))
    ||new Set(layer.fields.map(f=>f.name)).size!==layer.fields.length||layer.fields.filter(f=>f.fieldType==='esriFieldTypeOID').length!==1||!layer.fields.some(f=>f.name===layer.objectIdField&&f.fieldType==='esriFieldTypeOID')
    ||!Number.isSafeInteger(layer.maxRecordCount)||layer.maxRecordCount<=0||layer.maxRecordCount>1000000||!hashPattern.test(layer.metadataSha256)||typeof layer.copyrightText!=='string'||utf8Length(layer.copyrightText)>16384)throw new Error('Invalid ArcGIS layer metadata.');
  return layer;
}
const wfsEpsg4326='urn:ogc:def:crs:EPSG::4326',wfsCrs84='urn:ogc:def:crs:OGC:1.3:CRS84';
const wfsIntegerTypes=new Set(['integer','long','int','short','byte','unsignedLong','unsignedInt','unsignedShort','unsignedByte','positiveInteger','nonNegativeInteger','negativeInteger','nonPositiveInteger']);
const wfsFieldTypes=new Set(['string','normalizedString','token','anyURI','boolean',...wfsIntegerTypes,'decimal','double','float','date','dateTime','time']);
export const validWfsQName=value=>typeof value==='string'&&value.length<=160&&/^[A-Za-z_][A-Za-z\d_.-]*(?::[A-Za-z_][A-Za-z\d_.-]*)?$/.test(value);
export const validWfsText=(value,max=16384)=>typeof value==='string'&&utf8Length(value)<=max&&!/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(value);
const wfsFormatId=mime=>typeof mime==='string'?({'application/gml+xml;version=3.2':'gml32','text/xml;subtype=gml/3.2.1':'gml32',gml32:'gml32','application/json':'geojson','application/geo+json':'geojson','application/json;subtype=geojson':'geojson',json:'geojson'}[mime.toLowerCase().replace(/\s/g,'')]):undefined;
export function validateWfsLayer(layer) {
  if(!plain(layer)||!validWfsQName(layer.typeName)||!clean(layer.namespace,2048)||!clean(layer.defaultCrs,256)
    ||!Array.isArray(layer.otherCrs)||layer.otherCrs.length>64||layer.otherCrs.some(value=>!clean(value,256))
    ||!Array.isArray(layer.formats)||!layer.formats.length||layer.formats.length>2||layer.formats.some(format=>!plain(format)||!clean(format.mime,120)||wfsFormatId(format.mime)!==format.id||!['gml32','geojson'].includes(format.id))
    ||new Set(layer.formats.map(format=>format.id)).size!==layer.formats.length||layer.defaultFormat!==(layer.formats.some(format=>format.id==='gml32')?'gml32':'geojson'))throw new Error('Invalid WFS feature type metadata.');
  return layer;
}
export function validateWfsSchema(schema) {
  const ncname=value=>typeof value==='string'&&utf8Length(value)<=256&&/^[_\p{Alphabetic}][_\p{Alphabetic}\p{N}.-]*$/u.test(value);
  if(!plain(schema)||typeof schema.namespace!=='string'||!schema.namespace.length||utf8Length(schema.namespace)>2048||/[\u0000-\u001f\u007f-\u009f]/.test(schema.namespace)
    ||!ncname(schema.elementName)||!ncname(schema.geometryField)||!['Point','LineString','Polygon','MultiPoint','MultiLineString','MultiPolygon','GeometryCollection','Geometry'].includes(schema.geometryType)
    ||typeof schema.geometryNullable!=='boolean'||typeof schema.geometryOptional!=='boolean'||!hashPattern.test(schema.sha256)||!Array.isArray(schema.fields)||schema.fields.length>512
    ||schema.fields.some(field=>!plain(field)||!ncname(field.name)||!wfsFieldTypes.has(field.fieldType)||typeof field.nullable!=='boolean'||typeof field.optional!=='boolean')
    ||new Set([schema.geometryField,...schema.fields.map(field=>field.name)]).size!==schema.fields.length+1)throw new Error('Invalid WFS feature schema.');
  return schema;
}
export function validateWfsProvenance(source,count) {
  const fail=()=>{throw new Error('Invalid WFS query provenance.');};
  const bounds=source?.requestedBounds,wfs=source?.wfs;
  if(!plain(source)||!plain(wfs)||source.arcgis!==undefined&&source.arcgis!==null||source.overpass!==undefined&&source.overpass!==null
    ||!validOverpassEndpoint(source.serviceUrl)||!clean(source.serviceName,80)||!clean(source.collectionTitle,240)||!validWfsQName(source.collectionId)
    ||source.selection!=='wfs-bbox-full-features'||!Number.isSafeInteger(count)||count<0||count>50000||source.featureCount!==count||source.numberMatched!==count||typeof source.requestedAt!=='string'||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(source.requestedAt)||!Number.isFinite(Date.parse(source.requestedAt))
    ||!Array.isArray(bounds)||bounds.length!==4||!bounds.every(Number.isFinite)||bounds[0]<-180||bounds[2]>180||bounds[1]<-90||bounds[3]>90||bounds[0]>=bounds[2]||bounds[1]>=bounds[3]
    ||!validRecordedArea(source.areaGeometry,bounds)||!Array.isArray(source.licenseLinks)||source.licenseLinks.length||wfs.version!=='2.0.0'||!hashPattern.test(wfs.capabilitiesSha256)
    ||wfs.matchedCount!==count||typeof wfs.pagingSupported!=='boolean'||wfs.rawArchiveVersion!==1||!Number.isSafeInteger(wfs.pageSize)||wfs.pageSize<1||wfs.pageSize>200||!validWfsText(wfs.fees)||!validWfsText(wfs.accessConstraints))fail();
  validateWfsLayer(wfs.layer);validateWfsSchema(wfs.schema);
  if(wfs.layer.typeName!==source.collectionId||wfs.schema.namespace!==wfs.layer.namespace||wfs.schema.elementName!==source.collectionId.split(':').at(-1)
    ||!plain(wfs.format)||!wfs.layer.formats.some(format=>format.id===wfs.format.id&&format.mime===wfs.format.mime)||wfs.requestCrs!==wfsEpsg4326||wfs.responseCrs!==(wfs.format.id==='gml32'?wfsEpsg4326:wfsCrs84)
    ||wfs.sortField!==(wfs.schema.fields.find(field=>['id','fid','objectid'].includes(field.name.toLowerCase())&&wfsIntegerTypes.has(field.fieldType)&&!field.nullable&&!field.optional)?.name??null))fail();
  let bytes=0;
  const receipt=(page,expected)=>{
    if(!plain(page)||page.parameters!==undefined&&page.parameters!==null||typeof page.url!=='string'||utf8Length(page.url)>8192||!hashPattern.test(page.sha256)||!Number.isSafeInteger(page.bytes)||page.bytes<=0||page.bytes>VECTOR_MAX_BYTES||!Number.isSafeInteger(page.returned)||page.returned<0||page.returned>50000)fail();
    const url=new URL(page.url),parameters=[...url.searchParams];url.search='';
    if(url.href!==source.serviceUrl||parameters.length!==Object.keys(expected).length||parameters.some(([key,value])=>{
      if(key!=='bbox')return expected[key]!==value;
      const pieces=value.split(','),coordinates=[bounds[1],bounds[0],bounds[3],bounds[2]];
      return !Object.hasOwn(expected,key)||pieces.length!==5||pieces[4]!==wfsEpsg4326||pieces.slice(0,4).some((n,i)=>!/^\-?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(n)||Number(n)!==coordinates[i]);
    })||new Set(parameters.map(([key])=>key)).size!==parameters.length)fail();
    bytes+=page.bytes;
  };
  const base={service:'WFS',version:'2.0.0'},schemaParameters={...base,request:'DescribeFeatureType',typeNames:wfs.layer.typeName};
  const featureParameters={...base,request:'GetFeature',typeNames:wfs.layer.typeName,bbox:true};
  for(const page of [wfs.schemaReceipt,wfs.schemaAfterReceipt]){receipt(page,schemaParameters);if(page.returned!==0)fail();}
  if(wfs.schemaReceipt.sha256!==wfs.schema.sha256)fail();
  for(const page of [wfs.hitsBefore,wfs.hitsAfter]){receipt(page,{...featureParameters,resultType:'hits'});if(page.returned!==0)fail();}
  if(!Array.isArray(source.pages)||source.pages.length>250||!Array.isArray(wfs.verificationPages)||wfs.verificationPages.length!==source.pages.length||(count===0?source.pages.length!==0:source.pages.length===0)||!wfs.pagingSupported&&source.pages.length>1)fail();
  let total=0;
  for(let index=0;index<source.pages.length;index++) {
    const page=source.pages[index],verification=wfs.verificationPages[index];
    if(!plain(page)||page.returned===0||page.returned>wfs.pageSize)fail();
    const expected={...featureParameters,outputFormat:wfs.format.mime,srsName:wfs.requestCrs,startIndex:String(total),count:String(wfs.pageSize),...(wfs.sortField?{sortBy:`${wfs.sortField} A`}:{})};
    receipt(page,expected);receipt(verification,expected);
    if(verification.returned!==page.returned)fail();total+=page.returned;
  }
  if(total!==count||bytes>VECTOR_MAX_BYTES)fail();
  return source;
}
export function validateFeatureProvenance(source,count) {
  const publicUrl=raw=>{try{const u=new URL(raw);return typeof raw==='string'&&raw.length<=2048&&u.protocol==='https:'&&(!u.port||u.port==='443')&&!u.username&&!u.password&&!u.hash;}catch{return false;}};
  const bounds=source?.requestedBounds;
  if(source?.wfs!==undefined&&source.wfs!==null)return validateWfsProvenance(source,count);
  if(source?.arcgis!==undefined&&source.arcgis!==null) {
    const a=source.arcgis;
    const fail=()=>{throw new Error('Invalid ArcGIS query provenance.');};
    if(!plain(a))fail();
    const layer=a.layer,root=arcgisServiceRoot(source.serviceUrl);
    if(source.selection!=='bbox-full-features'||!clean(source.serviceName,80)||!clean(source.collectionTitle,240)||!validArcgisLayerId(source.collectionId)||source.featureCount!==count||!Number.isSafeInteger(count)||count<0||count>5000||source.numberMatched!==count||!Number.isFinite(Date.parse(source.requestedAt))
      ||!Array.isArray(bounds)||bounds.length!==4||!bounds.every(Number.isFinite)||bounds[0]<-180||bounds[2]>180||bounds[1]<-90||bounds[3]>90||bounds[0]>=bounds[2]||bounds[1]>=bounds[3]
      ||!Array.isArray(source.licenseLinks)||source.licenseLinks.length)fail();
    validateArcgisLayer(layer);
    if(!Array.isArray(a.objectIds)||a.objectIds.length!==count||a.objectIds.some((id,i)=>!Number.isSafeInteger(id)||id<0||(i>0&&id<=a.objectIds[i-1]))||!Array.isArray(a.idReceipts)||a.idReceipts.length!==2||!Array.isArray(a.countReceipts)||a.countReceipts.length!==2||!Array.isArray(source.pages)||source.pages.length>25||(count===0?source.pages.length!==0:source.pages.length===0))fail();
    const endpoint=`${root}/${source.collectionId}/query`;
    let bytes=0;
    const receipt=p=>{
      if(!plain(p)||p.url!==endpoint||!hashPattern.test(p.sha256)||!Number.isSafeInteger(p.bytes)||p.bytes<=0||p.bytes>VECTOR_MAX_BYTES||!Number.isSafeInteger(p.returned)||p.returned<0||p.returned>5000||!plain(p.parameters)||Object.values(p.parameters).some(v=>typeof v!=='string'))fail();
      bytes+=p.bytes;return p.parameters;
    };
    const expectedIds={geometry:bounds.join(','),geometryType:'esriGeometryEnvelope',inSR:'4326',spatialRel:'esriSpatialRelIntersects',where:'1=1',returnIdsOnly:'true',f:'json'};
    const expectedCount={...expectedIds,returnCountOnly:'true'};delete expectedCount.returnIdsOnly;
    // Rust and JavaScript format very small/large floats differently; compare
    // decimal bounds numerically without accepting hex or whitespace coercions.
    const exact=(parameters,expected)=>Object.keys(parameters).length===Object.keys(expected).length&&Object.entries(parameters).every(([key,value])=>key==='geometry'?typeof expected.geometry==='string'&&value.split(',').length===4&&value.split(',').every((v,i)=>/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(v)&&Number(v)===bounds[i]):expected[key]===value);
    for(const [receipts,expected]of [[a.idReceipts,expectedIds],[a.countReceipts,expectedCount]])for(const p of receipts)if(!exact(receipt(p),expected)||p.returned!==count)fail();
    const received=[],identities=new Set();
    for(const p of source.pages) {
      const parameters=receipt(p),ids=typeof parameters.objectIds==='string'?parameters.objectIds.split(',').map(Number):[];
      if(!ids.length||ids.length>Math.min(200,layer.maxRecordCount)||ids.some(id=>!Number.isSafeInteger(id)||id<0)||parameters.objectIds!==ids.join(',')||p.returned!==ids.length||!exact(parameters,{f:'geojson',objectIds:ids.join(','),outFields:'*',outSR:'4326',returnGeometry:'true'}))fail();
      const identity=p.url+JSON.stringify(Object.entries(parameters).sort(([a],[b])=>a.localeCompare(b)));if(identities.has(identity))fail();identities.add(identity);received.push(...ids);
    }
    if(bytes>VECTOR_MAX_BYTES||received.length!==count||received.some((id,i)=>id!==a.objectIds[i]))fail();
    return source;
  }
  if(!source||source.selection!=='bbox-full-features'||!publicUrl(source.serviceUrl)||typeof source.serviceName!=='string'||typeof source.collectionId!=='string'||typeof source.collectionTitle!=='string'
    ||source.featureCount!==count||(source.numberMatched!==null&&source.numberMatched!==count)||!Number.isFinite(Date.parse(source.requestedAt))
    ||!Array.isArray(bounds)||bounds.length!==4||!bounds.every(Number.isFinite)||bounds[0]<-180||bounds[2]>180||bounds[1]<-90||bounds[3]>90||bounds[0]>=bounds[2]||bounds[1]>=bounds[3]
    ||!Array.isArray(source.pages)||!source.pages.length||source.pages.length>25||source.pages.some(p=>!plain(p)||p.parameters!==undefined&&p.parameters!==null||!publicUrl(p.url)||new URL(p.url).origin!==new URL(source.serviceUrl).origin||!hashPattern.test(p.sha256)||!Number.isSafeInteger(p.bytes)||p.bytes<=0||p.bytes>VECTOR_MAX_BYTES||!Number.isSafeInteger(p.returned)||p.returned<0||p.returned>50000)
    ||source.pages.reduce((n,p)=>n+p.bytes,0)>VECTOR_MAX_BYTES||source.pages.reduce((n,p)=>n+p.returned,0)<count||new Set(source.pages.map(p=>p.url)).size!==source.pages.length
    ||!Array.isArray(source.licenseLinks)||source.licenseLinks.length>16||source.licenseLinks.some(raw=>{try{const u=new URL(raw);return !['http:','https:'].includes(u.protocol)||u.username||u.password;}catch{return true;}}))throw new Error('Invalid feature query provenance.');
  return source;
}
export function validateVectorAsset(asset) {
  const integer=(n,max)=>Number.isSafeInteger(n)&&n>=0&&n<=max;
  if(!asset||!idPattern.test(asset.id)||typeof asset.name!=='string'||asset.name!==asset.name.trim()||!asset.name||Array.from(asset.name).length>120||/[\u0000-\u001f\u007f-\u009f]/.test(asset.name)
    ||!['geojson','overpass-json','wfs-snapshot','geopackage','shapefile','osm-xml','osm-pbf'].includes(asset.format)||!['managed','reference'].includes(asset.storageMode)
    ||asset.crs!=='EPSG:4326'||!hashPattern.test(asset.sourceSha256)||!hashPattern.test(asset.geojsonSha256)
    ||!integer(asset.bytes,VECTOR_MAX_BYTES)||!asset.bytes||!integer(asset.featureCount,50000)||!integer(asset.coordinateCount,500000)
    ||!asset.geometryCounts||typeof asset.geometryCounts!=='object'||Array.isArray(asset.geometryCounts)
    ||Object.entries(asset.geometryCounts).some(([key,value])=>!['Point','MultiPoint','LineString','MultiLineString','Polygon','MultiPolygon','GeometryCollection'].includes(key)||!integer(value,500000)||!value)
    ||(asset.bounds!==null&&(!Array.isArray(asset.bounds)||asset.bounds.length!==4||!asset.bounds.every(Number.isFinite)
      ||asset.bounds[0]>asset.bounds[2]||asset.bounds[1]>asset.bounds[3]||asset.bounds[0]<-180||asset.bounds[2]>180||asset.bounds[1]<-90||asset.bounds[3]>90))
    ||(asset.coordinateCount>0)!==(asset.bounds!==null)||!Number.isFinite(Date.parse(asset.createdAt))
    ||(asset.dataTimestamp!==null&&!Number.isFinite(Date.parse(asset.dataTimestamp)))
    ||(asset.attribution!==null&&typeof asset.attribution!=='string')
    ||(['overpass-json','osm-xml','osm-pbf'].includes(asset.format)?(asset.attribution!=='© OpenStreetMap contributors'||asset.licenseUrl!=='https://www.openstreetmap.org/copyright'||asset.format==='overpass-json'&&asset.dataTimestamp===null)
      :(asset.attribution!==null||asset.licenseUrl!==null||asset.dataTimestamp!==null))) {
    throw new Error('The vector service returned invalid source or geometry metadata.');
  }
  if(asset.remoteSource!==undefined&&asset.remoteSource!==null) {
    if(asset.format!==(asset.remoteSource.wfs!==undefined&&asset.remoteSource.wfs!==null?'wfs-snapshot':'geojson')||asset.storageMode!=='managed')throw new Error('Invalid feature query storage.');
    validateFeatureProvenance(asset.remoteSource,asset.featureCount);
  }
  if((asset.format==='geopackage')!==Boolean(asset.geoPackage))throw new Error('Missing or unexpected GeoPackage source metadata.');
  if(asset.geoPackage) {
    if(asset.remoteSource||asset.osmSource||asset.osmConversion||asset.shapefile||asset.localOsm)throw new Error('GeoPackage original files require distinct provenance.');
    validateGeoPackage(asset.geoPackage,asset.featureCount);
  }
  if((asset.format==='shapefile')!==Boolean(asset.shapefile))throw new Error('Missing or unexpected Shapefile source metadata.');
  if(asset.shapefile){if(asset.geoPackage||asset.remoteSource||asset.osmSource||asset.osmConversion||asset.localOsm)throw new Error('Shapefile original files require distinct provenance.');validateShapefile(asset.shapefile,asset.featureCount);}
  if(['osm-xml','osm-pbf'].includes(asset.format)!==Boolean(asset.localOsm))throw new Error('Missing or unexpected local OSM source metadata.');
  if(asset.localOsm){validateLocalOsm(asset.localOsm,asset.featureCount);if(asset.geoPackage||asset.shapefile||asset.remoteSource||asset.osmSource||asset.osmConversion||asset.format!==(asset.localOsm.encoding==='xml'?'osm-xml':'osm-pbf')||asset.dataTimestamp!==asset.localOsm.datasetTimestamp)throw new Error('Local OSM original files require distinct provenance.');}
  if(asset.format==='wfs-snapshot'&&!asset.remoteSource?.wfs)throw new Error('Missing WFS original response provenance.');
  if(asset.osmConversion!==undefined&&asset.osmConversion!==null&&(asset.osmConversion!==2||asset.format!=='overpass-json'))throw new Error('Invalid OSM geometry conversion version.');
  if(asset.osmSource!==undefined&&asset.osmSource!==null) {
    if(asset.remoteSource!==undefined&&asset.remoteSource!==null||asset.format!=='overpass-json'||asset.storageMode!=='managed'||asset.osmConversion!==2)throw new Error('Invalid OSM query storage.');
    validateOsmProvenance(asset.osmSource,asset.featureCount);
    if(asset.sourceSha256!==asset.osmSource.responseSha256||asset.bytes!==asset.osmSource.bytes||asset.dataTimestamp!==asset.osmSource.dataTimestamp)throw new Error('OSM source metadata does not match the saved response.');
  }
  return asset;
}
export function validateVectorInspection(data,id) {
  validateVectorAsset(data?.asset);
  if(data.asset.id!==id||data.geojson?.type!=='FeatureCollection'||!Array.isArray(data.geojson.features)
    ||data.geojson.features.length!==data.asset.featureCount||data.geojson.features.some(f=>f?.type!=='Feature')
    ||data.asset.remoteSource&&JSON.stringify(canonical(data.geojson.geodSource))!==JSON.stringify(canonical(data.asset.remoteSource))
    ||data.asset.osmSource&&JSON.stringify(canonical(data.geojson.geodOsmSource))!==JSON.stringify(canonical(data.asset.osmSource))) {
    throw new Error('The vector content does not match the selected file.');
  }
  if(data.asset.geoPackage) {
    if(JSON.stringify(canonical(data.geojson.geodGeoPackage))!==JSON.stringify(canonical(data.asset.geoPackage)))throw new Error('GeoPackage conversion does not match its saved source.');
    const layers=new Map(data.asset.geoPackage.layers.map(l=>[l.table,0]));
    for(const f of data.geojson.features){if(!layers.has(f.geodLayer))throw new Error('Unknown GeoPackage feature layer.');layers.set(f.geodLayer,layers.get(f.geodLayer)+1);}
    if(data.asset.geoPackage.layers.some(l=>layers.get(l.table)!==l.featureCount))throw new Error('GeoPackage layer counts differ from the selected file.');
  }
  if(data.asset.shapefile){
    if(JSON.stringify(canonical(data.geojson.geodShapefile))!==JSON.stringify(canonical(data.asset.shapefile)))throw new Error('Shapefile conversion does not match its saved source.');
    const layers=new Map(data.asset.shapefile.layers.map(l=>[l.table,{count:0,deleted:0,empty:0}]));
    for(const f of data.geojson.features){const l=layers.get(f.geodLayer);if(!l||!Number.isSafeInteger(f.id)||f.id<0||f.id!==l.count||f.geodDeleted!==undefined&&f.geodDeleted!==true)throw new Error('Invalid Shapefile record identity or source layer.');l.count++;l.deleted+=f.geodDeleted?1:0;l.empty+=f.geometry===null?1:0;}
    if(data.asset.shapefile.layers.some(l=>{const n=layers.get(l.table);return n.count!==l.featureCount||n.deleted!==l.deletedCount||n.empty!==l.nullGeometryCount;}))throw new Error('Shapefile record counts differ from the selected file.');
  }
  if(data.asset.localOsm){if(JSON.stringify(canonical(data.geojson.geodLocalOsm))!==JSON.stringify(canonical(data.asset.localOsm)))throw new Error('Local OSM conversion does not match its saved source.');validateLocalOsmFeatures(data.geojson.features,data.asset.localOsm);}
  return data;
}
export async function vectorRequest(operation,payload={},signal) {
  const checkAbort=()=>{if(signal?.aborted)throw new DOMException('Vector request aborted','AbortError');};
  checkAbort();
  const commands={list:'list_vectors',import:'import_vector',open:'open_vector',inspect:'inspect_vector',forget:'forget_vector',export:'export_vector'};
  if(!commands[operation])throw new Error('Unknown vector operation.');
  if(['inspect','forget','export'].includes(operation)&&!idPattern.test(payload.id))throw new Error('Invalid vector identifier.');
  let value;
  if(desktopAvailable()) {
    try {value=await window.__TAURI__.core.invoke(commands[operation],operation==='import'?{request:payload}:payload);}
    catch(error) {throw error instanceof Error?error:new Error(typeof error==='string'?error:'Vector request failed.');}
  }else{
    if(['open','export'].includes(operation))throw new Error('Native file selection requires the desktop application.');
    const resource=operation==='list'||operation==='import'?'/vectors':`/vectors/${payload.id}${operation==='forget'?'/forget':''}`;
    const mutation=['import','forget'].includes(operation);
    const response=await fetch('http://127.0.0.1:4318'+resource,{method:mutation?'POST':'GET',signal,
      headers:mutation?{'Content-Type':'application/json','X-GeoD-Client':'geod-global'}:undefined,
      body:mutation?JSON.stringify(operation==='import'?payload:{}):undefined});
    value=await response.json();if(!response.ok)throw new Error(value.error||`Vector service returned HTTP ${response.status}`);
  }
  checkAbort();
  if(operation==='list') {if(!Array.isArray(value)||value.length>1024)throw new Error('Invalid vector registry.');value.forEach(validateVectorAsset);}
  if(operation==='import'||operation==='open'&&value!==null)validateVectorAsset(value);
  if(operation==='inspect')validateVectorInspection(value,payload.id);
  return value;
}
export async function importVectorFile(file) {
  if(!file||file.size<1||file.size>VECTOR_MAX_BYTES)throw new Error('Vector file must contain at most 20 MiB.');
  if(/\.(gpkg|zip|osm|xml|pbf)$/i.test(file.name)) {
    const response=await fetch('http://127.0.0.1:4318/vectors/import-file?name='+encodeURIComponent(file.name),{method:'POST',headers:{'Content-Type':'application/octet-stream','X-GeoD-Client':'geod-global'},body:file,signal:AbortSignal.timeout(190000)});
    const value=await response.json();if(!response.ok)throw new Error(value.error||'Vector file import failed.');validateVectorAsset(value);const ext=file.name.toLowerCase().split('.').pop();const expected={gpkg:'geopackage',zip:'shapefile',osm:'osm-xml',xml:'osm-xml',pbf:'osm-pbf'}[ext];if(value.format!==expected)throw new Error('The selected file returned a different vector format.');return value;
  }
  const text=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(await file.arrayBuffer());
  return vectorRequest('import',{name:file.name,text});
}
export async function exportVector(id,original=false) {
  if(desktopAvailable())return vectorRequest('export',original?{id,original:true}:{id});
  const data=await vectorRequest('inspect',{id});
  let blob,suffix='geojson';
  if(original) {
    const r=await fetch(`http://127.0.0.1:4318/vectors/${id}/source`,{signal:AbortSignal.timeout(190000)});if(!r.ok)throw new Error((await r.json()).error||'Original vector export failed.');
    const expected=data.asset.format==='geopackage'?'application/geopackage+sqlite3':data.asset.format==='shapefile'?'application/zip':data.asset.format==='osm-xml'?'application/xml':data.asset.format==='osm-pbf'?'application/vnd.openstreetmap.data+pbf':'application/json';if(r.headers.get('content-type')!==expected)throw new Error('Original vector file format changed.');
    const bytes=new Uint8Array(await r.arrayBuffer()),hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');
    if(bytes.length!==data.asset.bytes||hash!==data.asset.sourceSha256)throw new Error('Original vector file checksum changed.');
    blob=new Blob([bytes],{type:expected});suffix=data.asset.format==='geopackage'?'gpkg':data.asset.format==='shapefile'?'zip':data.asset.format==='osm-xml'?'osm':data.asset.format==='osm-pbf'?'pbf':'json';
  }else{blob=new Blob([JSON.stringify(data.geojson)],{type:'application/geo+json'});}
  const url=URL.createObjectURL(blob);
  const a=document.createElement('a');a.href=url;a.download=data.asset.name.replace(/\.[^.]+$/,'').replace(/[<>:"/\\|?*]/g,'_')+'.'+suffix;
  document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);return true;
}
