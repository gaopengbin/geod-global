import test from 'node:test';import assert from 'node:assert/strict';
import {validateVectorAsset,validateVectorInspection,vectorRequest,importVectorFile} from './vector-client.js';
const id='735f227b-5f95-473f-967b-077f3419bc68';
const asset={id,name:'source.geojson',format:'geojson',storageMode:'reference',crs:'EPSG:4326',sourceSha256:'a'.repeat(64),geojsonSha256:'b'.repeat(64),bytes:100,featureCount:1,coordinateCount:1,geometryCounts:{Point:1},bounds:[116.4,39.9,116.4,39.9],createdAt:'2026-10-02T00:00:00Z',attribution:null,licenseUrl:null,dataTimestamp:null};
const geoPackage={conversion:1,userVersion:10400,horizontalCrs:'EPSG:4326',verticalConversion:'source-z-retained; no-vertical-transformation',measureEncoding:'geodMeasures; source-values; NaN-as-null',otherContents:[],layers:[{table:'all',identifier:null,description:'',geometryColumn:'geom',geometryType:'POINT',srsId:4326,organization:'EPSG',organizationCoordsysId:4326,definition:'WKT definition',definition12063:null,z:0,m:0,featureCount:1,coordinateOperation:'Identity',fields:[{name:'fid',fieldType:'INTEGER',nullable:false,primaryKey:true,jsonEncoding:'number-or-decimal-string'},{name:'geom',fieldType:'POINT',nullable:true,primaryKey:false,jsonEncoding:'geometry'}]}]};
geoPackage.layers[0].coordinateDefinition='WKT definition';
Object.assign(geoPackage.layers[0],{coordinateOperationId:null,coordinateOperationDirection:'forward',coordinateAccuracyMeters:0,coordinateOperationArea:null,coordinatesOutsideOperationArea:0});
const gpAsset={...asset,format:'geopackage',name:'图层.gpkg',geoPackage};
const shapefile={conversion:1,container:'sidecars',horizontalCrs:'EPSG:4326',verticalConversion:'source-z-retained; no-vertical-transformation',measureEncoding:'geodMeasures; source-values; below-minus-1e38-as-null',numericEncoding:'DBF N/F as exact trimmed decimal strings',deletedRecords:'retained-with-geodDeleted; excluded-from-map',polygonRings:'source-XY orientation-and-containment; no-topology-repair',files:['shp','shx','dbf','prj','cpg'].map(ext=>({name:`places.${ext}`,bytes:100,sha256:'a'.repeat(64)})),layers:[{table:'places',shapeType:11,featureCount:1,deletedCount:1,nullGeometryCount:0,fields:[{name:'large',fieldType:'N',width:20,decimals:0,jsonEncoding:'decimal-string'}],encoding:'UTF-8',encodingSource:'cpg',cpg:'UTF-8',languageDriverId:0,definition:'declared WKT',coordinateDefinition:'adapted WKT',coordinateOperation:'Identity',coordinateOperationId:null,coordinateOperationDirection:'forward',coordinateAccuracyMeters:0,coordinateOperationArea:null,coordinatesOutsideOperationArea:0,coordinatesClampedToBounds:0}]};
const shpAsset={...asset,name:'places.shp',format:'shapefile',shapefile};
const localOsm={conversionVersion:1,encoding:'xml',schemaVersion:'0.6',objectCounts:{node:1,way:0,relation:0},generator:'independent writer',source:null,datasetTimestamp:null,declaredBounds:null,requiredFeatures:[],optionalFeatures:[],replicationSequence:null,replicationBaseUrl:null,ignoredBlockTypes:[],xmlRootAttributes:{version:'0.6'}};
const osmAsset={...asset,format:'osm-xml',localOsm,attribution:'© OpenStreetMap contributors',licenseUrl:'https://www.openstreetmap.org/copyright'};
test('original OSM snapshots bind source format, identities, typed counts and optional file time',()=>{
  const data={asset:osmAsset,geojson:{type:'FeatureCollection',geodLocalOsm:localOsm,features:[{type:'Feature',id:'node/1',properties:{osm_id:1,osm_type:'node',tags:{name:'中文 & café'}},geometry:{type:'Point',coordinates:[116.4,39.9]}}]}};
  assert.equal(validateVectorInspection(data,id),data);
  for(const mutate of [s=>s.objectCounts.node=2,s=>s.requiredFeatures=['HistoricalInformation'],s=>s.encoding='pbf',s=>s.objectCounts.extra=0,s=>s.conversionVersion=99,s=>s.datasetTimestamp='unknown',s=>s.declaredBounds=[-180,0,181,90],s=>s.ignoredBlockTypes=['extension']]){const s=structuredClone(localOsm);mutate(s);assert.throws(()=>validateVectorAsset({...osmAsset,localOsm:s}));}
  for(const mutate of [d=>d.geojson.features[0].id='way/1',d=>d.geojson.features[0].properties.tags.name=3,d=>d.geojson.geodLocalOsm={...d.geojson.geodLocalOsm,generator:'changed'}]){const d=structuredClone(data);mutate(d);assert.throws(()=>validateVectorInspection(d,id));}
  assert.throws(()=>validateVectorAsset({...osmAsset,shapefile}));assert.throws(()=>validateVectorAsset({...asset,localOsm}));
});
test('browser OSM XML and PBF uploads keep original bytes and reject mismatched file encodings',async()=>{
  const before=globalThis.fetch,calls=[];
  try {for(const ext of ['osm','xml','pbf']){const file=new Blob([new Uint8Array([0,0,0,1])]);Object.defineProperty(file,'name',{value:'区域.'+ext});const pbf=ext==='pbf';globalThis.fetch=async(url,request)=>{calls.push({url,request});return {ok:true,json:async()=>({...osmAsset,storageMode:'managed',format:pbf?'osm-pbf':'osm-xml',localOsm:{...localOsm,encoding:pbf?'pbf':'xml',requiredFeatures:pbf?['OsmSchema-V0.6']:[],xmlRootAttributes:pbf?{}:localOsm.xmlRootAttributes}})};};await importVectorFile(file);assert.equal(calls.at(-1).request.body,file);assert.ok(!calls.at(-1).url.includes('path='));}
    const file=new Blob(['<osm/>']);Object.defineProperty(file,'name',{value:'wrong.pbf'});globalThis.fetch=async()=>({ok:true,json:async()=>osmAsset});await assert.rejects(importVectorFile(file),/different vector format/);
  }finally{globalThis.fetch=before;}
});
test('Shapefile inspections bind companion receipts, deletion flags and positional record identities',()=>{
  const data={asset:shpAsset,geojson:{type:'FeatureCollection',geodShapefile:shapefile,features:[{type:'Feature',id:0,geodLayer:'places',geodDeleted:true,properties:{large:'9007199254740993'},geometry:{type:'Point',coordinates:[12,48,18]},geodMeasures:7.5}]}};
  assert.equal(validateVectorInspection(data,id),data);
  for(const mutate of [p=>p.files.splice(3,1),p=>p.files[0].name='../places.shp',p=>p.files.push({...p.files[0],name:'PLACES.SHP'}),p=>p.layers[0].encodingSource='guessed',p=>p.layers[0].fields[0].jsonEncoding='number']){const p=structuredClone(shapefile);mutate(p);assert.throws(()=>validateVectorAsset({...shpAsset,shapefile:p}));}
  for(const mutate of [f=>delete f.geodDeleted,f=>f.geodDeleted=false,f=>f.geodLayer='unregistered',f=>f.id=1]){const d=structuredClone(data);mutate(d.geojson.features[0]);assert.throws(()=>validateVectorInspection(d,id));}
  assert.throws(()=>validateVectorAsset({...shpAsset,geoPackage}));
  assert.throws(()=>validateVectorInspection({...data,geojson:{...data.geojson,geodShapefile:{...shapefile,container:'zip'}}},id));
});
test('browser Shapefile import uploads ZIP bytes without supplying native companion paths',async()=>{
  const previous=globalThis.fetch,previousWindow=globalThis.window;delete globalThis.window;const calls=[];
  const blob=new Blob([new Uint8Array([80,75,3,4])]);Object.defineProperty(blob,'name',{value:'中文文件.zip'});
  try{globalThis.fetch=async(url,request)=>{calls.push({url,request});return {ok:true,json:async()=>({...shpAsset,storageMode:'managed',shapefile:{...shapefile,container:'zip'}})};};
    const result=await importVectorFile(blob);assert.equal(result.format,'shapefile');assert.equal(calls[0].request.body,blob);assert.equal(calls[0].request.headers['X-GeoD-Client'],'geod-global');assert.match(calls[0].url,/import-file\?name=/);assert.ok(!calls[0].url.includes('path='));
  }finally{globalThis.fetch=previous;if(previousWindow!==undefined)globalThis.window=previousWindow;}
});
test('vector responses bind file identity, WGS84 geometry, storage and source hashes',()=>{
  assert.equal(validateVectorAsset(asset),asset);
  const data={asset,geojson:{type:'FeatureCollection',features:[{type:'Feature',properties:{},geometry:{type:'Point',coordinates:[116.4,39.9]}}]}};
  assert.equal(validateVectorInspection(data,id),data);
  for(const bad of [{crs:'EPSG:3857'},{format:'shapefile'},{sourceSha256:'changed'},{licenseUrl:'https://evil.invalid'},{bounds:[-181,0,0,1]},{coordinateCount:0},{featureCount:50001}])assert.throws(()=>validateVectorAsset({...asset,...bad}));
  assert.throws(()=>validateVectorInspection(data,'other'));
  assert.throws(()=>validateVectorInspection({...data,geojson:{type:'FeatureCollection',features:[]}},id));
});
test('native vector failures expose their string message',async()=>{
  const previous=globalThis.window;globalThis.window={__TAURI__:{core:{invoke:async()=>{throw 'Source was changed';}}}};
  try {await assert.rejects(()=>vectorRequest('list'),error=>error instanceof Error&&error.message==='Source was changed');}
  finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
test('development vector imports send file content only and never allow arbitrary local paths',async()=>{
  const previous=globalThis.fetch;const calls=[];
  try {
    globalThis.fetch=async(url,request)=>{calls.push({url,request});return {ok:true,json:async()=>asset};};
    await vectorRequest('import',{name:'source.geojson',text:'{}'});
    assert.equal(calls[0].url,'http://127.0.0.1:4318/vectors');
    assert.equal(calls[0].request.headers['X-GeoD-Client'],'geod-global');
    assert.deepEqual(JSON.parse(calls[0].request.body),{name:'source.geojson',text:'{}'});
    await assert.rejects(vectorRequest('open',{managed:false}),/desktop/);
    await assert.rejects(vectorRequest('inspect',{id:'../source'}),/identifier/);
    assert.equal(calls.length,1);
  } finally {globalThis.fetch=previous;}
});
test('native request results arriving after cancellation cannot replace the active file',async()=>{
  const original=globalThis.window;let complete;
  try {
    globalThis.window={__TAURI__:{core:{invoke:()=>new Promise(resolve=>{complete=resolve;})}}};
    const abort=new AbortController();
    const request=vectorRequest('list',{},abort.signal);
    abort.abort();complete([asset]);
    await assert.rejects(request,error=>error.name==='AbortError');
  } finally {globalThis.window=original;}
});
test('GeoPackage inspections bind every feature to original layer and coordinate provenance',()=>{
  const g={type:'FeatureCollection',geodGeoPackage:geoPackage,features:[{type:'Feature',id:1,geodLayer:'all',properties:{fid:1},geometry:{type:'Point',coordinates:[116.4,39.9]}}]};
  assert.equal(validateVectorInspection({asset:gpAsset,geojson:g},id).asset,gpAsset);
  for(const source of [{...geoPackage,userVersion:10100},{...geoPackage,layers:[]},{...geoPackage,horizontalCrs:'EPSG:3857'},{...geoPackage,layers:[{...geoPackage.layers[0],featureCount:2}]}])assert.throws(()=>validateVectorAsset({...gpAsset,geoPackage:source}));
  assert.throws(()=>validateVectorInspection({asset:gpAsset,geojson:{...g,features:[{...g.features[0],geodLayer:'missing'}]}},id));
  assert.throws(()=>validateVectorAsset({...gpAsset,geoPackage:undefined}));
  assert.throws(()=>validateVectorAsset({...gpAsset,remoteSource:{}}));
});
test('browser GeoPackage import sends actual bytes, bounded filename and client identity',async()=>{
  const previous=globalThis.fetch,calls=[];const bytes=new Uint8Array([83,81,76,105,116,101]);
  try {
    globalThis.fetch=async(url,request)=>{calls.push({url,request});return {ok:true,json:async()=>gpAsset};};
    await importVectorFile(new File([bytes],'图层.gpkg'));
    assert.match(calls[0].url,/\/vectors\/import-file\?name=/);assert.equal(new URL(calls[0].url).searchParams.get('name'),'图层.gpkg');
    assert.deepEqual(new Uint8Array(await calls[0].request.body.arrayBuffer()),bytes);assert.equal(calls[0].request.headers['X-GeoD-Client'],'geod-global');
    await assert.rejects(importVectorFile({name:'huge.gpkg',size:20*1024*1024+1}),/20 MiB/);assert.equal(calls.length,1);
  }finally{globalThis.fetch=previous;}
});
