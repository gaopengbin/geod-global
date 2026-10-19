import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {validateGeoBoundariesLayer,validateGeoBoundariesService,validateGeoBoundariesProvenance,validateGeoBoundariesFeatures,geoBoundariesServiceRoot,GEOBOUNDARIES_LICENSE,GEOBOUNDARIES_ATTRIBUTION,GEOBOUNDARIES_SELECTION} from './geoboundaries-client.js';
import {validateFeatureService,featureRequest} from './features-client.js';
import {validateFeatureProvenance,validateVectorAsset,validateVectorInspection} from './vector-client.js';

const uuid='735f227b-5f95-473f-967b-077f3419bc68';
const root='https://www.geoboundaries.org/api/current/gbOpen/NPL/ALL/';
const gitRef='9469f09';
const fullRef=`${gitRef}${'a'.repeat(33)}`;
const hash='a'.repeat(64);
const fileUrl=ref=>`https://github.com/wmgeolab/geoBoundaries/raw/${ref}/releaseData/gbOpen/NPL/ADM0/geoBoundaries-NPL-ADM0.geojson`;
const canonical=value=>value&&typeof value==='object'?(Array.isArray(value)?`[${value.map(canonical).join(',')}]`:`{${Object.keys(value).sort().map(key=>`${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`):JSON.stringify(value);
const rehash=layer=>{layer.metadataSha256=createHash('sha256').update(canonical(layer.metadata)).digest('hex');};
function layer() {
  const metadata={boundaryID:'NPL-ADM0-63295607',boundaryName:'Nepal',boundaryISO:'NPL',boundaryYearRepresented:'2019',boundaryType:'ADM0',boundarySource:'Geoboundaries, Open Data Nepal',boundaryLicense:'Creative Commons Attribution 4.0 International (CC BY 4.0)',licenseSource:'opendatanepal.com/dataset/new-political-and-administrative-boundaries-shapefile-of-nepal',sourceDataUpdateDate:'Thu Jan 19 07:31:04 2023',buildDate:'Dec 12, 2023',admUnitCount:'1',gjDownloadURL:fileUrl(gitRef),boundaryCanonical:'Federal Democratic Republic of Nepal'};
  const result={boundaryId:metadata.boundaryID,countryIso3:metadata.boundaryISO,countryName:metadata.boundaryName,adminLevel:metadata.boundaryType,yearRepresented:metadata.boundaryYearRepresented,originalSource:metadata.boundarySource,originalLicense:metadata.boundaryLicense,licenseSource:metadata.licenseSource,sourceDataUpdateDate:metadata.sourceDataUpdateDate,buildDate:metadata.buildDate,unitCount:1,releaseRef:gitRef,downloadUrl:metadata.gjDownloadURL,metadata,metadataSha256:''};
  rehash(result); return result;
}
const discovery=()=>({url:root,sha256:hash,bytes:4100,returned:1});
function service() {
  return {id:uuid,name:'Nepal boundaries',title:'geoBoundaries · NPL · gbOpen',url:root,connectedAt:'2026-10-13T00:00:00Z',geoboundaries:{releaseType:'gbOpen',countryIso3:'NPL',discoveryReceipt:discovery()},collections:[{id:'ADM0',title:'Nepal · ADM0',description:'Complete country boundaries; not a polygon clip.',itemsUrl:'https://www.geoboundaries.org/api/current/gbOpen/NPL/ADM0/',licenseLinks:[GEOBOUNDARIES_LICENSE],geoboundaries:layer()}]};
}
function source() {
  return {serviceUrl:root,serviceName:'Nepal boundaries',collectionId:'ADM0',collectionTitle:'Nepal · ADM0',licenseLinks:[GEOBOUNDARIES_LICENSE],requestedBounds:[84,26,86,28],areaGeometry:null,requestedAt:'2026-10-13T00:00:00Z',pages:[{url:fileUrl(fullRef),sha256:hash,bytes:1000,returned:1}],numberMatched:1,featureCount:1,selection:GEOBOUNDARIES_SELECTION,geoboundaries:{releaseType:'gbOpen',layer:layer(),discoveryReceipt:discovery(),resolvedReleaseCommit:fullRef,attribution:GEOBOUNDARIES_ATTRIBUTION,geometrySelection:GEOBOUNDARIES_SELECTION}};
}
function asset() {
  return {id:uuid,name:'Nepal ADM0.geojson',format:'geojson',storageMode:'managed',crs:'EPSG:4326',sourceSha256:hash,geojsonSha256:'b'.repeat(64),bytes:1100,featureCount:1,coordinateCount:5,geometryCounts:{Polygon:1},bounds:[80,26,88,31],createdAt:'2026-10-13T00:00:00Z',attribution:null,licenseUrl:null,dataTimestamp:null,remoteSource:source()};
}
const feature=()=>({type:'Feature',properties:{shapeID:'63295607B1',shapeName:'Nepal',shapeGroup:'NPL',shapeType:'ADM0'},geometry:{type:'Polygon',coordinates:[[[80,26],[88,26],[88,31],[80,31],[80,26]]]}});
function rejects(fixture,changes,validate) {
  for(const [label,change]of changes){const value=structuredClone(fixture);change(value);assert.throws(()=>validate(value),undefined,label);}
}

test('gbOpen registry and complete-file provenance preserve the publisher metadata and source epoch',()=>{
  const saved=service(),snapshot=source();
  assert.equal(validateFeatureService(saved),saved);
  assert.equal(validateGeoBoundariesService(saved),saved);
  assert.equal(validateFeatureProvenance(snapshot,1),snapshot);
  assert.equal(validateVectorAsset(asset()).remoteSource.geoboundaries.layer.yearRepresented,'2019');
  const explicitNulls=structuredClone(saved); explicitNulls.arcgis=null;explicitNulls.overpass=null;explicitNulls.wfs=null;explicitNulls.collections[0].wfs=null;explicitNulls.collections[0].arcgis=null;
  assert.equal(validateFeatureService(explicitNulls),explicitNulls);
});
test('metadata SHA-256 matches independent UTF-8 hashing and detects optional-field tampering',()=>{
  const verified=layer(); verified.metadata.optional={'emoji':'🌍','label':'地理\n边界','nested':[null,true,1,'x'.repeat(200)],'__proto__':'literal'}; rehash(verified);
  assert.equal(validateGeoBoundariesLayer(verified),verified);
  verified.metadata.optional.label+='changed';assert.throws(()=>validateGeoBoundariesLayer(verified),/fingerprint/);
  rejects(layer(),[
    ['raw metadata replaced',v=>v.metadata={}],['hash malformed',v=>v.metadataSha256='x'.repeat(64)],['valid but different fingerprint',v=>v.metadataSha256='c'.repeat(64)],
    ['optional field changed',v=>v.metadata.boundaryCanonical='Another country'],['unbounded metadata',v=>{v.metadata.large='x'.repeat(65536);rehash(v);}],['unbounded nesting',v=>{let x=[];for(let n=0;n<34;n++)x=[x];v.metadata.deep=x;rehash(v);}],
    ['unknown layer fields',v=>v.simplified=true],['missing declared source',v=>delete v.originalSource],
  ],validateGeoBoundariesLayer);
});
test('derived layer identity must agree with every retained publisher field even after rehashing',()=>{
  rejects(layer(),[
    ['country label changed',v=>v.countryName='Bhutan'],['country changed',v=>v.countryIso3='BTN'],['admin changed',v=>v.adminLevel='ADM1'],['version ref changed',v=>v.releaseRef='bbbbbbb'],
    ['units changed',v=>v.unitCount=2],['license changed',v=>v.originalLicense='Public domain'],['source year replaced by build year',v=>v.yearRepresented='2023'],
    ['raw country changed and rehashed',v=>{v.metadata.boundaryISO='BTN';rehash(v);}],['raw epoch changed and rehashed',v=>{v.metadata.boundaryYearRepresented='2023';rehash(v);}],
    ['zero declared units',v=>{v.metadata.admUnitCount='0';v.unitCount=0;rehash(v);}],['partial count',v=>{v.metadata.admUnitCount='1.5';rehash(v);}],['oversized units',v=>{v.metadata.admUnitCount='50001';v.unitCount=50001;rehash(v);}],
    ['wrong boundary ID',v=>{v.metadata.boundaryID='NPL-ADM1-other';v.boundaryId=v.metadata.boundaryID;rehash(v);}],
  ],validateGeoBoundariesLayer);
});
test('only the exact gbOpen ISO3 ALL root and level API endpoints are accepted',()=>{
  assert.equal(geoBoundariesServiceRoot(root),'NPL');
  for(const endpoint of [root.replace('gbOpen','gbHumanitarian'),root.replace('NPL','ALL'),root.replace('NPL','np'),root.replace('ALL','ADM0'),root.replace('www.',''),root+'?bbox=84,26,86,28',root+'#',root.replace('https:','http:'),root.replace('/NPL/','/%4ePL/'),root.replace('/ALL/','/ALL')])assert.throws(()=>geoBoundariesServiceRoot(endpoint),undefined,endpoint);
  rejects(service(),[
    ['cross-country root',v=>v.url=root.replace('NPL','BTN')],['changed receipt URL',v=>v.geoboundaries.discoveryReceipt.url=root.replace('NPL','BTN')],
    ['layer API all endpoint',v=>v.collections[0].itemsUrl=root],['layer API other country',v=>v.collections[0].itemsUrl=v.collections[0].itemsUrl.replace('NPL','BTN')],['fake bbox API filter',v=>v.collections[0].itemsUrl+='?bbox=84,26,86,28'],
    ['unsupported release type',v=>v.geoboundaries.releaseType='gbHumanitarian'],['discovery count changed',v=>v.geoboundaries.discoveryReceipt.returned=2],['discovery parameters',v=>v.geoboundaries.discoveryReceipt.parameters={bbox:'84,26,86,28'}],
    ['duplicate level',v=>{v.collections.push(v.collections[0]);v.geoboundaries.discoveryReceipt.returned=2;}],['missing levels',v=>v.collections=[]],['altered CC license URL',v=>v.collections[0].licenseLinks=['https://creativecommons.org/licenses/by-nc/4.0/']],
  ],validateFeatureService);
});
test('release files cannot be simplified, floating branches, cross-country or arbitrary mirrors',()=>{
  const rawLayer=layer();rawLayer.downloadUrl=rawLayer.metadata.gjDownloadURL=fileUrl(gitRef).replace('https://github.com/wmgeolab/geoBoundaries/raw/','https://raw.githubusercontent.com/wmgeolab/geoBoundaries/');rehash(rawLayer);assert.equal(validateGeoBoundariesLayer(rawLayer),rawLayer);
  const media=source();media.pages[0].url=fileUrl(fullRef).replace('https://github.com/wmgeolab/geoBoundaries/raw/','https://media.githubusercontent.com/media/wmgeolab/geoBoundaries/');assert.equal(validateFeatureProvenance(media,1),media);
  for(const change of [url=>url.replace(gitRef,'main'),url=>url.replace(gitRef,'9469f0'),url=>url.replace(gitRef,'A469f09'),url=>url.replace('NPL','BTN'),url=>url.replace('/ADM0/','/ADM1/'),url=>url.replace('.geojson','_simplified.geojson'),url=>url.replace('github.com','example.com'),url=>url.replace('wmgeolab','another-owner'),url=>url+'?download=1',url=>url.replace('.geojson','.topojson'),url=>url.replace('/raw/','/blob/')]) {
    const value=layer();value.downloadUrl=value.metadata.gjDownloadURL=change(value.downloadUrl);rehash(value);assert.throws(()=>validateGeoBoundariesLayer(value),undefined,value.downloadUrl);
  }
});
test('protocol metadata is mutually exclusive at service, collection and query levels',()=>{
  for(const protocol of ['arcgis','wfs','overpass']) {
    const saved=service();saved[protocol]={};assert.throws(()=>validateFeatureService(saved),/Ambiguous/);
    const snapshot=source();snapshot[protocol]={};assert.throws(()=>validateFeatureProvenance(snapshot,1));
    const collection=service();collection.collections[0][protocol]={};assert.throws(()=>validateFeatureService(collection));
  }
  const stripped=service();delete stripped.geoboundaries;assert.throws(()=>validateFeatureService(stripped),/Unexpected geoBoundaries/);
  const missingLayer=service();delete missingLayer.collections[0].geoboundaries;assert.throws(()=>validateFeatureService(missingLayer));
  const removed=source();delete removed.geoboundaries;assert.throws(()=>validateFeatureProvenance(removed,1));
});
test('complete country counts, immutable redirect ref and original receipt remain inseparable',()=>{
  rejects(source(),[
    ['pretend bbox extraction',v=>v.selection='bbox-full-features'],['pretend polygon clipping',v=>v.geoboundaries.geometrySelection='polygon-clip'],['country altered',v=>v.serviceUrl=root.replace('NPL','BTN')],
    ['declared count altered',v=>v.featureCount=2],['total count altered',v=>v.numberMatched=2],['page count altered',v=>v.pages[0].returned=2],['no pages',v=>v.pages=[]],['duplicated full file',v=>v.pages.push(v.pages[0])],
    ['bbox query parameters on raw original',v=>v.pages[0].parameters={bbox:'84,26,86,28'}],['changed redirect ref',v=>v.pages[0].url=fileUrl('b'.repeat(40))],['fake resolved commit',v=>v.geoboundaries.resolvedReleaseCommit='b'.repeat(40)],['different API level',v=>v.collectionId='ADM1'],
    ['empty raw file',v=>v.pages[0].bytes=0],['over cap original',v=>v.pages[0].bytes=20*1024*1024+1],['invalid checksum',v=>v.pages[0].sha256='bad'],['unknown receipt property',v=>v.pages[0].sourceCount=1],['forged sourceCount instead of unitCount',v=>v.sourceCount=1],
    ['other attribution',v=>v.geoboundaries.attribution='World borders'],['discovery endpoint substituted',v=>v.geoboundaries.discoveryReceipt.url='https://www.geoboundaries.org/api/current/gbOpen/NPL/ADM0/'],
    ['invalid requested bounds',v=>v.requestedBounds=[88,28,80,26]],['false world bounds',v=>v.requestedBounds=[-181,-90,180,90]],['degenerate area',v=>v.areaGeometry={type:'Polygon',coordinates:[[[84,26],[84,26],[84,26],[84,26]]]}],
    ['fake query region polygon',v=>v.areaGeometry={type:'Polygon',coordinates:[[[83,26],[85,26],[85,27],[83,26]]]}],['non RFC timestamp',v=>v.requestedAt='2026-10-13'],
  ],v=>validateGeoBoundariesProvenance(v,1));
});
test('full country features retain identities outside the query bbox without being clipped',()=>{
  const snapshot=source(),features=[feature()];
  assert.equal(validateGeoBoundariesFeatures(features,snapshot),features);
  const inspected={asset:asset(),geojson:{type:'FeatureCollection',features,geodSource:snapshot}};
  assert.equal(validateVectorInspection(inspected,uuid),inspected);
  for(const edit of [f=>f.properties.shapeGroup='BTN',f=>f.properties.shapeType='ADM1',f=>delete f.properties.shapeID,f=>f.geometry.type='LineString',f=>delete f.properties.shapeName]){
    const tampered=structuredClone(inspected);edit(tampered.geojson.features[0]);assert.throws(()=>validateVectorInspection(tampered,uuid));
  }
  const twice=source();twice.featureCount=twice.numberMatched=2;twice.pages[0].returned=2;twice.geoboundaries.layer.unitCount=2;twice.geoboundaries.layer.metadata.admUnitCount='2';rehash(twice.geoboundaries.layer);
  assert.throws(()=>validateGeoBoundariesFeatures([feature(),feature()],twice),/identity/);
});
test('native query accepts only the independent full-administrative-level selection',async()=>{
  const previous=globalThis.window,calls=[],output=asset();
  globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return output;}}}};
  try {
    const payload={serviceId:uuid,collectionId:'ADM0',bounds:[84,26,86,28]};
    assert.equal(await featureRequest('query',payload),output);
    assert.deepEqual(calls,[{command:'query_features',payload:{request:payload}}]);
    output.remoteSource.selection='bbox-full-features';await assert.rejects(()=>featureRequest('query',payload));
  }finally {if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
test('legacy CRS84 is recorded exactly while the display copy omits only the redundant CRS field',()=>{
  const snapshot=source();snapshot.geoboundaries.sourceCrs={type:'name',properties:{name:'urn:ogc:def:crs:OGC:1.3:CRS84'}};
  assert.equal(validateFeatureProvenance(snapshot,1),snapshot);
  const inspected={asset:{...asset(),remoteSource:snapshot},geojson:{type:'FeatureCollection',features:[feature()],geodSource:snapshot}};
  assert.equal(validateVectorInspection(inspected,uuid),inspected);
  for(const declaration of [{type:'name',properties:{name:'EPSG:3857'}},{type:'name',properties:{name:'EPSG:4326'}},{type:'link',properties:{href:'https://example.com/crs'}},{type:'name',properties:{name:'urn:ogc:def:crs:OGC:1.3:CRS84',axis:'lat-lon'}},{type:'name',properties:{name:'urn:ogc:def:crs:OGC:1.3:CRS84'},extra:true},false]){
    const tampered=structuredClone(snapshot);tampered.geoboundaries.sourceCrs=declaration;assert.throws(()=>validateFeatureProvenance(tampered,1));
  }
  const changed=structuredClone(inspected);changed.geojson.crs=snapshot.geoboundaries.sourceCrs;assert.throws(()=>validateVectorInspection(changed,uuid),/display geometry/);
});
