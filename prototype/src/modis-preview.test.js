import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {normalizeScene} from './catalog.js';
import {prepareAssetAccess,assetReadURL} from './providers.js';
import {modisPreviewOriginals,modisPreviewGrid,validateModisPreviewImages} from './modis-preview.js';
import {MODIS_PROJECTION} from './modis.js';
import proj4 from 'proj4';
const captured=JSON.parse(readFileSync(new URL('../qa/public-preview-catalog.json',import.meta.url)));
const scene=normalizeScene(captured.catalogs['planetary-modis'][0],'planetary-modis');

test('MODIS sinusoidal forward and inverse stay finite and match the original sphere in both hemispheres',()=>{
  const radius=6371007.181;
  for(const lonLat of [[-122.4,37.76],[116.4,39.9],[-48,-16],[0,0]]){
    const [lon,lat]=lonLat.map(x=>x*Math.PI/180),xy=[radius*lon*Math.cos(lat),radius*lat];
    const forward=proj4('EPSG:4326',MODIS_PROJECTION,lonLat),inverse=proj4(MODIS_PROJECTION,'EPSG:4326',xy);
    for(let i=0;i<2;i++){assert.ok(Math.abs(forward[i]-xy[i])<1e-6);assert.ok(Math.abs(inverse[i]-lonLat[i])<1e-8);}
  }
});

test('MODIS original preview obtains one strictly read-only token and never alters source pins',async()=>{
  const hrefs=modisPreviewOriginals(scene),snapshot=JSON.stringify(scene),now=Date.now();let calls=0;
  const token='sp=r&sr=c&sig=test-only&se='+encodeURIComponent(new Date(now+3600000).toISOString());
  const fetcher=async(url)=>{calls++;assert.equal(url,'https://planetarycomputer.microsoft.com/api/sas/v1/token/modiseuwest/modis-061-cogs');return new Response(JSON.stringify({token,'msft:expiry':new Date(now+3600000).toISOString()}),{headers:{'content-type':'application/json'}});};
  await Promise.all(hrefs.map(href=>prepareAssetAccess([href],{force:true,fetcher,now})));
  assert.equal(calls,1);
  for(const href of hrefs)assert.equal(new URL(assetReadURL(href,now)).searchParams.get('sig'),'test-only');
  assert.equal(JSON.stringify(scene),snapshot);
  for(const badToken of [token.replace('sp=r','sp=rw'),token+'&sig=duplicate',token+'#fragment',token.replace('sr=c','sr=b')])
    await assert.rejects(prepareAssetAccess(hrefs,{force:true,fetcher:async()=>new Response(JSON.stringify({token:badToken,'msft:expiry':new Date(now+3600000).toISOString()})),now}),/invalid data access/);
  await assert.rejects(prepareAssetAccess([hrefs[0]+'?sig=unreviewed'],{fetcher,now}),/Unsupported MODIS/);
});

test('MODIS preview accepts only matching signed-DN sinusoidal originals and rejects shifted grids or masks',()=>{
  const t=modisPreviewGrid(scene);
  const keys={GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjCoordTransGeoKey:24,ProjLinearUnitsGeoKey:9001,GeogAngularUnitsGeoKey:9102,GeogSemiMajorAxisGeoKey:6371007.181,GeogSemiMinorAxisGeoKey:6371007.181,ProjCenterLongGeoKey:0,ProjFalseEastingGeoKey:0,ProjFalseNorthingGeoKey:0};
  const tags={BitsPerSample:[16],SampleFormat:[2],PhotometricInterpretation:1};
  const image={getWidth:()=>2400,getHeight:()=>2400,getSamplesPerPixel:()=>1,getGeoKeys:()=>keys,getGDALNoData:()=>-28672,getOrigin:()=>[t[2],t[5],0],getResolution:()=>[t[0],t[4],0],fileDirectory:{getValue:key=>tags[key]}};
  validateModisPreviewImages([[image],[image],[image]],scene);
  for(const changed of [{...image,getGDALNoData:()=>0},{...image,getOrigin:()=>[t[2]+t[0],t[5],0]},{...image,getGeoKeys:()=>({...keys,GeogSemiMajorAxisGeoKey:6378137})},{...image,getSamplesPerPixel:()=>3}])
    assert.throws(()=>validateModisPreviewImages([[image],[changed],[image]],scene),/metadata does not match/);
  assert.throws(()=>modisPreviewOriginals({...scene,grid:{...scene.grid,transform:t.map((v,i)=>i===2?v+1:v)}}),/metadata does not match/);
});
