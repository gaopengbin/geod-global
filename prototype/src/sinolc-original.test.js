import test from 'node:test';
import assert from 'node:assert/strict';
import sino from '../../crates/geod-runtime/src/datasets/sinolc-reviewed-index.json' with {type:'json'};
import {reviewedLargeRasterInspection,reviewedArchiveSelection,sinolcCategoricalEncoding,validateOriginalRasterInspection} from './original-backlog-client.js';
import {validateStacInspection} from './stac-client.js';
import {genericCoordinateToPixel,genericMapMetadata} from './stac-map-model.js';

const id='00000000-0000-4000-8000-000000000001',t=sino.member;
const selection={archiveBytes:sino.archiveBytes,archiveSha256:sino.archiveSha256,member:t.name,memberBytes:t.bytes,memberSha256:t.sha256};
const proof={schemaVersion:2,dataset:{id:'sinolc-1'},request:{datasetId:'sinolc-1',layer:'classes',bounds:[121.4,31.1,121.5,31.2]},
  layer:{id:'classes',scale:1,offset:0,nodata:0},sourceId:'East_Shanghai',sourceBounds:t.bounds,
  declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
const asset={key:'raster',href:sino.archiveUrl,mediaType:'application/zip',metadata:{kind:'reviewed-raster-archive',href:sino.archiveUrl,datasetPreset:proof,reviewedArchive:selection,'file:size':selection.archiveBytes,'file:checksum':`1220${selection.archiveSha256}`}};
const job={id,status:'succeeded',href:sino.archiveUrl,mediaType:'application/zip',bytesDownloaded:selection.archiveBytes,sha256:selection.archiveSha256,stacSource:{snapshotId:'a'.repeat(64),assetKey:'raster'}};
const inspection={jobId:id,sha256:t.sha256,width:t.width,height:t.height,storageLayout:'tile',bands:[{index:1,dataType:'UInt8',nodata:0}],
  crs:t.crs,horizontalCrs:t.crs,pixelInterpretation:t.registration,bounds:t.bounds,sourceModelPixelScale:t.pixelScale,sourceModelTiepoint:t.tiepoint,
  transform:[t.pixelScale[0],0,t.bounds[0],0,-t.pixelScale[1],t.bounds[3]],previewDataUrl:null,previewWidth:null,previewHeight:null,
  displayBand:1,displayRange:null,limitations:[],categoricalEncoding:sinolcCategoricalEncoding(),
  archiveOriginal:{schemaVersion:1,jobId:id,sourcePin:job.stacSource,sourceProvenance:proof,archiveUrl:sino.archiveUrl,archiveFile:`${id}.zip`,memberFile:`${id}.member.tif`,selection}};

test('only the reviewed complete Shanghai grid and dual-checksum receipt bypass ordinary dimensions',()=>{
  assert.deepEqual(reviewedArchiveSelection(asset),selection);
  assert.equal(reviewedLargeRasterInspection(inspection),true);
  assert.equal(validateStacInspection(inspection,id),inspection);
  assert.equal(validateOriginalRasterInspection(inspection,job,{assets:[asset]}),inspection);
  assert.equal(genericMapMetadata(inspection),inspection);
  assert.deepEqual(genericCoordinateToPixel([t.bounds[0]+t.pixelScale[0]*131999.5,t.bounds[3]-t.pixelScale[1]*113999.5],inspection),{column:131999,row:113999});
});

test('large preview cannot be unlocked by dimensions, forged palette, source identity or receipt',()=>{
  for(const change of [d=>d.sha256='b'.repeat(64),d=>delete d.archiveOriginal,d=>d.width--,d=>d.storageLayout='strip',d=>d.bands[0].nodata=null,
    d=>d.sourceModelPixelScale[0]*=2,d=>d.transform[0]*=2,d=>d.verticalCrs='EPSG:3855',d=>d.pixelInterpretation='point',
    d=>d.archiveOriginal.selection.archiveSha256='c'.repeat(64),d=>d.archiveOriginal.archiveUrl='https://example.com/Shanghai.zip',
    d=>d.archiveOriginal.memberFile='another.tif',d=>d.archiveOriginal.sourcePin.assetKey='other',
    d=>d.archiveOriginal.sourceProvenance.sourceId='West_Shanghai',d=>d.archiveOriginal.sourceProvenance.request.year=2021,
    d=>d.archiveOriginal.sourceProvenance.request.bounds=[10,20,11,21],d=>delete d.categoricalEncoding,
    d=>d.categoricalEncoding.nodata=255,d=>d.categoricalEncoding.unlistedCodes=[],d=>d.categoricalEncoding.classes[0].name='Invented class']){
    const changed=structuredClone(inspection);change(changed);assert.equal(reviewedLargeRasterInspection(changed),false);assert.throws(()=>validateStacInspection(changed,id));
  }
});

test('scientific classification has explicit missing and unknown codes without assigning a false year',()=>{
  assert.equal(inspection.categoricalEncoding.nodata,0);
  assert.deepEqual(inspection.categoricalEncoding.unlistedCodes,[11,250,255]);
  assert.equal(inspection.categoricalEncoding.palette.length,256);
  for(const code of [11,250,255])assert.equal(inspection.categoricalEncoding.classes.some(c=>c.code===code),false);
  const altered=structuredClone(asset);altered.metadata.datasetPreset.request.year=2021;
  assert.equal(reviewedArchiveSelection(altered),null);
});
