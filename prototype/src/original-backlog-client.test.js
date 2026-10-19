import test from 'node:test';import assert from 'node:assert/strict';
import {reviewedArchiveSelection,validateOriginalRasterInspection} from './original-backlog-client.js';
import glc from '../../crates/geod-runtime/src/datasets/glc-fcs10-reviewed-index.json' with {type:'json'};
import glc30 from '../../crates/geod-runtime/src/datasets/glc-fcs30d-reviewed-index.json' with {type:'json'};
const id='00000000-0000-4000-8000-000000000001',url='https://data.bris.ac.uk/datasets/s5hqmjcdj8yo2ibzi9b4ew3sn/N00E050-N10E060_FABDEM_V1-2.zip';
const selection={archiveBytes:21627700,archiveSha256:'be348eecf2fb835792c1d446d5528a34956a8509c37004da9b72101c313cacfe',member:'N09E050_FABDEM_V1-2.tif',memberBytes:16035307,memberSha256:'c8d2ffb9777dddabefa856636a072af81563eb9fed3bf3cfcfebfe95a9ed9928'};
const proof={schemaVersion:2,dataset:{id:'fabdem'},request:{datasetId:'fabdem',layer:'elevation'},layer:{id:'elevation',scale:1,offset:0,nodata:-9999},sourceId:'N09E050_FABDEM_V1-2',sourceBounds:[49.99986111111111,9.000138888888891,50.99986111111111,10.00013888888889],declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
const asset={key:'raster',href:url,mediaType:'application/zip',metadata:{kind:'reviewed-raster-archive',href:url,datasetPreset:proof,reviewedArchive:selection,'file:size':selection.archiveBytes,'file:checksum':`1220${selection.archiveSha256}`}};
const job={id,status:'succeeded',href:url,mediaType:'application/zip',bytesDownloaded:selection.archiveBytes,sha256:selection.archiveSha256,stacSource:{snapshotId:'a'.repeat(64),assetKey:'raster'}};
const data={jobId:id,sha256:selection.memberSha256,bounds:proof.sourceBounds,archiveOriginal:{schemaVersion:1,jobId:id,sourcePin:job.stacSource,sourceProvenance:proof,archiveUrl:url,archiveFile:`${id}.zip`,memberFile:`${id}.member.tif`,selection}};
test('ZIP and whole TIFF retain separate validated hashes and unchanged source proof',()=>{assert.deepEqual(reviewedArchiveSelection(asset),selection);assert.equal(validateOriginalRasterInspection(data,job,{assets:[asset]}),data);});
test('tampered native archive receipt or aliased original cannot become a valid inspected TIFF',()=>{
  for(const mutate of [d=>d.archiveOriginal.sourcePin.assetKey='other',d=>d.sha256=selection.archiveSha256,d=>d.archiveOriginal.selection.memberSha256='0'.repeat(64),d=>d.archiveOriginal.archiveUrl='https://example.com/a.zip',d=>d.archiveOriginal.sourceProvenance.layer.scale=0.1]){const changed=structuredClone(data);mutate(changed);assert.throws(()=>validateOriginalRasterInspection(changed,job,{assets:[asset]}));}
  const modified=structuredClone(asset);modified.metadata.reviewedArchive.member='../a.tif';assert.equal(reviewedArchiveSelection(modified),null);
});
test('GLC10 and historical three-band GLC30D keep their exact original ZIP/member identity and year scope',()=>{
  for(const [index,datasetId,layer,year] of [[glc,'glc-fcs10','classes',2023],[glc30,'glc-fcs30d','classes-1985',1985]]){
    const tile=index.tiles[0],selected={archiveBytes:index.archiveBytes,archiveSha256:index.archiveSha256,member:tile.name,memberBytes:tile.bytes,memberSha256:tile.sha256};
    const p={...proof,dataset:{id:datasetId},request:{datasetId,layer,year},layer:{id:layer,scale:1,offset:0,nodata:null},sourceId:tile.name.replace(/\.tif$/,''),sourceBounds:tile.bounds};
    const original={key:'raster',href:index.archiveUrl,mediaType:'application/zip',metadata:{kind:'reviewed-raster-archive',href:index.archiveUrl,datasetPreset:p,reviewedArchive:selected,'file:size':selected.archiveBytes,'file:checksum':'1220'+selected.archiveSha256}};
    assert.deepEqual(reviewedArchiveSelection(original),selected);const changed=structuredClone(original);changed.metadata.datasetPreset.request.year=2022;assert.equal(reviewedArchiveSelection(changed),null);changed.metadata.datasetPreset.request.year=year;changed.metadata.datasetPreset.layer.nodata=0;assert.equal(reviewedArchiveSelection(changed),null);
  }
});
test('a visual component declaration cannot be attached to a numerical raster or omitted from its reviewed Vantor source',()=>{
  assert.throws(()=>validateOriginalRasterInspection({...data,sampleEncoding:{}},job,{assets:[asset]}));
  assert.throws(()=>validateOriginalRasterInspection({sha256:'a'.repeat(64)}, {sha256:'a'.repeat(64)}, {provenance:{reviewedDataset:{dataset:{id:'vantor-open'}}}}));
});
