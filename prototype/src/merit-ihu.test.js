import test from 'node:test';
import assert from 'node:assert/strict';
import index from '../../crates/geod-runtime/src/datasets/merit-ihu-reviewed-index.json' with {type:'json'};
import {reviewedIhuSelection,reviewedIhuDownloadCandidates,validateOriginalRasterInspection} from './original-backlog-client.js';
import {SOURCE_DIRECTORY} from './source-directory.js';

// Contract scaffolding only; actual original downloads and GUI checks are separate.
const sid='a'.repeat(64);
function selection(o) {
 return {key:'raster',eligible:true,href:o.url,metadata:{kind:'direct-raster',datasetPreset:{schemaVersion:2,
  dataset:{id:'merit-hydro',version:index.release},request:{datasetId:'merit-hydro',layer:o.layer,bounds:[116,39,117,40]},
  sourceId:o.id,sourceBounds:[-180,-60,180,85],declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately',
  layer:{id:o.layer,scale:1,offset:0,nodata:o.nodata}}}};
}
test('IHU map reuse requires complete original identity and keeps the 90 m parent pending',()=>{
 const source=SOURCE_DIRECTORY.find(s=>s.id==='merit-hydro');
 assert.equal(source.status,'planned');assert.equal(source.partialAction,true);assert.equal(SOURCE_DIRECTORY.length,133);
 for(const o of index.originals) {
  const asset=selection(o),job={id:'test-download',status:'succeeded',href:o.url,sha256:o.sha256,bytesDownloaded:o.bytes,stacSource:{snapshotId:sid,assetKey:'raster'}};
  assert.equal(reviewedIhuSelection(asset).id,o.id);assert.equal(reviewedIhuDownloadCandidates(asset,[job]).length,1);
  for(const change of [a=>{a.href=a.href.replace('05min','30sec');},a=>{a.metadata.datasetPreset.layer.nodata=0;},
    a=>{a.metadata.datasetPreset.request.layer='other';},a=>{a.metadata.datasetPreset.request.year=2020;},a=>{a.metadata.datasetPreset.sourceId='other';}]) {
    const bad=structuredClone(asset);change(bad);assert.equal(reviewedIhuSelection(bad),null);
  }
  for(const bad of [{...job,status:'running'},{...job,bytesDownloaded:o.bytes-1},{...job,sha256:'0'.repeat(64)}])assert.equal(reviewedIhuDownloadCandidates(asset,[bad]).length,0);
  const data={jobId:job.id,sha256:o.sha256,width:o.width,height:o.height,crs:'EPSG:4326',pixelInterpretation:'area',bands:[{dataType:o.dataType,nodata:o.nodata}],bounds:o.bounds,transform:o.transform};
  const item={assets:[asset]};validateOriginalRasterInspection(data,job,item);
  const changed=structuredClone(data);changed.bands[0].dataType=o.dataType==='float64'?'Float32':'Int8';
  assert.throws(()=>validateOriginalRasterInspection(changed,job,item),/raw data type/);
 }
});
