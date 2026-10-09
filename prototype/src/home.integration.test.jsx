import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './main.jsx';
import { I18nProvider } from './i18n.jsx';
import { RuntimeProvider } from './runtime-ui.jsx';
import { agentRequest, subscribeAgentUpdates } from './agent-client.js';
import { createSearchRunner } from './catalog.js';
import { PROVIDERS } from './providers.js';
import { originalsReleased } from './release-policy.js';
import { SOURCE_DIRECTORY } from './source-directory.js';
import { loadPlanPreviewScenes } from './agent-map-preview.js';
import {signedOutIdentity} from './identity-client.js';

vi.mock('./agent-client.js',()=>({agentRequest:vi.fn(),subscribeAgentUpdates:vi.fn().mockResolvedValue(()=>{})}));
vi.mock('./runtime-client.js',async original=>({...await original(),desktopAvailable:()=>true,syncDesktopLocale:vi.fn().mockResolvedValue(undefined),runtimeRequest:vi.fn(async operation=>operation==='health'?{}:[])}));
vi.mock('./catalog.js',async original=>({...await original(),createSearchRunner:vi.fn()}));
vi.mock('./agent-map-preview.js',async original=>({...await original(),loadPlanPreviewScenes:vi.fn()}));
vi.mock('./explore-map.jsx',()=>({ExploreMap:({scene,area})=><div aria-label="Task map adapter test">{scene?.id} · {area.join(',')}</div>}));
let search, backend;
beforeEach(()=>{
  window.history.replaceState(null,'','/');
  const storage=new Map([['geod-global-locale','en']]);
  const local={getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value)};
  Object.defineProperty(window,'localStorage',{configurable:true,value:local});
  vi.stubGlobal('localStorage',local);
  Object.defineProperty(window,'innerWidth',{configurable:true,value:720});
  vi.clearAllMocks();
  window.__TAURI__={core:{invoke:vi.fn(async command=>command==='activate_desktop_frame'?'native':command==='identity_snapshot'?signedOutIdentity():[])}};
  search=vi.fn();createSearchRunner.mockReturnValue({cancel:vi.fn(),run:search});
  backend={version:1,revision:1,runtimeAvailable:true,configured:true,busy:false,mode:'review-first',model:{label:'Owned QA',model:'test-model',protocol:'openai-compatible'},selected:null,sessions:[],plans:[]};
  agentRequest.mockImplementation(async()=>structuredClone(backend));
});
afterEach(()=>{cleanup();delete window.__TAURI__;vi.unstubAllGlobals();});
const mount=()=>render(<I18nProvider><RuntimeProvider><App/></RuntimeProvider></I18nProvider>);
it('opens a dedicated account page from Home and returns to the local Agent as a guest',async()=>{
  mount();await screen.findByRole('region',{name:'2D data source directory'});
  await userEvent.click(screen.getByRole('button',{name:'Sign in',exact:true}));
  await screen.findByRole('heading',{name:'Welcome to GeoD Global'});
  expect(location.hash).toBe('#SignIn');expect(screen.queryByRole('button',{name:'Close Agent'})).toBeNull();expect(screen.queryByRole('textbox',{name:'Agent message'})).toBeNull();
  expect(document.querySelector('.app-header')).toBeNull();expect(document.getElementById('primary-navigation')).toBeNull();
  expect(document.documentElement.dataset.theme).toBe('dark');expect(JSON.parse(localStorage.getItem('geod-design-theme'))).toBe('light');
  await userEvent.click(screen.getByRole('button',{name:'Continue without an account'}));
  await screen.findByRole('region',{name:'2D data source directory'});expect(location.hash).toBe('#Home');
  expect(document.querySelector('.app-header')).toBeTruthy();expect(document.getElementById('primary-navigation')).toBeTruthy();
  expect(document.documentElement.dataset.theme).toBe('light');expect(JSON.parse(localStorage.getItem('geod-design-theme'))).toBe('light');
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);expect(search).not.toHaveBeenCalled();
});
it('opens a right map panel while keeping the same conversation, review and editable draft on the left',async()=>{
  const sessionId='a1234567-1234-1234-1234-123456789abc',planId='b1234567-1234-1234-1234-123456789abc';
  const plan={planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Earth Search',bounds:[-74.3,40.4,-73.7,41],files:[{itemId:'S2A_selected',assetKey:'visual',bytes:100}],expectedBytes:100,jobs:[],notes:[]};
  const preview={planId,planHash:plan.planHash,provider:'earth-search',bounds:plan.bounds,geometry:null,selections:[]};
  loadPlanPreviewScenes.mockResolvedValue({scenes:[],failed:0});
  agentRequest.mockImplementation(async(operation,args)=>{
    if(operation==='planMapPreview')return preview;
    if(operation==='send'){
      backend.selected={id:sessionId,status:'completed',entries:[{id:'human',type:'user',status:'completed',text:args.text},{id:'plan',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download review'}]}]};
      backend.sessions=[{id:sessionId,title:'Imagery task',status:'completed',compatible:true}];backend.plans=[plan];
    }
    return structuredClone(backend);
  });
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Find New York imagery'}});fireEvent.click(screen.getByRole('button',{name:'Send message'}));
  await screen.findByRole('button',{name:'Map preview'});fireEvent.change(input,{target:{value:'Keep this follow-up draft'}});
  fireEvent.click(screen.getByRole('button',{name:'Map preview'}));
  const panel=await screen.findByRole('region',{name:'Task map preview'});await screen.findByLabelText('Task map adapter test');
  expect(screen.queryByRole('dialog')).toBeNull();expect(location.hash).toBe('#Home');expect(input.value).toBe('Keep this follow-up draft');
  expect(screen.getByRole('button',{name:'Confirm download'}).disabled).toBe(false);expect(document.querySelector('#agent-pane').nextElementSibling.id).toBe('agent-map-preview-pane');
  expect(panel.closest('#agent-map-preview-pane')).toBeTruthy();expect(subscribeAgentUpdates).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole('button',{name:'Close map preview'}));await waitFor(()=>expect(screen.queryByRole('region',{name:'Task map preview'})).toBeNull());
  expect(input.value).toBe('Keep this follow-up draft');fireEvent.click(screen.getByRole('button',{name:'Map preview'}));await screen.findByRole('region',{name:'Task map preview'});
  fireEvent.click(screen.getByRole('button',{name:'New conversation'}));await waitFor(()=>expect(screen.queryByRole('region',{name:'Task map preview'})).toBeNull());
  expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);expect(search).not.toHaveBeenCalled();
});
const chooseFilter=async(board,label,name)=>{
  await userEvent.click(within(board).getByRole('combobox',{name:label}));
  await userEvent.click(screen.getByRole('option',{name,exact:true}));
};

it('opens the real AI homepage without a sample AOI, map or catalog request',async()=>{
  mount();await screen.findByRole('heading',{name:'What geographic data do you need?'});
  await waitFor(()=>expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).disabled).toBe(false));
  expect(location.hash).toBe('#Home');expect(search).not.toHaveBeenCalled();
  expect(document.querySelector('.workspace')).toBeNull();
  const board=screen.getByRole('region',{name:'2D data source directory'});
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(SOURCE_DIRECTORY.length);
  expect(within(board).getAllByRole('article')).toHaveLength(SOURCE_DIRECTORY.filter(source=>source.status==='planned').length);
  expect(within(board).getAllByRole('article').every(card=>within(card).getByText('Pending integration'))).toBe(true);
  expect(within(board).queryByText(/3D assets|3D Tiles|glTF|3DGS/)).toBeNull();
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(PROVIDERS.length);
  expect(within(board).getAllByText('Downloadable')).toHaveLength(PROVIDERS.filter(originalsReleased).length);
  expect(within(board).getAllByText('Catalog only')).toHaveLength(PROVIDERS.filter(p=>!originalsReleased(p)).length);
  const sentinel=within(board).getByRole('button',{name:'Explore Sentinel-2 · Earth Search'});
  expect(within(sentinel).getAllByText('Earth Search',{exact:true})).toHaveLength(1);
  const mark=within(sentinel).getByRole('img',{name:'Sentinel-2 mission mark'});
  expect(mark.getAttribute('src')).toBe('/source-marks/sentinel-2.jpg');
  fireEvent.error(mark);
  expect(within(sentinel).queryByRole('img')).toBeNull();
  expect(within(sentinel).getByText('Sentinel-2',{exact:true})).toBeTruthy();
  expect(search).not.toHaveBeenCalled();
});

it('source cards open the chosen manual catalog and preserve the same composer draft without searching',async()=>{
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Find my imagery'}});
  await userEvent.click(screen.getByRole('button',{name:'Explore Sentinel-1 RTC'}));
  await screen.findByRole('heading',{name:'Imagery scenes'});
  expect(location.hash).toBe('#Explore');
  expect(screen.getByRole('combobox',{name:'Data source'}).textContent).toBe('Sentinel-1 RTC');
  expect(screen.getByRole('button',{name:/Choose a search area/})).toBeTruthy();
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Find my imagery');expect(search).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole('link',{name:'Home'}));
  await screen.findByRole('heading',{name:'What geographic data do you need?'});
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Find my imagery');
  expect(subscribeAgentUpdates).toHaveBeenCalledOnce();
  expect(agentRequest.mock.calls.some(([operation])=>operation==='interrupt'||operation==='send')).toBe(false);
});

it('suggestions fill a draft, and only an explicit send enters the real conversation with null map context',async()=>{
  agentRequest.mockImplementation(async(operation,args)=>{
    if(operation==='send')backend.selected={id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[{id:'human',type:'user',status:'completed',text:args.text}]};
    return structuredClone(backend);
  });
  mount();await waitFor(()=>expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).disabled).toBe(false));
  await userEvent.click(screen.getByRole('button',{name:'Find the latest satellite imagery of New York City.'}));
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
  await userEvent.click(screen.getByRole('button',{name:'Send message'}));
  expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:null,text:'Find the latest satellite imagery of New York City.',context:null});
  await screen.findByRole('region',{name:'Conversation messages'});
  expect(screen.queryByRole('region',{name:'2D data source directory'})).toBeNull();
});

it('filters the actual provider registry and keeps protected catalogs available before model setup',async()=>{
  backend.configured=false;mount();await screen.findByText('Connect a model to start. You can explore data sources below.');
  const board=screen.getByRole('region',{name:'2D data source directory'});
  await chooseFilter(board,'Data source category','Elevation');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(PROVIDERS.filter(p=>p.domain==='elevation').length);
  expect(within(board).getByText('Authorization required · originals pending verification')).toBeTruthy();
  await userEvent.click(screen.getByRole('button',{name:'Connect model'}));
  expect(await screen.findByRole('dialog',{name:'Agent model connection'})).toBeTruthy();
  expect(search).not.toHaveBeenCalled();
});

it('groups Sentinel across platforms, combines series and data type, and resets an empty combination without losing the draft',async()=>{
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Find Sentinel imagery'}});
  const board=screen.getByRole('region',{name:'2D data source directory'});
  await userEvent.click(within(board).getByRole('combobox',{name:'Series / source family'}));
  await userEvent.click(screen.getByRole('option',{name:'Sentinel',exact:true}));
  expect(within(board).getAllByRole('button',{name:/^Explore /}).map(button=>button.getAttribute('aria-label'))).toEqual([
    'Explore Sentinel-2 · Earth Search','Explore Sentinel-2 · Planetary Computer','Explore Sentinel-2 SAFE · Copernicus','Explore Sentinel-1 RTC',
  ]);
  expect(within(board).getAllByText('Downloadable')).toHaveLength(3);
  expect(within(board).getAllByText('Catalog only')).toHaveLength(1);
  await chooseFilter(board,'Data source category','Radar');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(1);
  expect(within(board).getByRole('button',{name:'Explore Sentinel-1 RTC'})).toBeTruthy();
  await chooseFilter(board,'Data source category','Elevation');
  expect(within(board).queryByRole('button',{name:/^Explore /})).toBeNull();
  expect(within(board).getByText('No sources match these filters.')).toBeTruthy();
  expect(within(board).getByRole('combobox',{name:'Series / source family'}).textContent).toBe('Sentinel');
  await userEvent.click(within(board).getByRole('button',{name:'Reset filters'}));
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(15);
  expect(input.value).toBe('Find Sentinel imagery');
  expect(search).not.toHaveBeenCalled();
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
});

it('keeps derived HLS distinct from Landsat and includes all protected VIIRS platforms in their own series',async()=>{
  backend.configured=false;mount();
  const board=await screen.findByRole('region',{name:'2D data source directory'});
  const chooseSeries=async name=>{
    await userEvent.click(within(board).getByRole('combobox',{name:'Series / source family'}));
    await userEvent.click(screen.getByRole('option',{name,exact:true}));
  };
  await chooseSeries('Landsat');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(1);
  expect(within(board).getByRole('button',{name:'Explore Landsat 8 / 9'})).toBeTruthy();
  await chooseSeries('HLS');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(1);
  expect(within(board).getByRole('button',{name:'Explore NASA HLS · Landsat L30'})).toBeTruthy();
  expect(within(board).getByText('Catalog only')).toBeTruthy();
  await chooseSeries('VIIRS');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(3);
  expect(within(board).getAllByText('Catalog only')).toHaveLength(3);
  expect(search).not.toHaveBeenCalled();
});

it('shows the complete pending inventory as readable noninteractive cards and combines all three filters',async()=>{
  mount();const board=await screen.findByRole('region',{name:'2D data source directory'});
  await chooseFilter(board,'Integration status','Pending integration');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(SOURCE_DIRECTORY.filter(source=>source.status==='planned').length);
  expect(within(board).queryByRole('button')).toBeNull();
  const wayback=within(board).getByRole('article',{name:'Wayback'});
  expect(within(wayback).getByText('Pending integration')).toBeTruthy();
  await userEvent.click(wayback);
  expect(screen.queryByRole('dialog')).toBeNull();expect(location.hash).toBe('#Home');
  await chooseFilter(board,'Series / source family','Landsat');
  await chooseFilter(board,'Data source category','Imagery');
  expect(within(board).getAllByRole('article')).toHaveLength(1);
  expect(within(board).getByRole('article',{name:'Landsat · USGS direct access'})).toBeTruthy();
  await chooseFilter(board,'Integration status','Authorization required');
  expect(within(board).getByText('No sources match these filters.')).toBeTruthy();
  await userEvent.click(within(board).getByRole('button',{name:'Reset filters'}));
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(SOURCE_DIRECTORY.length);
  await chooseFilter(board,'Integration status','Available');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(31);
  await chooseFilter(board,'Integration status','Authorization required');
  expect(within(board).getAllByRole('button',{name:/^Explore /})).toHaveLength(6);
  await chooseFilter(board,'Integration status','All statuses');
  await chooseFilter(board,'Data source category','Offline tiles');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(3);
  await chooseFilter(board,'Data source category','Vector data');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(SOURCE_DIRECTORY.filter(source=>source.category==='vectors').length);
  await chooseFilter(board,'Series / source family','Local files');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(4);
  await chooseFilter(board,'Data source category','Local files');
  expect(board.querySelectorAll('[data-source-id]')).toHaveLength(7);
  expect(search).not.toHaveBeenCalled();
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
},15000);

it('opens each service card with its correct form and performs only local registry reads',async()=>{
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Keep this draft'}});
  const board=screen.getByRole('region',{name:'2D data source directory'});
  const cases=[
    ['STAC API','Custom raster sources','Source type','STAC API'],
    ['STAC API','Custom raster sources','Source type','STAC API'], // Reopen during its close animation.
    ['Static STAC catalog','Custom raster sources','Source type','Static STAC catalog'],
    ['STAC item','Custom raster sources','Source type','STAC item'],
    ['COG / GeoTIFF URL','Custom raster sources','Source type','COG / GeoTIFF URL'],
    ['WCS 2.0.1','Coverage services (WCS)'],
    ['NASA GIBS','Get map imagery','Map service type','WMS'],
    ['WMS','Get map imagery','Map service type','WMS'],
    ['WMTS','Get map imagery','Map service type','WMTS'],
    ['XYZ','Get map imagery','Map service type','XYZ'],
    ['TMS','Get map imagery','Map service type','TMS'],
    ['ArcGIS MapServer / ImageServer','Get map imagery','Map service type','ArcGIS MapServer / ImageServer'],
    ['OpenStreetMap / Overpass','Get vector data','Service protocol','OSM Overpass'],
    ['OGC API Features','Get vector data','Service protocol','OGC API Features'],
    ['WFS 2.0','Get vector data','Service protocol','WFS 2.0'],
    ['ArcGIS Feature Service','Get vector data','Service protocol','ArcGIS Feature Service'],
    ['Protomaps / PMTiles','Extract offline tiles'],
  ];
  for(const[name,title,field,value]of cases){
    await userEvent.click(within(board).getByRole('button',{name:`Connect ${name}`}));
    const dialog=await screen.findByRole('dialog',{name:title});
    if(field)expect((await within(dialog).findByRole('combobox',{name:field})).textContent).toBe(value);
    if(name==='NASA GIBS')expect(within(dialog).getByRole('textbox',{name:'WMS endpoint URL'}).value).toBe('https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi');
    if(name==='OpenStreetMap / Overpass')expect(within(dialog).getByRole('textbox',{name:'Overpass endpoint URL'}).value).toBe('');
    await userEvent.click(within(dialog).getByRole('button',{name:'Close',exact:true}));
  }
  const commands=window.__TAURI__.core.invoke.mock.calls.map(([command])=>command);
  expect(commands.length).toBeGreaterThan(0);
  expect(commands.every(command=>command.startsWith('list_')||['identity_snapshot','distribution_snapshot','activate_desktop_frame','set_desktop_appearance'].includes(command))).toBe(true);
  expect(input.value).toBe('Keep this draft');expect(search).not.toHaveBeenCalled();
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
},20000);

it.each([
  ['Local PMTiles','Offline tiles','tiles'],['Local MBTiles','Offline tiles','tiles'],
  ['GeoJSON / Overpass JSON','Vector files','vectors'],['GeoPackage','Vector files','vectors'],
  ['Shapefile / ZIP','Vector files','vectors'],['OSM XML / PBF','Vector files','vectors'],
])('opens the existing 2D file library from %s without importing or sending a message',async(name,view,key)=>{
  mount();await screen.findByRole('region',{name:'2D data source directory'});
  await userEvent.click(screen.getByRole('button',{name:`Open ${name}`}));
  await screen.findByRole('heading',{name:'My Data'});
  expect(location.hash).toBe(`#My%20Data?view=${key}`);
  expect(screen.getByRole('radio',{name:view}).getAttribute('aria-checked')).toBe('true');
  expect(screen.queryByRole('radio',{name:'3D assets'})).toBeNull();
  expect(search).not.toHaveBeenCalled();
  expect(window.__TAURI__.core.invoke.mock.calls.every(([command])=>command.startsWith('list_')||['identity_snapshot','distribution_snapshot','activate_desktop_frame','set_desktop_appearance'].includes(command))).toBe(true);
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
});
