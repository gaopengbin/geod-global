import test from 'node:test';
import assert from 'node:assert/strict';
import {gebcoRequest,validateGebcoRequest,validateGebcoRecord} from './gebco-client.js';

const id='01234567-1234-4234-9234-0123456789ab';
const bounds=[-30.1,40,-30,40.1];
const record={id,name:'Atlantic',createdAt:'2026-10-14T11:23:00Z',bounds,gridName:'gebco_2026_global',gridId:1,dataSourceName:'gebco_2026',dataSourceId:1,formatId:2,resolutionArcSeconds:15,provenanceKind:'service-generated-regional-numerical-subset',publisherStaticWholeOriginal:false,generationStatus:'finished',verificationStatus:'verified',basketId:'GEBCO_14_Oct_2026_test_transport',archiveBytes:421346,archiveSha256:'b'.repeat(64),memberName:'gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif',bytes:8992,sha256:'a'.repeat(64),requestSha256:'c'.repeat(64),gridsSha256:'d'.repeat(64),formatsSha256:'e'.repeat(64),officialRequestSha256:'f'.repeat(64),queueResponseSha256:'0'.repeat(64),stages:[{stage:'official_finished',at:'2026-10-14T11:23:00Z'}],attribution:'GEBCO',limitations:['Official service-generated subset']};
const raster={jobId:id,sha256:record.sha256,width:24,height:24,storageLayout:'strip',bands:[{index:1,dataType:'Int16',nodata:-32767}],crs:'EPSG:4326',transform:[1/240,0,-30.1,0,-1/240,40.1],bounds,pixelInterpretation:'area',previewDataUrl:'data:image/png;base64,AA==',previewWidth:24,previewHeight:24,displayBand:1,displayRange:[-1941,-1209],limitations:[]};
test('GEBCO request only accepts region and name, and never supplied URLs, email, TID or credentials',()=>{
  assert.equal(validateGebcoRequest({name:'Atlantic',bounds}).name,'Atlantic');
  for(const extra of ['url','email','credential','gridId','dataSourceId'])assert.throws(()=>validateGebcoRequest({name:'Atlantic',bounds,[extra]:'untrusted'}));
  for(const b of [[-180,-80,180,80],[1,1,1,2],[180,1,182,2],[1,2,0,3]])assert.throws(()=>validateGebcoRequest({name:'Atlantic',bounds:b}));
});
test('GEBCO records retain generated subset provenance and reject TID, whole-original claims, region and dual-hash changes',()=>{
  assert.equal(validateGebcoRecord(structuredClone(record)).publisherStaticWholeOriginal,false);
  for(const patch of [{publisherStaticWholeOriginal:true},{provenanceKind:'publisher-original'},{gridName:'gebco_2025_global'},{dataSourceName:'gebco_2026_tid'},{formatId:4},{archiveSha256:''},{officialRequestSha256:''},{bounds:[-31,40,-30,40.1]}])assert.throws(()=>validateGebcoRecord({...record,...patch}));
});
test('create binds native numerical result to exact request and preserves raw signed elevations',async()=>{
  const original=globalThis.fetch;let body;
  try{
    globalThis.fetch=async(url,options)=>{assert.equal(url,'http://127.0.0.1:4318/gebco/subsets');body=JSON.parse(options.body);return {ok:true,json:async()=>({record,raster})};};
    const value=await gebcoRequest('create',{name:'Atlantic',bounds});assert.deepEqual(body,{name:'Atlantic',bounds});assert.equal(value.raster.bands[0].nodata,-32767);
    globalThis.fetch=async()=>({ok:true,json:async()=>({record:{...record,name:'Other area'},raster})});await assert.rejects(gebcoRequest('create',{name:'Atlantic',bounds}),/different region/);
    globalThis.fetch=async()=>({ok:true,json:async()=>({record,raster:{...raster,sha256:'b'.repeat(64)}})});await assert.rejects(gebcoRequest('create',{name:'Atlantic',bounds}),/checksum/);
    globalThis.fetch=async()=>({ok:true,json:async()=>({record,raster:{...raster,bands:[{index:1,dataType:'UInt8',nodata:0}]}})});await assert.rejects(gebcoRequest('create',{name:'Atlantic',bounds}),/numerical grid/);
  }finally{globalThis.fetch=original;}
});
test('pixel reads are bounded numeric coordinates and retain negative values without depth sign conversion',async()=>{
  const original=globalThis.fetch;
  try{
    globalThis.fetch=async(url,options)=>{assert.equal(url,`http://127.0.0.1:4318/gebco/subsets/${id}/pixel`);assert.deepEqual(JSON.parse(options.body),{column:12,row:12});return {ok:true,json:async()=>({jobId:id,sha256:record.sha256,column:12,row:12,values:[-1705],noData:[false]})};};
    assert.deepEqual((await gebcoRequest('pixel',{id,column:12,row:12})).values,[-1705]);
    await assert.rejects(gebcoRequest('pixel',{id,column:-1,row:12}));await assert.rejects(gebcoRequest('inspect',{id:'../outside'}));
  }finally{globalThis.fetch=original;}
});
