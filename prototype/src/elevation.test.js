import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene, searchURL } from './catalog.js';
import { demCell, DEM_HOST, isSupportedAsset, providerForSearch } from './providers.js';
import { validElevationPixel } from './elevation.js';
import { validateRasterInspection, downloadableAssets } from './runtime-client.js';
import { verifiedMapMetadata, verifyPixelResult, coordinateToPixel } from './workspace-map-geometry.js';
import { projectRequest } from './projects-client.js';
import { projectCatalogScenes, projectExploreSearch } from './project-explore.js';

const item = JSON.parse(readFileSync('prototype/public/samples/cop-dem-response.json','utf8')).features[0];
const scene = normalizeScene(item, 'copernicus-dem');
const job = {id:'dem',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'elevation',href:scene.assets.elevation.href,sha256:'a'.repeat(64)};
const step=1/3600;
const metadata = {width:3600,height:3600,bandCount:1,dataType:'Float32',crs:'EPSG:4326',bounds:[-123-step/2,37+step/2,-122-step/2,38+step/2],pixelSize:[step,step],nodata:null,previewWidth:2,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:job.sha256,
  elevation:{product:'cop-dem-glo-30-public',heightUnit:'metre',coordinateUnit:'degree',verticalReference:'EPSG:3855',pixelInterpretation:'PixelIsPoint',displayRange:[-80,1000],sampleCount:4,validSampleCount:4,nodataIsNan:false}};

test('public DEM search uses bounds without optical dates, cloud filters or observation sorting',()=>{
  const url=new URL(searchURL({provider:'copernicus-dem',bbox:[-122.55,37.68,-122.32,37.84],start:'invalid',cloud:'invalid',limit:20}));
  assert.equal(url.searchParams.get('collections'),'cop-dem-glo-30');
  for(const key of ['query','datetime','sortby']) assert.equal(url.searchParams.has(key),false);
  assert.equal(providerForSearch(url.href).id,'copernicus-dem');
});
test('DEM storage URI resolves only the exact public geocell and rejects changed products or signed URLs',()=>{
  assert.equal(scene.assets.elevation.href,`https://${DEM_HOST}/${item.id}/${item.id}.tif`);
  assert.deepEqual(downloadableAssets(scene).map(a=>a.key),['elevation']);
  for(const id of ['Copernicus_DSM_COG_10_N90_00_E000_00_DEM','Copernicus_DSM_COG_10_S00_00_E000_00_DEM','Copernicus_DSM_COG_10_N00_00_W000_00_DEM']) assert.equal(demCell(id),null);
  assert.equal(isSupportedAsset(scene.assets.elevation.href+'?sig=secret','elevation'),false);
  assert.equal(isSupportedAsset(scene.assets.elevation.href,'scl'),false);
  const wrong=structuredClone(item);wrong.assets.data.href=wrong.assets.data.href.replace('W123','W122');
  assert.throws(()=>normalizeScene(wrong,'copernicus-dem'));
});
test('persisted DEM project restores the same stable asset and source-specific search',()=>{
  const project=projectRequest({scenes:[scene],bounds:[-122.55,37.68,-122.32,37.84],name:'DEM'});
  assert.equal(project.scenes[0].assets.elevation.rasterBand,undefined);
  assert.equal(projectCatalogScenes(project)[0].assets.elevation.href,scene.assets.elevation.href);
  assert.equal(new URL(searchURL({...projectExploreSearch(project,{}),limit:100})).searchParams.has('datetime'),false);
});
test('DEM map metadata pins Float32, Point cell geometry, geographic units and EGM2008 independently of display stretch',()=>{
  assert.equal(validateRasterInspection(metadata),metadata);
  assert.equal(verifiedMapMetadata(job,metadata),metadata);
  for(const patch of [{dataType:'UInt16'},{crs:'EPSG:32610'},{nodata:0},{elevation:{...metadata.elevation,heightUnit:'foot'}},{bounds:[-122,37,-121,38]}]){
    const data={...metadata,...patch};
    if(patch.nodata===0) { assert.equal(validElevationPixel({value:0,isNoData:false},data),false); continue; }
    assert.throws(()=>verifiedMapMetadata(job,data));
  }
});
test('original Float32 pixel values retain fractions, valid zero and negatives; NaN requires explicit NoData metadata',()=>{
  const coordinate=[-123,38];
  assert.deepEqual(coordinateToPixel(coordinate,metadata),[0,0]);
  for(const value of [0,Math.fround(-79.079376),Math.fround(657.42194)]) {
    const result={jobId:job.id,sha256:job.sha256,crs:metadata.crs,coordinate,pixel:[0,0],center:coordinate,value,isNoData:false,label:'Elevation',color:'#808080'};
    assert.equal(verifyPixelResult(result,job,metadata,coordinate),result);
    assert.equal(validElevationPixel({...result,value:0.1},metadata),false);
    assert.equal(validElevationPixel({...result,value:null,isNoData:true},metadata),false);
    assert.equal(validElevationPixel({...result,value:null,isNoData:true},{...metadata,elevation:{...metadata.elevation,nodataIsNan:true}}),true);
  }
});

test('processed DEM map geometry is pinned to its project grid and explicit NaN masks',()=>{
  const profile = Object.fromEntries(['product','heightUnit','coordinateUnit','verticalReference','pixelInterpretation'].map(key=>[key,metadata.elevation[key]]));
  const data={...metadata,width:3,height:2,bounds:[-123-step/2,38-1.5*step,-123+2.5*step,38+step/2],
    elevation:{...metadata.elevation,nodataIsNan:true}};
  const processed={...job,mediaType:'image/tiff',kind:'raster_mosaic',itemId:'project:dem-project',mosaic:{projectId:'dem-project',assetKey:'elevation',sources:[{jobId:job.id,sha256:job.sha256}]},
    mosaicOutput:{width:data.width,height:data.height,bandCount:1,crs:data.crs,bounds:data.bounds,pixelSize:data.pixelSize,elevation:profile}};
  assert.equal(verifiedMapMetadata(processed,data),data);
  assert.equal(validElevationPixel({value:null,isNoData:true},data),true);
  assert.equal(validElevationPixel({value:0,isNoData:false},data),true);
  for(const plan of [{...processed.mosaicOutput,bounds:[-123,37,-122,38]},
    {...processed.mosaicOutput,elevation:{...profile,verticalReference:'EPSG:5773'}},
    {...processed.mosaicOutput,calibration:{scale:0.0001}}]) assert.throws(()=>verifiedMapMetadata({...processed,mosaicOutput:plan},data));
  assert.throws(()=>verifiedMapMetadata({...processed,itemId:'project:other'},data));
  assert.throws(()=>verifiedMapMetadata(job,data));
});
