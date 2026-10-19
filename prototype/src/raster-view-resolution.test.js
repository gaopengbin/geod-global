import test from 'node:test';
import assert from 'node:assert/strict';
import View from 'ol/View.js';
import {rasterPixelResolution, setRasterZoomLimit} from './raster-view-resolution.js';

const mercator = () => new View({projection:'EPSG:3857', center:[0,0], resolution:100, maxZoom:28});
const aerial = {crs:'EPSG:26910', grid:{shape:[10000,10000], transform:[0.3,0,543846,0,-0.3,4178430]},
  gsd:1, bbox:[-123,37,-122,38]};

test('NAIP uses its NAD83 pixel grid before declared GSD and permits native detail in Mercator', () => {
  const view=mercator(), original=structuredClone(aerial), pixel=rasterPixelResolution(aerial,view.getProjection());
  assert.ok(pixel>0.3 && pixel<0.5);
  setRasterZoomLimit(view,[aerial]);
  assert.ok(view.getMinResolution()>=pixel/4-1e-9);
  assert.ok(view.getMinResolution()<pixel/2);
  assert.ok(view.getMinResolution()<0.15);
  view.setResolution(1e-6);
  assert.equal(view.getResolution(),view.getMinResolution());
  assert.deepEqual(aerial,original);
});

test('declared metre GSD accounts for latitude instead of treating Mercator units as ground metres', () => {
  const equator={gsd:0.3,bbox:[-1,-1,1,1]}, north={gsd:0.3,bbox:[-1,59,1,61]};
  const a=rasterPixelResolution(equator,'EPSG:3857'), b=rasterPixelResolution(north,'EPSG:3857');
  assert.ok(Math.abs(b/a-2)<0.01);
});

test('local projected and geographic workspaces use original pixels regardless of preview dimensions', () => {
  const raster={crs:'EPSG:26910',bounds:[500000,4100000,503000,4103000],pixelSize:[0.3,0.3],
    width:10000,height:10000,previewWidth:768,previewHeight:768};
  const view=new View({projection:'EPSG:26910',center:[501500,4101500],resolution:1000,maxResolution:100000,maxZoom:28});
  setRasterZoomLimit(view,[raster]);
  assert.ok(view.getMinResolution()>=0.075 && view.getMinResolution()<0.15);
  const geographic={crs:'EPSG:4326',bounds:[115,39,116,40],width:3600,height:3600};
  assert.equal(rasterPixelResolution(geographic,'EPSG:4326'),1/3600);
});

test('mixed resolutions preserve the finest layer and changing limits preserves center and legal zoom', () => {
  const view=mercator(), coarse={gsd:30,bbox:[-1,-1,1,1]}, fine={gsd:0.3,bbox:[-1,-1,1,1]};
  view.setCenter([100,200]); view.setResolution(100);
  setRasterZoomLimit(view,[coarse,fine]);
  const minimum=view.getMinResolution();
  assert.ok(minimum<0.15);
  assert.equal(view.getResolution(),100); assert.deepEqual(view.getCenter(),[100,200]);
  setRasterZoomLimit(view,[coarse]);
  assert.ok(view.getMinResolution()>minimum*50);
  view.setResolution(1e-6);
  assert.equal(view.getResolution(),view.getMinResolution());
});

test('unknown or invalid grids keep a finite fallback without fabricating pixel geometry', () => {
  const view=mercator();
  const invalid=[null,{gsd:0,bbox:[0,0,1,1]},{crs:'EPSG:unknown',bounds:[0,0,1,1],pixelSize:[0.3,0.3]},
    {crs:'EPSG:4326',bounds:[0,0,1,1],pixelSize:[NaN,0.3]}];
  invalid.forEach(value=>assert.equal(rasterPixelResolution(value,'EPSG:3857'),null));
  setRasterZoomLimit(view,invalid);
  assert.ok(Number.isFinite(view.getMinResolution()) && view.getMinResolution()>=2.5);
});
