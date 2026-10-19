import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './sinolc-reuse.fixture.json' with {type:'json'};
import {reviewedArchiveDownloadCandidates,reviewedArchiveSourceMatches,inspectReviewedArchiveDownload} from './original-backlog-client.js';
import {validateStacInspection} from './stac-client.js';

const clone=value=>structuredClone(value);
const currentSource=()=>({...clone(fixture.snapshot),id:'c'.repeat(64),retrievedAt:'2026-10-14T13:00:00+00:00'});
const currentAsset=()=>currentSource().assets[0];

test('a fresh metadata retrieval reuses the complete original and its immutable old pin without a download',async()=>{
  const current=currentSource(),before=clone(fixture),calls=[];
  const candidates=reviewedArchiveDownloadCandidates(current.assets[0],[fixture.job],current.id);
  assert.deepEqual(candidates,[fixture.job]);
  const request=async(operation,payload)=>{
    calls.push([operation,payload]);
    if(operation==='snapshot')return fixture.snapshot;
    if(operation==='inspect')return validateStacInspection(fixture.inspection,fixture.job.id);
    assert.fail('Reusing a checked original must not create a download.');
  };
  const loaded=await inspectReviewedArchiveDownload(current.assets[0],candidates,request);
  assert.deepEqual(calls,[['snapshot',{id:fixture.job.stacSource.snapshotId}],['inspect',{id:fixture.job.id}]]);
  assert.equal(loaded.job,fixture.job);
  assert.equal(loaded.item,fixture.snapshot);
  assert.equal(loaded.data,fixture.inspection);
  assert.equal(loaded.data.archiveOriginal.sourcePin.snapshotId,fixture.snapshot.id);
  assert.notEqual(loaded.item.id,current.id);
  assert.deepEqual(fixture,before);
});

test('archive/member identity, full product declarations and requested scope cannot be substituted during reuse',async()=>{
  const mutations=[
    a=>{a.href=a.metadata.href='https://example.com/East_Shanghai.zip';},
    a=>{a.metadata.reviewedArchive.archiveSha256='a'.repeat(64);a.metadata['file:checksum']='1220'+'a'.repeat(64);},
    a=>a.metadata.reviewedArchive.memberSha256='a'.repeat(64),
    a=>a.metadata.reviewedArchive.member='Other_Shanghai.tif',
    a=>a.metadata.datasetPreset.dataset.version='A different release',
    a=>a.metadata.datasetPreset.sourceTitle='A different source declaration',
    a=>a.metadata.datasetPreset.request.bounds=[121.4,31.1,121.6,31.3],
    a=>{a.metadata.datasetPreset.dataset.id='glc-fcs10';a.metadata.datasetPreset.request.datasetId='glc-fcs10';},
    a=>a.eligible=false,
  ];
  for(const change of mutations){
    const asset=currentAsset();change(asset);
    assert.equal(reviewedArchiveSourceMatches(asset,fixture.job,fixture.snapshot),false);
    const candidates=reviewedArchiveDownloadCandidates(asset,[fixture.job]);
    const operations=[];
    await assert.rejects(inspectReviewedArchiveDownload(asset,candidates,async operation=>{
      operations.push(operation);assert.equal(operation,'snapshot');return fixture.snapshot;
    }),/dual-checksum receipt/);
    assert.equal(operations.includes('inspect'),false);
  }
});

test('only successful jobs with the exact whole archive identity can be reuse candidates',()=>{
  for(const change of [j=>j.sha256='a'.repeat(64),j=>j.bytesDownloaded--,j=>j.href+='?other=true',
    j=>j.status='running',j=>j.mediaType='image/tiff',j=>j.stacSource.assetKey='other',
    j=>j.stacSource.snapshotId='missing']){
    const job=clone(fixture.job);change(job);
    assert.deepEqual(reviewedArchiveDownloadCandidates(currentAsset(),[job]),[]);
  }
});

test('same archive candidates require their own matching source proof and the original receipt validator',async()=>{
  const otherJob=clone(fixture.job),otherSource=clone(fixture.snapshot);
  otherJob.id='00000000-0000-4000-8000-000000000001';otherJob.stacSource.snapshotId='d'.repeat(64);
  otherSource.id=otherJob.stacSource.snapshotId;
  otherSource.assets[0].metadata.datasetPreset.request.bounds=[121.4,31.1,121.6,31.3];
  const calls=[];
  const loaded=await inspectReviewedArchiveDownload(currentAsset(),[otherJob,fixture.job],async(operation,payload)=>{
    calls.push([operation,payload]);
    if(operation==='snapshot')return payload.id===otherSource.id?otherSource:fixture.snapshot;
    assert.equal(operation,'inspect');assert.equal(payload.id,fixture.job.id);return fixture.inspection;
  });
  assert.equal(loaded.job,fixture.job);
  assert.deepEqual(calls.map(([operation])=>operation),['snapshot','snapshot','inspect']);
  const forged=clone(fixture.inspection);forged.archiveOriginal.sourcePin.snapshotId='c'.repeat(64);
  await assert.rejects(inspectReviewedArchiveDownload(currentAsset(),[fixture.job],async operation=>operation==='snapshot'?fixture.snapshot:forged),/dual-checksum receipt/);
});
