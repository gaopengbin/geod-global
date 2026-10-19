import test from 'node:test';
import assert from 'node:assert/strict';
import { loadFileThumbnail } from './file-thumbnail.js';
import { validateFileThumbnail } from './runtime-client.js';

test('reviewed ZIP cards read the verified original member PNG instead of rejecting ZIP/TIFF dual hashes',async()=>{
  const id='00000000-0000-4000-8000-000000000041',snapshotId='e'.repeat(64),href='https://data.bris.ac.uk/datasets/s5hqmjcdj8yo2ibzi9b4ew3sn/N00E050-N10E060_FABDEM_V1-2.zip';
  const selection={archiveBytes:21627700,archiveSha256:'be348eecf2fb835792c1d446d5528a34956a8509c37004da9b72101c313cacfe',member:'N09E050_FABDEM_V1-2.tif',memberBytes:16035307,memberSha256:'c8d2ffb9777dddabefa856636a072af81563eb9fed3bf3cfcfebfe95a9ed9928'};
  const sourceBounds=[49.99986111111111,9.000138888888891,50.99986111111111,10.00013888888889],proof={schemaVersion:2,dataset:{id:'fabdem'},request:{datasetId:'fabdem',layer:'elevation'},layer:{id:'elevation',scale:1,offset:0,nodata:-9999},sourceId:'N09E050_FABDEM_V1-2',sourceBounds,declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  const asset={key:'raster',href,mediaType:'application/zip',metadata:{kind:'reviewed-raster-archive',href,datasetPreset:proof,reviewedArchive:selection,'file:size':selection.archiveBytes,'file:checksum':'1220'+selection.archiveSha256}};
  const job={id,status:'succeeded',href,mediaType:'application/zip',bytesDownloaded:selection.archiveBytes,sha256:selection.archiveSha256,stacSource:{snapshotId,assetKey:'raster'}};
  const original={jobId:id,sha256:selection.memberSha256,width:3600,height:3600,storageLayout:'tile',bands:[{index:1,dataType:'Float32',nodata:-9999}],crs:'EPSG:4326',transform:[1/3600,0,sourceBounds[0],0,-1/3600,sourceBounds[3]],bounds:sourceBounds,pixelInterpretation:'point',displayBand:1,displayRange:[0,730],previewWidth:2,previewHeight:2,previewDataUrl:'data:image/png;base64,YQ==',limitations:[],archiveOriginal:{schemaVersion:1,jobId:id,sourcePin:job.stacSource,sourceProvenance:proof,archiveUrl:href,archiveFile:id+'.zip',memberFile:id+'.member.tif',selection}};
  let genericCalls=0;const calls=[],sourceRequest=async(operation)=>{calls.push(operation);return operation==='snapshot'?{assets:[asset]}:original;};
  const thumbnail=await loadFileThumbnail(job,async()=>{genericCalls++;throw Error('Legacy TIFF-only thumbnail route');},()=>true,sourceRequest);
  assert.equal(thumbnail.sha256,selection.memberSha256);assert.equal(thumbnail.dataUrl,original.previewDataUrl);assert.equal(thumbnail.width,original.previewWidth);assert.equal(genericCalls,0);assert.deepEqual(calls,['snapshot','inspect']);
  const changedId='00000000-0000-4000-8000-000000000042',changedJob={...job,id:changedId},changed=structuredClone(original);changed.jobId=changedId;changed.archiveOriginal.jobId=changedId;changed.archiveOriginal.archiveFile=changedId+'.zip';changed.archiveOriginal.memberFile=changedId+'.member.tif';changed.archiveOriginal.selection.memberSha256='f'.repeat(64);
  await assert.rejects(loadFileThumbnail(changedJob,async()=>{},()=>true,async operation=>operation==='snapshot'?{assets:[asset]}:changed),/dual-checksum receipt/);
});

test('previews validate file identity, dimensions and PNG payload', () => {
  const valid = { jobId: 'test-file', sha256: 'a'.repeat(64), width: 160, height: 120, dataUrl: 'data:image/png;base64,iVBORw0KGgo=' };
  assert.equal(validateFileThumbnail(valid, 'test-file'), valid);
  for (const change of [{ jobId: 'another-file' }, { width: 161 }, { height: 0 }, { dataUrl: 'https://example.com/scene.jpg' }]) {
    assert.throws(() => validateFileThumbnail({ ...valid, ...change }, 'test-file'), /invalid data/);
  }
});

test('duplicate file previews share work and run serially; changed checksums invalidate cache', async () => {
  let calls = 0, active = 0, max = 0;
  const request = async (_, { id }) => {
    calls++; active++; max = Math.max(max, active);
    await new Promise(resolve => setTimeout(resolve, 5));
    active--;
    return { jobId: id, sha256: 'a'.repeat(64), dataUrl: 'data:image/png;base64,AAAA', width: 1, height: 1 };
  };
  const one = { id: 'cache-one', sha256: 'a'.repeat(64) };
  const two = { id: 'cache-two', sha256: 'a'.repeat(64) };
  const result = await Promise.all([loadFileThumbnail(one, request), loadFileThumbnail(one, request), loadFileThumbnail(two, request)]);
  assert.equal(result[0], result[1]);
  assert.equal(calls, 2);
  assert.equal(max, 1);
  await loadFileThumbnail(one, request);
  assert.equal(calls, 2);
  await assert.rejects(loadFileThumbnail({ ...one, sha256: 'b'.repeat(64) }, request), /does not match/);
});

test('failed preview does not poison the queue and can be retried', async () => {
  const job = { id: 'retry-file', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => { throw new Error('offline'); }), /offline/);
  const data = await loadFileThumbnail(job, async () => ({ jobId: job.id, sha256: job.sha256 }));
  assert.equal(data.jobId, job.id);
});

test('another page owning the preview worker waits without failing all queued cards', async () => {
  const first = { id: 'busy-first', sha256: 'a'.repeat(64) };
  const second = { id: 'busy-second', sha256: 'a'.repeat(64) };
  const order = [];
  let attempts = 0;
  const request = async (_, { id }) => {
    order.push(id);
    if (id === first.id && ++attempts <= 2) throw new Error('Preview worker is busy. Try again shortly.');
    return { jobId: id, sha256: 'a'.repeat(64) };
  };
  const results = await Promise.all([loadFileThumbnail(first, request), loadFileThumbnail(first, request), loadFileThumbnail(second, request)]);
  assert.equal(results[0], results[1]);
  assert.deepEqual(order, [first.id, first.id, first.id, second.id]);
});

test('persistent busy replies have a deadline and do not poison the next file', async context => {
  let reads = 0, calls = 0;
  context.mock.method(Date, 'now', () => ++reads === 1 ? 0 : 60001);
  const job = { id: 'busy-expired', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => { calls++; throw new Error('Preview worker is busy. Try again shortly.'); }), /Preview worker is busy/);
  assert.equal(calls, 1);
  const next = { id: 'after-busy-expired', sha256: 'a'.repeat(64) };
  assert.equal((await loadFileThumbnail(next, async (_, { id }) => ({ jobId: id, sha256: next.sha256 }))).jobId, next.id);
});

test('disconnecting stops busy preview retries before another request', async () => {
  let connected = true, calls = 0;
  const job = { id: 'busy-disconnected', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => {
    calls++; connected = false; throw new Error('Preview worker is busy. Try again shortly.');
  }, () => connected), /task service is offline/);
  assert.equal(calls, 1);
});
