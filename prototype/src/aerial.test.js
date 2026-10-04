import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene, searchURL } from './catalog.js';
import { assetReadURL, isSupportedAsset, naipIdentity, naipMatchesItem, naipPixelSize, prepareAssetAccess, providerForSearch } from './providers.js';
import { projectRequest } from './projects-client.js';
import { projectCatalogScenes, projectExploreSearch, mergeProjectCatalog } from './project-explore.js';
import { AERIAL_VIEWS, aerialMatchesJob, aerialPixelColor, aerialTitle, aerialView, validateNaipImages, verifyAerialView } from './aerial.js';
import { imageryHrefs } from './explore-imagery.js';
import { validateRasterInspection } from './runtime-client.js';
import { utmDefinition, verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';

const item=JSON.parse(readFileSync(new URL('../public/samples/naip-response.json',import.meta.url))).features[0];
const scene=normalizeScene(item,'planetary-naip');
const job={id:'fixture',status:'succeeded',kind:'download',assetKey:'aerial',itemId:scene.id,href:scene.assets.aerial.href,sha256:'a'.repeat(64)};
const metadata={width:9920,height:12280,bandCount:4,dataType:'UInt8',crs:'EPSG:26910',bounds:[543846,4171062,549798,4178430],pixelSize:[0.6,0.6],nodata:null,previewWidth:620,previewHeight:768,previewDataUrl:'data:image/png;base64,YQ==',sha256:job.sha256,classes:[],aerial:{product:'naip',bands:['red','green','blue','nir'],displayBands:[1,2,3],pixelInterpretation:'PixelIsArea'}};
test('aerial display selections retain source identity and raw NIR including valid zero',()=>{
 const pixel={values:[126,138,122],nearInfrared:0,color:'#7e8a7a'};
 const modes={rgb:[[1,2,3],'#7e8a7a'],cir:[[4,1,2],'#007e8a'],nir:[[4,4,4],'#000000']};
 assert.deepEqual(AERIAL_VIEWS,Object.keys(modes));
 for(const [view,[bands,color]] of Object.entries(modes)) {
  const next={...metadata,aerial:{...metadata.aerial,displayBands:bands}};
  assert.equal(aerialView(next),view);assert.equal(verifiedMapMetadata(job,next),next);
  assert.equal(verifyAerialView(metadata,next,view),next);assert.equal(aerialPixelColor(pixel,view),color);
  const point=[544446.3,4177829.7];
  const raw={...pixel,jobId:job.id,sha256:job.sha256,crs:metadata.crs,coordinate:point,pixel:[1000,1000],value:126,label:'RGB + NIR',isNoData:false};
  assert.equal(verifyPixelResult(raw,job,next,point),raw);
  for(const change of [{sha256:'b'.repeat(64)},{bounds:[0,1,2,3]},{pixelSize:[1,1]},{previewWidth:1},{aerial:{...next.aerial,coverageMask:'internal-1bit'}}]) assert.throws(()=>verifyAerialView(metadata,{...next,...change},view));
  assert.throws(()=>verifyAerialView(metadata,next,view==='nir'?'cir':'nir'));
 }
 assert.equal(aerialPixelColor({values:[0,0,0],nearInfrared:99},'cir'),'#630000');
 assert.equal(aerialPixelColor({values:[0,0,0],nearInfrared:99},'nir'),'#636363');
 assert.throws(()=>validateRasterInspection({...metadata,aerial:{...metadata.aerial,displayBands:[1,2,4]}}));
});
test('NAIP source pins original four-band identity and omits cloud filters',()=>{
 const url=new URL(searchURL({provider:'planetary-naip',bbox:scene.bbox,start:'2022-01-01',end:'2022-12-31',limit:20,cloud:'invalid'}));
 assert.equal(url.searchParams.get('collections'),'naip'); assert.equal(url.searchParams.has('query'),false);
 assert.equal(providerForSearch(url.href).id,'planetary-naip');
 assert.equal(scene.cloud,null); assert.equal(scene.dataset,'USDA NAIP RGB + NIR'); assert.equal(scene.gsd,0.6);
 assert.equal(naipIdentity(new URL(job.href).pathname),scene.id); assert.equal(isSupportedAsset(job.href,'visual'),false);
 assert.deepEqual(imageryHrefs(scene),[job.href]); assert.equal(scene.grid.transform.length,6);
 for(const href of [job.href+'?sig=secret',job.href.replace('naipeuwest','other'),job.href.replace('060cm','100cm'),job.href.replace('/2022/','/2021/'),job.href.replace('20220518.tif','20220230.tif')]) assert.equal(isSupportedAsset(href,'aerial'),false);
 for(const value of [{...item,id:item.id+'other'},{...item,assets:{...item.assets,image:{...item.assets.image,'eo:bands':[{common_name:'red'},{common_name:'green'},{common_name:'blue'},{common_name:'alpha'}]}}}]) assert.throws(()=>normalizeScene(value,'planetary-naip'),/unsupported NAIP/);
});
test('NAIP projects restore aerial identity, resolution and original unsigned href',()=>{
 const project={id:'project',...projectRequest({scenes:[scene],bounds:scene.bbox,name:'NAIP'})};
 assert.deepEqual(Object.keys(project.scenes[0].assets),['aerial']);
 const restored=projectCatalogScenes(project)[0]; assert.equal(restored.provider,'planetary-naip');assert.equal(restored.gsd,0.6);
 assert.equal(restored.assets.aerial.href,job.href); assert.equal(projectExploreSearch(project,{}).provider,'planetary-naip');
 assert.deepEqual(imageryHrefs(restored),[]);
 const merged=mergeProjectCatalog({scenes:[scene],pages:1,complete:true},[restored]);
 assert.deepEqual(merged.scenes[0].grid,scene.grid); assert.deepEqual(imageryHrefs(merged.scenes[0]),[job.href]);
});
test('captured 0.3 m and 1 m NAIP records restore full IDs and correct local raster spacing',()=>{
 const captured=JSON.parse(readFileSync(new URL('../qa/naip-resolution-catalog.json',import.meta.url)));
 for(const feature of captured.features) {
  const current=normalizeScene(feature,'planetary-naip'),spacing=feature.properties.gsd;
  assert.equal(current.gsd,spacing);assert.equal(naipPixelSize(current.id),spacing);
  assert.deepEqual(imageryHrefs(current),[feature.assets.image.href]);
  assert.equal(naipMatchesItem(new URL(current.assets.aerial.href).pathname,current.id),true);
  const request=projectRequest({scenes:[current],bounds:current.bbox,name:'NAIP resolution replay'});
  const restored=projectCatalogScenes({id:'replay',...request})[0];
  assert.equal(restored.id,feature.id);assert.equal(restored.gsd,spacing);
  assert.equal(restored.assets.aerial.href,feature.assets.image.href);
  const nativeJob={...job,itemId:current.id,href:current.assets.aerial.href};
  const data={...metadata,crs:current.crs,pixelSize:[spacing,spacing]};
  assert.equal(aerialMatchesJob(nativeJob,data),true);
  assert.equal(aerialMatchesJob(nativeJob,{...data,pixelSize:[spacing/100,spacing/100]}),false);
  assert.match(aerialTitle(current.id,date=>date),new RegExp(current.date.slice(0,10)));
 }
 const one=captured.features.find(feature=>feature.properties.gsd===1);
 assert.notEqual(naipIdentity(new URL(one.assets.image.href).pathname),one.id);
});
test('NAIP extra date binding rejects wrong grids, invalid dates and additional suffixes',()=>{
 const one='https://naipeuwest.blob.core.windows.net/naip/v002/fl/2017/fl_100cm_2017/28080/m_2808060_se_17_1_20171211.tif';
 const id='fl_m_2808060_se_17_1_20171211_20180201',path=new URL(one).pathname;
 for(const bad of [id+'_20190101',id.replace('20180201','20180230'),id.replace('20171211','20171212'),id.replace('2808060','2808061'),id.replace('_17_','_18_'),id.replace('fl_','me_')]) assert.equal(naipMatchesItem(path,bad),false);
 for(const bad of [one.replace('100cm','1cm'),one.replace('/28080/','/28081/'),one.replace('_17_','_+1_'),one.replace('.tif','_20180230.tif'),one.replace('.tif','_20180201_20190101.tif')]) assert.equal(isSupportedAsset(bad,'aerial'),false);
 const two='/naip/v002/me/2023/me_030cm_2023/45069/m_4506963_se_19_030_20231115_20240103.tif';
 for(const bad of ['me_m_4506963_se_19_030_20231115','me_m_4506963_se_19_030_20231115_20240104','me_m_4506963_se_19_030_20231115_20240103_20250101']) assert.equal(naipMatchesItem(two,bad),false);
});
test('captured legacy NAIP h filenames bind the full .6 catalogue ID without failing a page',()=>{
 const captured=JSON.parse(readFileSync(new URL('../qa/naip-legacy-catalog.json',import.meta.url)));
 const scenes=captured.features.map(feature=>normalizeScene(feature,'planetary-naip'));
 assert.equal(scenes.length,15);
 for (const [index,current] of scenes.entries()) {
  const feature=captured.features[index], path=new URL(feature.assets.image.href).pathname;
  assert.equal(current.id,feature.id);assert.equal(current.gsd,0.6);assert.equal(naipPixelSize(current.id),0.6);
  assert.equal(current.assets.aerial.href,feature.assets.image.href);assert.equal(naipMatchesItem(path,current.id),true);
  assert.match(aerialTitle(current.id,value=>value),new RegExp(current.date.slice(0,10)));
  const restored=projectCatalogScenes({id:'legacy',...projectRequest({scenes:[current],bounds:current.bbox,name:'Legacy NAIP'})})[0];
  assert.equal(restored.id,current.id);assert.equal(restored.assets.aerial.href,current.assets.aerial.href);
  for(const bad of [current.id.replace('_.6_','_060_'),current.id.replace('_10_','_11_'),current.id.replace('20161004','20161005')+'_20250101',current.id.replace('20161004','20160230')]) assert.equal(naipMatchesItem(path,bad),false);
  assert.equal(isSupportedAsset(feature.assets.image.href.replace('060cm','100cm'),'aerial'),false);
 }
});
test('legacy 0.6 m NIR tags require the reviewed h/.6 source, four band roles and matching TIFF grid',()=>{
 const feature=JSON.parse(readFileSync(new URL('../qa/naip-legacy-catalog.json',import.meta.url))).features[0];
 const current=normalizeScene(feature,'planetary-naip'),[height,width]=current.grid.shape,t=current.grid.transform;
 const tags={PhotometricInterpretation:2,BitsPerSample:[8,8,8,8],ExtraSamples:[2],SampleFormat:[1,1,1,1]};
 const image={getGeoKeys:()=>({GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjectedCSTypeGeoKey:26910}),getSamplesPerPixel:()=>4,fileDirectory:{getValue:name=>tags[name]},getGDALNoData:()=>null,getWidth:()=>width,getHeight:()=>height,getResolution:()=>[0.6,-0.6,0],getOrigin:()=>[t[2],t[5],0]};
 validateNaipImages([[image]],current);
 const data={...metadata,width,height,bounds:[t[2],t[5]+t[4]*height,t[2]+t[0]*width,t[5]],crs:current.crs,aerial:{...metadata.aerial,sourceExtraSample:2}};
 const record={...job,itemId:current.id,href:current.assets.aerial.href};
 assert.equal(verifiedMapMetadata(record,data),data);
 assert.throws(()=>verifiedMapMetadata(job,data));
 assert.throws(()=>validateNaipImages([[image]],{...current,id:current.id.replace('_.6_','_060_')}));
 assert.throws(()=>validateNaipImages([[image]],{...current,grid:{...current.grid,transform:[0.6,0,t[2]+0.6,0,-0.6,t[5]]}}));
 const wrongBands={...current,assets:{aerial:{...current.assets.aerial,'eo:bands':['red','green','blue','alpha'].map(common_name=>({common_name}))}}};
 assert.throws(()=>validateNaipImages([[image]],wrongBands));
 tags.ExtraSamples=[1];assert.throws(()=>validateNaipImages([[image]],current));
});
test('local aerial metadata and NIR pixel validation reject swapped datum and alpha semantics',()=>{
 assert.equal(validateRasterInspection(metadata),metadata); assert.equal(verifiedMapMetadata(job,metadata),metadata);
 assert.match(utmDefinition('EPSG:26910'),/datum=NAD83/); assert.equal(aerialMatchesJob(job,{...metadata,crs:'EPSG:32610'}),false);
 for(const data of [{...metadata,aerial:undefined},{...metadata,nodata:0},{...metadata,aerial:{...metadata.aerial,bands:['red','green','blue','alpha']}},{...metadata,dataType:'UInt16'}]) assert.throws(()=>validateRasterInspection(data));
 const coordinate=[544446.3,4177829.7]; const pixel={jobId:job.id,sha256:job.sha256,crs:metadata.crs,coordinate,pixel:[1000,1000],value:126,values:[126,138,122],nearInfrared:0,label:'RGB + NIR',color:'#7e8a7a',isNoData:false};
 assert.equal(verifyPixelResult(pixel,job,metadata,coordinate),pixel);
 for(const result of [{...pixel,nearInfrared:undefined},{...pixel,nearInfrared:256},{...pixel,isNoData:true}]) assert.throws(()=>verifyPixelResult(result,job,metadata,coordinate));
});
test('NAIP online original header validation includes extra sample, datum and grid',()=>{
 const values={PhotometricInterpretation:2,BitsPerSample:[8,8,8,8],ExtraSamples:[0],SampleFormat:[1,1,1,1]};
 const image={getGeoKeys:()=>({GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjectedCSTypeGeoKey:26910}),getSamplesPerPixel:()=>4,fileDirectory:{getValue:name=>values[name]},getGDALNoData:()=>null,getWidth:()=>9920,getHeight:()=>12280,getResolution:()=>[0.6,-0.6,0],getOrigin:()=>[543846,4178430,0]};
 validateNaipImages([[image]],scene);
 for(const extra of [1,2]) {values.ExtraSamples=[extra];assert.throws(()=>validateNaipImages([[image]],scene));}
 values.ExtraSamples=[0];assert.throws(()=>validateNaipImages([[image]],{...scene,crs:'EPSG:32610'}));
});
test('legacy one metre NIR tags are admitted only with matching original NAIP band roles and grid',()=>{
 const feature=JSON.parse(readFileSync(new URL('../qa/naip-resolution-catalog.json',import.meta.url))).features.find(item=>item.properties.gsd===1);
 const current=normalizeScene(feature,'planetary-naip'),[height,width]=current.grid.shape,t=current.grid.transform;
 const tags={PhotometricInterpretation:2,BitsPerSample:[8,8,8,8],ExtraSamples:[2],SampleFormat:[1,1,1,1]};
 const image={getGeoKeys:()=>({GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjectedCSTypeGeoKey:26917}),getSamplesPerPixel:()=>4,fileDirectory:{getValue:name=>tags[name]},getGDALNoData:()=>null,getWidth:()=>width,getHeight:()=>height,getResolution:()=>[1,-1,0],getOrigin:()=>[t[2],t[5],0]};
 validateNaipImages([[image]],current);
 for(const extra of [1,3]) {tags.ExtraSamples=[extra];assert.throws(()=>validateNaipImages([[image]],current));}
 tags.ExtraSamples=[2];
 const wrongBands={...current,assets:{aerial:{...current.assets.aerial,'eo:bands':['red','green','blue','alpha'].map(common_name=>({common_name}))}}};
 assert.throws(()=>validateNaipImages([[image]],wrongBands));
 assert.throws(()=>validateNaipImages([[image]],{...current,id:current.id.replace('_1_','_100_')}));
 const record={...job,itemId:current.id,href:current.assets.aerial.href};
 const data={...metadata,width,height,bounds:[t[2],t[5]+t[4]*height,t[2]+t[0]*width,t[5]],crs:current.crs,pixelSize:[1,1],previewWidth:Math.floor(width*768/height),previewHeight:768,aerial:{...metadata.aerial,sourceExtraSample:2}};
 assert.equal(verifiedMapMetadata(record,data),data);
 assert.throws(()=>verifiedMapMetadata(job,{...metadata,aerial:{...metadata.aerial,sourceExtraSample:2}}));
 assert.throws(()=>verifiedMapMetadata(record,{...data,aerial:{...data.aerial,sourceExtraSample:1}}));
});
test('derived NAIP grids require their recorded independent mask and permit transparent empty pixels',()=>{
 const derived={...metadata,aerial:{...metadata.aerial,coverageMask:'internal-1bit'}};
 const resultJob={...job,kind:'raster_mosaic',mosaic:{assetKey:'aerial',sources:[{jobId:'source',sha256:job.sha256}]},
   mosaicOutput:{width:derived.width,height:derived.height,bandCount:4,crs:derived.crs,bounds:derived.bounds,pixelSize:derived.pixelSize,aerial:derived.aerial}};
 assert.equal(verifiedMapMetadata(resultJob,derived),derived);
 const infrared={...derived,aerial:{...derived.aerial,displayBands:[4,1,2]}};
 assert.equal(verifiedMapMetadata(resultJob,infrared),infrared);
 assert.throws(()=>verifiedMapMetadata({...resultJob,mosaicOutput:{...resultJob.mosaicOutput,aerial:infrared.aerial}},infrared));
 const coordinate=[544446.3,4177829.7];
 const empty={jobId:job.id,sha256:job.sha256,crs:derived.crs,coordinate,pixel:[1000,1000],value:0,values:[0,0,0],nearInfrared:0,label:'RGB + NIR',color:'#000000',isNoData:true};
 assert.equal(verifyPixelResult(empty,resultJob,derived,coordinate),empty);
 assert.throws(()=>verifiedMapMetadata(resultJob,metadata));
 assert.throws(()=>verifiedMapMetadata({...resultJob,mosaicOutput:{...resultJob.mosaicOutput,width:1}},derived));
 assert.throws(()=>verifyPixelResult({...empty,nearInfrared:99},resultJob,derived,coordinate));
 assert.throws(()=>verifiedMapMetadata(job,derived));
});
test('NAIP container access coalesces concurrent requests and remains outside saved source URLs',async()=>{
 let calls=0;const now=Date.now(),expiry=new Date(now+3600000).toISOString();
 const fetcher=async url=>{calls++;assert.equal(url,'https://planetarycomputer.microsoft.com/api/sas/v1/token/naipeuwest/naip');return new Response(JSON.stringify({'msft:expiry':expiry,token:`sp=r&sr=c&se=${encodeURIComponent(expiry)}&sig=EPHEMERAL-TEST`}),{status:200});};
 await Promise.all(Array.from({length:8},()=>prepareAssetAccess([job.href],{fetcher,force:true,now})));
 assert.equal(calls,1);assert.equal(new URL(assetReadURL(job.href,now)).searchParams.get('sig'),'EPHEMERAL-TEST');
 assert.equal(new URL(scene.assets.aerial.href).search,'');
 assert.throws(()=>assetReadURL(job.href,now+3600000),/expired/);
});
