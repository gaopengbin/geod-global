import test from 'node:test';
import assert from 'node:assert/strict';
import {ENMAP,enmapRecipe,enmapEncoding,enmapSelection,reviewedEnmapInspection} from './enmap-client.js';
import {validateStacInspection,validateStacPixel} from './stac-client.js';
import {reviewedArchiveDownloadCandidates,validateOriginalRasterInspection} from './original-backlog-client.js';
// Synthetic contract scaffolding only. Real 224-band reads remain private and
// are checked separately against independently downloaded publisher originals.
const id='11111111-1111-4111-8111-111111111111',sid='a'.repeat(64);
function contract() {
  const p={schemaVersion:2,dataset:{id:'enmap'},sourceId:'enmap-nevada-l2a-20230707',sourceBounds:ENMAP.profile.sourceBounds,
    declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately',
    request:{datasetId:'enmap',layer:'spectral-cube',bounds:[-117.3,37.45,-117.1,37.65]},layer:{id:'spectral-cube',scale:0.0001,offset:0,nodata:-32768}};
  const asset={key:'raster',href:ENMAP.url,eligible:true,mediaType:'application/zip',metadata:{href:ENMAP.url,kind:'reviewed-raster-archive',datasetPreset:p,
    reviewedArchive:enmapRecipe,'file:size':ENMAP.archiveBytes,'file:checksum':`1220${ENMAP.archiveSha256}`}};
  const job={id,status:'succeeded',href:ENMAP.url,mediaType:'application/zip',sha256:ENMAP.archiveSha256,bytesDownloaded:ENMAP.archiveBytes,stacSource:{snapshotId:sid,assetKey:'raster'}};
  const data={jobId:id,sha256:ENMAP.main.sha256,width:1286,height:1197,storageLayout:'bsq',bands:Array.from({length:224},(_,i)=>({index:i+1,dataType:'Int16',nodata:-32768})),
    crs:'EPSG:32611',horizontalCrs:'EPSG:32611',verticalCrs:null,transform:ENMAP.profile.transform,bounds:ENMAP.profile.bounds,pixelInterpretation:'area',
    spectralEncoding:enmapEncoding(),displayBand:50,displayRange:[-1,20000],previewDataUrl:null,previewWidth:null,previewHeight:null,limitations:[],
    archiveOriginal:{schemaVersion:1,jobId:id,archiveUrl:ENMAP.url,archiveFile:`${id}.zip`,memberFile:`${id}.member.bsq`,selection:enmapRecipe,sourcePin:job.stacSource,sourceProvenance:p}};
  return {asset,job,data,snapshot:{id:sid,assets:[asset]}};
}
test('224-band EnMAP access requires exact cube, original header, source pin and complete archive',()=>{
  const {asset,job,data,snapshot}=contract();assert.ok(enmapSelection(asset));assert.ok(reviewedEnmapInspection(data));
  validateStacInspection(data,id);validateOriginalRasterInspection(data,job,snapshot);
  assert.equal(reviewedArchiveDownloadCandidates(asset,[job]).length,1);
  for(const change of [d=>d.sha256='0'.repeat(64),d=>d.spectralEncoding.headerSha256='0'.repeat(64),
    d=>d.spectralEncoding.wavelengths[100]++,d=>d.spectralEncoding.gain=1,d=>d.bands.pop(),d=>d.bands[0].nodata=0,
    d=>d.crs='EPSG:32612',d=>d.transform[2]++,d=>d.displayBand=225,d=>d.archiveOriginal.selection.memberBytes--,
    d=>d.archiveOriginal.memberFile=`${id}.member.tif`]) {
    const bad=structuredClone(data);change(bad);assert.throws(()=>validateStacInspection(bad,id));
  }
  for(const change of [a=>a.href='https://example.org/Nevada_USA_L2A.zip',a=>a.metadata.datasetPreset.request.year=2023,
    a=>a.metadata.datasetPreset.request.bounds=[116,39,117,40],a=>a.metadata.datasetPreset.layer.scale=1]) {
    const bad=structuredClone(asset);change(bad);assert.equal(enmapSelection(bad),null);
  }
  const changed=structuredClone(job);changed.bytesDownloaded--;assert.equal(reviewedArchiveDownloadCandidates(asset,[changed]).length,0);
});
test('synthetic spectra preserve negative, above-one, zero and missing samples without relaxing ordinary pixels',()=>{
  const values=Array.from({length:224},(_,i)=>i===0?-32768:i===1?-100:i===2?20000:i===3?0:i);
  const p={jobId:id,sha256:ENMAP.main.sha256,column:100,row:100,values,noData:values.map(v=>v===-32768)};
  validateStacPixel(p,{id,column:100,row:100});
  for(const change of [p=>p.sha256='0'.repeat(64),p=>p.values[2]=32768,p=>p.noData[1]=true,p=>p.column=1286]) {
    const bad=structuredClone(p);change(bad);assert.throws(()=>validateStacPixel(bad,{id,column:bad.column,row:bad.row}));
  }
});
