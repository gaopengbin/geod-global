import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './main.jsx';
import { I18nProvider } from './i18n.jsx';
import { RuntimeProvider } from './runtime-ui.jsx';
import { agentRequest, subscribeAgentUpdates } from './agent-client.js';
import { createSearchRunner } from './catalog.js';
import { normalizeScene } from './catalog.js';
import sentinelFixture from '../public/samples/earth-search-response.json';
import numFixture from './srtm-num.fixture.json';
import { SOURCE_DIRECTORY } from './source-directory.js';
import { loadPlanPreviewScenes } from './agent-map-preview.js';
import {signedOutIdentity} from './identity-client.js';
import {publicDatasetRequest} from './public-dataset-client.js';
import {publicDataset} from './public-dataset-fixtures.js';
import {runtimeRequest} from './runtime-client.js';

vi.mock('./agent-client.js',()=>({agentRequest:vi.fn(),subscribeAgentUpdates:vi.fn().mockResolvedValue(()=>{})}));
vi.mock('./runtime-client.js',async original=>({...await original(),desktopAvailable:()=>true,syncDesktopLocale:vi.fn().mockResolvedValue(undefined),runtimeRequest:vi.fn(async operation=>operation==='health'?{}:[])}));
vi.mock('./catalog.js',async original=>({...await original(),createSearchRunner:vi.fn()}));
vi.mock('./agent-map-preview.js',async original=>({...await original(),loadPlanPreviewScenes:vi.fn()}));
vi.mock('./explore-map.jsx',()=>({ExploreMap:({scene,area})=><div aria-label="Task map adapter test">{scene?.id} · {area.join(',')}</div>}));
vi.mock('./public-dataset-client.js',async original=>({...await original(),publicDatasetRequest:vi.fn()}));
let search, backend;
const originalScrollTo=Element.prototype.scrollTo;
beforeEach(()=>{
  Element.prototype.scrollTo=vi.fn();
  window.history.replaceState(null,'','/');
  const storage=new Map([['geod-global-locale','en']]);
  const local={getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value)};
  Object.defineProperty(window,'localStorage',{configurable:true,value:local});
  vi.stubGlobal('localStorage',local);
  Object.defineProperty(window,'innerWidth',{configurable:true,value:720});
  vi.clearAllMocks();
  runtimeRequest.mockImplementation(async operation=>operation==='health'?{}:[]);
  window.__TAURI__={core:{invoke:vi.fn(async command=>command==='activate_desktop_frame'?'native':command==='identity_snapshot'?signedOutIdentity():[])}};
  search=vi.fn();createSearchRunner.mockReturnValue({cancel:vi.fn(),run:search});
  backend={version:1,revision:1,runtimeAvailable:true,configured:true,busy:false,mode:'review-first',model:{label:'Owned QA',model:'test-model',protocol:'openai-compatible'},selected:null,sessions:[],plans:[]};
  agentRequest.mockImplementation(async()=>structuredClone(backend));
  publicDatasetRequest.mockImplementation(async()=>[publicDataset,{...publicDataset,id:'worldpop',title:'WorldPop historical population',requiresCountry:true}]);
});
afterEach(()=>{cleanup();delete window.__TAURI__;vi.unstubAllGlobals();if(originalScrollTo)Element.prototype.scrollTo=originalScrollTo;else delete Element.prototype.scrollTo;});
const mount=()=>render(<I18nProvider><RuntimeProvider><App/></RuntimeProvider></I18nProvider>);

it('keeps real NUM source-code search distinct from heights and never claims account-free protected downloads',async()=>{
  const user=userEvent.setup(),scene=normalizeScene(structuredClone(numFixture),'srtm-num');
  createSearchRunner.mockReturnValue({cancel:vi.fn(),runAll:vi.fn(async(_url,{onPage})=>{const result={scenes:[scene],complete:true,pages:1};onPage(result);return result;})});
  mount();await chooseHomeSource('srtm-num');
  await user.click(screen.getByRole('button',{name:'Filters',exact:true}));
  fireEvent.change(screen.getByRole('textbox',{name:'Search bounding box'}),{target:{value:'116.3,39.8,116.5,40'}});
  await user.click(screen.getByRole('button',{name:'Search catalog'}));
  await user.click(await screen.findByRole('button',{name:`Locate scene ${scene.id}`}));
  await user.click(screen.getByRole('button',{name:'Show inspector'}));
  expect(await screen.findByText('UInt8 · NUM')).toBeTruthy();
  expect(screen.getByText('source tiles')).toBeTruthy();
  expect(screen.getByText('Downloads require your own connected source account.')).toBeTruthy();
  expect(screen.queryByText('Original downloads available without an account.')).toBeNull();
  expect(screen.queryByText('Float32 · EGM2008')).toBeNull();
  expect(screen.queryByText('Int16 · EGM96')).toBeNull();
  expect(screen.getByText('NASA SRTMGL1N v003 · NASA Earthdata · SRTM NUM · Natural Earth overview')).toBeTruthy();
});

it('selects public products directly in Explore, preserves the current area, ignores late satellite pages and sends only the current product form',async()=>{
  const user=userEvent.setup();let onPage,settle;
  const cancel=vi.fn(),runAll=vi.fn((url,options)=>{onPage=options.onPage;return new Promise(resolve=>{settle=resolve;});});
  createSearchRunner.mockReturnValue({cancel,runAll});
  mount();await chooseHomeSource('earth-search');
  await user.click(screen.getByRole('button',{name:'Filters',exact:true}));
  fireEvent.change(screen.getByRole('textbox',{name:'Search bounding box'}),{target:{value:'-169.96,-19.17,-169.77,-18.94'}});
  await user.click(screen.getByRole('button',{name:'Search catalog'}));
  expect(runAll).toHaveBeenCalledOnce();
  await user.click(screen.getByRole('button',{name:'Choose a data source'}));
  await user.click(within(screen.getByRole('dialog',{name:'Choose a data source'})).getByRole('button',{name:/Choose WorldPop/}));
  await screen.findByRole('textbox',{name:'Dataset country code'});
  expect(screen.queryByRole('dialog')).toBeNull();expect(screen.queryByRole('textbox',{name:'Search scenes'})).toBeNull();
  expect(screen.getByRole('region',{name:'Product footprint map'})).toBeTruthy();
  expect(screen.queryByRole('textbox',{name:'Dataset search bounds'})).toBeNull();
  expect(screen.getByLabelText('Task map adapter test').textContent).toContain('-169.96,-19.17,-169.77,-18.94');
  const old={scenes:[{...normalizeScene(sentinelFixture.features[0]),id:'late-old-scene'}],complete:true,pages:1};
  await act(async()=>{onPage(old);settle(old);});
  expect(screen.queryByText('late-old-scene')).toBeNull();expect(cancel).toHaveBeenCalled();expect(runAll).toHaveBeenCalledOnce();
  await user.type(screen.getByRole('textbox',{name:'Dataset country code'}),'NIU');
  await userEvent.click(screen.getByRole('button',{name:'Open Agent'}));
  const input=screen.getByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Use this product'}});
  await user.click(screen.getByRole('button',{name:'Send message'}));
  const sent=agentRequest.mock.calls.filter(([operation])=>operation==='send').at(-1)[1];
  expect(sent.context).toMatchObject({provider:'dataset-worldpop',bounds:[-169.96,-19.17,-169.77,-18.94],catalog:null,dataset:{datasetId:'worldpop',country:'NIU'}});
  expect(sent.context.start).toBe('');expect(sent.context.end).toBe('');
});

it('continues catalog pagination after selecting a first-page scene',async()=>{
  const user=userEvent.setup();let onPage,settle;
  createSearchRunner.mockReturnValue({cancel:vi.fn(),runAll:vi.fn((url,options)=>{onPage=options.onPage;return new Promise(resolve=>{settle=resolve;});})});
  mount();await chooseHomeSource('earth-search');
  await user.click(screen.getByRole('button',{name:'Filters',exact:true}));
  fireEvent.change(screen.getByRole('textbox',{name:'Search bounding box'}),{target:{value:'10,20,11,21'}});
  await user.click(screen.getByRole('button',{name:'Search catalog'}));
  const first={...normalizeScene(sentinelFixture.features[0]),id:'first-page-scene'};
  await act(async()=>onPage({scenes:[first],complete:false,pages:1}));
  await user.click(screen.getByRole('checkbox',{name:/Select scene .*first-page-scene/}));
  const final={scenes:[first,{...first,id:'second-page-scene'}],complete:true,pages:2};
  await act(async()=>{onPage(final);settle(final);});
  expect(await screen.findByText('Catalog complete · 2 scenes')).toBeTruthy();
  expect(screen.getByRole('checkbox',{name:/Select scene .*second-page-scene/})).toBeTruthy();
});

it('a late project restoration cannot replace a public product explicitly selected in the classified picker',async()=>{
  const user=userEvent.setup(),id='a1234567-1234-1234-1234-123456789abc';let settle;
  const pending=new Promise(resolve=>{settle=resolve;});
  runtimeRequest.mockImplementation(async operation=>operation==='projects'?pending:operation==='health'?{}:[]);
  window.history.replaceState(null,'',`/#Explore?project=${id}`);
  mount();await user.click(await screen.findByRole('button',{name:'Choose a data source'}));
  await user.click(within(screen.getByRole('dialog',{name:'Choose a data source'})).getByRole('button',{name:/Choose WorldPop/}));
  await screen.findByRole('textbox',{name:'Dataset country code'});
  await act(async()=>settle([{id,name:'Delayed old project',bounds:[10,20,11,21]}]));
  expect(screen.getByRole('button',{name:'Choose a data source'}).textContent).toContain('WorldPop');
  expect(screen.getByRole('textbox',{name:'Dataset country code'})).toBeTruthy();
  expect(screen.queryByText('Loading project scenes…')).toBeNull();expect(location.hash).toBe('#Explore');
  expect(search).not.toHaveBeenCalled();
});

it('restores each entry point independently and explicitly hands the same workspace task to Home without its ambient map',async()=>{
  const homeId='a1234567-1234-1234-1234-123456789abc',workspaceId='b1234567-1234-1234-1234-123456789abc';
  const sessions=[{id:workspaceId,title:'Workspace task',status:'completed',compatible:true,scene:{kind:'workspace',page:'Explore'},entries:[{id:'workspace-user',type:'user',text:'Workspace request',status:'completed'}]},
    {id:homeId,title:'Independent task',status:'completed',compatible:true,scene:{kind:'conversation',page:'Home'},entries:[{id:'home-user',type:'user',text:'Home request',status:'completed'}]}];
  backend.sessions=sessions.map(({entries,...session})=>session);backend.selected=sessions[0];
  agentRequest.mockImplementation(async(op,args)=>{
    if(op==='select')backend.selected=sessions.find(session=>session.id===args.id)??null;
    return structuredClone(backend);
  });
  mount();await screen.findByText('Home request');expect(screen.queryByText('Workspace request')).toBeNull();
  await userEvent.click(screen.getByRole('link',{name:'Explore'}));expect(screen.queryByText('Workspace request')).toBeNull();await userEvent.click(screen.getByRole('button',{name:'Open Agent'}));await screen.findByText('Workspace request');
  expect(screen.queryByText('Home request')).toBeNull();
  await userEvent.click(screen.getByRole('button',{name:'Continue in conversation'}));
  await waitFor(()=>expect(location.hash).toBe('#Home'));expect(screen.getByText('Workspace request')).toBeTruthy();
  fireEvent.change(screen.getByRole('textbox',{name:'Message GeoD Agent'}),{target:{value:'Continue this task'}});
  await userEvent.click(screen.getByRole('button',{name:'Send message'}));
  expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:workspaceId,text:'Continue this task',context:null,scene:{kind:'conversation',page:'Home'}});
});

it('a workspace task running elsewhere stays hidden until explicitly opened and does not expose its stop control',async()=>{
  const id='b1234567-1234-1234-1234-123456789abc';
  backend.busy=true;backend.activity={sessionId:id,kind:'turn',turnId:'active-turn'};
  backend.selected={id,status:'running',scene:{kind:'workspace',page:'Explore'},entries:[{id:'old-user',type:'user',status:'completed',text:'Running workspace request'}]};
  backend.sessions=[{...backend.selected,title:'Workspace task',compatible:true}];
  mount();await screen.findByText('Another conversation is running.');
  expect(screen.queryByText('Running workspace request')).toBeNull();expect(screen.queryByRole('button',{name:'Stop response'})).toBeNull();
  await userEvent.click(screen.getByRole('button',{name:'View running task'}));await screen.findByText('Running workspace request');
  expect(agentRequest.mock.calls.some(([op])=>op==='interrupt'||op==='send')).toBe(false);
});

it('sends the displayed Explore catalog and updated list filters with the real Agent composer request',async({onTestFinished})=>{
  Object.defineProperty(window,'innerWidth',{configurable:true,value:1440});
  const scroll=Element.prototype.scrollTo;
  Element.prototype.scrollTo=vi.fn();
  onTestFinished(()=>{if(scroll)Element.prototype.scrollTo=scroll;else delete Element.prototype.scrollTo;});
  // Catalog transport is mocked; App state, list filtering and Agent send run normally.
  const scenes=Array.from({length:4},(_,i)=>({...normalizeScene(sentinelFixture.features[0]),id:`context-scene-${i}`,date:i===3?'2026-10-10T12:00:00Z':'2026-10-09T12:00:00Z'}));
  const runAll=vi.fn(async(url,{onPage})=>{
    const catalog={scenes,complete:true,pages:1,retrievedAt:'2026-10-10T12:00:00Z'};
    onPage(catalog);return catalog;
  });
  createSearchRunner.mockReturnValue({cancel:vi.fn(),runAll});
  mount();await screen.findByRole('heading',{name:'What geographic data do you need?'});
  await chooseHomeSource('earth-search');
  await userEvent.click(screen.getByRole('button',{name:'Filters',exact:true}));
  fireEvent.change(screen.getByRole('textbox',{name:'Search bounding box'}),{target:{value:'-14,-12,51,37'}});
  screen.getByRole('slider',{name:'Live maximum cloud cover'}).focus();await userEvent.keyboard('{Home}');
  await userEvent.click(screen.getByRole('button',{name:'Search catalog'}));
  await screen.findByText('Catalog complete · 4 scenes');
  await userEvent.click(screen.getByRole('button',{name:'Open Agent'}));
  const input=screen.getByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));
  fireEvent.change(input,{target:{value:'How many scenes are there now?'}});
  await userEvent.click(screen.getByRole('button',{name:'Send message'}));
  let request=agentRequest.mock.calls.filter(([op])=>op==='send').at(-1)[1];
  expect(request.context.catalog).toMatchObject({totalCount:4,filteredCount:4,complete:true,state:'ready',dateCounts:[{date:'2026-10-10',count:1},{date:'2026-10-09',count:3}]});
  expect(request.context.cloudMin).toBe(0);expect(request.context.cloudMax).toBe(0);
  fireEvent.change(screen.getByRole('textbox',{name:'Search scenes'}),{target:{value:'context-scene-3'}});
  fireEvent.change(input,{target:{value:'And after filtering?'}});
  await userEvent.click(screen.getByRole('button',{name:'Send message'}));
  request=agentRequest.mock.calls.filter(([op])=>op==='send').at(-1)[1];
  expect(request.context.catalog).toMatchObject({totalCount:4,filteredCount:1,textFilter:'context-scene-3'});
  expect(runAll).toHaveBeenCalledOnce();
});
it('opens a dedicated account page from Home and returns to the local Agent as a guest',async()=>{
  mount();await screen.findByRole('heading',{name:'What geographic data do you need?'});
  await userEvent.click(screen.getByRole('button',{name:'Sign in',exact:true}));
  await screen.findByRole('heading',{name:'Welcome to GeoD Global'});
  expect(location.hash).toBe('#SignIn');expect(screen.queryByRole('button',{name:'Close Agent'})).toBeNull();expect(screen.queryByRole('textbox',{name:'Agent message'})).toBeNull();
  expect(document.querySelector('.app-header')).toBeNull();expect(document.getElementById('primary-navigation')).toBeNull();
  expect(document.documentElement.dataset.theme).toBe('dark');expect(JSON.parse(localStorage.getItem('geod-design-theme'))).toBe('light');
  await userEvent.click(screen.getByRole('button',{name:'Continue without an account'}));
  await screen.findByRole('heading',{name:'What geographic data do you need?'});expect(location.hash).toBe('#Home');
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
async function chooseHomeSource(id){
  await userEvent.click(await screen.findByRole('button',{name:/^Explore data/}));
  const dialog=await screen.findByRole('dialog',{name:'Choose a data source'});
  const card=dialog.querySelector(`[data-source-id="${id}"]`);
  expect(card).not.toBeNull();await userEvent.click(within(card).getByRole('button',{name:/^Choose /}));
  await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Choose a data source'})).toBeNull());
}

it('keeps Home focused on a draft and three workflow shortcuts without fetching a sample area or exposing unfinished monitoring',async()=>{
  mount();await screen.findByRole('heading',{name:'What geographic data do you need?'});
  await waitFor(()=>expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).disabled).toBe(false));
  expect(location.hash).toBe('#Home');expect(search).not.toHaveBeenCalled();
  expect(document.querySelector('.workspace')).toBeNull();
  expect(screen.queryByRole('region',{name:'2D data source directory'})).toBeNull();
  expect(screen.queryByRole('link',{name:'Imagery monitoring'})).toBeNull();
  const shortcuts=screen.getByRole('navigation',{name:'Workspace shortcuts'});
  expect(within(shortcuts).getByRole('button',{name:/^Explore data/})).toBeTruthy();
  expect(within(shortcuts).getAllByRole('link').map(link=>link.hash)).toEqual(['#My%20Data','#Tasks']);
  await userEvent.click(within(shortcuts).getByRole('button',{name:/^Explore data/}));
  const picker=await screen.findByRole('dialog',{name:'Choose a data source'});
  expect(picker.querySelector('[data-source-id="earth-search"]')).toBeTruthy();
  expect(document.activeElement).toBe(within(picker).getByRole('searchbox'));
  await userEvent.keyboard('{Escape}');
  await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
  expect(document.activeElement).toBe(within(shortcuts).getByRole('button',{name:/^Explore data/}));
  expect(search).not.toHaveBeenCalled();
});

it('source cards open an independent workspace draft while preserving the conversation draft on return',async()=>{
  const unsubscribe=vi.fn();subscribeAgentUpdates.mockResolvedValue(unsubscribe);
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Find my imagery'}});
  await chooseHomeSource('planetary-radar');
  await screen.findByRole('heading',{name:'Search results'});
  expect(location.hash).toBe('#Explore');
  expect(screen.getByRole('button',{name:'Choose a data source'}).textContent).toBe('Sentinel-1 RTC');
  expect(screen.getByRole('button',{name:/Choose a search area/})).toBeTruthy();
  expect(screen.queryByRole('textbox',{name:'Message GeoD Agent'})).toBeNull();await userEvent.click(screen.getByRole('button',{name:'Open Agent'}));
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('');expect(search).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole('textbox',{name:'Message GeoD Agent'}),{target:{value:'Workspace draft'}});
  await userEvent.click(screen.getByRole('link',{name:'Home'}));
  await screen.findByRole('heading',{name:'What geographic data do you need?'});
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Find my imagery');
  await chooseHomeSource('planetary-radar');
  await userEvent.click(screen.getByRole('button',{name:'Open Agent'}));
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Workspace draft');
  expect(subscribeAgentUpdates).toHaveBeenCalledTimes(3);expect(unsubscribe).toHaveBeenCalledTimes(2);
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
  expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:null,text:'Find the latest satellite imagery of New York City.',context:null,scene:{kind:'conversation',page:'Home'}});
  await screen.findByRole('region',{name:'Conversation messages'});
  expect(screen.queryByRole('region',{name:'2D data source directory'})).toBeNull();
});

it('keeps source exploration available without model setup, and model configuration one click away',async()=>{
  backend.configured=false;mount();
  await screen.findByText('Connect a model to chat, or explore data directly.');
  await userEvent.click(screen.getByRole('button',{name:'Connect model'}));
  expect(await screen.findByRole('dialog',{name:'Agent model connection'})).toBeTruthy();
  await userEvent.click(within(screen.getByRole('dialog')).getByRole('button',{name:'Close'}));
  await chooseHomeSource('srtm-num');
  expect(location.hash).toBe('#Explore');expect(search).not.toHaveBeenCalled();
});

it('opens each service card inside the shared Explore sidebar and performs only local registry reads',async()=>{
  mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});
  await waitFor(()=>expect(input.disabled).toBe(false));fireEvent.change(input,{target:{value:'Keep this draft'}});
  const cases=[
    ['STAC API','Source type','STAC API'],
    ['STAC API','Source type','STAC API'], // Reopen the same adapter after leaving Explore.
    ['Static STAC catalog','Source type','Static STAC catalog'],
    ['STAC item','Source type','STAC item'],
    ['COG / GeoTIFF URL','Source type','COG / GeoTIFF URL'],
    ['WCS 2.0.1'],['NASA GIBS'],['WMS'],['WMTS'],['XYZ'],['TMS'],
    ['ArcGIS MapServer / ImageServer'],['OpenStreetMap / Overpass'],
    ['OGC API Features'],['WFS 2.0'],['ArcGIS Feature Service'],['Protomaps / PMTiles'],
  ];
  for(const[name,field,value]of cases){
    await chooseHomeSource(SOURCE_DIRECTORY.find(source=>source.name===name).id);
    await screen.findByRole('main',{name:'Data source exploration'});
    const controls=screen.getByRole('region',{name:'Source search and results'});
    expect(document.querySelector('#explore-discovery').contains(controls)).toBe(true);
    expect(screen.queryByRole('dialog')).toBeNull();expect(location.hash).toBe('#Explore');
    expect(screen.getByRole('button',{name:'Choose a data source'}).textContent).toContain(name);
    if(field)expect((await within(controls).findByRole('combobox',{name:field})).textContent).toBe(value);
    if(name==='WCS 2.0.1')await within(controls).findByRole('textbox',{name:'WCS service URL'});
    if(name==='NASA GIBS')expect((await within(controls).findByRole('textbox',{name:'Service URL'})).value).toBe('https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi');
    if(name==='OpenStreetMap / Overpass')expect((await within(controls).findByRole('textbox',{name:'Service URL'})).value).toBe('');
    if(name==='Protomaps / PMTiles')await within(controls).findByRole('textbox',{name:'PMTiles archive URL'});
    await userEvent.click(screen.getByRole('link',{name:'Home',exact:true}));
    await screen.findByRole('heading',{name:'What geographic data do you need?'});
  }
  const commands=window.__TAURI__.core.invoke.mock.calls.map(([command])=>command);
  expect(commands.length).toBeGreaterThan(0);
  expect(commands.every(command=>command.startsWith('list_')||['identity_snapshot','distribution_snapshot','activate_desktop_frame','set_desktop_appearance'].includes(command))).toBe(true);
  expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Keep this draft');expect(search).not.toHaveBeenCalled();
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
},20000);

it.each([
  ['Local PMTiles','Offline tiles','tiles'],['Local MBTiles','Offline tiles','tiles'],
  ['GeoJSON / Overpass JSON','Vector files','vectors'],['GeoPackage','Vector files','vectors'],
  ['Shapefile / ZIP','Vector files','vectors'],['OSM XML / PBF','Vector files','vectors'],
])('opens the existing 2D file library from %s without importing or sending a message',async(name,view,key)=>{
  mount();await screen.findByRole('heading',{name:'What geographic data do you need?'});
  await chooseHomeSource(SOURCE_DIRECTORY.find(source=>source.name===name).id);
  await screen.findByRole('heading',{name:'My Data'});
  expect(location.hash).toBe(`#My%20Data?view=${key}`);
  expect(screen.getByRole('radio',{name:view}).getAttribute('aria-checked')).toBe('true');
  expect(screen.queryByRole('radio',{name:'3D assets'})).toBeNull();
  expect(search).not.toHaveBeenCalled();
  expect(window.__TAURI__.core.invoke.mock.calls.every(([command])=>command.startsWith('list_')||['identity_snapshot','distribution_snapshot','activate_desktop_frame','set_desktop_appearance'].includes(command))).toBe(true);
  expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
});
