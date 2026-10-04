import {test} from 'node:test';import assert from 'node:assert/strict';
import {validateFeatureService,validServiceUrl,validQueryBounds,featureRequest} from './features-client.js';
import {validateFeatureProvenance,validateVectorAsset,validateVectorInspection,validateOsmProvenance,validOverpassBounds,buildOverpassQuery,OVERPASS_PRESETS,OVERPASS_DESCRIPTION,OSM_LICENSE_URL,VECTOR_MAX_BYTES} from './vector-client.js';
const id='735f227b-5f95-473f-967b-077f3419bc68';
const service={id,name:'Public lake service',title:'Lakes',url:'https://example.com/api',connectedAt:'2026-10-02T00:00:00Z',collections:[{id:'lakes',title:'Lakes',description:'',itemsUrl:'https://example.com/api/collections/lakes/items?f=json',licenseLinks:[]}]};
test('feature collection registry rejects duplicate identities, cross-origin items and executable license URLs',()=>{
  assert.equal(validateFeatureService(service),service);
  for(const change of [s=>s.collections.push(s.collections[0]),s=>s.collections[0].itemsUrl='https://other.example/items',s=>s.collections[0].licenseLinks=['javascript:alert(1)']]) {const s=structuredClone(service);change(s);assert.throws(()=>validateFeatureService(s));}
});
test('query requires an explicit non-empty WGS84 region and does not fall back to the world',async()=>{
  assert.ok(validQueryBounds([0,0,10,10]));for(const b of [undefined,[180,0,-180,10],[0,0,0,10],[0,-91,10,10]])assert.equal(validQueryBounds(b),false);
  await assert.rejects(()=>featureRequest('query',{serviceId:id,collectionId:'lakes',bounds:null}),/valid query/);
});
test('feature query provenance validates complete receipts and no private arbitrary attributes are treated as source',()=>{
  const p={serviceUrl:'https://example.com/api',serviceName:'Lakes',collectionId:'lakes',collectionTitle:'Lakes',requestedBounds:[0,0,10,10],requestedAt:'2026-10-02T00:00:00Z',selection:'bbox-full-features',featureCount:2,numberMatched:2,pages:[{url:'https://example.com/api/items?bbox=0,0,10,10',sha256:'a'.repeat(64),bytes:400,returned:2}],licenseLinks:[]};
  assert.equal(validateFeatureProvenance(p,2),p);
  for(const change of [s=>s.numberMatched=3,s=>s.pages[0].returned=1,s=>s.pages.push(s.pages[0]),s=>s.pages[0].parameters={f:'geojson'},s=>s.selection='clipped',s=>s.licenseLinks=['javascript:alert(1)']]) {const v=structuredClone(p);change(v);assert.throws(()=>validateFeatureProvenance(v,2));}
});
test('desktop connection and query use scoped native commands with structured requests',async()=>{
  const previous=globalThis.window;const calls=[];
  globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return service;}}}};
  try {await featureRequest('connect',{name:service.name,url:service.url});assert.deepEqual(calls,[{command:'connect_feature_service',payload:{request:{name:service.name,url:service.url}}}]);}finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
test('native string failures stay visible as recoverable errors',async()=>{
  const previous=globalThis.window;globalThis.window={__TAURI__:{core:{invoke:async()=>{throw 'Service DNS unavailable';}}}};
  try {await assert.rejects(()=>featureRequest('list'),error=>error instanceof Error&&error.message==='Service DNS unavailable');}
  finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});

const arcgisUrl='https://example.com/arcgis/rest/services/Lakes/FeatureServer';
const arcgisLayer={objectIdField:'OBJECTID',geometryType:'esriGeometryPolygon',spatialReference:{wkid:102100,latestWkid:3857},fields:[{name:'OBJECTID',alias:'Object ID',fieldType:'esriFieldTypeOID'},{name:'name',alias:'Name',fieldType:'esriFieldTypeString'}],maxRecordCount:2000,metadataSha256:'b'.repeat(64),copyrightText:'Example source'};
const arcgisService={...service,url:arcgisUrl,arcgis:{currentVersion:11.3,copyrightText:'Example source',metadataSha256:'a'.repeat(64),excludedLayers:[{id:'1',name:'3D lakes',reason:'Z and M geometry is unsupported'}]},collections:[{id:'0',title:'Lakes',description:'',itemsUrl:`${arcgisUrl}/0/query`,licenseLinks:[],arcgis:arcgisLayer}]};
function arcgisSource(ids=[3,7]) {
  const receipt=(parameters,returned)=>({url:`${arcgisUrl}/0/query`,sha256:'c'.repeat(64),bytes:200,returned,parameters});
  const selection={geometry:'0,0,10,10',geometryType:'esriGeometryEnvelope',inSR:'4326',spatialRel:'esriSpatialRelIntersects',where:'1=1',f:'json'};
  return {serviceUrl:arcgisUrl,serviceName:'Lakes',collectionId:'0',collectionTitle:'Lakes',requestedBounds:[0,0,10,10],requestedAt:'2026-10-02T00:00:00Z',areaGeometry:null,selection:'bbox-full-features',featureCount:ids.length,numberMatched:ids.length,licenseLinks:[],
    pages:ids.length?[receipt({f:'geojson',objectIds:ids.join(','),outFields:'*',outSR:'4326',returnGeometry:'true'},ids.length)]:[],
    arcgis:{layer:structuredClone(arcgisLayer),objectIds:[...ids],idReceipts:[receipt({...selection,returnIdsOnly:'true'},ids.length),receipt({...selection,returnIdsOnly:'true'},ids.length)],countReceipts:[receipt({...selection,returnCountOnly:'true'},ids.length),receipt({...selection,returnCountOnly:'true'},ids.length)]}};
}
function rejectChanges(fixture,changes,validate) {
  for(const [label,change]of changes) {const value=structuredClone(fixture);change(value);assert.throws(()=>validate(value),undefined,label);}
}

test('ArcGIS registry preserves layer schema, source CRS, attribution and excluded layers',()=>{
  assert.equal(validateFeatureService(arcgisService),arcgisService);
  const unicode=structuredClone(arcgisService);
  unicode.collections[0].arcgis.fields[1].alias='\u{1F30D}'.repeat(256);
  unicode.arcgis.excludedLayers[0].name='';
  assert.equal(validateFeatureService(unicode),unicode);
});

test('ArcGIS registry rejects malformed metadata and ambiguous layer identities',()=>{
  rejectChanges(arcgisService,[
    ['old service version',s=>s.arcgis.currentVersion=9.9],
    ['malformed metadata marker',s=>s.arcgis=false],
    ['wrong service digest',s=>s.arcgis.metadataSha256='invalid'],
    ['unsupported attribute type',s=>s.collections[0].arcgis.fields[1].fieldType='esriFieldTypeBlob'],
    ['multiple OID fields',s=>s.collections[0].arcgis.fields[1].fieldType='esriFieldTypeOID'],
    ['repeated field name',s=>s.collections[0].arcgis.fields[1].name='OBJECTID'],
    ['wrong OID field',s=>s.collections[0].arcgis.objectIdField='name'],
    ['unclean field name',s=>s.collections[0].arcgis.fields[1].name=' name'],
    ['unsupported geometry',s=>s.collections[0].arcgis.geometryType='esriGeometryMultipatch'],
    ['missing source CRS',s=>s.collections[0].arcgis.spatialReference={}],
    ['oversize CRS in UTF-8',s=>s.collections[0].arcgis.spatialReference={wkt:'\u5730'.repeat(22000)}],
    ['invalid record limit',s=>s.collections[0].arcgis.maxRecordCount=0],
    ['oversize layer attribution',s=>s.collections[0].arcgis.copyrightText='\u5730'.repeat(5462)],
    ['oversize service attribution',s=>s.arcgis.copyrightText='a'.repeat(16385)],
    ['empty layer metadata',s=>s.collections[0].arcgis=null],
    ['unexpected license inference',s=>s.collections[0].licenseLinks=['https://example.com/license']],
    ['overlapping exclusion',s=>s.arcgis.excludedLayers[0].id='0'],
    ['duplicate exclusions',s=>s.arcgis.excludedLayers.push(s.arcgis.excludedLayers[0])],
    ['non-numeric exclusion',s=>s.arcgis.excludedLayers[0].id='table'],
    ['unclean exclusion reason',s=>s.arcgis.excludedLayers[0].reason=' excluded'],
    ['too many combined layers',s=>s.arcgis.excludedLayers=Array.from({length:512},(_,i)=>({id:String(i+1),name:'Excluded',reason:'Unsupported geometry'}))],
  ],validateFeatureService);
});

test('ArcGIS endpoints stay on the canonical public FeatureServer and exact layer query',()=>{
  for(const url of ['https://127.0.0.1/FeatureServer','https://192.168.1.1/FeatureServer','https://100.64.1.1/FeatureServer','https://host.local/FeatureServer','https://host.localhost/FeatureServer','https://example.com/FeatureServer#','https://user:password@example.com/FeatureServer'])assert.equal(validServiceUrl(url),false,url);
  assert.equal(validServiceUrl('https://services.arcgis.com/example/FeatureServer'),true);
  rejectChanges(arcgisService,[
    ['query on root',s=>s.url+='?token=secret'],
    ['empty query on root',s=>s.url+='?'],
    ['unnormalized root',s=>s.url+='/'],
    ['wrong protocol endpoint',s=>s.url=s.url.replace('FeatureServer','MapServer')],
    ['different layer endpoint',s=>s.collections[0].itemsUrl=`${arcgisUrl}/1/query`],
    ['extra query parameter',s=>s.collections[0].itemsUrl+='?f=geojson'],
    ['cross-origin endpoint',s=>s.collections[0].itemsUrl='https://other.example/FeatureServer/0/query'],
    ['out-of-range layer ID',s=>{s.collections[0].id='4294967296';s.collections[0].itemsUrl=`${arcgisUrl}/4294967296/query`;}],
    ['noncanonical layer ID',s=>{s.collections[0].id='00';s.collections[0].itemsUrl=`${arcgisUrl}/00/query`;}],
  ],validateFeatureService);
});

test('ArcGIS zero matches require two independent count and ID checks but no feature pages',()=>{
  const source=arcgisSource([]);
  assert.equal(validateFeatureProvenance(source,0),source);
  rejectChanges(source,[
    ['missing before count',s=>s.arcgis.countReceipts.shift()],
    ['missing after IDs',s=>s.arcgis.idReceipts.pop()],
    ['nonzero independent count',s=>s.arcgis.countReceipts[1].returned=1],
    ['phantom object ID',s=>s.arcgis.objectIds.push(3)],
    ['unexpected empty batch',s=>s.pages=[{...s.arcgis.idReceipts[0],parameters:{f:'geojson',objectIds:'',outFields:'*',outSR:'4326',returnGeometry:'true'}}]],
    ['missing snapshot',s=>delete s.arcgis],
  ],s=>validateFeatureProvenance(s,0));
});

test('ArcGIS provenance ties exact membership to every batch and all receipt bytes',()=>{
  const source=arcgisSource();
  assert.equal(validateFeatureProvenance(source,2),source);
  const split=structuredClone(source);
  split.pages=source.arcgis.objectIds.map(objectId=>({...structuredClone(source.pages[0]),returned:1,parameters:{...source.pages[0].parameters,objectIds:String(objectId)}}));
  split.arcgis.layer.maxRecordCount=1;
  assert.equal(validateFeatureProvenance(split,2),split);
  rejectChanges(source,[
    ['wrong count',s=>s.numberMatched=3],
    ['count response changed',s=>s.arcgis.countReceipts[1].returned=3],
    ['ID response changed',s=>s.arcgis.idReceipts[1].returned=3],
    ['missing membership receipt',s=>s.arcgis.idReceipts.pop()],
    ['extra count receipt',s=>s.arcgis.countReceipts.push(s.arcgis.countReceipts[0])],
    ['duplicate object ID',s=>s.arcgis.objectIds=[3,3]],
    ['unsorted object IDs',s=>s.arcgis.objectIds=[7,3]],
    ['unsafe object ID',s=>s.arcgis.objectIds[1]=Number.MAX_SAFE_INTEGER+1],
    ['missing feature batch',s=>s.pages=[]],
    ['incomplete feature batch',s=>s.pages[0].returned=1],
    ['duplicate feature batch',s=>s.pages.push(s.pages[0])],
    ['changed batch ID',s=>s.pages[0].parameters.objectIds='3,8'],
    ['reordered batch IDs',s=>s.pages[0].parameters.objectIds='7,3'],
    ['duplicate batch IDs',s=>s.pages[0].parameters.objectIds='3,3'],
    ['oversize server batch',s=>s.arcgis.layer.maxRecordCount=1],
    ['wrong layer endpoint',s=>s.pages[0].url=`${arcgisUrl}/1/query`],
    ['receipt URL carries token',s=>s.arcgis.idReceipts[0].url+='?token=secret'],
    ['wrong selection bounds',s=>s.arcgis.countReceipts[1].parameters.geometry='0,0,11,10'],
    ['non-decimal bounds',s=>s.arcgis.countReceipts[1].parameters.geometry='0x0,0,10,10'],
    ['arbitrary filter',s=>s.arcgis.idReceipts[0].parameters.where='OBJECTID=3'],
    ['missing selection parameter',s=>delete s.arcgis.idReceipts[0].parameters.inSR],
    ['unexpected batch parameter',s=>s.pages[0].parameters.token='secret'],
    ['changed geometry CRS',s=>s.pages[0].parameters.outSR='3857'],
    ['partial attributes',s=>s.pages[0].parameters.outFields='OBJECTID'],
    ['false snapshot marker',s=>s.arcgis=false],
    ['snapshot removed from POST receipts',s=>delete s.arcgis],
    ['invalid source metadata',s=>s.arcgis.layer.fields[1].fieldType='unknown'],
    ['private service URL',s=>s.serviceUrl='https://127.0.0.1/FeatureServer'],
    ['out-of-range layer ID',s=>s.collectionId='4294967296'],
    ['unsupported inferred license',s=>s.licenseLinks=['https://example.com/license']],
    ['receipt budget overflow',s=>s.arcgis.countReceipts[1].bytes=VECTOR_MAX_BYTES],
  ],s=>validateFeatureProvenance(s,2));
});

test('ArcGIS floating point bounds retain valid Rust decimal receipts',()=>{
  const source=arcgisSource([]);
  source.requestedBounds=[0.0000001,0,0.0000002,10];
  for(const p of [...source.arcgis.idReceipts,...source.arcgis.countReceipts])p.parameters.geometry='0.0000001,0,0.0000002,10';
  assert.equal(validateFeatureProvenance(source,0),source);
});

test('empty ArcGIS query is accepted as a managed export with its complete provenance',async()=>{
  const remoteSource=arcgisSource([]);
  const asset={id,name:'Lakes.geojson',format:'geojson',storageMode:'managed',crs:'EPSG:4326',sourceSha256:'a'.repeat(64),geojsonSha256:'b'.repeat(64),bytes:600,featureCount:0,coordinateCount:0,geometryCounts:{},bounds:null,createdAt:'2026-10-02T00:00:00Z',attribution:null,licenseUrl:null,dataTimestamp:null,remoteSource};
  const previous=globalThis.window,calls=[];
  globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return asset;}}}};
  try {
    const payload={serviceId:id,collectionId:'0',bounds:[0,0,10,10]};
    assert.equal(await featureRequest('query',payload),asset);
    assert.deepEqual(calls,[{command:'query_features',payload:{request:payload}}]);
    const inspection={asset,geojson:{type:'FeatureCollection',features:[],geodSource:structuredClone(remoteSource)}};
    assert.equal(validateVectorInspection(inspection,id),inspection);
    inspection.geojson.geodSource.arcgis.countReceipts[1].sha256='d'.repeat(64);
    assert.throws(()=>validateVectorInspection(inspection,id),/does not match/);
  }finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
const osmUrl='https://example.com/overpass/api/interpreter';
const osmCopyright='Data from www.openstreetmap.org, available under ODbL.';
const osmService={...service,name:'Authorized OSM service',title:'Authorized OSM service',url:osmUrl,
  overpass:{generator:'Overpass API 0.7.62',apiVersion:0.6,metadataSha256:'d'.repeat(64),copyrightText:osmCopyright},
  collections:Object.entries(OVERPASS_PRESETS).map(([id,title])=>({id,title,description:OVERPASS_DESCRIPTION,itemsUrl:osmUrl,licenseLinks:[OSM_LICENSE_URL]}))};
function osmSource(bounds=[13.4,52.5,13.41,52.51],preset='buildings') {
  return {serviceUrl:osmUrl,serviceName:'Authorized OSM service',preset,presetTitle:OVERPASS_PRESETS[preset],requestedBounds:bounds,areaGeometry:null,requestedAt:'2026-10-02T00:00:00Z',query:buildOverpassQuery(preset,bounds),responseSha256:'e'.repeat(64),bytes:4000,
    elementCounts:{nodes:0,ways:1,relations:1,total:2},dependencyCounts:{nodes:10,ways:2,relations:0,total:12},dataTimestamp:'2026-10-01T23:59:00Z',generator:'Overpass API 0.7.62',apiVersion:0.6,copyrightText:osmCopyright,selection:'overpass-bbox-full-geometry'};
}
function osmAsset(source=osmSource()) {
  return {id,name:'OSM Buildings.json',format:'overpass-json',storageMode:'managed',crs:'EPSG:4326',sourceSha256:source.responseSha256,geojsonSha256:'f'.repeat(64),bytes:source.bytes,featureCount:source.elementCounts.total,coordinateCount:8,geometryCounts:{Polygon:2},bounds:source.requestedBounds,createdAt:source.requestedAt,attribution:'© OpenStreetMap contributors',licenseUrl:OSM_LICENSE_URL,dataTimestamp:source.dataTimestamp,osmConversion:2,osmSource:source};
}
test('Overpass registry contains exactly five bounded presets and an explicit canonical endpoint',()=>{
  assert.equal(validateFeatureService(osmService),osmService);
  rejectChanges(osmService,[
    ['ambiguous protocol',s=>s.arcgis=arcgisService.arcgis],['false metadata',s=>s.overpass=false],['unsupported API',s=>s.overpass.apiVersion=0.7],
    ['unknown generator',s=>s.overpass.generator='Other API'],['undeclared attribution',s=>delete s.overpass.copyrightText],['invented license',s=>s.overpass.copyrightText='Public domain'],
    ['changed metadata digest',s=>s.overpass.metadataSha256='invalid'],['oversized copyright',s=>s.overpass.copyrightText+='地'.repeat(6000)],
    ['private endpoint',s=>s.url='https://127.0.0.1/api/interpreter'],['query credentials',s=>s.url+='?token=secret'],['noncanonical endpoint',s=>s.url=s.url.replace('example.com','EXAMPLE.COM')],
    ['missing category',s=>s.collections.pop()],['repeated category',s=>s.collections[4]=s.collections[0]],['custom category',s=>s.collections[0].id='custom'],
    ['changed title',s=>s.collections[0].title='Anything'],['changed description',s=>s.collections[0].description='All world data'],['different request URL',s=>s.collections[0].itemsUrl+='?data=all'],
    ['wrong license',s=>s.collections[0].licenseLinks=['https://example.com/license']],['ArcGIS category marker',s=>s.collections[0].arcgis=arcgisLayer],
  ],validateFeatureService);
});
test('Overpass region guard enforces area and individual spans including polar regions',()=>{
  assert.equal(validOverpassBounds([13.4,52.5,13.41,52.51]),true);
  assert.equal(validOverpassBounds([0,89.9,1,90]),true);
  for(const b of [undefined,[0,0,0.1,0.1],[0,89.9,1.1,90],[0,88.8,0.01,90],[179.9,0,-179.9,0.01],[0,0,0,1]])assert.equal(validOverpassBounds(b),false,JSON.stringify(b));
});
test('OSM provenance binds exact preset query, response identity, selected elements and geometry dependencies',()=>{
  const source=osmSource();assert.equal(validateOsmProvenance(source,2),source);
  assert.equal(validateVectorAsset(osmAsset(source)).osmSource,source);
  rejectChanges(source,[
    ['wrong total',s=>s.elementCounts.total=3],['negative node count',s=>s.dependencyCounts.nodes=-1],['extra count field',s=>s.elementCounts.extra=1],
    ['count overflow',s=>s.dependencyCounts={nodes:50000,ways:0,relations:0,total:50000}],['fractional count',s=>s.elementCounts.ways=0.5],
    ['wrong preset title',s=>s.presetTitle='Roads'],['nonstring preset',s=>s.preset=['buildings']],['unbounded region',s=>s.requestedBounds=[0,0,1,1]],
    ['wrong bbox order',s=>s.query=s.query.replaceAll('52.5,13.4,52.51,13.41','13.4,52.5,13.41,52.51')],
    ['user query appended',s=>s.query+='node;out;'],['arbitrary filter',s=>s.query=s.query.replace('[building]','[amenity]')],['unsafe timeout',s=>s.query=s.query.replace('timeout:25','timeout:180')],
    ['no dependency completion count',s=>s.query=s.query.replace('.dependencies out count;','')],['wrong selection',s=>s.selection='bbox-full-features'],
    ['no raw response hash',s=>s.responseSha256=''],['empty raw response',s=>s.bytes=0],['oversize response',s=>s.bytes=VECTOR_MAX_BYTES+1],
    ['invalid timestamp',s=>s.dataTimestamp='today'],['wrong API version',s=>s.apiVersion=0.7],['undeclared copyright',s=>s.copyrightText='No license'],
    ['recorded polygon outside bounds',s=>s.areaGeometry={type:'Polygon',coordinates:[[[0,0],[1,0],[1,1],[0,0]]]}],
  ],s=>validateOsmProvenance(s,2));
});
test('OSM query accepts Rust decimal formatting without allowing extra expressions',()=>{
  const source=osmSource([0.0000001,0,0.0000002,0.0000001]);
  source.query=source.query.replaceAll('1e-7','0.0000001').replaceAll('2e-7','0.0000002');assert.equal(validateOsmProvenance(source,2),source);
  source.query=source.query.replace('0.0000001','0x1');assert.throws(()=>validateOsmProvenance(source,2));
});
test('a completed empty OSM query retains its raw response and both zero element groups',()=>{
  const source=osmSource();source.elementCounts={nodes:0,ways:0,relations:0,total:0};source.dependencyCounts={nodes:0,ways:0,relations:0,total:0};
  const asset={...osmAsset(source),featureCount:0,coordinateCount:0,geometryCounts:{},bounds:null};assert.equal(validateVectorAsset(asset),asset);
  assert.equal(validateVectorInspection({asset,geojson:{type:'FeatureCollection',features:[],geodOsmSource:source}},id).asset,asset);
  delete asset.osmSource.dependencyCounts;assert.throws(()=>validateVectorAsset(asset),/OSM query provenance/);
});
test('online OSM results require raw response metadata and conversion version while legacy imports remain valid',()=>{
  const asset=osmAsset();
  rejectChanges(asset,[
    ['mismatched raw bytes',v=>v.bytes++],['mismatched raw hash',v=>v.sourceSha256='a'.repeat(64)],['mismatched database time',v=>v.dataTimestamp='2020-01-01T00:00:00Z'],
    ['wrong geometry conversion',v=>v.osmConversion=1],['missing conversion version',v=>delete v.osmConversion],['reference instead of managed',v=>v.storageMode='reference'],
    ['competing remote provenance',v=>v.remoteSource=arcgisSource()],['false source marker',v=>v.osmSource=false],
  ],validateVectorAsset);
  const imported={...asset,storageMode:'reference'};delete imported.osmSource;delete imported.osmConversion;assert.equal(validateVectorAsset(imported),imported);
  const inspection={asset,geojson:{type:'FeatureCollection',features:[{type:'Feature'},{type:'Feature'}],geodOsmSource:structuredClone(asset.osmSource)}};
  assert.equal(validateVectorInspection(inspection,id),inspection);inspection.geojson.geodOsmSource.query+=' ';assert.throws(()=>validateVectorInspection(inspection,id),/does not match/);
});
test('feature extraction accepts a validated native OSM result without OGC provenance or pageSize',async()=>{
  const value=osmAsset(),previous=globalThis.window,calls=[];
  globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return value;}}}};
  try {const payload={serviceId:id,collectionId:'buildings',bounds:value.osmSource.requestedBounds,areaGeometry:null};assert.equal(await featureRequest('query',payload),value);assert.deepEqual(calls,[{command:'query_features',payload:{request:payload}}]);}
  finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});

const wfsUrl='https://example.com/ows/wfs';
const wfsLayer={typeName:'demo:lakes',namespace:'demo',defaultCrs:'urn:ogc:def:crs:EPSG::3857',otherCrs:[],formats:[{id:'geojson',mime:'application/json'},{id:'gml32',mime:'application/gml+xml; version=3.2'}],defaultFormat:'gml32'};
const wfsService={...service,url:wfsUrl,wfs:{version:'2.0.0',capabilitiesSha256:'c'.repeat(64),fees:'NONE',accessConstraints:'<b>Consult the data provider</b>',pagingSupported:true,excludedLayers:[{id:'demo:complex',title:'Nested geometry',reason:'Complex feature properties are not supported'}]},collections:[{id:wfsLayer.typeName,title:'Lakes',description:'Original WFS features',itemsUrl:wfsUrl,licenseLinks:[],wfs:wfsLayer}]};
function wfsSource(count=2,formatId='gml32') {
  const layer=structuredClone(wfsLayer),format=layer.formats.find(f=>f.id===formatId);
  const receipt=(request,parameters,returned=0)=>{const url=new URL(wfsUrl);url.search=new URLSearchParams({service:'WFS',version:'2.0.0',request,typeNames:layer.typeName,...parameters});return {url:url.href,sha256:'a'.repeat(64),bytes:200,returned};};
  const bbox='20,10,21,11,urn:ogc:def:crs:EPSG::4326';
  const pages=count?[receipt('GetFeature',{bbox,outputFormat:format.mime,srsName:'urn:ogc:def:crs:EPSG::4326',startIndex:'0',count:'200'},count)]:[];
  return {serviceUrl:wfsUrl,serviceName:'Example WFS',collectionId:layer.typeName,collectionTitle:'Lakes',requestedBounds:[10,20,11,21],areaGeometry:null,requestedAt:'2026-10-02T00:00:00Z',selection:'wfs-bbox-full-features',featureCount:count,numberMatched:count,licenseLinks:[],pages,
    wfs:{version:'2.0.0',layer,format,requestCrs:'urn:ogc:def:crs:EPSG::4326',responseCrs:formatId==='gml32'?'urn:ogc:def:crs:EPSG::4326':'urn:ogc:def:crs:OGC:1.3:CRS84',capabilitiesSha256:'c'.repeat(64),
      schema:{namespace:'demo',elementName:'lakes',geometryField:'geom',geometryType:'Polygon',geometryNullable:true,geometryOptional:false,fields:[{name:'name',fieldType:'string',nullable:true,optional:false}],sha256:'a'.repeat(64)},
      schemaReceipt:receipt('DescribeFeatureType',{}),schemaAfterReceipt:{...receipt('DescribeFeatureType',{}),sha256:'b'.repeat(64)},hitsBefore:receipt('GetFeature',{bbox,resultType:'hits'}),hitsAfter:receipt('GetFeature',{bbox,resultType:'hits'}),verificationPages:structuredClone(pages),matchedCount:count,pagingSupported:true,pageSize:200,rawArchiveVersion:1,sortField:null,fees:'NONE',accessConstraints:'Consult the data provider'}};
}
function wfsAsset(source=wfsSource()) {
  return {id,name:'Lakes WFS',format:'wfs-snapshot',storageMode:'managed',crs:'EPSG:4326',sourceSha256:'d'.repeat(64),geojsonSha256:'e'.repeat(64),bytes:2500,featureCount:source.featureCount,coordinateCount:source.featureCount?10:0,geometryCounts:source.featureCount?{Polygon:source.featureCount}:{},bounds:source.featureCount?[10,20,11,21]:null,createdAt:'2026-10-02T00:00:00Z',attribution:null,licenseUrl:null,dataTimestamp:null,remoteSource:source};
}
test('WFS discovery retains declared formats, relative namespaces and unsupported feature type explanations',()=>{
  assert.equal(validateFeatureService(wfsService),wfsService);
  const onlyJson=structuredClone(wfsService);onlyJson.collections[0].wfs.formats.splice(1);onlyJson.collections[0].wfs.defaultFormat='geojson';assert.equal(validateFeatureService(onlyJson),onlyJson);
  rejectChanges(wfsService,[
    ['ambiguous protocol',s=>s.arcgis=arcgisService.arcgis],['false metadata marker',s=>s.wfs=false],['wrong WFS version',s=>s.wfs.version='1.1.0'],['invalid capability hash',s=>s.wfs.capabilitiesSha256='x'],['invalid paging capability',s=>s.wfs.pagingSupported='true'],
    ['fees too large',s=>s.wfs.fees='地'.repeat(5462)],['control character in constraints',s=>s.wfs.accessConstraints='a\0b'],['query endpoint differs',s=>s.collections[0].itemsUrl+='?request=GetFeature'],['canonical endpoint query',s=>s.url+='?service=WFS'],
    ['missing layer metadata',s=>delete s.collections[0].wfs],['namespace missing',s=>s.collections[0].wfs.namespace=''],['typeName mismatch',s=>s.collections[0].wfs.typeName='demo:rivers'],['unsupported QName',s=>s.collections[0].wfs.typeName='bad:type:name'],
    ['invalid mime id pairing',s=>s.collections[0].wfs.formats[0].mime='text/xml'],['duplicate formats',s=>s.collections[0].wfs.formats[0]={...s.collections[0].wfs.formats[1]}],['wrong preferred format',s=>s.collections[0].wfs.defaultFormat='geojson'],['no formats',s=>s.collections[0].wfs.formats=[]],
    ['fabricated license',s=>s.collections[0].licenseLinks=['https://example.com/license']],['overlapping exclusion',s=>s.wfs.excludedLayers[0].id=wfsLayer.typeName],['duplicate exclusion',s=>s.wfs.excludedLayers.push(s.wfs.excludedLayers[0])],
  ],validateFeatureService);
  rejectChanges(service,[['WFS metadata on OGC collection',s=>s.collections[0].wfs=wfsLayer]],validateFeatureService);
});
test('WFS source preserves GML and GeoJSON formats, two reads and independent original archive and derived hashes',()=>{
  for(const format of ['gml32','geojson']) {
    const source=wfsSource(2,format),asset=wfsAsset(source);
    assert.equal(validateFeatureProvenance(source,2),source);assert.equal(validateVectorAsset(asset),asset);
    const inspection={asset,geojson:{type:'FeatureCollection',features:[{type:'Feature'},{type:'Feature'}],geodSource:structuredClone(source)}};
    assert.equal(validateVectorInspection(inspection,id),inspection);inspection.geojson.geodSource.wfs.format.mime='fake';assert.throws(()=>validateVectorInspection(inspection,id),/does not match/);
  }
  const empty=wfsAsset(wfsSource(0));assert.equal(validateVectorAsset(empty),empty);
  rejectChanges(wfsAsset(),[
    ['source mislabeled GeoJSON',a=>a.format='geojson'],['nonmanaged original archive',a=>a.storageMode='reference'],['missing WFS provenance',a=>delete a.remoteSource],['OSM conversion metadata',a=>a.osmConversion=2],['fabricated observation time',a=>a.dataTimestamp='2026-10-02T00:00:00Z'],['archive too large',a=>a.bytes=VECTOR_MAX_BYTES+1],
  ],validateVectorAsset);
});
test('WFS provenance rejects changed endpoints, axis order, hidden parameters and incomplete consistency receipts',()=>{
  const changeUrl=(source,field,key,value)=>{const receipt=field==='page'?source.pages[0]:source.wfs[field];const url=new URL(receipt.url);url.searchParams.set(key,value);receipt.url=url.href;};
  rejectChanges(wfsSource(),[
    ['ambiguous provenance',s=>s.arcgis=arcgisSource().arcgis],['wrong selection',s=>s.selection='bbox-full-features'],['wrong feature count',s=>s.numberMatched++],['wrong matched count',s=>s.wfs.matchedCount++],['wrong request CRS',s=>s.wfs.requestCrs=s.wfs.responseCrs='CRS:84'],['wrong response CRS',s=>s.wfs.responseCrs='CRS:84'],
    ['archive version',s=>s.wfs.rawArchiveVersion=2],['schema hash mismatch',s=>s.wfs.schema.sha256='f'.repeat(64)],['unsupported field type',s=>s.wfs.schema.fields[0].fieldType='custom:Type'],['schema namespace mismatch',s=>s.wfs.schema.namespace='wrong'],['field name collision',s=>s.wfs.schema.fields[0].name='geom'],['schema unknown geometry',s=>s.wfs.schema.geometryType='Curve'],['untyped schema optional',s=>s.wfs.schema.geometryOptional=1],
    ['missing second pass',s=>delete s.wfs.verificationPages],['missing final schema',s=>delete s.wfs.schemaAfterReceipt],['nonzero hits returned',s=>s.wfs.hitsBefore.returned=2],['second pass count mismatch',s=>s.wfs.verificationPages[0].returned=1],['empty first pass',s=>s.pages=[]],['incomplete page',s=>s.pages[0].returned=1],['POST receipt',s=>s.pages[0].parameters={service:'WFS'}],
    ['changed endpoint',s=>s.pages[0].url=s.pages[0].url.replace('/ows/wfs','/other')],['longitude first bbox',s=>changeUrl(s,'page','bbox','10,20,11,21,urn:ogc:def:crs:EPSG::4326')],['hidden filter',s=>changeUrl(s,'page','CQL_FILTER','1=1')],['page size overflow',s=>changeUrl(s,'page','count','201')],['invalid offset',s=>changeUrl(s,'page','startIndex','1')],['schema wrong type',s=>changeUrl(s,'schemaAfterReceipt','typeNames','demo:other')],['unrecorded sort',s=>changeUrl(s,'page','sortBy','name A')],['wrong sort field',s=>s.wfs.sortField='name'],
    ['aggregate response size',s=>s.pages[0].bytes=VECTOR_MAX_BYTES],['fabricated license',s=>s.licenseLinks=['https://example.com/license']],
  ],s=>validateFeatureProvenance(s,2));
  const empty=wfsSource(0);empty.pages=[wfsSource().pages[0]];assert.throws(()=>validateFeatureProvenance(empty,0));
});
test('WFS paged reads validate cumulative offsets and a stable nonnullable integer sort property',()=>{
  const source=wfsSource(2),schema=source.wfs.schema;
  schema.fields.unshift({name:'OBJECTID',fieldType:'long',nullable:false,optional:false});source.wfs.sortField='OBJECTID';source.wfs.pageSize=1;
  const page=(start)=>{const value=structuredClone(source.pages[0]),url=new URL(value.url);url.searchParams.set('startIndex',String(start));url.searchParams.set('count','1');url.searchParams.set('sortBy','OBJECTID A');return {...value,url:url.href,returned:1};};
  source.pages=[page(0),page(1)];source.wfs.verificationPages=structuredClone(source.pages);
  assert.equal(validateFeatureProvenance(source,2),source);
  source.wfs.pagingSupported=false;assert.throws(()=>validateFeatureProvenance(source,2));source.wfs.pagingSupported=true;
  schema.fields[0].nullable=true;assert.throws(()=>validateFeatureProvenance(source,2));
});
test('WFS receipt URLs may include their bounded query beyond the configured endpoint length',()=>{
  const source=wfsSource();source.serviceUrl='https://example.com/'+'w'.repeat(2015);
  for(const page of [...source.pages,...source.wfs.verificationPages,source.wfs.schemaReceipt,source.wfs.schemaAfterReceipt,source.wfs.hitsBefore,source.wfs.hitsAfter]){const url=new URL(page.url);url.pathname=new URL(source.serviceUrl).pathname;page.url=url.href;}
  assert.ok(source.pages[0].url.length>2048);assert.equal(validateFeatureProvenance(source,2),source);
});
test('feature extraction accepts WFS archives and forwards only the selected native response format',async()=>{
  const value=wfsAsset(),previous=globalThis.window,calls=[];globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return value;}}}};
  try {const payload={serviceId:id,collectionId:wfsLayer.typeName,bounds:value.remoteSource.requestedBounds,responseFormat:'gml32'};assert.equal(await featureRequest('query',payload),value);assert.deepEqual(calls,[{command:'query_features',payload:{request:payload}}]);}
  finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
