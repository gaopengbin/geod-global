import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {normalizeScene} from './catalog.js';
import {downloadableAssets} from './runtime-client.js';
import {projectRequest} from './projects-client.js';
import {projectCatalogScenes,mergeProjectCatalog} from './project-explore.js';
import {isSupportedAsset} from './providers.js';
import {landsatQualityIdentity,decodeLandsatQuality,validLandsatQualityPixel} from './landsat-quality.js';
const item=JSON.parse(readFileSync(new URL('../public/samples/landsat-quality-response.json',import.meta.url))).features[0];
test('official Landsat QA fields are downloadable and survive project catalogue recovery without calibration',()=>{
  const scene=normalizeScene(item,'planetary-landsat');
  for (const key of ['qa_pixel','qa_radsat']) {
    assert.equal(landsatQualityIdentity(scene.assets[key].href,key).id,item.id);
    assert(downloadableAssets(scene).some(a=>a.key===key));
    assert.equal(scene.assets[key].rasterBand.dataType,'uint16');
    assert.equal(scene.assets[key].rasterBand.scale,undefined);
  }
  const request=projectRequest({name:'Landsat QA test',scenes:[scene],areaName:'test',bbox:[-122.55,37.68,-122.32,37.84]});
  const saved=projectCatalogScenes({...request,id:'local-test'})[0];
  assert.equal(saved.assets.qa_pixel.href,scene.assets.qa_pixel.href);
  assert.equal(saved.assets.qa_radsat.rasterBand.nodata,undefined);
});
test('Landsat QA rejects signed URLs, wrong file keys, calibration and another scene',()=>{
  const href=item.assets.qa_pixel.href;
  assert(!isSupportedAsset(href+'?sig=private','qa_pixel'));
  assert(!isSupportedAsset(href,'qa_radsat'));
  assert.equal(landsatQualityIdentity(href.replace('/2025/','/2024/'),'qa_pixel'),null);
  for(const mutate of [v=>v.assets.qa_pixel['raster:bands'][0].data_type='int16',v=>v.assets.qa_pixel['raster:bands'][0].scale=.0000275,v=>v.assets.qa_pixel.href=v.assets.qa_pixel.href.replaceAll('20250628','20250627'),v=>v.assets.qa_radsat['raster:bands'][0].nodata=65535]) {
    const bad=structuredClone(item);mutate(bad);assert.throws(()=>normalizeScene(bad,'planetary-landsat'));
  }
});
test('unsigned high bits and reserved codes stay explicit; RADSAT zero remains valid',()=>{
  const raw=0x8040,decoded=decodeLandsatQuality('qa_pixel',raw);
  assert.equal(decoded.hex,'0x8040');assert.equal(decoded.fields[11].value,2);assert.equal(decoded.fields[11].defined,false);
  const r=decodeLandsatQuality('qa_radsat',65535);
  assert.equal(r.fields[12].value,15);assert.equal(r.fields[12].defined,false);
  const zero={value:0,isNoData:false,label:'No saturation or terrain occlusion flags',color:'#2563eb',quality:decodeLandsatQuality('qa_radsat',0)};
  assert(validLandsatQualityPixel(zero,{quality:{band:'qa_radsat'}}));
  assert(!validLandsatQualityPixel({...zero,isNoData:true},{quality:{band:'qa_radsat'}}));
});
test('catalogue refresh adds only quality from the original RGB processing directory',()=>{
  const current=normalizeScene(item,'planetary-landsat'),saved=structuredClone(current);
  delete saved.assets.qa_pixel;delete saved.assets.qa_radsat;
  assert.equal(mergeProjectCatalog({scenes:[current]},[saved]).scenes[0].assets.qa_pixel.href,current.assets.qa_pixel.href);
  const changed=structuredClone(current);for(const key of ['red','green','blue','qa_pixel','qa_radsat'])changed.assets[key].href=changed.assets[key].href.replaceAll('_20250629_','_20250630_');
  const kept=mergeProjectCatalog({scenes:[changed]},[saved]).scenes[0];
  assert.equal(kept.assets.red.href,saved.assets.red.href);assert.equal(kept.assets.qa_pixel,undefined);assert.equal(kept.assets.qa_radsat,undefined);
});
