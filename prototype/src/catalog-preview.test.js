import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene } from './catalog.js';
import { catalogPreviewChannel, catalogPreviewKind, modisReflectancePreview, radarPreview } from './catalog-preview.js';
import { elevationPreview, validateElevationImages } from './elevation-preview.js';
import { canDisplayImagery, providerById } from './providers.js';
import { itemTileBlob } from './item-preview-source.js';

const captured=JSON.parse(readFileSync(new URL('../qa/public-preview-catalog.json',import.meta.url)));
const modis=captured.catalogs['planetary-modis'].map(item=>normalizeScene(item,'planetary-modis'));
const radar=JSON.parse(readFileSync(new URL('../qa/sentinel-1d-rtc-catalog.json',import.meta.url))).features.map(item=>normalizeScene(item,'planetary-radar'));

test('transient preview failures recover within bounded retries; authorization, invalid content and cancellation remain errors',async()=>{
 let calls=0;
 const recovered=await itemTileBlob('https://example.test/tile',undefined,async()=>{
  calls++;
  if(calls===1)throw new TypeError('Failed to fetch');
  if(calls===2)return new Response('unavailable',{status:503});
  return new Response('png',{headers:{'content-type':'image/png'}});
 });
 assert.equal(await recovered.text(),'png');assert.equal(calls,3);
 for(const response of [new Response('denied',{status:401}),new Response('html',{headers:{'content-type':'text/html'}})]) {
  let count=0;await assert.rejects(itemTileBlob('https://example.test/tile',undefined,async()=>{count++;return response;}));assert.equal(count,1);
 }
 let exhausted=0;await assert.rejects(itemTileBlob('https://example.test/tile',undefined,async()=>{exhausted++;return new Response('unavailable',{status:503});}));assert.equal(exhausted,3);
 const controller=new AbortController();let cancelled=0;
 const pending=itemTileBlob('https://example.test/tile',controller.signal,async()=>{cancelled++;return new Response('unavailable',{status:503});});
 controller.abort();await assert.rejects(pending,{name:'AbortError'});assert.equal(cancelled,1);
});

test('MODIS RGB preview retains the full composite ID and maps signed source bands 1/4/3 without enabling the optical loader',()=>{
 for (const scene of modis) {
  const preview=modisReflectancePreview(scene), url=new URL(preview.url.replace('{z}/{x}/{y}','11/327/791'));
  assert.equal(url.searchParams.get('item'),scene.id);assert.equal(url.searchParams.get('collection'),'modis-09A1-061');
  assert.deepEqual(url.searchParams.getAll('assets'),['sur_refl_b01','sur_refl_b04','sur_refl_b03']);
  assert.equal(url.searchParams.get('rescale'),'0,3000');assert.equal(url.searchParams.get('nodata'),'-28672');
  assert.equal(url.searchParams.get('unscale'),'false');assert.equal(url.searchParams.get('reproject'),'nearest');
  assert.equal(url.searchParams.get('color_formula'),'Gamma RGB 2.2');
  assert.equal(catalogPreviewKind(scene.provider),'reflectance');assert.equal(catalogPreviewChannel(scene),'rgb');
 }
 assert.equal(canDisplayImagery(providerById('planetary-modis')),false);
 for(const mutate of [s=>s.id=s.id.replace('h08v05','h09v05'),s=>s.assets.green=s.assets.red,s=>s.assets.red.rasterBand.scale=1,
  s=>s.assets.red.rasterBand.dataType='uint16',s=>s.assets.blue.rasterBand.nodata=0,s=>s.crs='EPSG:4326',s=>s.bbox[0]=-190]) {
  const invalid=structuredClone(modis[0]);mutate(invalid);assert.throws(()=>modisReflectancePreview(invalid),/no verified/);
 }
});
test('Sentinel-1 C/D previews pin the actual polarization and express linear gamma0 as a display-only dB stretch',()=>{
 for(const scene of radar) for(const channel of ['vv','vh']) {
  const url=new URL(radarPreview(scene,channel).url.replace('{z}/{x}/{y}','11/327/791'));
  assert.equal(url.searchParams.get('item'),scene.id);assert.equal(url.searchParams.get('assets'),channel);
  assert.equal(url.searchParams.get('expression'),`where(${channel}>0,10*log10(${channel}),-30)`);
  assert.equal(url.searchParams.get('rescale'),'-30,0');assert.equal(url.searchParams.get('nodata'),'-32768');
  assert.equal(url.searchParams.get('unscale'),'false');assert.equal(url.searchParams.get('asset_as_band'),'true');
  assert.equal(catalogPreviewChannel(scene,'hh'),'vv');
  assert.throws(()=>radarPreview(scene,'hh'),/no verified/);
 }
 for(const mutate of [s=>s.assets.vv=s.assets.vh,s=>s.assets.vv['raster:bands'][0].data_type='uint16',s=>s.assets.vv['raster:bands'][0].nodata=0,
  s=>s.id=s.id.replace('_rtc','_other'),s=>s.bbox=[-122,38,-123,37]]) {
  const invalid=structuredClone(radar[0]);mutate(invalid);assert.throws(()=>radarPreview(invalid,'vv'),/no verified/);
 }
});
test('GLO-30 and GLO-90 preview geometry uses Float32 heights and half-pixel Point bounds, including valid zero',()=>{
 for(const provider of ['copernicus-dem','copernicus-dem-90']) {
  const scene=normalizeScene(captured.catalogs[provider][0],provider), preview=elevationPreview(scene);
  const [height,width]=scene.grid.shape,t=scene.grid.transform;
  assert.equal(preview.href,scene.assets.elevation.href);assert.deepEqual(preview.bounds,scene.bbox);
  const tags={BitsPerSample:[32],SampleFormat:[3],PhotometricInterpretation:1};
  const image={getGeoKeys:()=>({GTModelTypeGeoKey:2,GTRasterTypeGeoKey:2,GeographicTypeGeoKey:4326,GeogAngularUnitsGeoKey:9102}),
   getSamplesPerPixel:()=>1,getWidth:()=>width,getHeight:()=>height,fileDirectory:{getValue:key=>tags[key]},getGDALNoData:()=>null,
   getOrigin:()=>[t[2]+t[0]/2,t[5]+t[4]/2,0],getResolution:()=>[t[0],t[4],0]};
  assert.deepEqual(validateElevationImages([[image]],scene),[1,0,0,1,t[0]/2,t[4]/2]);
  assert.throws(()=>validateElevationImages([[{...image,getWidth:()=>1}]],scene));
  assert.throws(()=>validateElevationImages([[{...image,getOrigin:()=>[t[2],t[5],0]}]],scene));
  assert.throws(()=>validateElevationImages([[{...image,getGeoKeys:()=>({...image.getGeoKeys(),GTRasterTypeGeoKey:1})}]],scene));
  tags.SampleFormat=[1];assert.throws(()=>validateElevationImages([[image]],scene));
  const wrong=structuredClone(scene);wrong.assets.elevation.href=wrong.assets.elevation.href.replace('_W123_','_W122_');
  assert.throws(()=>elevationPreview(wrong));
 }
});
