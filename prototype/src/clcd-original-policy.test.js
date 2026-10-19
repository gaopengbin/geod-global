import test from 'node:test';
import assert from 'node:assert/strict';
import index from '../../crates/geod-runtime/src/datasets/clcd-reviewed-index.json' with {type:'json'};
import {reviewedClcdInspection,clcdCategoricalEncoding,reviewedClcdSelection} from './original-backlog-client.js';
import {validateStacInspection} from './stac-client.js';

const p=index.profile,id='11111111-1111-4111-8111-111111111111';
const inspection=()=>({jobId:id,sha256:index.originalSha256||'a'.repeat(64),width:p.width,height:p.height,storageLayout:'tile',
  bands:[{index:1,dataType:'UInt8',nodata:0}],crs:p.crs,horizontalCrs:p.crs,verticalCrs:null,pixelInterpretation:p.registration,
  sourceModelPixelScale:p.pixelScale,sourceModelTiepoint:p.tiepoint,bounds:p.bounds,transform:[30,0,p.bounds[0],0,-30,p.bounds[3]],
  categoricalEncoding:clcdCategoricalEncoding(),archiveOriginal:null,limitations:[],displayBand:1,displayRange:null,
  previewDataUrl:null,previewWidth:null,previewHeight:null});
test('a metadata-prefix digest cannot activate a whole-original CLCD grid or size exception',()=>{
  const data={...inspection(),sha256:index.metadataRangeSha256};
  assert.equal(reviewedClcdInspection(data),false);
  assert.throws(()=>validateStacInspection(data,id));
  assert.equal(reviewedClcdSelection({key:'raster',href:index.url,metadata:{kind:'direct-raster','file:size':index.originalBytes,
    'file:checksum':`1220${index.metadataRangeSha256}`,reviewedOriginal:{bytes:index.originalBytes,sha256:index.metadataRangeSha256}}}),false);
});
test('the classification allowance requires a complete compiled review and the unchanged physical grid',()=>{
  const data=inspection(),active=index.wholeFileVerified&&index.publisherMd5Verified&&index.originalSha256!=null;
  assert.equal(reviewedClcdInspection(data),active);
  if(!active)assert.throws(()=>validateStacInspection(data,id));
  for(const patch of [{sha256:'0'.repeat(64)},{crs:'EPSG:4326'},{pixelInterpretation:'point'},
    {width:p.width-1},{bands:[{index:1,dataType:'UInt8',nodata:null}]},{categoricalEncoding:null},
    {transform:[30,0,p.bounds[0]+30,0,-30,p.bounds[3]]}]) assert.equal(reviewedClcdInspection({...data,...patch}),false);
});
