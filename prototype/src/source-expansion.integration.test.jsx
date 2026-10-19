import React from 'react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {act,cleanup,render,screen,waitFor,within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {FeatureServiceExplorer} from './service-explore-ui.jsx';
import {featureRequest} from './features-client.js';
import {vectorRequest} from './vector-client.js';

// Synthetic boundary fixtures exercise UI state and request ownership only.
// They are deliberately labelled QA and establish no live download evidence.
const probe=vi.hoisted(()=>({map:null}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),number:String,locale:'en-US'})}));
vi.mock('./features-client.js',async original=>({...await original(),featureRequest:vi.fn()}));
vi.mock('./vector-client.js',async original=>({...await original(),vectorRequest:vi.fn(),exportVector:vi.fn()}));
vi.mock('./explore-source-workspace.jsx',()=>({ExploreSourceFrame:({children,actions,map})=><main><aside>{children}{actions}</aside><section>{map}</section></main>}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:props=>{
  probe.map=props;
  return <div><output data-testid="loaded-boundaries">{props.entries.map(entry=>`${entry.title}:${entry.data.asset.id}`).join('|')}</output></div>;
}}));

const sourceId='735f227b-5f95-473f-967b-077f3419bc68';
const bounds=[116,39,116.1,39.1];
const polygon={type:'Polygon',coordinates:[[[116,39],[116.1,39],[116.1,39.1],[116,39]]]};
const world=[-180,-90,180,90];
const originalGeometry={type:'Polygon',coordinates:[[[100,20],[130,20],[130,50],[100,50],[100,20]]]};
let registry,inspections,sequence;
function syntheticSource(country='CHN',release='a'.repeat(40)) {
  const url=`https://www.geoboundaries.org/api/current/gbOpen/${country}/ALL/`;
  return {id:sourceId,name:`QA geoBoundaries · ${country}`,url,title:`QA synthetic national boundaries · ${country}`,connectedAt:'2026-10-13T00:00:00Z',geoboundaries:{releaseType:'gbOpen',countryIso3:country,discoveryReceipt:{url,sha256:'b'.repeat(64),bytes:1000,returned:2}},collections:['ADM0','ADM1'].map((level,index)=>({
    id:level,title:`QA ${country} · ${level}`,description:'Synthetic complete administrative level for UI regression.',itemsUrl:`https://www.geoboundaries.org/api/current/gbOpen/${country}/${level}/`,licenseLinks:['https://creativecommons.org/licenses/by/4.0/'],geoboundaries:{countryIso3:country,countryName:`QA country ${country}`,adminLevel:level,yearRepresented:'2018',originalSource:'QA original survey office',originalLicense:'QA original CC BY 4.0',unitCount:index+1,releaseRef:release,downloadUrl:`https://raw.githubusercontent.com/wmgeolab/geoBoundaries/${release}/releaseData/gbOpen/${country}/${level}/geoBoundaries-${country}-${level}.geojson`,metadata:{syntheticFixture:true,publication:'QA metadata retained'},metadataSha256:'c'.repeat(64)}
  }))};
}
function queryAsset(payload,selected=registry.find(source=>source.id===payload.serviceId)) {
  const collection=selected.collections.find(item=>item.id===payload.collectionId),id=`qa-vector-${++sequence}`;
  const remoteSource={serviceUrl:selected.url,requestedBounds:payload.bounds,areaGeometry:payload.areaGeometry,selection:'geoboundaries-full-administrative-level',featureCount:collection.geoboundaries.unitCount,numberMatched:collection.geoboundaries.unitCount,geoboundaries:{layer:structuredClone(collection.geoboundaries),geometrySelection:'geoboundaries-full-administrative-level'}};
  const asset={id,featureCount:collection.geoboundaries.unitCount,remoteSource};
  inspections.set(id,{asset,geojson:{type:'FeatureCollection',features:Array.from({length:asset.featureCount},(_,index)=>({type:'Feature',properties:{shapeID:`qa-${index}`,shapeGroup:collection.geoboundaries.countryIso3,shapeType:collection.id,shapeName:`QA unit ${index}`},geometry:structuredClone(originalGeometry)})),geodSource:structuredClone(remoteSource)}});
  return asset;
}
beforeEach(()=>{
  vi.clearAllMocks();probe.map=null;registry=[syntheticSource()];inspections=new Map();sequence=0;
  featureRequest.mockImplementation(async(operation,payload)=>operation==='list'?structuredClone(registry):operation==='connect'?structuredClone(registry[0]):queryAsset(payload));
  vectorRequest.mockImplementation(async(operation,payload)=>structuredClone(inspections.get(payload.id)));
});
afterEach(cleanup);
const mount=props=>render(<FeatureServiceExplorer initialProtocol="geoBoundaries" areaBounds={bounds} {...props}/>);
const checked=level=>screen.getByRole('checkbox',{name:`Select result QA CHN · ${level}`});
async function chooseLevel(user,level='ADM0') {await user.click(await screen.findByRole('checkbox',{name:`Select result QA CHN · ${level}`}));}
async function load(user,count=1,pending=false) {
  expect(screen.getByRole('region',{name:'Selected product actions'}).textContent).toContain(`${count} selected`);
  const before=featureRequest.mock.calls.filter(([operation])=>operation==='query').length;
  await user.click(screen.getByRole('button',{name:'Save and download'}));
  expect(featureRequest.mock.calls.filter(([operation])=>operation==='query')).toHaveLength(before);
  await user.click(screen.getByRole('button',{name:'Confirm and download'}));
  if(!pending){await screen.findByRole('dialog',{name:'Confirm download'});await waitFor(()=>expect(screen.getByRole('dialog',{name:'Confirm download'}).textContent).toContain('Results saved locally'));await user.keyboard('{Escape}');}
}

it('country connection defaults to CHN and blocks ALL, short codes and empty codes before requesting discovery',async()=>{
  registry=[];const user=userEvent.setup();mount();
  const country=await screen.findByRole('textbox',{name:'Boundary country code'});
  expect(country.value).toBe('CHN');expect(screen.queryByRole('textbox',{name:'Service URL'})).toBeNull();
  for(const code of ['ALL','de','']) {
    await user.clear(country);if(code)await user.type(country,code);
    expect(screen.getByRole('button',{name:'Connect service'}).disabled).toBe(true);
    await user.click(screen.getByRole('button',{name:'Connect service'}));
  }
  expect(featureRequest.mock.calls.map(([operation])=>operation)).toEqual(['list']);
  await user.type(country,'deu');expect(country.value).toBe('DEU');registry=[syntheticSource('DEU')];
  await user.click(screen.getByRole('button',{name:'Connect service'}));
  await screen.findByRole('checkbox',{name:'Select result QA DEU · ADM0'});
  expect(featureRequest.mock.calls.filter(([operation])=>operation==='connect')[0][1]).toEqual({name:'geoBoundaries · DEU',url:'https://www.geoboundaries.org/api/current/gbOpen/DEU/ALL/',protocol:'geoBoundaries'});
});

it('saved boundary results retain source year, original source, license and fixed release separately from the map area',async()=>{
  const user=userEvent.setup();mount();await screen.findByRole('checkbox',{name:'Select result QA CHN · ADM0'});
  const result=checked('ADM0').closest('article');
  expect(result.textContent).toContain('2018');expect(result.textContent).toContain('1 administrative units');
  await user.click(within(result).getByRole('button',{name:'Boundary source and license'}));
  const details=screen.getByRole('dialog',{name:'Boundary source and license'});
  expect(result.querySelector('.bui-disclosure')).toBeNull();
  expect(details.textContent).toContain('Complete country boundary file');
  expect(details.textContent).toContain('QA original survey office');expect(details.textContent).toContain('QA original CC BY 4.0');expect(details.textContent).toContain('a'.repeat(40));expect(details.textContent).toContain('https://creativecommons.org/licenses/by/4.0/');
  await user.keyboard('{Escape}');
  await user.click(screen.getByRole('button',{name:'Preview details'}));
  expect(screen.getByText('Loading saves the complete published country administrative level. The map area is reference only; boundaries are not clipped or filtered.')).toBeTruthy();
  expect(featureRequest.mock.calls.every(([operation])=>operation==='list')).toBe(true);
});

it('selects two ADM levels without side effects and explicitly loads both complete national geometries',async()=>{
  const user=userEvent.setup();mount({areaGeometry:polygon});
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));
  expect(featureRequest.mock.calls.every(([operation])=>operation==='list')).toBe(true);expect(vectorRequest).not.toHaveBeenCalled();
  await load(user,2);await waitFor(()=>expect(probe.map.entries).toHaveLength(2));
  expect(featureRequest.mock.calls.filter(([operation])=>operation==='query').map(([,payload])=>payload)).toEqual(['ADM0','ADM1'].map(collectionId=>({serviceId:sourceId,collectionId,bounds,areaGeometry:polygon})));
  expect(probe.map.areaBounds).toEqual(bounds);expect(probe.map.areaGeometry).toEqual(polygon);
  for(const entry of probe.map.entries) {
    expect(entry.data.asset.remoteSource.selection).toBe('geoboundaries-full-administrative-level');
    expect(entry.data.asset.remoteSource.geoboundaries.layer.metadata).toEqual({syntheticFixture:true,publication:'QA metadata retained'});
    expect(entry.data.geojson.features.every(feature=>JSON.stringify(feature.geometry)===JSON.stringify(originalGeometry))).toBe(true);
    expect(entry.data.geojson.features[0].geometry.coordinates[0][0][0]).toBeLessThan(bounds[0]);
  }
  expect(vectorRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(2);
});

it('can load an explicitly selected national level without an AOI using world bounds only as reference',async()=>{
  const user=userEvent.setup();mount({areaBounds:null,areaGeometry:null});await chooseLevel(user);
  expect(screen.getByRole('button',{name:'Save and download'}).disabled).toBe(false);
  await load(user);await waitFor(()=>expect(probe.map.entries).toHaveLength(1));
  expect(featureRequest.mock.calls.find(([operation])=>operation==='query')[1]).toEqual({serviceId:sourceId,collectionId:'ADM0',bounds:world,areaGeometry:null});
  expect(probe.map.entries[0].data.geojson.features[0].geometry).toEqual(originalGeometry);
});

it('the geoBoundaries entry excludes unrelated OGC, WFS, ArcGIS and Overpass registry connections',async()=>{
  registry=[{id:'ogc',name:'QA unrelated OGC',url:'https://features.example/api',collections:[{id:'river',title:'QA river'}]},{id:'wfs',name:'QA unrelated WFS',wfs:{},collections:[]},{id:'arcgis',name:'QA unrelated ArcGIS',arcgis:{},collections:[]},{id:'osm',name:'QA unrelated OSM',overpass:{},collections:[]},syntheticSource()];
  const user=userEvent.setup();mount();const source=await screen.findByRole('combobox',{name:'Connected source'});
  expect(source.textContent).toBe('QA geoBoundaries · CHN');await user.click(source);
  expect(screen.getByRole('option',{name:'QA geoBoundaries · CHN'})).toBeTruthy();
  for(const name of ['QA unrelated OGC','QA unrelated WFS','QA unrelated ArcGIS','QA unrelated OSM'])expect(screen.queryByRole('option',{name})).toBeNull();
  expect(screen.queryByRole('checkbox',{name:'Select result QA river'})).toBeNull();
});

it('same-ID metadata refresh clears old map, selected levels and saved result cache while preserving the AOI',async()=>{
  const user=userEvent.setup();mount({areaGeometry:polygon});await chooseLevel(user);await load(user);await waitFor(()=>expect(probe.map.entries).toHaveLength(1));
  expect(screen.getByRole('button',{name:'Export result QA CHN · ADM0'})).toBeTruthy();
  registry=[syntheticSource('CHN','d'.repeat(40))];await user.click(screen.getByRole('button',{name:'Refresh source list'}));
  await screen.findByRole('checkbox',{name:'Select result QA CHN · ADM0'});
  await waitFor(()=>expect(probe.map.entries).toEqual([]));
  expect(checked('ADM0').getAttribute('aria-checked')).toBe('false');expect(screen.queryByRole('button',{name:'Export result QA CHN · ADM0'})).toBeNull();
  expect(probe.map.areaBounds).toEqual(bounds);expect(probe.map.areaGeometry).toEqual(polygon);
  await chooseLevel(user);await load(user);await waitFor(()=>expect(probe.map.entries).toHaveLength(1));
  expect(featureRequest.mock.calls.filter(([operation])=>operation==='query')).toHaveLength(2);
  expect(probe.map.entries[0].data.asset.remoteSource.geoboundaries.layer.releaseRef).toBe('d'.repeat(40));
  expect(probe.map.entries[0].data.asset.id).toBe('qa-vector-2');
});

it('reconnecting the same URL and ID with a new release invalidates the earlier selected and saved result',async()=>{
  const user=userEvent.setup();mount();await chooseLevel(user);await load(user);await waitFor(()=>expect(probe.map.entries).toHaveLength(1));
  await user.click(screen.getByRole('button',{name:'Add source connection'}));registry=[syntheticSource('CHN','e'.repeat(40))];
  await user.click(screen.getByRole('button',{name:'Connect service'}));
  await screen.findByRole('checkbox',{name:'Select result QA CHN · ADM0'});await waitFor(()=>expect(probe.map.entries).toEqual([]));
  expect(checked('ADM0').getAttribute('aria-checked')).toBe('false');expect(screen.queryByRole('button',{name:'Export result QA CHN · ADM0'})).toBeNull();
  await chooseLevel(user);await load(user);await waitFor(()=>expect(probe.map.entries).toHaveLength(1));
  expect(featureRequest.mock.calls.filter(([operation])=>operation==='query')).toHaveLength(2);
  expect(probe.map.entries[0].data.asset.remoteSource.geoboundaries.layer.releaseRef).toBe('e'.repeat(40));
});

it('same-ID metadata replacement aborts an in-flight original query and never inspects its late reply',async()=>{
  let resolveQuery,querySignal;
  const original=featureRequest.getMockImplementation();
  featureRequest.mockImplementation((operation,payload,signal)=>operation==='query'?(querySignal=signal,new Promise(resolve=>{resolveQuery=resolve;})):original(operation,payload,signal));
  const user=userEvent.setup(),view=mount({initialPreset:'qa-registry-generation-one'});
  await chooseLevel(user);await load(user,1,true);await waitFor(()=>expect(resolveQuery).toBeTypeOf('function'));
  registry=[syntheticSource('CHN','f'.repeat(40))];
  // The declared registry-refresh dependency triggers a new read. Country,
  // URL, source ID and AOI all remain unchanged, so only metadata owns reset.
  view.rerender(<FeatureServiceExplorer initialProtocol="geoBoundaries" initialPreset="qa-registry-generation-two" areaBounds={bounds}/>);
  await screen.findByRole('checkbox',{name:'Select result QA CHN · ADM0'});await waitFor(()=>expect(querySignal.aborted).toBe(true));
  await act(async()=>{resolveQuery({id:'late-old-country-file'});await Promise.resolve();});
  expect(probe.map.entries).toEqual([]);expect(checked('ADM0').getAttribute('aria-checked')).toBe('false');expect(vectorRequest).not.toHaveBeenCalled();
});
