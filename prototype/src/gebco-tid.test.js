import test from 'node:test';
import assert from 'node:assert/strict';
import index from '../../crates/geod-runtime/src/datasets/gebco-tid-reviewed-index.json' with {type:'json'};
import {reviewedArchiveSelection,validateOriginalRasterInspection} from './original-backlog-client.js';

function fixture() {
  const v=index.tiles[0],selection={archiveBytes:index.archiveBytes,archiveSha256:index.archiveSha256,member:v.name,memberBytes:v.bytes,memberSha256:v.sha256};
  const preset={schemaVersion:2,declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately',dataset:{id:'gebco-tid'},request:{datasetId:'gebco-tid',layer:'tid',bounds:[10,-40,11,-39]},layer:{id:'tid',scale:1,offset:0,nodata:127},sourceId:v.name.replace(/\.tif$/,''),sourceBounds:v.bounds};
  const asset={key:'raster',href:index.archiveUrl,mediaType:'application/zip',eligible:true,metadata:{kind:'reviewed-raster-archive',href:index.archiveUrl,datasetPreset:preset,reviewedArchive:selection,'file:size':index.archiveBytes,'file:checksum':`1220${index.archiveSha256}`}};
  const job={id:'job',status:'succeeded',href:index.archiveUrl,mediaType:'application/zip',sha256:index.archiveSha256,bytesDownloaded:index.archiveBytes,stacSource:{snapshotId:'a'.repeat(64),assetKey:'raster'}};
  const snapshot={id:job.stacSource.snapshotId,assets:[asset]};
  const data={jobId:job.id,sha256:v.sha256,width:v.width,height:v.height,storageLayout:'strip',crs:'EPSG:4326',pixelInterpretation:'area',bands:[{dataType:'Int8',nodata:127}],transform:v.transform,bounds:v.bounds,archiveOriginal:{schemaVersion:1,jobId:job.id,sourcePin:job.stacSource,sourceProvenance:preset,archiveUrl:index.archiveUrl,archiveFile:'job.zip',memberFile:'job.member.tif',selection}};
  return {v,asset,job,snapshot,data};
}
test('whole global TID admission preserves signed source codes and exact finite archive identity',()=>{
  const f=fixture();assert.equal(index.tiles.length,8);assert.ok(reviewedArchiveSelection(f.asset));
  assert.equal(validateOriginalRasterInspection(f.data,f.job,f.snapshot),f.data);
  for(const mutate of [a=>a.metadata.datasetPreset.layer.nodata=0,a=>a.metadata.datasetPreset.request.year=2025,a=>a.metadata.reviewedArchive.archiveSha256='0'.repeat(64),a=>a.metadata.datasetPreset.request.bounds=[10,20,11,21]]) {
    const altered=structuredClone(f.asset);mutate(altered);assert.equal(reviewedArchiveSelection(altered),null);
  }
  for(const mutate of [d=>d.bands[0].dataType='UInt8',d=>d.bands[0].nodata=0,d=>d.transform[0]*=2,d=>d.sha256='f'.repeat(64)]) {
    const altered=structuredClone(f.data);mutate(altered);assert.throws(()=>validateOriginalRasterInspection(altered,f.job,f.snapshot));
  }
});
