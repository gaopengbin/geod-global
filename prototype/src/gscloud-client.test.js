import test from 'node:test';
import assert from 'node:assert/strict';
import {gscloudAuth,validateGSCloudSearch,validateGSCloudResponse,validateGSCloudOriginalJob,downloadGSCloudOriginal,GSCloudOriginalProducts} from './gscloud-client.js';
const request={productId:'305',bounds:[116.2,39.8,116.5,40.1],start:'1980-01-01',end:'2026-10-15',cloudMax:null,presence:'all',page:1,pageSize:20};
const receipt={retrievedAt:'2026-10-15T00:00:00Z',metadataSha256:'a'.repeat(64),downloadsStarted:false};
const item={id:'tile',productId:'305',datetime:null,cloudCover:null,bytes:null,presence:'reservation',downloadStatus:'reservation-required',geometry:null,bbox:null};
test('GSCloud rejects invalid dates, scope, credentials and oversized pages before transport',()=>{
  assert.deepEqual(validateGSCloudSearch(request),request);
  for(const changed of [{start:'2025-02-30'},{bounds:[180,0,-180,2]},{productId:'../accounts'},{page:0},{pageSize:21},{password:'secret'},{presence:'free'},{cloudMax:101}])assert.throws(()=>validateGSCloudSearch({...request,...changed}));
});
test('each public original keeps its own product, tile family and byte budget',async()=>{
  const oldWindow=globalThis.window;const calls=[];
  const tiles={'305':'srtm_60_04','307':'slope_60_04','308':'tpi_60_04','310':'ASTGTM_N40E116','421':'ASTGTM2_N40E116','aeab8000652a45b38afbb7ff023ddabb':'ASTGTMV003_N40E116'};
  globalThis.window={__TAURI__:{core:{invoke:async(command,{request})=>{
    calls.push(request);return {id:'619674d3-4507-4be8-aa80-312bd58010ea',kind:'download',assetKey:GSCloudOriginalProducts[request.productId].key,itemId:request.itemId,href:`https://bjdl.gscloud.cn/sources/download/${request.productId}/${request.itemId}`,mediaType:'application/zip',status:'queued',bytesDownloaded:0,totalBytes:null};
  }}}};
  try{
    for(const [productId,itemId] of Object.entries(tiles)){
      const job=await downloadGSCloudOriginal({productId,itemId});assert.equal(job.itemId,itemId);
      assert.throws(()=>validateGSCloudOriginalJob({...job,href:job.href+'?sid=secret'}));
      assert.throws(()=>validateGSCloudOriginalJob({...job,totalBytes:GSCloudOriginalProducts[productId].maxBytes+1}));
      assert.throws(()=>validateGSCloudOriginalJob({...job,itemId:'srtm_99_00'}));
    }
    assert.equal(calls.length,6);
    for(const request of [{productId:'306',itemId:'srtm_60_04'},{productId:'309',itemId:'asp_60_04'},{productId:'421',itemId:'ASTGTMV003_N40E116'},{productId:'307',itemId:'asp_60_04'},{productId:'421',itemId:'ASTGTM2_N90E116'}])await assert.rejects(downloadGSCloudOriginal(request));
    assert.equal(calls.length,6);
  }finally{if(oldWindow===undefined)delete globalThis.window;else globalThis.window=oldWindow;}
});
test('GSCloud responses cannot cross area/product/page or turn reservation metadata into a queued download',()=>{
  const response={...receipt,request,product:{id:'305'},items:[item],total:1,totalPages:1,hasMore:false};
  assert.equal(validateGSCloudResponse('search',response,request),response);
  for(const changed of [{request:{...request,bounds:[115,39,116,40]}},{request:{...request,page:2}},{items:[{...item,productId:'310'}]},{items:[{...item,downloadStatus:'ready'}]},{downloadsStarted:true},{hasMore:true}])assert.throws(()=>validateGSCloudResponse('search',{...response,...changed},request));
  assert.throws(()=>validateGSCloudResponse('detail',{...receipt,item:{...item,id:'other'}},{productId:'305',itemId:'tile'}));
  assert.throws(()=>validateGSCloudResponse('detail',{...receipt,item:{...item,bbox:[116,39,117,40]}},{productId:'305',itemId:'tile'}));
});
test('GSCloud credentials have no browser HTTP fallback',async()=>{
  const original=globalThis.fetch;let requests=0;globalThis.fetch=async()=>{requests++;throw new Error('Unexpected HTTP credential request');};
  try{await assert.rejects(gscloudAuth('begin',{email:'private@example.com',password:'never-send',code:'ABCD'}),/desktop app/);assert.equal(requests,0);}finally{globalThis.fetch=original;}
});

test('native website sign-in accepts no credential payload or unverified success',async()=>{
  const oldWindow=globalThis.window;let calls=0;
  globalThis.window={__TAURI__:{core:{invoke:async command=>{calls++;return {status:command==='gscloud_login_begin'?'pending':'connected',verified:false,downloadVerified:false};}}}};
  try{
    await assert.rejects(gscloudAuth('begin',{password:'never-send'}),/does not accept credentials/);assert.equal(calls,0);
    assert.equal((await gscloudAuth('begin')).status,'pending');
    await assert.rejects(gscloudAuth('finish'),/Invalid GSCloud account status/);
  }finally{if(oldWindow===undefined)delete globalThis.window;else globalThis.window=oldWindow;}
});
test('original tasks cannot expose session URLs or cross the reviewed product and transfer budget',async()=>{
  const job={id:'619674d3-4507-4be8-aa80-312bd58010ea',kind:'download',assetKey:'gscloud_srtm90',itemId:'srtm_60_04',href:'https://bjdl.gscloud.cn/sources/download/305/srtm_60_04',mediaType:'application/zip',status:'succeeded',bytesDownloaded:34890696,totalBytes:34890696,sha256:'a'.repeat(64)};
  assert.equal(validateGSCloudOriginalJob(job),job);
  for(const change of [{href:job.href+'?sid=private&uid=123'},{assetKey:'product'},{bytesDownloaded:65*1024*1024},{sha256:null},{totalBytes:1}])assert.throws(()=>validateGSCloudOriginalJob({...job,...change}));
  await assert.rejects(downloadGSCloudOriginal({productId:'305',itemId:'srtm_60_04'}),/desktop app/);
});
