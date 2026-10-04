import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene } from './catalog.js';
import { projectRequest, jobsForProject, pendingProjectJobs } from './projects-client.js';
import { projectCatalogScenes } from './project-explore.js';
import { modisAssetIdentity, MODIS_CRS, MODIS_PIXEL } from './modis.js';
import { isSupportedAsset } from './providers.js';
import { validateRasterInspection, downloadableAssets } from './runtime-client.js';
import { verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';
import { decodeQuality, qualityPalette, qualityMatchesJob, validQualityPixel } from './quality.js';

const item = JSON.parse(readFileSync(new URL('../qa/modis-quality-catalog-item.json',import.meta.url),'utf8'));
const scene = normalizeScene(item,'planetary-modis');
const jobFor = key => ({id:'qa-job',kind:'download',status:'succeeded',itemId:scene.id,assetKey:key,href:scene.assets[key].href,sha256:'a'.repeat(64)});
const metadataFor = key => ({width:2400,height:2400,bandCount:1,dataType:key === 'modis_qc' ? 'UInt32' : 'UInt16',nodata:key === 'modis_qc' ? 4294967295 : 65535,
  crs:MODIS_CRS,sha256:'a'.repeat(64),pixelSize:[MODIS_PIXEL,MODIS_PIXEL],bounds:[-10*2400*MODIS_PIXEL,3*2400*MODIS_PIXEL,-9*2400*MODIS_PIXEL,4*2400*MODIS_PIXEL],
  previewWidth:1,previewHeight:1,previewDataUrl:'data:image/png;base64,AAAA',classes:qualityPalette(key).map(([label,color],value)=>({value,label,color,count:value === 0 ? 5759999 : 0})),
  quality:{product:'modis-09a1-v061',band:key,layer:key === 'modis_qc' ? 'sur_refl_qc_500m' : 'sur_refl_state_500m',pixelInterpretation:'PixelIsArea',displayField:key === 'modis_qc' ? 'MODLAND quality' : 'Cloud state',sampleCount:5760000,validSampleCount:5759999,countsFullResolution:true,definition:'https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf'}});

test('QA catalogue subset exposes five bound original COGs and restores legacy project pins without reflectance scaling on QA',()=>{
  assert.equal(downloadableAssets(scene).length,5);
  const project = {...projectRequest({name:'QA',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]}),id:'quality-project'};
  assert.equal(Object.keys(project.scenes[0].assets).length,5);
  for (const key of ['modis_qc','modis_state']) {
    assert.equal(project.scenes[0].assets[key].rasterBand,undefined);
    assert.equal(projectCatalogScenes(project)[0].assets[key].href,scene.assets[key].href);
    assert.equal(modisAssetIdentity(scene.assets[key].href,key).id,scene.id);
    for (const href of [scene.assets[key].href+'?sig=secret',scene.assets[key].href.replace('/08/','/09/'),scene.assets[key].href.replace('modiseuwest','landsateuwest')]) assert.equal(isSupportedAsset(href,key),false);
  }
  assert.equal(jobsForProject(project,[jobFor('modis_qc'),jobFor('modis_state')]).length,2);
});
test('QA catalogue cannot substitute signed samples, scale, a different band or an EO display channel',()=>{
  for (const mutate of [d=>d.assets.sur_refl_qc_500m['raster:bands'][0].data_type='int32',d=>d.assets.sur_refl_state_500m['raster:bands'][0].scale=.0001,
    d=>d.assets.sur_refl_qc_500m.href=d.assets.sur_refl_b01.href,d=>d.assets.sur_refl_state_500m['eo:bands']=[]]) {
    const changed=structuredClone(item);mutate(changed);assert.throws(()=>normalizeScene(changed,'planetary-modis'));
  }
});
test('a checked original removes only superseded source failures from project work while retaining all task history',()=>{
  const done=jobFor('modis_state'),failed={...done,id:'old-failure',status:'failed',sha256:null};
  const different={...failed,id:'changed-source',href:failed.href+'?changed'},processing={...failed,id:'processing',kind:'raster_mosaic'};
  const all=[failed,done,different,processing];
  assert.deepEqual(pendingProjectJobs(all),[different,processing]);assert.equal(all.length,4);
  assert.deepEqual(pendingProjectJobs([failed,{...done,sha256:null}]),[failed]);
  assert.deepEqual(pendingProjectJobs([failed,{...done,assetKey:'modis_qc'}]),[failed]);
});
test('32-bit QA retains the high unsigned bit, all seven band fields, and undocumented codes',()=>{
  const raw=2 + 7*2**2 + 8*2**6 + 9*2**10 + 10*2**14 + 11*2**18 + 12*2**22 + 13*2**26 + 2**30 + 2**31;
  const decoded=decodeQuality('modis_qc',raw);
  assert(raw>2**31);assert.equal(decoded.binary.length,32);assert.equal(decoded.hex,`0x${raw.toString(16).toUpperCase()}`);
  assert.deepEqual(decoded.fields.map(f=>f.value),[2,7,8,9,10,11,12,13,1,1]);
  const unknown=decodeQuality('modis_qc',2**18).fields[5];assert.equal(unknown.value,1);assert.equal(unknown.defined,false);assert.equal(unknown.label,'Undocumented quality code');
  assert.deepEqual(decodeQuality('modis_qc',4294967295).fields,[]);
  for(const value of [-1,4294967296,2.5,NaN])assert.throws(()=>decodeQuality('modis_qc',value));
});
test('C61 state decodes salt pan and internal snow separately and never calls unset cloud flags verified clear',()=>{
  const state=decodeQuality('modis_state',3+2**14+2**15);
  assert.equal(state.fields[0].label,'Unset; product assumes clear');
  assert.equal(state.fields.find(f=>f.name==='Salt pan').value,1);assert.equal(state.fields.find(f=>f.name==='Internal snow').value,1);
  assert.equal(state.binary.length,16);assert.deepEqual(decodeQuality('modis_state',65535).fields,[]);
});
test('QA map inspection binds full-resolution counts, geometry, palette and source SHA instead of accepting display-only values',()=>{
  for (const key of ['modis_qc','modis_state']) {
    const data=metadataFor(key),job=jobFor(key);
    validateRasterInspection(data);verifiedMapMetadata(job,data);assert(qualityMatchesJob(job,data));
    for(const wrong of [{...data,dataType:'Int16'},{...data,nodata:0},{...data,crs:'EPSG:4326'},{...data,sha256:'b'.repeat(64)},
      {...data,quality:{...data.quality,countsFullResolution:false}},{...data,classes:data.classes.map(c=>({...c,count:c.count+1}))}])assert.throws(()=>verifiedMapMetadata(job,wrong));
  }
});
test('QA pixel validation checks every original bit field and unsigned fill value against the requested full-resolution cell',()=>{
  for(const key of ['modis_qc','modis_state']){
    const data=metadataFor(key),job=jobFor(key),coordinate=[data.bounds[0]+MODIS_PIXEL/2,data.bounds[3]-MODIS_PIXEL/2];
    for(const value of [key==='modis_qc'?2**31+1:2**15+1,data.nodata]){
      const noData=value===data.nodata,[label,color]=qualityPalette(key)[value%4];
      const pixel={jobId:job.id,sha256:job.sha256,crs:data.crs,coordinate,center:coordinate,pixel:[0,0],value,isNoData:noData,label:noData?'NoData':label,color:noData?'#000000':color,quality:decodeQuality(key,value)};
      assert(validQualityPixel(pixel,data));verifyPixelResult(pixel,job,data,coordinate);
      for(const wrong of [{...pixel,value:-1},{...pixel,reflectance:.1},{...pixel,quality:{...pixel.quality,hex:'0x0'}},{...pixel,pixel:[1,0]}])assert.throws(()=>verifyPixelResult(wrong,job,data,coordinate));
    }
  }
});

test('processed quality binds the unsigned profile, source pins, recorded clipped grid and full output counts',()=>{
  for(const key of ['modis_qc','modis_state']) {
    const data=metadataFor(key); data.width=100;data.height=67;
    data.bounds=[data.bounds[0],data.bounds[3]-67*MODIS_PIXEL,data.bounds[0]+100*MODIS_PIXEL,data.bounds[3]];
    data.quality.sampleCount=6700;data.quality.validSampleCount=6690;
    data.classes[0].count=6690;
    const original=jobFor(key),projectId='b62b8b84-e3b9-4e9f-8469-17297c53cc5a';
    const job={...original,kind:'raster_mosaic',itemId:`project:${projectId}`,
      mosaic:{projectId,assetKey:key,sources:[{jobId:original.id,sha256:original.sha256}]},
      mosaicOutput:{width:100,height:67,bandCount:1,crs:MODIS_CRS,bounds:data.bounds,pixelSize:data.pixelSize,sourceCount:1,coveredPixels:6690,maskedPixels:10,
        quality:{product:data.quality.product,band:key,layer:data.quality.layer,bits:key==='modis_qc'?32:16,nodata:data.nodata,pixelInterpretation:'PixelIsArea',definition:data.quality.definition}}};
    validateRasterInspection(data);verifiedMapMetadata(job,data);assert(qualityMatchesJob(job,data));
    assert.equal(qualityMatchesJob(original,data),false);
    for(const change of [j=>delete j.mosaicOutput.quality,j=>j.mosaicOutput.quality.bits=8,j=>j.mosaicOutput.calibration={},
      j=>j.mosaicOutput.coveredPixels++,j=>j.mosaicOutput.sourceCount++,j=>j.mosaicOutput.bounds=[0,0,1,1],j=>j.mosaic.assetKey='scl']) {
      const wrong=structuredClone(job);change(wrong);assert.throws(()=>verifiedMapMetadata(wrong,data));
    }
  }
});
