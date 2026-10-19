
async function acquire(user,{wait=true}={}){
  await user.click(screen.getByRole('button',{name:'Save and download'}));
  expect(screen.getByRole('dialog',{name:'Confirm download'})).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Confirm and download'}));
  if(wait){
    await waitFor(()=>expect(screen.getAllByRole('button',{name:'Close'})[0].disabled).toBe(false));
    await user.click(screen.getAllByRole('button',{name:'Close'})[0]);
    await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Confirm download'})).toBeNull());
  }
}
import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {MapServiceExplorer,FeatureServiceExplorer,TileSourceExplorer,exploreMapImageRequest} from './service-explore-ui.jsx';
import {mapRequest} from './wms-client.js';
import {featureRequest} from './features-client.js';
import {vectorRequest} from './vector-client.js';
import {tileRequest} from './tiles-client.js';

vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),number:String,locale:'en-US'})}));
vi.mock('./wms-client.js',async load=>({...await load(),mapRequest:vi.fn(),exportMapImage:vi.fn()}));
vi.mock('./features-client.js',async load=>({...await load(),featureRequest:vi.fn()}));
vi.mock('./vector-client.js',async load=>({...await load(),vectorRequest:vi.fn(),exportVector:vi.fn()}));
vi.mock('./tiles-client.js',async load=>({...await load(),tileRequest:vi.fn(),exportTilePackage:vi.fn()}));
vi.mock('./explore-source-workspace.jsx',()=>({ExploreSourceFrame:({children,actions,map})=><main><aside>{children}{actions}</aside><section>{map}</section></main>}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:({entries,onRemove,onPatch})=><div><output data-testid="loaded-layers">{entries.map(entry=>`${entry.kind}:${entry.title}:${entry.data.asset.id}`).join('|')}</output>{entries.map(entry=><div key={entry.key}><button onClick={()=>onRemove(entry.key)}>Remove {entry.title}</button><button onClick={()=>onPatch(entry.key,{visible:false})}>Hide {entry.title}</button></div>)}</div>}));

const bounds=[116,39,116.1,39.1];
const mapSource={id:'maps',name:'Test map service',url:'https://maps.example/wms',layers:[{name:'first',title:'First map',styles:[],time:null},{name:'second',title:'Second map',styles:[],time:null}],maxWidth:2048,maxHeight:2048};
const featureSource={id:'features',name:'Test feature service',url:'https://features.example',collections:[{id:'first',title:'First vector'},{id:'second',title:'Second vector'}]};
beforeEach(()=>{
  vi.clearAllMocks();
  mapRequest.mockImplementation(async(operation,payload)=>operation==='services'?[mapSource]:operation==='get'?{id:`map-${payload.layerName}`}:{asset:{id:payload.id,bounds,crs:'EPSG:4326',source:{}},imageUrl:'data:image/png;base64,AA=='});
  featureRequest.mockImplementation(async(operation,payload)=>operation==='list'?[featureSource]:{id:`vector-${payload.collectionId}`});
  vectorRequest.mockImplementation(async(operation,payload)=>({asset:{id:payload.id,featureCount:1},geojson:{type:'FeatureCollection',features:[]}}));
});

it('keeps service layer selection separate from two real retrieval and inspection requests',async()=>{
  const user=userEvent.setup();render(<MapServiceExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('checkbox',{name:'Select result First map'}));await user.click(screen.getByRole('checkbox',{name:'Select result Second map'}));
  expect(mapRequest.mock.calls.every(([operation])=>operation==='services')).toBe(true);
  await acquire(user);
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toContain('map:Second map:map-second'));
  expect(mapRequest.mock.calls.filter(([operation])=>operation==='get').map(([,payload])=>payload.layerName)).toEqual(['first','second']);
  expect(mapRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(2);
  await user.click(screen.getByRole('button',{name:'Clear map previews'}));
  expect(screen.getByTestId('loaded-layers').textContent).toBe('');
  expect(screen.getByRole('checkbox',{name:'Select result First map'}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('checkbox',{name:'Select result Second map'}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('button',{name:'Export result First map'})).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Preview selected'}));
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toContain('map:Second map:map-second'));
  expect(mapRequest.mock.calls.filter(([operation])=>operation==='get')).toHaveLength(2);
});

it('does not discard a good map result when another service layer fails',async()=>{
  const original=mapRequest.getMockImplementation();mapRequest.mockImplementation((operation,payload,signal)=>operation==='get'&&payload.layerName==='first'?Promise.reject(new Error('First layer unavailable')):original(operation,payload,signal));
  const user=userEvent.setup();render(<MapServiceExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));await acquire(user);
  expect((await screen.findByRole('alert')).textContent).toContain('First layer unavailable');
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toBe('map:Second map:map-second'));
});

it('changing the shared AOI aborts a pending batch and excludes late results',async()=>{
  let resolveGet,signal;
  const original=mapRequest.getMockImplementation();mapRequest.mockImplementation((operation,payload,requestSignal)=>{if(operation==='get'){signal=requestSignal;return new Promise(resolve=>{resolveGet=resolve;});}return original(operation,payload,requestSignal);});
  const user=userEvent.setup(),props={areaBounds:bounds};const view=render(<MapServiceExplorer {...props}/>);
  await user.click(await screen.findByRole('checkbox',{name:'Select result First map'}));await acquire(user,{wait:false});
  await waitFor(()=>expect(resolveGet).toBeTypeOf('function'));
  view.rerender(<MapServiceExplorer areaBounds={[117,39,117.1,39.1]}/>);
  expect(signal.aborted).toBe(true);resolveGet({id:'late'});
  await waitFor(()=>expect(screen.getByRole('checkbox',{name:'Select result First map'}).getAttribute('aria-checked')).toBe('false'));
  expect(screen.getByTestId('loaded-layers').textContent).toBe('');
  expect(mapRequest.mock.calls.some(([operation,payload])=>operation==='inspect'&&payload.id==='late')).toBe(false);
});

it('queries and inspects multiple feature collections while keeping the shared AOI',async()=>{
  const geometry={type:'Polygon',coordinates:[[[116,39],[116.1,39],[116.1,39.1],[116,39]]]};
  const user=userEvent.setup();render(<FeatureServiceExplorer areaBounds={bounds} areaGeometry={geometry}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));
  expect(featureRequest.mock.calls.every(([operation])=>operation==='list')).toBe(true);
  await acquire(user);
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toContain('vector:Second vector:vector-second'));
  const requests=featureRequest.mock.calls.filter(([operation])=>operation==='query');expect(requests).toHaveLength(2);
  expect(requests[0][1]).toEqual({serviceId:'features',collectionId:'first',bounds,areaGeometry:geometry});
  expect(vectorRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(2);
});

it('extracts one bounded PMTiles package for several selected tile layers',async()=>{
  const source={id:'tiles',name:'PMTiles source',url:'https://tiles.example/data.pmtiles',header:{minZoom:0,maxZoom:3},metadata:{vector_layers:[{id:'roads'},{id:'buildings'}]}};
  tileRequest.mockImplementation(async(operation)=>operation==='sources'?[source]:operation==='extract'?{id:'package'}:{asset:{id:'package',source,minZoom:0,maxZoom:3,requestedBounds:bounds}});
  const user=userEvent.setup();render(<TileSourceExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));await acquire(user);
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toBe('tiles:roads:package|tiles:buildings:package'));
  expect(tileRequest.mock.calls.filter(([operation])=>operation==='extract')).toHaveLength(1);
  expect(tileRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(1);
  await user.click(screen.getByRole('button',{name:'Clear map previews'}));await user.click(screen.getByRole('button',{name:'Preview selected'}));
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toContain('tiles:roads:package'));
  expect(tileRequest.mock.calls.filter(([operation])=>operation==='extract')).toHaveLength(1);
});

it('does not allow an oversized Overpass AOI to be queried',async()=>{
  featureRequest.mockResolvedValue([{...featureSource,overpass:{},collections:[{id:'roads',title:'Roads'}]}]);
  const user=userEvent.setup();render(<FeatureServiceExplorer areaBounds={[115,38,117,40]} initialProtocol="Overpass"/>);
  await user.click(await screen.findByRole('checkbox',{name:'Select result Roads'}));
  expect(screen.getByRole('button',{name:'Save and download'}).disabled).toBe(true);
  expect(featureRequest.mock.calls.every(([operation])=>operation==='list')).toBe(true);
});

it('filters saved connections by selected protocol instead of mixing source channels',async()=>{
  mapRequest.mockResolvedValue([{...mapSource,id:'wms'}, {...mapSource,id:'wmts',name:'WMTS source',wmts:{matrixSets:[]}}]);
  render(<MapServiceExplorer areaBounds={bounds} initialProtocol="WMTS"/>);
  expect((await screen.findByRole('combobox',{name:'Connected source'})).textContent).toBe('WMTS source');
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('changing a loaded service time invalidates its preview and retrieves the new time',async()=>{
  const timed={...mapSource,layers:[{...mapSource.layers[0],time:{default:'2025-01-01',values:'2025-01-01/2025-01-31/P1D'}}]};
  const original=mapRequest.getMockImplementation();mapRequest.mockImplementation((operation,payload,signal)=>operation==='services'?Promise.resolve([timed]):original(operation,payload,signal));
  const user=userEvent.setup();render(<MapServiceExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('checkbox',{name:'Select result First map'}));await acquire(user);
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent).toContain('map:First map'));
  const time=screen.getByRole('textbox',{name:'Service time for First map'});await user.clear(time);await user.type(time,'2025-01-02');
  expect(screen.getByTestId('loaded-layers').textContent).toBe('');
  await acquire(user);
  await waitFor(()=>expect(mapRequest.mock.calls.filter(([operation])=>operation==='get')).toHaveLength(2));
  expect(mapRequest.mock.calls.filter(([operation])=>operation==='get').map(([,payload])=>payload.time)).toEqual(['2025-01-01','2025-01-02']);
});

it('enforces the preview limit across earlier loaded and newly selected layers',async()=>{
  const many={...mapSource,layers:Array.from({length:5},(_,index)=>({name:`layer-${index}`,title:`Layer ${index}`,styles:[],time:null}))};
  const original=mapRequest.getMockImplementation();mapRequest.mockImplementation((operation,payload,signal)=>operation==='services'?Promise.resolve([many]):original(operation,payload,signal));
  const user=userEvent.setup();render(<MapServiceExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));await acquire(user);
  await waitFor(()=>expect(screen.getByTestId('loaded-layers').textContent.split('|')).toHaveLength(4));
  for(let index=0;index<4;index++)await user.click(screen.getByRole('checkbox',{name:`Select result Layer ${index}`}));
  await user.click(screen.getByRole('checkbox',{name:'Select result Layer 4'}));
  expect(screen.getByRole('button',{name:'Save and download'}).disabled).toBe(true);
  expect(mapRequest.mock.calls.filter(([operation])=>operation==='get')).toHaveLength(4);
});

it('retains the rendered-image request bounds and distinguishes the output grid',()=>{
  expect(exploreMapImageRequest(mapSource,mapSource.layers[0],bounds,null)).toEqual({serviceId:'maps',layerName:'first',style:'',time:null,bounds,width:1024,height:1024,areaGeometry:null});
});
