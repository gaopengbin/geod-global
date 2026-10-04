import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {normalizeScene,searchURL} from './catalog.js';
import {DEM90_HOST,DEM_HOST,demProduct,providerForSearch,copDemLabel,isSupportedAsset} from './providers.js';
import {projectRequest} from './projects-client.js';
import {projectCatalogScenes,projectExploreSearch} from './project-explore.js';
import {verifiedMapMetadata,coordinateToPixel,verifyPixelResult} from './workspace-map-geometry.js';

const item=JSON.parse(readFileSync('prototype/public/samples/cop-dem-90-response.json','utf8')).features[0];
const scene=normalizeScene(item,'copernicus-dem-90');
const job={id:'glo90',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'elevation',href:scene.assets.elevation.href,sha256:'a'.repeat(64)};
const metadata={width:800,height:1200,bandCount:1,dataType:'Float32',crs:'EPSG:4326',bounds:scene.bbox,pixelSize:[1/800,1/1200],nodata:null,previewWidth:2,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:job.sha256,
  elevation:{product:'cop-dem-glo-90',heightUnit:'metre',coordinateUnit:'degree',verticalReference:'EPSG:3855',pixelInterpretation:'PixelIsPoint',displayRange:[-80,1000],sampleCount:4,validSampleCount:4,nodataIsNan:false}};

test('GLO-90 catalog keeps original declarations, resolves exact bucket and restores its own collection',()=>{
  const raw=JSON.stringify(item), source=structuredClone(item);source.properties.gsd=30;source.assets.data['raster:bands'][0].spatial_resolution=30;
  const copy=JSON.stringify(source), normalized=normalizeScene(source,'copernicus-dem-90');
  assert.equal(JSON.stringify(source),copy);assert.equal(JSON.stringify(item),raw);
  assert.equal(normalized.gsd,90);assert.equal(normalized.properties.gsd,30);assert.equal(normalized.assets.elevation['raster:bands'][0].spatial_resolution,30);
  assert.equal(demProduct(scene.id).height,1200);assert.equal(copDemLabel([scene]),'GLO-90');
  assert.equal(scene.assets.elevation.href,`https://${DEM90_HOST}/${item.id}/${item.id}.tif`);
  assert.equal(isSupportedAsset(scene.assets.elevation.href.replace(DEM90_HOST,DEM_HOST),'elevation'),false);
  assert.throws(()=>normalizeScene(item,'copernicus-dem'));
  const project=projectRequest({scenes:[scene],bounds:[-.1,51.5,.1,51.6],name:'GLO-90'});
  const restored=projectCatalogScenes(project)[0];assert.equal(restored.assets.elevation.href,job.href);assert.equal(restored.gsd,90);
  const url=new URL(searchURL({...projectExploreSearch(project,{}),limit:10}));
  assert.equal(url.searchParams.get('collections'),'cop-dem-glo-90');assert.equal(url.searchParams.has('datetime'),false);assert.equal(providerForSearch(url.href).id,'copernicus-dem-90');
});
test('GLO-90 geographic Point grid accepts 800 by 1200 high-latitude pixels and rejects GLO-30 substitutions',()=>{
  assert.equal(verifiedMapMetadata(job,metadata),metadata);
  for(const patch of [{height:3600},{pixelSize:[1/2400,1/3600]},{elevation:{...metadata.elevation,product:'cop-dem-glo-30-public'}},{bounds:[-1,51,0,52]}]) assert.throws(()=>verifiedMapMetadata(job,{...metadata,...patch}));
  const coordinate=[-1,52];assert.deepEqual(coordinateToPixel(coordinate,metadata),[0,0]);
  for(const value of [0,Math.fround(-79.07938),Math.fround(657.42194)]) {
    const result={jobId:job.id,sha256:job.sha256,crs:'EPSG:4326',coordinate,pixel:[0,0],center:coordinate,value,isNoData:false,label:'Elevation',color:'#808080'};
    assert.equal(verifyPixelResult(result,job,metadata,coordinate),result);
  }
});
