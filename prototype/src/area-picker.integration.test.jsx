import React from 'react';
import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import GeoJSON from 'ol/format/GeoJSON.js';
import { fromExtent } from 'ol/geom/Polygon.js';
import { AreaPicker } from './area-picker.jsx';

// Keep real bundled boundaries and GeoJSON conversion. Only the canvas/view
// host is replaced here; browser QA exercises OpenLayers' actual rendering.
const host = vi.hoisted(()=>({sources:[],map:null,picked:null,countries:[]}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({locale:'en',t:(text,vars={})=>text.replace(/\{(\w+)\}/g,(_,key)=>vars[key]??''),number:value=>String(value)})}));
vi.mock('ol/color.js',async load=>{const original=await load();return {...original,asArray:value=>original.asArray(value||'#2563eb')};});
vi.mock('ol/source/Vector.js',()=>({default:class {
  constructor(options={}){this.features=options.url?.includes('admin-0')?host.countries:[];this.url=options.url;host.sources.push(this);}
  on(event,handler){if(this.url&&event==='featuresloadend')queueMicrotask(handler);}
  getFeatures(){return this.features;}
  clear(){this.features=[];}
  addFeature(feature){this.features.push(feature);}
  addFeatures(features){this.features.push(...features);}
  changed(){}
}}));
vi.mock('ol/layer/Vector.js',()=>({default:class{constructor(options){Object.assign(this,options);}}}));
vi.mock('ol/Map.js',()=>({default:class {
  constructor(options){this.layers=options.layers;this.view=options.view;this.events={};host.map=this;}
  getView(){return this.view;}
  addInteraction(box){this.box=box;}
  on(event,handler){this.events[event]=handler;}
  forEachFeatureAtPixel(pixel,callback){return callback(host.picked.feature,this.layers[host.picked.layer]);}
  updateSize(){} getSize(){return [600,270];} setTarget(){} dispose(){}
}}));

const json=name=>JSON.parse(readFileSync(`prototype/public/basemaps/${name}`,'utf8'));
const index=json('admin1-10m/index.json');
const countries=json('natural-earth-50m-admin-0-countries.geojson');
const china=json('admin1-10m/CHN.geojson');
const germany=json('admin1-10m/DEU.geojson');
const beijing=index.areas.find(place=>place.code==='CHN-1155');
const original=(data,code)=>data.features.find(feature=>feature.properties.adm1_code===code);
const format=new GeoJSON();
const readFeature=feature=>format.readFeature(feature,{dataProjection:'EPSG:4326',featureProjection:'EPSG:4326'});
const response=data=>({ok:true,json:async()=>data});
let pending;
beforeEach(()=>{
  host.sources=[];host.picked=null;host.countries=countries.features.map(readFeature);pending=new Map();
  vi.stubGlobal('fetch',vi.fn(async url=>{
    if(url.endsWith('index.json'))return response(index);
    return new Promise((resolve,reject)=>pending.set(url.split('/').at(-1),{resolve,reject}));
  }));
});
afterEach(()=>vi.unstubAllGlobals());
const mount=()=>{
  const apply=vi.fn(),exportArea=vi.fn();
  render(<AreaPicker initialBbox={[73,18,135,54]} onApply={apply} onExport={exportArea} onClose={()=>{}}/>);
  return {apply,exportArea};
};
const choose=async(query,name)=>{
  await userEvent.type(screen.getByRole('textbox',{name:'Search administrative areas'}),query);
  await userEvent.click(await screen.findByRole('button',{name:new RegExp(`^${name}`)}));
};
const resolveBoundary=async(name,data)=>{
  await waitFor(()=>expect(pending.has(name)).toBe(true));
  await act(async()=>pending.get(name).resolve(response(data)));
};
const bounds=()=>['West','South','East','North'].map(name=>Number(screen.getByRole('spinbutton',{name}).value));

it('selects Beijing immediately, then automatically retains its real polygon in search and export',async()=>{
  const {apply,exportArea}=mount();await choose('Beijing','Beijing');
  expect(bounds()).toEqual(beijing.bounds);expect(apply).not.toHaveBeenCalled();
  expect(screen.getByRole('button',{name:'Search this area'}).disabled).toBe(true);
  await resolveBoundary('CHN.geojson',china);
  expect(await screen.findByText('The administrative polygon is selected. Search uses its bounding box; local SCL clipping uses the polygon.')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Search this area'}).disabled).toBe(false);
  const source=original(china,beijing.code);
  const expected={bounds:readFeature(source).getGeometry().getExtent(),geometry:source.geometry,place:{kind:'subdivision',code:beijing.code,name:'Beijing',source:'Natural Earth 1:10m'}};
  expect(bounds()).toEqual(expected.bounds);
  expect(format.writeGeometryObject(host.sources[3].getFeatures()[0].getGeometry())).toEqual(source.geometry);
  expect(screen.getByRole('button',{name:'Use region polygon'}).getAttribute('aria-pressed')).toBe('true');
  await userEvent.click(screen.getByRole('button',{name:'Download area GeoJSON'}));
  expect(exportArea).toHaveBeenCalledWith(expected);expect(apply).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));expect(apply).toHaveBeenCalledWith(expected);
});

it('a map region click selects the actual country polygon without another selection button',async()=>{
  const {apply}=mount();await screen.findByRole('button',{name:'Search this area'});
  const source=countries.features.find(feature=>feature.properties.ADM0_A3==='FRA');
  host.picked={feature:readFeature(source),layer:1};
  act(()=>host.map.events.singleclick({pixel:[10,10]}));
  expect(screen.getByRole('button',{name:'Use region polygon'}).getAttribute('aria-pressed')).toBe('true');
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply.mock.calls[0][0].geometry).toEqual(source.geometry);
  expect(apply.mock.calls[0][0].place.code).toBe('FRA');
});

it('late geometry does not overwrite manual coordinates or erase the next search text',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');
  fireEvent.change(screen.getByRole('spinbutton',{name:'West'}),{target:{value:'115.5'}});
  fireEvent.change(screen.getByRole('textbox',{name:'Search administrative areas'}),{target:{value:'Berlin'}});
  await resolveBoundary('CHN.geojson',china);
  expect(bounds()).toEqual([115.5,...beijing.bounds.slice(1)]);
  expect(screen.getByRole('textbox',{name:'Search administrative areas'}).value).toBe('Berlin');
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply).toHaveBeenCalledWith({bounds:[115.5,...beijing.bounds.slice(1)],geometry:null,place:null});
  expect(screen.getByRole('button',{name:'Use region polygon'}).disabled).toBe(false);
});

it('late geometry does not replace a newly drawn rectangle',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');
  await userEvent.click(screen.getByRole('button',{name:'Draw rectangle'}));
  const selected=[116,39.5,116.5,40];vi.spyOn(host.map.box,'getGeometry').mockReturnValue(fromExtent(selected));
  act(()=>host.map.box.dispatchEvent('boxend'));
  await resolveBoundary('CHN.geojson',china);
  expect(bounds()).toEqual(selected);
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply).toHaveBeenCalledWith({bounds:selected,geometry:null,place:null});
});

it('the latest subdivision wins when two selections share a pending geometry request',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');await choose('Guangdong','Guangdong');
  await resolveBoundary('CHN.geojson',china);
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply.mock.calls[0][0].place.code).toBe('CHN-1180');
  expect(apply.mock.calls[0][0].geometry).toEqual(original(china,'CHN-1180').geometry);
});

it('a cancelled older country response cannot replace the newly selected subdivision',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');await choose('Bavaria','Bavaria');
  await resolveBoundary('DEU.geojson',germany);await resolveBoundary('CHN.geojson',china);
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply.mock.calls[0][0].place.code).toBe('DEU-1591');
  expect(apply.mock.calls[0][0].geometry).toEqual(original(germany,'DEU-1591').geometry);
});

it('an explicit rectangle choice remains selected when the polygon arrives, and can be switched back',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');
  await userEvent.click(screen.getByRole('button',{name:'Use bounding rectangle'}));
  expect(screen.getByRole('button',{name:'Search this area'}).disabled).toBe(false);
  await resolveBoundary('CHN.geojson',china);
  expect(screen.getByRole('button',{name:'Use bounding rectangle'}).getAttribute('aria-pressed')).toBe('true');
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));expect(apply.mock.calls[0][0].geometry).toBeNull();
  await userEvent.click(screen.getByRole('button',{name:'Use region polygon'}));
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply.mock.calls[1][0].geometry).toEqual(original(china,beijing.code).geometry);
});

it('a boundary load failure retains the newly selected rectangle instead of the previous AOI',async()=>{
  const {apply}=mount();await choose('Beijing','Beijing');
  await waitFor(()=>expect(pending.has('CHN.geojson')).toBe(true));
  await act(async()=>pending.get('CHN.geojson').reject(new Error('Boundary load failed')));
  expect(await screen.findByRole('alert')).toBeTruthy();expect(bounds()).toEqual(beijing.bounds);
  await userEvent.click(screen.getByRole('button',{name:'Search this area'}));
  expect(apply).toHaveBeenCalledWith({bounds:beijing.bounds,geometry:null,place:null});
});

it('a subdivision crossing the date line clears the previous AOI and explains the unsupported selection',async()=>{
  mount();await choose('Alaska','Alaska');
  expect(screen.getByText('This region crosses the date line. Draw separate rectangles on either side for catalogue search; local polygon clipping is unavailable.')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Search this area'}).disabled).toBe(true);
  expect(['West','South','East','North'].map(name=>screen.getByRole('spinbutton',{name}).value)).toEqual(['','','','']);
});
