import React from 'react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {act,cleanup,render,waitFor} from '@testing-library/react';
import {transformExtent} from 'ol/proj.js';
import {ExploreMap} from './explore-map.jsx';
import {rasterPixelResolution} from './raster-view-resolution.js';

const maps=vi.hoisted(()=>({instances:[]}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:value=>value})}));
vi.mock('ol/Map.js',()=>({default:class{
  constructor(options){this.view=options.view;this.layers=options.layers;vi.spyOn(this.view,'fit').mockImplementation(()=>{});maps.instances.push(this);}
  getView(){return this.view;}getSize(){return [900,600];}on(){}updateSize(){}setTarget(){}dispose(){}
  addLayer(layer){this.layers.push(layer);}removeLayer(layer){this.layers=this.layers.filter(value=>value!==layer);}
}}));
beforeEach(()=>{maps.instances=[];});afterEach(()=>{cleanup();vi.restoreAllMocks();});
const bounds=[-80,40,-71,45];
const first={id:'first',date:'2023-09-26',provider:'planetary-naip',assets:{},bbox:[-73.5,44.1,-73.4,44.2],geometry:null};
const second={...first,id:'second',bbox:[-74,42,-73.8,42.2]};

it('uses the real projected catalog footprint and re-centers on repeated requests without loading original imagery',async()=>{
  const expected=transformExtent(second.bbox,'EPSG:4326','EPSG:3857');
  const request={id:second.id,revision:1};
  const view=render(<ExploreMap area={bounds} scene={first} scenes={[first,second]} focusRequest={request}/>);
  await waitFor(()=>expect(maps.instances[0].getView().fit).toHaveBeenCalled());
  expect(maps.instances[0].getView().fit.mock.calls.at(-1)[0]).toEqual(expected);
  const count=maps.instances[0].getView().fit.mock.calls.length;
  view.rerender(<ExploreMap area={bounds} scene={first} scenes={[first,second]} focusRequest={{id:second.id,revision:2}}/>);
  expect(maps.instances).toHaveLength(1);
  expect(maps.instances[0].getView().fit.mock.calls.length).toBeGreaterThan(count);
  expect(maps.instances[0].getView().fit.mock.calls.at(-1)[0]).toEqual(expected);
  expect(maps.instances[0].layers.some(layer=>layer.getClassName?.()==='explore-base-layer')).toBe(false);
});

it('retains the requested grid after the active scene rebuilds the map and does not jump on selection alone',async()=>{
  const expected=transformExtent(second.bbox,'EPSG:4326','EPSG:3857');
  const view=render(<ExploreMap area={bounds} scene={first} scenes={[first,second]}/>);
  await act(async()=>{await new Promise(resolve=>requestAnimationFrame(resolve));});
  const initial=maps.instances[0],count=initial.getView().fit.mock.calls.length;
  view.rerender(<ExploreMap area={bounds} scene={first} scenes={[first,second]} selectedIds={[first.id]}/>);
  expect(initial.getView().fit.mock.calls).toHaveLength(count);
  view.rerender(<ExploreMap area={bounds} scene={second} scenes={[first,second]} focusRequest={{id:second.id,revision:1}}/>);
  await act(async()=>{await new Promise(resolve=>requestAnimationFrame(resolve));});
  const current=maps.instances.at(-1);
  expect(current.getView().fit.mock.calls.at(-1)[0]).toEqual(expected);
  const previous=current.getView().fit.mock.calls.length;
  view.rerender(<ExploreMap area={bounds} scene={second} scenes={[first,second]} focusRequest={{id:'absent',revision:2}}/>);
  expect(current.getView().fit.mock.calls).toHaveLength(previous);
});

it('allows high-resolution scenes to zoom to original detail and refreshes the limit without rebuilding the map',async()=>{
  const fine={...first,gsd:0.3}, view=render(<ExploreMap area={bounds} scene={fine} scenes={[fine]}/>);
  await act(async()=>{await new Promise(resolve=>requestAnimationFrame(resolve));});
  const instance=maps.instances[0], mapView=instance.getView();
  const pixel=rasterPixelResolution(fine,mapView.getProjection());
  expect(mapView.getMinResolution()).toBeGreaterThanOrEqual(pixel/4-1e-9);
  expect(mapView.getMinResolution()).toBeLessThan(pixel/2);
  expect(mapView.getMinResolution()).toBeLessThan(0.15);
  expect(mapView.fit.mock.calls.at(-1)[1].maxZoom).toBe(mapView.getMaxZoom());
  mapView.setResolution(0.00001);
  expect(mapView.getResolution()).toBe(mapView.getMinResolution());
  const coarse={...fine,gsd:30};
  view.rerender(<ExploreMap area={bounds} scene={coarse} scenes={[coarse]}/>);
  expect(maps.instances).toHaveLength(1);
  expect(mapView.getMinResolution()).toBeGreaterThan(5);
  expect(mapView.getResolution()).toBe(mapView.getMinResolution());
  expect(instance.layers.some(layer=>layer.getClassName?.()==='explore-base-layer')).toBe(false);
});
