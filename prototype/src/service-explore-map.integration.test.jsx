import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {act,render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {transformExtent} from 'ol/proj.js';
import {ServiceExploreMap} from './service-explore-map.jsx';
import {parseWaybackCatalog,WAYBACK_ATTRIBUTION} from './wayback-client.js';

const maps=vi.hoisted(()=>({instances:[]}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??'')})}));
vi.mock('ol/Map.js',()=>({default:class {
  constructor(options){this.layers=[...options.layers];this.handlers={};this.view={fit:vi.fn(),getZoom:()=>4,animate:vi.fn()};this.addLayer=vi.fn(layer=>this.layers.push(layer));this.removeLayer=vi.fn(layer=>{this.layers=this.layers.filter(value=>value!==layer);});maps.instances.push(this);}
  getView(){return this.view;}
  on(name,callback){this.handlers[name]=callback;}
  updateSize(){}
  setTarget(){}
  forEachFeatureAtPixel(pixel,callback){for(const layer of this.layers){const features=layer.getSource()?.getFeatures?.()||[];for(const feature of features){if(callback(feature,layer))return;}}}
}}));

const bounds=[116,39,116.1,39.1];
const imageEntry=(key,title=key)=>({key,title,kind:'map',visible:true,opacity:1,data:{asset:{id:key,bounds,crs:'EPSG:4326',source:{serviceTitle:'Public maps'}},imageUrl:'data:image/png;base64,AA=='}});
const vectorEntry=(key,name='Original feature')=>({key,title:key,kind:'vector',visible:true,opacity:1,data:{asset:{id:key,featureCount:1},geojson:{type:'FeatureCollection',features:[{type:'Feature',id:'source-id',properties:{name},geometry:{type:'Point',coordinates:[116,39]}}]}}});
const tileEntry=key=>({key,title:key,kind:'tiles',visible:true,opacity:1,layerName:'roads',data:{asset:{id:key,minZoom:0,maxZoom:2,source:{name:'Local tile package'}}}});
const boundaryEntry=(key,extent)=>({key,title:key,kind:'vector',visible:true,opacity:1,data:{asset:{id:key,featureCount:1,bounds:[-180,-90,180,90]},geojson:{type:'FeatureCollection',features:[{type:'Feature',properties:{shapeName:key},geometry:{type:'Polygon',coordinates:[[[extent[0],extent[1]],[extent[2],extent[1]],[extent[2],extent[3]],[extent[0],extent[3]],[extent[0],extent[1]]]]}}]}}});
beforeEach(()=>{maps.instances.length=0;});

const waybackVersion=parseWaybackCatalog({63116:{itemID:'ab83ada1bd3145978eecfb115aaa10c3',itemTitle:'World Imagery (Wayback 2026-09-24)',itemURL:'https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/63116/{level}/{row}/{col}',metadataLayerUrl:'https://metadata.maptiles.arcgis.com/arcgis/rest/services/World_Imagery_Metadata_2026_r09/MapServer',metadataLayerItemID:'f2696faf5a3643fe9f9ee097b09caa55',layerIdentifier:'WB_2026_R09'}})[0];

it('Wayback uses the publisher version as an online XYZ layer with visible attribution',async()=>{
  render(<ServiceExploreMap entries={[{key:waybackVersion.id,title:waybackVersion.title,kind:'wayback',data:{version:waybackVersion}}]} areaBounds={bounds}/>);
  const map=maps.instances[0],source=map.layers.find(layer=>layer.get('exploreTitle')).getSource();
  expect(source.getUrls()).toEqual([waybackVersion.tileTemplate]);expect(screen.getByText(WAYBACK_ATTRIBUTION)).toBeTruthy();
  act(()=>source.dispatchEvent('tileloadstart'));expect(screen.getByRole('status')).toBeTruthy();act(()=>source.dispatchEvent('tileloadend'));expect(screen.queryByRole('status')).toBeNull();
  act(()=>source.dispatchEvent('tileloaderror'));expect((await screen.findByRole('alert')).textContent).toContain('Wayback online preview unavailable');
});

it('Wayback map clicks report the current display point and zoom for the visible version',()=>{
  const inspect=vi.fn();render(<ServiceExploreMap entries={[{key:waybackVersion.id,title:waybackVersion.title,kind:'wayback',data:{version:waybackVersion}}]} areaBounds={bounds} onInspectWaybackPoint={inspect}/>);
  const map=maps.instances[0];map.handlers.singleclick({pixel:[0,0],coordinate:[0,0]});
  expect(inspect).toHaveBeenCalledWith([0,0],4,[waybackVersion]);
});

it('Wayback refuses a modified tile template instead of requesting another source',async()=>{
  render(<ServiceExploreMap entries={[{key:'changed',title:'Changed',kind:'wayback',data:{version:{...waybackVersion,tileTemplate:'https://example.com/{z}/{y}/{x}'}}}]} areaBounds={bounds}/>);
  expect(maps.instances[0].layers.filter(layer=>layer.get('exploreTitle'))).toHaveLength(0);expect((await screen.findByRole('alert')).textContent).toContain('Wayback preview version identity changed');
});

it('attaches distinct georeferenced service image sources and exposes actual image events',async()=>{
  const entries=[imageEntry('first'),imageEntry('second')];render(<ServiceExploreMap entries={entries} areaBounds={bounds}/>);
  const map=maps.instances[0],layers=map.layers.filter(layer=>layer.get('exploreTitle'));
  expect(layers).toHaveLength(2);expect(layers[0].getSource()).not.toBe(layers[1].getSource());
  expect(layers[0].getSource().getProjection().getCode()).toBe('EPSG:4326');
  expect(layers[0].getSource().getImageExtent()).toEqual(bounds);
  expect(screen.getAllByRole('status')).toHaveLength(2);
  layers[0].getSource().dispatchEvent('imageloadend');
  await waitFor(()=>expect(screen.getAllByRole('status')).toHaveLength(1));
  layers[1].getSource().dispatchEvent('imageloaderror');
  expect((await screen.findByRole('alert')).textContent).toContain('Map preview unavailable');
});

it('a layer retry replaces only its failed source and ignores late events from the removed source',async()=>{
  const entries=[imageEntry('first'),imageEntry('second')];const user=userEvent.setup();render(<ServiceExploreMap entries={entries} areaBounds={bounds}/>);
  const map=maps.instances[0],original=map.layers.filter(layer=>layer.get('exploreTitle')),first=original[0],second=original[1];
  first.getSource().dispatchEvent('imageloaderror');await user.click(await screen.findByRole('button',{name:'Retry layer first'}));
  const current=map.layers.filter(layer=>layer.get('exploreTitle'));
  expect(current).toHaveLength(2);expect(current).toContain(second);expect(current).not.toContain(first);
  expect(map.removeLayer).toHaveBeenCalledWith(first);
  first.getSource().dispatchEvent('imageloaderror');
  expect(screen.queryByRole('alert')).toBeNull();
});

it('changes source data for an existing key without reusing the old image or recreating unrelated layers',()=>{
  const first=imageEntry('first'),second=imageEntry('second');const view=render(<ServiceExploreMap entries={[first,second]} areaBounds={bounds}/>);
  const map=maps.instances[0],old=map.layers.filter(layer=>layer.get('exploreTitle'));
  const changed={...first,data:{...first.data,imageUrl:'data:image/png;base64,BB=='}};
  view.rerender(<ServiceExploreMap entries={[changed,second]} areaBounds={bounds}/>);
  const next=map.layers.filter(layer=>layer.get('exploreTitle'));
  expect(next).toContain(old[1]);expect(next).not.toContain(old[0]);expect(map.removeLayer).toHaveBeenCalledWith(old[0]);
});

it('applies independent visibility and opacity without changing the underlying image source',()=>{
  const entry=imageEntry('first');const view=render(<ServiceExploreMap entries={[entry]} areaBounds={bounds}/>);
  const map=maps.instances[0],layer=map.layers.find(value=>value.get('exploreTitle'));const source=layer.getSource();
  view.rerender(<ServiceExploreMap entries={[{...entry,visible:false,opacity:.35}]} areaBounds={bounds}/>);
  expect(layer.getVisible()).toBe(false);expect(layer.getOpacity()).toBe(.35);expect(layer.getSource()).toBe(source);
});

it('clears every preview via owner callbacks and supports restoring the hidden result list',async()=>{
  const remove=vi.fn(),show=vi.fn(),user=userEvent.setup();render(<ServiceExploreMap entries={[imageEntry('first'),imageEntry('second')]} areaBounds={bounds} onRemove={remove} listHidden onShowList={show}/>);
  await user.click(screen.getByRole('button',{name:'Show results'}));expect(show).toHaveBeenCalledOnce();
  await user.click(screen.getByRole('button',{name:'Clear map previews'}));expect(remove.mock.calls).toEqual([['first'],['second']]);
});

it('transforms verified vector coordinates for display while retaining original property values',async()=>{
  const data={asset:{id:'vector',featureCount:1},geojson:{type:'FeatureCollection',features:[{type:'Feature',id:'source-id',properties:{geometry:'original attribute',name:'Sample'},geometry:{type:'Point',coordinates:[116,39]}}]}};
  render(<ServiceExploreMap entries={[{key:'vector',title:'Vector',kind:'vector',data}]} areaBounds={bounds}/>);
  const map=maps.instances[0],source=map.layers.find(layer=>layer.get('exploreTitle')).getSource();
  expect(source.getFeatures()[0].getGeometry().getCoordinates()[0]).toBeGreaterThan(12000000);
  map.handlers.singleclick({pixel:[0,0]});
  expect(await screen.findByText('original attribute')).toBeTruthy();expect(screen.getByText('source-id')).toBeTruthy();
  expect(data.geojson.features[0].geometry.coordinates).toEqual([116,39]);
});

it('ignores a late PMTiles load error from the source replaced by retry',async()=>{
  const entry=tileEntry('tiles'),user=userEvent.setup();render(<ServiceExploreMap entries={[entry]} areaBounds={bounds}/>);
  const map=maps.instances[0],original=map.layers.find(layer=>layer.get('exploreTitle')).getSource();
  act(()=>original.dispatchEvent('tileloaderror'));
  expect((await screen.findByRole('alert')).textContent).toContain('Tile preview unavailable');
  await user.click(screen.getByRole('button',{name:'Retry layer tiles'}));
  const current=map.layers.find(layer=>layer.get('exploreTitle')).getSource();
  expect(current).not.toBe(original);expect(screen.queryByRole('alert')).toBeNull();
  act(()=>original.dispatchEvent('tileloaderror'));
  expect(screen.queryByRole('alert')).toBeNull();
  act(()=>current.dispatchEvent('tileloaderror'));
  expect(screen.getByRole('alert').textContent).toContain('Tile preview unavailable');
});

it('removing the inspected vector layer closes its old property card',async()=>{
  const entry=vectorEntry('vector'),other=imageEntry('other');const view=render(<ServiceExploreMap entries={[entry,other]} areaBounds={bounds}/>);
  const map=maps.instances[0];act(()=>map.handlers.singleclick({pixel:[0,0]}));
  expect(await screen.findByText('Original feature')).toBeTruthy();
  view.rerender(<ServiceExploreMap entries={[other]} areaBounds={bounds}/>);
  expect(screen.queryByText('Original feature')).toBeNull();
});

it('replacing the inspected vector data closes old attributes without closing unchanged layer details',async()=>{
  const entry=vectorEntry('vector');const view=render(<ServiceExploreMap entries={[entry]} areaBounds={bounds}/>);
  const map=maps.instances[0];act(()=>map.handlers.singleclick({pixel:[0,0]}));
  expect(await screen.findByText('Original feature')).toBeTruthy();
  view.rerender(<ServiceExploreMap entries={[{...entry,opacity:.5}]} areaBounds={bounds}/>);
  expect(screen.getByText('Original feature')).toBeTruthy();
  const changed={...entry,data:vectorEntry('vector','Replacement feature').data};
  view.rerender(<ServiceExploreMap entries={[changed]} areaBounds={bounds}/>);
  expect(screen.queryByText('Original feature')).toBeNull();
  act(()=>map.handlers.singleclick({pixel:[0,0]}));
  expect(await screen.findByText('Replacement feature')).toBeTruthy();
});

it('fits first loaded country boundaries from actual vector geometry when no search area exists',()=>{
  const countryBounds=[73,18,135,54],country=boundaryEntry('Synthetic country boundaries',countryBounds);
  const original=JSON.parse(JSON.stringify(country.data.geojson));
  const view=render(<ServiceExploreMap entries={[]}/>),map=maps.instances[0];
  expect(map.getView().fit).not.toHaveBeenCalled();
  view.rerender(<ServiceExploreMap entries={[country]}/>);
  expect(map.getView().fit).toHaveBeenCalledOnce();
  expect(map.getView().fit.mock.calls[0][0]).toEqual(transformExtent(countryBounds,'EPSG:4326','EPSG:3857'));
  expect(screen.getByRole('button',{name:'Fit loaded layers'})).toBeTruthy();
  expect(country.data.geojson).toEqual(original);
});

it('preserves a real selected AOI instead of refitting the complete country boundaries',async()=>{
  const country=boundaryEntry('Synthetic country boundaries',[73,18,135,54]),user=userEvent.setup();
  const view=render(<ServiceExploreMap entries={[]} areaBounds={bounds}/>),map=maps.instances[0];
  view.rerender(<ServiceExploreMap entries={[country]} areaBounds={bounds}/>);
  expect(map.getView().fit).toHaveBeenCalledOnce();
  expect(map.getView().fit.mock.calls[0][0]).toEqual(transformExtent(bounds,'EPSG:4326','EPSG:3857'));
  await user.click(screen.getByRole('button',{name:'Fit search area'}));
  expect(map.getView().fit).toHaveBeenCalledTimes(2);
  expect(map.getView().fit.mock.calls[1][0]).toEqual(transformExtent(bounds,'EPSG:4326','EPSG:3857'));
});

it('does not steal the view on visibility, opacity or appended layers and manually fits only visible data',async()=>{
  const first=boundaryEntry('First',[73,18,135,54]),second=boundaryEntry('Second',[2,48,4,50]),user=userEvent.setup();
  const view=render(<ServiceExploreMap entries={[first]}/>),map=maps.instances[0];
  expect(map.getView().fit).toHaveBeenCalledOnce();map.getView().fit.mockClear();
  view.rerender(<ServiceExploreMap entries={[{...first,opacity:.35}]}/>);
  view.rerender(<ServiceExploreMap entries={[{...first,opacity:.35,visible:false}]}/>);
  view.rerender(<ServiceExploreMap entries={[{...first,opacity:.35,visible:false},second]}/>);
  expect(map.getView().fit).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Fit loaded layers'}));
  expect(map.getView().fit).toHaveBeenCalledOnce();
  expect(map.getView().fit.mock.calls[0][0]).toEqual(transformExtent([2,48,4,50],'EPSG:4326','EPSG:3857'));
  map.getView().fit.mockClear();
  view.rerender(<ServiceExploreMap entries={[first,second]}/>);
  expect(map.getView().fit).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Fit loaded layers'}));
  expect(map.getView().fit.mock.calls[0][0]).toEqual(transformExtent([2,18,135,54],'EPSG:4326','EPSG:3857'));
});

it('re-fits newly loaded data after clearing every preview',()=>{
  const first=boundaryEntry('First',[73,18,135,54]),second=boundaryEntry('Second',[2,48,4,50]);
  const view=render(<ServiceExploreMap entries={[first]}/>),map=maps.instances[0];
  map.getView().fit.mockClear();
  view.rerender(<ServiceExploreMap entries={[]}/>);
  view.rerender(<ServiceExploreMap entries={[second]}/>);
  expect(map.getView().fit).toHaveBeenCalledOnce();
  expect(map.getView().fit.mock.calls[0][0]).toEqual(transformExtent([2,48,4,50],'EPSG:4326','EPSG:3857'));
});

it('fits the native georeferenced image grid in the map projection',()=>{
  const grid=transformExtent(bounds,'EPSG:4326','EPSG:3857');
  const entry=imageEntry('Projected image');entry.data.asset={...entry.data.asset,crs:'EPSG:3857',imageExtent:grid};
  render(<ServiceExploreMap entries={[entry]}/>);
  expect(maps.instances[0].getView().fit).toHaveBeenCalledOnce();
  expect(maps.instances[0].getView().fit.mock.calls[0][0]).toEqual(grid);
});

it('fits actual extracted tile coverage rather than archive-wide metadata bounds',()=>{
  const coverage=[112,36,118,42],entry=tileEntry('Extracted tiles');
  entry.data.asset={...entry.data.asset,tileCoverageBounds:coverage,requestedBounds:bounds,source:{...entry.data.asset.source,header:{bounds:[-180,-85,180,85]}}};
  render(<ServiceExploreMap entries={[entry]}/>);
  expect(maps.instances[0].getView().fit).toHaveBeenCalledOnce();
  expect(maps.instances[0].getView().fit.mock.calls[0][0]).toEqual(transformExtent(coverage,'EPSG:4326','EPSG:3857'));
});

it('does not invent an extent for empty geometry or missing tile coverage',async()=>{
  const empty={...vectorEntry('Empty'),data:{asset:{id:'Empty',featureCount:0,bounds:[-180,-90,180,90]},geojson:{type:'FeatureCollection',features:[]}}};
  const user=userEvent.setup();render(<ServiceExploreMap entries={[empty,tileEntry('Missing coverage')]}/>);
  const map=maps.instances[0];expect(map.getView().fit).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Fit loaded layers'}));
  expect(map.getView().fit).not.toHaveBeenCalled();
});
