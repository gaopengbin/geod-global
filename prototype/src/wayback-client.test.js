import test from 'node:test';
import assert from 'node:assert/strict';
import {WAYBACK_CATALOG_URL,fetchWaybackCatalog,parseWaybackCatalog,parseWaybackPointMetadata,waybackMetadataRequest,waybackReference,waybackOfficialUrl} from './wayback-client.js';

const record={itemID:'ab83ada1bd3145978eecfb115aaa10c3',itemTitle:'World Imagery (Wayback 2026-09-24)',itemURL:'https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/63116/{level}/{row}/{col}',metadataLayerUrl:'https://metadata.maptiles.arcgis.com/arcgis/rest/services/World_Imagery_Metadata_2026_r09/MapServer',metadataLayerItemID:'f2696faf5a3643fe9f9ee097b09caa55',layerIdentifier:'WB_2026_R09'};
const raw=JSON.stringify({63116:record}),version=()=>parseWaybackCatalog(JSON.parse(raw))[0];

test('publication dates and rendered tiles never assert location acquisition time or offline permission',()=>{
  const v=version();assert.equal(v.publicationDate,'2026-09-24');assert.equal(v.acquisitionDate,null);assert.equal(v.offlineExportAllowed,false);assert.equal(v.rendered,true);
  assert.equal(v.tileTemplate,'https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/63116/{z}/{y}/{x}');
  assert.deepEqual(v.publisherRecord,record);
});

test('catalog rejects remote origin changes, release substitutions, malformed dates and metadata URLs',()=>{
  for(const patch of [{itemURL:record.itemURL.replace('63116','26334')},{itemURL:record.itemURL.replace('wayback.maptiles.arcgis.com','example.com')},{itemURL:record.itemURL+'?token=secret'},{metadataLayerUrl:record.metadataLayerUrl+'/0/../MapServer'},{metadataLayerUrl:record.metadataLayerUrl.replace('https:','http:')},{itemTitle:'World Imagery (Wayback 2026-02-30)'},{itemID:'../other'}])assert.throws(()=>parseWaybackCatalog({63116:{...record,...patch}}));
  for(const key of ['0','-1','01','1.5','4294967296'])assert.throws(()=>parseWaybackCatalog({[key]:record}));
  assert.throws(()=>parseWaybackCatalog({}));assert.throws(()=>parseWaybackCatalog([]));
});

test('point metadata request uses the selected release service and scale without fabricating a capture date',()=>{
  const url=new URL(waybackMetadataRequest(version(),[116.4,39.9],14));
  assert.equal(url.pathname,'/arcgis/rest/services/World_Imagery_Metadata_2026_r09/MapServer/9/query');
  assert.deepEqual(JSON.parse(url.searchParams.get('geometry')),{spatialReference:{wkid:4326},x:116.4,y:39.9});
  assert.equal(url.searchParams.get('outFields'),'SRC_DATE2,NICE_DESC,SRC_DESC,SAMP_RES,SRC_ACC');
  for(const zoom of [0,9,24,14.5])assert.throws(()=>waybackMetadataRequest(version(),[116.4,39.9],zoom));
  assert.throws(()=>waybackMetadataRequest(version(),[181,39.9],14));
  assert.throws(()=>waybackMetadataRequest({...version(),metadataUrl:'https://example.com'},[116.4,39.9],14));
});

test('all intersecting metadata records and raw source attributes are retained',()=>{
  const first={SRC_DATE2:1769385600000,NICE_DESC:'Vantor',SRC_DESC:'LG06',SAMP_RES:.3,SRC_ACC:8.47};
  const rows=parseWaybackPointMetadata({features:[{attributes:first},{attributes:{SRC_DATE2:null,NICE_DESC:'Other',SAMP_RES:1}}]});
  assert.equal(rows.length,2);assert.equal(rows[0].acquisitionDate,'2026-01-26T00:00:00.000Z');assert.deepEqual(rows[0].attributes,first);assert.equal(rows[1].acquisitionDate,null);
  assert.deepEqual(parseWaybackPointMetadata({features:[]}),[]);
  for(const invalid of [{error:{code:400}},{features:[],exceededTransferLimit:true},{features:[{}]}])assert.throws(()=>parseWaybackPointMetadata(invalid));
});

test('saved reference pins actual AOI, directory and original metadata while containing no imagery',()=>{
  const bounds=[116.3,39.8,116.5,40],catalog={url:WAYBACK_CATALOG_URL,raw};
  const ref=waybackReference(version(),bounds,catalog);
  assert.equal(ref.datasetId,'wayback');assert.equal(ref.containsImagery,false);assert.deepEqual(ref.bounds,bounds);assert.equal(ref.catalog.raw,raw);assert.equal(ref.pointMetadata,null);
  assert.throws(()=>waybackReference({...version(),publisherRecord:{...record,itemTitle:'Different'}},bounds,catalog));
  assert.throws(()=>waybackReference(version(),bounds,catalog,{versionId:'wayback-10'}));
  assert.throws(()=>waybackReference(version(),[-181,0,0,1],catalog));
  const official=new URL(waybackOfficialUrl(version(),bounds)),parameters=new URLSearchParams(official.hash.slice(1));assert.equal(official.origin,'https://livingatlas.arcgis.com');assert.equal(parameters.get('active'),'63116');assert.equal(parameters.get('ext'),bounds.join(','));
});

test('directory fetch uses the official URL, anonymous credentials and rejects failed or oversized metadata',async()=>{
  const original=globalThis.fetch;
  try{
    let request;globalThis.fetch=async(url,options)=>{request={url,options};return new Response(raw,{status:200,headers:{'content-type':'application/json'}});};
    const result=await fetchWaybackCatalog();assert.equal(request.url,WAYBACK_CATALOG_URL);assert.equal(request.options.credentials,'omit');assert.equal(result.raw,raw);assert.equal(result.versions[0].id,'wayback-63116');
    globalThis.fetch=async()=>new Response('login',{status:401});await assert.rejects(fetchWaybackCatalog(),/HTTP 401/);
    globalThis.fetch=async()=>new Response(raw,{status:200,headers:{'content-type':'text/html'}});await assert.rejects(fetchWaybackCatalog(),/JSON metadata/);
    globalThis.fetch=async()=>new Response(raw,{status:200,headers:{'content-type':'application/json','content-length':String(3*1024*1024)}});await assert.rejects(fetchWaybackCatalog(),/response limit/);
  }finally{globalThis.fetch=original;}
});
