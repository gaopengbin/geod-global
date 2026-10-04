import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene, searchURL, fetchCatalogPage } from './catalog.js';
import { radarAssetIdentity, RADAR_KEYS, radarMatchesJob, validRadarPixel } from './radar.js';
import { isSupportedAsset, providerForAssets } from './providers.js';
import { projectRequest } from './projects-client.js';
import { projectCatalogScenes } from './project-explore.js';
import { validateRasterInspection, downloadableAssets } from './runtime-client.js';
import { verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';
const fixture=JSON.parse(readFileSync(new URL('../qa/sentinel-1-rtc-catalog.json',import.meta.url),'utf8'));

test('real RTC identities retain polarizations across both catalogue ID generations and project restore',()=>{
  for(const item of fixture.features) {
    const scene=normalizeScene(item,'planetary-radar');
    assert.equal(scene.cloud,null); assert.equal(scene.gsd,10);
    assert.deepEqual(downloadableAssets(scene).map(a=>a.key),['vv','vh']);
    for(const key of ['vv','vh']) {
      const href=scene.assets[key].href; assert.ok(isSupportedAsset(href,key)); assert.ok(radarAssetIdentity(href,key).ids.includes(item.id));
      assert.equal(isSupportedAsset(href,'visual'),false); assert.equal(isSupportedAsset(href+'?sig=secret',key),false);
      assert.equal(isSupportedAsset(href.replace('/2025/6/','/2025/7/'),key),false);
    }
    const request=projectRequest({name:'RTC',scenes:[scene],bounds:[-122.55,37.65,-122.4,37.8]});
    assert.equal(providerForAssets(request.scenes[0].assets),'planetary-radar');
    const restored=projectCatalogScenes(request)[0];
    assert.deepEqual(downloadableAssets(restored).map(a=>a.href),downloadableAssets(scene).map(a=>a.href));
    assert.equal(restored.properties['sat:orbit_state'],'unknown');
    assert.throws(()=>normalizeScene({...item,assets:{...item.assets,vv:{...item.assets.vv,href:item.assets.vh.href}}},'planetary-radar'),/polarization/);
  }
});

test('RTC search omits cloud filters and preserves polarization through an empty catalogue page',async()=>{
  const href=searchURL({provider:'planetary-radar',bbox:[-122.55,37.65,-122.4,37.8],start:'2025-06-01',end:'2025-06-30',cloud:'bad',limit:2,orbit:'descending',polarization:'hh'});
  const url=new URL(href); assert.equal(url.searchParams.get('geod-polarization'),'hh');
  const q=JSON.parse(url.searchParams.get('query')); assert.equal(q['eo:cloud_cover'],undefined); assert.deepEqual(q['sat:orbit_state'],{eq:'descending'});
  const page=await fetchCatalogPage(href,{fetcher:async actual=>{
    assert.equal(new URL(actual).searchParams.has('geod-polarization'),false);
    return {ok:true,json:async()=>({...fixture,links:[{rel:'next',href:actual+'&token=next'}]})};
  }});
  assert.deepEqual(page.scenes,[]); assert.equal(new URL(page.next).searchParams.get('geod-polarization'),'hh');
});

test('RTC display and map contracts keep Float32 linear values separate from derived dB and NoData',()=>{
  const scene=normalizeScene(fixture.features[0],'planetary-radar');
  const job={id:'aa3e5e7f-8c13-4b26-af73-d63f8c1a83be',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'vv',href:scene.assets.vv.href,sha256:'a'.repeat(64)};
  const metadata={width:2,height:2,bandCount:1,dataType:'Float32',crs:'EPSG:32610',bounds:[500000,4199980,500020,4200000],pixelSize:[10,10],nodata:-32768,previewWidth:2,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:job.sha256,radar:{product:'sentinel-1-iw-rtc',polarization:'VV',quantity:'gamma0',unit:'linear',displayUnit:'dB',displayRange:[-30,0],sampleCount:4,validSampleCount:3,overview:false}};
  assert.equal(validateRasterInspection(metadata),metadata); assert.equal(verifiedMapMetadata(job,metadata),metadata);
  const value=Math.fround(0.1);
  const pixel={jobId:job.id,sha256:job.sha256,crs:metadata.crs,coordinate:[500005,4199995],pixel:[0,0],value,decibels:10*Math.log10(value),label:'Gamma0 · VV',color:'#808080',isNoData:false};
  assert.equal(verifyPixelResult(pixel,job,metadata,pixel.coordinate),pixel);
  assert.equal(validRadarPixel({...pixel,value:0,decibels:undefined},metadata),true);
  assert.equal(validRadarPixel({...pixel,value:-32768,decibels:undefined,isNoData:true},metadata),true);
  assert.equal(validRadarPixel({...pixel,value:-1,decibels:undefined},metadata),false);
  assert.equal(radarMatchesJob(job,{...metadata,radar:{...metadata.radar,polarization:'VH'}}),false);
  assert.throws(()=>validateRasterInspection({...metadata,reflectance:{}}),/invalid/);
  assert.equal(RADAR_KEYS.length,4);
});

test('derived RTC map and inspection pin polarization, Float32 quantity, source hashes and output geometry',()=>{
  const original=JSON.parse(readFileSync(new URL('../qa/radar-native-verification.json',import.meta.url),'utf8'));
  const metadata={...original.raster,bandCount:1,previewWidth:768,previewHeight:582,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:'a'.repeat(64)};
  const profile={product:'sentinel-1-iw-rtc',polarization:'VV',quantity:'gamma0',unit:'linear'};
  const plan={width:metadata.width,height:metadata.height,bandCount:1,crs:metadata.crs,bounds:metadata.bounds,pixelSize:[10,10],sourceCount:1,coveredPixels:1,maskedPixels:0,radar:profile};
  const job={id:'aa3e5e7f-8c13-4b26-af73-d63f8c1a83be',kind:'raster_mosaic',status:'succeeded',itemId:'project:ec1c0fba-d2b4-4b65-9174-0b4cf7312313',assetKey:'vv',href:original.href,sha256:metadata.sha256,
    mosaic:{projectId:'ec1c0fba-d2b4-4b65-9174-0b4cf7312313',assetKey:'vv',sources:[{jobId:original.jobId,sha256:original.sha256}]},mosaicOutput:plan};
  assert.equal(verifiedMapMetadata(job,metadata),metadata);
  for(const change of [
    {radar:{...profile,polarization:'VH'}},{radar:{...profile,unit:'dB'}},
    {radar:{...profile,quantity:'sigma0'}},{sourceCount:2},{bounds:[534371,...plan.bounds.slice(1)]},
    {coveredPixels:0},{calibration:{scale:0.0001}},
  ]) assert.equal(radarMatchesJob({...job,mosaicOutput:{...plan,...change}},metadata),false);
  assert.equal(radarMatchesJob({...job,mosaic:{...job.mosaic,assetKey:'vh'}},metadata),false);
  assert.equal(radarMatchesJob({...job,mosaic:{...job.mosaic,sources:[{jobId:original.jobId,sha256:'bad'}]}},metadata),false);
  assert.equal(radarMatchesJob(job,{...metadata,aerial:{}}),false);
});
