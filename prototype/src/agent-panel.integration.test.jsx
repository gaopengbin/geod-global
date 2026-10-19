import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { IdentityEntry, IdentityProvider } from './identity-ui.jsx';
import { AgentPanel } from './agent-panel.jsx';
import { agentRequest, subscribeAgentUpdates } from './agent-client.js';
import { runtimeRequest } from './runtime-client.js';
import { MODEL_CAPABILITIES } from './agent-model-registry.js';
import officeRecords from '../../agent/fixtures/office-records.json';
import audioRecords from '../../agent/fixtures/audio-records.json';
import videoRecords from '../../agent/fixtures/video-records.json';
import {startToolRecord,finishToolRecord} from '../../agent/tool-record.mjs';
vi.mock('./agent-client.js',()=>({agentRequest:vi.fn(),subscribeAgentUpdates:vi.fn().mockResolvedValue(()=>{})}));
vi.mock('./runtime-client.js',async importOriginal=>({...await importOriginal(),runtimeRequest:vi.fn().mockResolvedValue(undefined),desktopAvailable:()=>true,syncDesktopLocale:vi.fn().mockResolvedValue(undefined)}));
const id='a1234567-1234-1234-1234-123456789abc', projectId='b1234567-1234-1234-1234-123456789abc';
let backend;
const base=()=>({version:1,revision:1,runtimeAvailable:true,mode:'read-only',configured:true,busy:false,
  model:{label:'Development connection',model:'test-model',protocol:'openai-compatible',baseUrl:'https://example.com/v1'},sessions:[],selected:null});
const props=()=>({onClose:vi.fn(),onOpenProject:vi.fn(),onOpenTasks:vi.fn()});
function mount(p=props()){render(<I18nProvider><AgentPanel {...p}/></I18nProvider>);return p;}
async function openDownloadDetail(){
  const group=await screen.findByRole('region',{name:'Download tasks'});
  await userEvent.click(within(group).getByRole('button',{name:'Download details · 1 plans'}));
  await userEvent.click(group.querySelector('.agent-download-item > .bui-disclosure-trigger'));
}
it('merges submitted downloads across tool calls into one compact group and preserves its expansion when the execution process is folded',async()=>{
  const planIds=['c1234567-1234-1234-1234-123456789abc','d1234567-1234-1234-1234-123456789abc'];
  backend.selected={id,status:'completed',workflow:{status:'waiting'},entries:[{id:'request',type:'user',text:'Download originals',status:'completed'},...planIds.map((planId,index)=>({id:'tool-'+index,type:'tool',turnId:'turn-'+index,name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId}]}))]};
  backend.sessions=[{id,title:'Download originals',status:'completed',compatible:true}];
  backend.plans=planIds.map((planId,index)=>({planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Earth Search',project:{id:projectId,name:'NYC',mode:'existing'},bounds:[-74,40,-73,41],expectedBytes:null,files:Array.from({length:3},(_,i)=>({itemId:'Scene '+i,assetKey:index?'scl':'visual',bytes:null})),notes:[],jobs:Array.from({length:3},(_,i)=>({id:'file-'+index+'-'+i,title:'Scene '+i,status:index||i?'queued':'running',settled:false,bytesDownloaded:100,totalBytes:null}))}));
  const p=mount();const downloads=await screen.findByRole('region',{name:'Download tasks'});
  expect(screen.getAllByRole('region',{name:'Download tasks'})).toHaveLength(1);
  expect(within(downloads).getByText('0/6 files ready')).toBeTruthy();
  expect(screen.queryByRole('region',{name:'Complete download task'})).toBeNull();
  expect(screen.queryByRole('region',{name:'Task progress list'})).toBeNull();
  await userEvent.click(within(downloads).getByRole('button',{name:'Download details · 2 plans'}));
  const visual=within(downloads).getByRole('button',{name:/NYC · VISUAL/});
  expect(visual.getAttribute('aria-expanded')).toBe('false');
  await userEvent.click(screen.getByRole('button',{name:/^Hide execution process/}));
  expect(within(downloads).getByRole('button',{name:'Download details · 2 plans'}).getAttribute('aria-expanded')).toBe('true');
  await userEvent.click(visual);await screen.findByRole('region',{name:'Complete download task'});
  await userEvent.click(within(downloads).getByRole('button',{name:/NYC · SCL/}));
  expect(screen.getAllByRole('region',{name:'Complete download task'})).toHaveLength(1);
  expect(screen.getAllByRole('region',{name:'Task progress list'})).toHaveLength(1);
  expect(agentRequest.mock.calls.every(([op])=>op==='snapshot')).toBe(true);
});

it('keeps failed download recovery and a separate pending confirmation visible while the process is collapsed',async()=>{
  const planId='c1234567-1234-1234-1234-123456789abc',pendingId='d1234567-1234-1234-1234-123456789abc';
  backend.selected={id,status:'completed',entries:[{id:'request',type:'user',text:'Download this area',status:'completed'},{id:'plans',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId},{kind:'plan',id:pendingId}]},{id:'answer',type:'assistant',phase:'final',status:'completed',text:'Please confirm the other download.'}]};
  backend.sessions=[{id,title:'Download this area',status:'completed',compatible:true}];
  const basePlan={kind:'download',planHash:'a'.repeat(64),source:'Earth Search',bounds:[-74,40,-73,41],expectedBytes:2000,files:[{itemId:'Scene',assetKey:'scl',bytes:2000}],notes:[]};
  backend.plans=[{...basePlan,planId,status:'submitted',jobs:[{id:projectId,status:'failed',settled:true,bytesDownloaded:100,totalBytes:2000,title:'Failed source'}]},{...basePlan,planId:pendingId,status:'pending',jobs:[]}];
  const p=mount();await screen.findByText('1 downloads need attention');
  expect(screen.getByRole('button',{name:'Confirm download'}).disabled).toBe(false);
  expect(screen.queryByRole('region',{name:'Task progress list'})).toBeNull();
  await userEvent.click(screen.getByRole('button',{name:'View failed task'}));
  expect(p.onOpenTasks).toHaveBeenCalledWith(projectId);
  expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
});

it('reveals the exact completed download from the results pane through both collapsed task levels without submitting anything',async()=>{
  const rect=vi.spyOn(HTMLElement.prototype,'getBoundingClientRect').mockReturnValue({width:1280,height:900,top:0,left:0,bottom:900,right:1280,x:0,y:0,toJSON:()=>({})});
  try{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'user',type:'user',status:'completed',text:'Download SCL'}, {id:'native',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId}]},{id:'summary',type:'assistant',phase:'final',status:'completed',text:'Original downloaded.'}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Earth Search',bounds:[-74,40,-73,41],expectedBytes:2000,files:[{itemId:'Scene',assetKey:'scl',bytes:2000}],notes:[],jobs:[{id:projectId,title:'Verified SCL',status:'succeeded',settled:true,bytesDownloaded:2000,totalBytes:2000}]}];
    mount({...props(),variant:'home'});await userEvent.click(await screen.findByRole('button',{name:/First history/}));const results=await screen.findByRole('region',{name:'Conversation results'});
    expect(screen.queryByRole('region',{name:'Download tasks'})).toBeNull();
    const viewport=screen.getByRole('region',{name:'Conversation messages'});viewport.scrollTo=vi.fn();
    await userEvent.click(within(results).getByRole('button',{name:'View result task'}));
    await waitFor(()=>expect(document.activeElement.dataset.planId).toBe(planId));
    expect(screen.getByRole('region',{name:'Complete download task'})).toBeTruthy();
    expect(agentRequest.mock.calls.every(([op])=>['snapshot','select'].includes(op))).toBe(true);
  }finally{rect.mockRestore();}
});
async function moreOption(name){
  if(!screen.queryByRole('button',{name}))await userEvent.click(await screen.findByRole('button',{name:'Conversation options'}));
  return screen.findByRole('button',{name});
}
async function toolButton(name){
  if(!screen.queryByRole('button',{name})){
    const work=await screen.findByRole('region',{name:/Execution records|执行记录/});
    const toggle=work.querySelector('.agent-work-records-trigger');
    if(toggle.getAttribute('aria-expanded')==='false')await userEvent.click(toggle);
    for(const group of work.querySelectorAll('.agent-activity-trigger')){
      if(group.getAttribute('aria-expanded')==='false')await userEvent.click(group);
    }
  }
  return screen.findByRole('button',{name});
}
function registryBackend(){
  const connections=[{id:'d1234567-1234-1234-1234-123456789abc',provider:'openai',label:'OpenAI saved',model:'first-model',baseUrl:'https://api.openai.com/v1'},
    {id:'e1234567-1234-1234-1234-123456789abc',provider:'deepseek',label:'DeepSeek saved',model:'second-model',baseUrl:'https://api.deepseek.com'}]
    .map(connection=>({...connection,protocol:'openai-compatible',capabilities:{...MODEL_CAPABILITIES},verification:'not-verified'}));
  return {...base(),model:connections[0],registry:{version:1,selectedId:connections[0].id,connections},
    selected:{id,status:'completed',entries:[{id:'answer',type:'assistant',status:'completed',text:'First connection answer'}]},
    sessions:[{id,title:'First history',status:'completed',compatible:true,modelConnectionId:connections[0].id,modelProvider:'openai',modelLabel:connections[0].label,modelId:'first-model'}]};
}
beforeEach(()=>{const storage=new Map([['geod-global-locale','en']]);Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value)}});vi.clearAllMocks();subscribeAgentUpdates.mockResolvedValue(()=>{});backend=base();agentRequest.mockImplementation(async operation=>structuredClone(backend));});
afterEach(cleanup);

it('shows one request usage footer with actual subset counts and incomplete receipts without calling the model',async()=>{
  const userId='c1234567-1234-1234-1234-123456789abc';
  backend.sessions=[{id,title:'Usage',status:'completed',compatible:true}];
  backend.selected={id,status:'completed',entries:[{id:userId,type:'user',status:'completed',text:'Check imagery'},
    {id:'lookup',type:'tool',name:'geod_sources_list',status:'completed',turnId:'turn',references:[]},
    {id:'final',type:'assistant',phase:'final',status:'completed',turnId:'turn',text:'Actual saved answer'}],
    usage:{version:1,requests:2,records:[{id:'resp_'+'a'.repeat(32),requestId:userId,turnId:'turn',origin:'human',state:'completed',context:{toolCount:12},usage:{input_tokens:1000,output_tokens:100,total_tokens:1100,input_tokens_details:{cached_tokens:800},output_tokens_details:{reasoning_tokens:60}}},
      {id:'resp_'+'b'.repeat(32),requestId:userId,turnId:'turn',origin:'workflow',state:'failed',context:{toolCount:8},usage:{input_tokens:null,output_tokens:null,total_tokens:null,input_tokens_details:{cached_tokens:null},output_tokens_details:{reasoning_tokens:null}}}]}};
  mount();await screen.findByText('Actual saved answer');
  const button=screen.getByRole('button',{name:'Token usage for this request'});
  expect(button.closest('.agent-message').textContent).toContain('Actual saved answer');
  const before=agentRequest.mock.calls.length;await userEvent.click(button);
  await screen.findByText('1,100');expect(screen.getByText('800')).toBeTruthy();expect(screen.getByText('60')).toBeTruthy();
  expect(screen.getByText('1 of 2 model requests have actual usage.')).toBeTruthy();
  expect(screen.getByText('Only known usage is shown; incomplete or older receipts may be missing.')).toBeTruthy();
  expect(agentRequest.mock.calls.slice(before).every(([op])=>op==='snapshot')).toBe(true);
  await userEvent.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByText('1,100')).toBeNull());
});
describe('Agent sidebar',()=>{
  it('keeps the entrance transcript and its results visible while another scene runs, then restores native selection before actions',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});
    const explore={id,status:'completed',lastScene:{kind:'workspace',page:'Explore'},entries:[{id:'u-explore',type:'user',status:'completed',text:'Download San Francisco SCL'},
      {id:'a-explore',type:'assistant',status:'completed',text:'San Francisco verified result'}]};
    const home={id:projectId,status:'running',lastScene:{kind:'conversation',page:'Home'},entries:[{id:'u-home',type:'user',status:'completed',text:'Download Manhattan SCL'}]};
    const planId='c1234567-1234-1234-1234-123456789abc';
    explore.entries.unshift({id:'native-explore',type:'tool',status:'completed',references:[{kind:'plan',id:planId}]});
    const plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Native source',bounds:[-123,37,-122,38],expectedBytes:null,notes:[],files:[{itemId:'SF scene',assetKey:'scl',bytes:null}],jobs:[{id:'f1234567-1234-1234-1234-123456789abc',status:'succeeded',settled:true,title:'San Francisco crop',bytesDownloaded:2000}]}];
    backend.selected=explore;backend.sessions=[{...explore,entries:undefined,compatible:true,title:'Explore'},{...home,entries:undefined,compatible:true,title:'Home'}];backend.plans=plans;
    agentRequest.mockImplementation(async(operation,args)=>{if(operation==='select')backend.selected=explore;return structuredClone(backend);});
    const panelProps={...props(),scene:{kind:'workspace',page:'Explore'},preferredSessionId:id,sessionViewStore:{current:new Map()}};
    mount(panelProps);
    await screen.findByText('San Francisco verified result');
    await userEvent.click(screen.getByRole('button',{name:'Show conversation results'}));
    await screen.findByRole('article',{name:'San Francisco crop'});
    backend.selected=home;backend.plans=[];backend.busy=true;backend.activity={sessionId:projectId,kind:'turn',turnId:'home-live'};backend.revision++;notify();
    await screen.findByText('Another conversation is running.');
    expect(screen.getByText('San Francisco verified result')).toBeTruthy();
    expect(screen.queryByText('Download Manhattan SCL')).toBeNull();
    expect(screen.getByRole('article',{name:'San Francisco crop'})).toBeTruthy();
    expect(agentRequest.mock.calls.some(([op])=>op==='select')).toBe(false);
    expect(screen.getByRole('button',{name:'Send message'}).disabled).toBe(true);
    cleanup();mount(panelProps);
    await screen.findByText('Another conversation is running.');
    expect(screen.getByText('San Francisco verified result')).toBeTruthy();
    expect(screen.queryByText('Download Manhattan SCL')).toBeNull();
    backend.busy=false;backend.activity=null;home.status='completed';backend.revision++;notify();
    await waitFor(()=>expect(agentRequest).toHaveBeenCalledWith('select',{id}));
    await screen.findByText('San Francisco verified result');
  });
  it('saves a custom context capacity, restores it per connection and can return to automatic without replacing the key',async()=>{
    backend=registryBackend();backend.model.contextWindow=131072;backend.registry.connections[1].contextWindow=65536;
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='saveModel') {
        backend.model.contextWindow=args.request.contextWindow;
        backend.registry.connections[0].contextWindow=args.request.contextWindow;
      }
      return structuredClone(backend);
    });
    mount();await userEvent.click(await moreOption('Agent model connection'));
    const input=screen.getByLabelText('Context size (tokens)');expect(input.value).toBe('131072');
    await userEvent.click(screen.getByRole('combobox',{name:'Saved connection'}));
    await userEvent.click(screen.getByRole('option',{name:'second-model',exact:true}));
    expect(input.value).toBe('65536');
    await userEvent.click(screen.getByRole('combobox',{name:'Saved connection'}));
    await userEvent.click(screen.getByRole('option',{name:'first-model',exact:true}));
    fireEvent.change(input,{target:{value:'2097152'}});
    await userEvent.click(screen.getByRole('button',{name:'Save connection'}));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    expect(agentRequest).toHaveBeenCalledWith('saveModel',{request:expect.objectContaining({id:backend.model.id,contextWindow:2097152,apiKey:''})});
    await userEvent.click(await moreOption('Agent model connection'));
    expect(screen.getByLabelText('Context size (tokens)').value).toBe('2097152');
    fireEvent.change(screen.getByLabelText('Context size (tokens)'),{target:{value:''}});
    await userEvent.click(screen.getByRole('button',{name:'Save connection'}));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    expect(agentRequest.mock.calls.filter(([operation])=>operation==='saveModel').at(-1)).toEqual(['saveModel',{request:expect.objectContaining({contextWindow:null,apiKey:''})}]);
    expect(backend.sessions[0].compatible).toBe(true);expect(backend.registry.connections[1].contextWindow).toBe(65536);
  });
  it('uses official automatic capacity, explains unknown aliases and prevents invalid capacity from being saved or tested',async()=>{
    backend=registryBackend();backend.model.provider='deepseek';backend.model.baseUrl='https://api.deepseek.com';backend.model.model='deepseek-flash';
    mount();await userEvent.click(await moreOption('Agent model connection'));
    const input=screen.getByLabelText('Context size (tokens)');expect(input.placeholder).toBe('Automatic · 1,048,576');
    expect(screen.getByText(/Effective context: 1,048,576/)).toBeTruthy();
    fireEvent.change(input,{target:{value:'8191'}});
    fireEvent.submit(input.closest('form'));
    await userEvent.click(screen.getByRole('button',{name:'Test connection'}));
    expect(agentRequest.mock.calls.some(([op])=>['saveModel','testModel'].includes(op))).toBe(false);
    fireEvent.change(input,{target:{value:'131072.5'}});fireEvent.submit(input.closest('form'));
    expect(agentRequest.mock.calls.some(([op])=>op==='saveModel')).toBe(false);
    fireEvent.change(input,{target:{value:''}});
    fireEvent.change(screen.getByLabelText('API endpoint'),{target:{value:'https://gateway.test/v1'}});
    expect(input.placeholder).toBe('Automatic · 32,768');expect(screen.getByText(/Model capacity is unknown/)).toBeTruthy();
  });
  it('keeps an unfinished turn expanded through completed intermediate messages and compaction, with one current activity',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});
    backend=registryBackend();backend.busy=true;backend.selected.status='running';backend.activity={sessionId:id,kind:'turn',turnId:'queens-turn'};
    backend.selected.entries=[{id:'u',type:'user',status:'completed',text:'Find Queens imagery'},
      {id:'boundary',type:'tool',turnId:'queens-turn',name:'geod_boundary_read',status:'completed',references:[]},
      {id:'note',type:'assistant',turnId:'queens-turn',status:'completed',text:'Boundary resolved. Now searching imagery.'}];
    mount();const group=await screen.findByRole('region',{name:'Execution records'}),toggle=group.querySelector('.agent-work-records-trigger');
    expect(toggle.getAttribute('aria-expanded')).toBe('true');expect(group.textContent).toContain('Boundary resolved.');
    const activity=screen.getByRole('status',{name:'Current activity'});
    expect(screen.getAllByText('Waiting for response…')).toHaveLength(1);
    expect(activity.closest('.agent-composer-area')).toBeNull();
    expect(group.contains(activity)).toBe(true);expect(toggle.hidden).toBe(false);
    expect(screen.getByText('Boundary resolved. Now searching imagery.').closest('.agent-work-records')).toBe(group);
    backend.selected.contextState={status:'organizing',count:0};backend.revision++;notify();
    await waitFor(()=>expect(screen.getByRole('status',{name:'Current activity'}).textContent).toContain('Organizing context'));
    expect(document.querySelectorAll('.agent-composer-area .agent-loading')).toHaveLength(0);expect(toggle.getAttribute('aria-expanded')).toBe('true');
    backend.selected.contextState.status='ready';backend.selected.entries.push({id:'search',type:'tool',turnId:'queens-turn',name:'geod_scene_search',status:'running',references:[]});backend.revision++;notify();
    await screen.findByRole('button',{name:'Checking 1 resources'});
    expect(screen.queryByRole('status',{name:'Current activity'})).toBeNull();expect(toggle.getAttribute('aria-expanded')).toBe('true');
    backend.busy=false;backend.activity=null;backend.selected.status='completed';backend.selected.entries[2].phase='progress';backend.selected.entries[3].status='completed';
    backend.selected.entries.push({id:'answer',type:'assistant',turnId:'queens-turn',phase:'final',status:'completed',text:'Queens imagery search finished.'});backend.revision++;notify();
    await screen.findByText('Queens imagery search finished.');await waitFor(()=>expect(toggle.getAttribute('aria-expanded')).toBe('false'));
    expect(screen.getByText('Find Queens imagery').closest('.agent-work-records')).toBeNull();expect(screen.queryByRole('status',{name:'Current activity'})).toBeNull();
    backend.busy=true;backend.activity={sessionId:id,kind:'compaction',turnId:null};backend.selected.contextState.status='organizing';backend.revision++;notify();
    await screen.findByRole('status',{name:'Current activity'});expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('keeps streamed commentary inside one turn, closes it on completion and renders the final Markdown response separately',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});
    backend=registryBackend();backend.busy=true;backend.selected.status='running';
    backend.selected.entries=[{id:'user',type:'user',status:'completed',text:'Find the requested imagery'},
      {id:'note',type:'assistant',phase:'progress',turnId:'turn-1',status:'running',text:'Checking native coverage'}];
    mount();const group=await screen.findByRole('region',{name:'Execution records'});
    expect(group.textContent).toContain('Checking native coverage');expect(group.querySelector('.bui-message-text').dataset.streaming).toBe('true');
    expect(group.querySelector('.agent-work-label')).toBeNull(); // Commentary has no formal reply header.
    backend.selected.entries[1].status='completed';backend.selected.entries.push({id:'tool',type:'tool',turnId:'turn-1',name:'geod_scene_coverage',status:'completed',references:[]});backend.revision++;notify();
    await screen.findByText('Checked 1 resources');expect(screen.getAllByRole('region',{name:'Execution records'})).toHaveLength(1);
    backend.busy=false;backend.selected.status='completed';backend.selected.entries.push({id:'final',type:'assistant',phase:'final',turnId:'turn-1',status:'completed',text:'### Coverage result\n\nThe requested extent is **incomplete**.\n\n| Item | Result |\n| --- | --- |\n| Coverage | 86% |'});backend.revision++;notify();
    await screen.findByRole('heading',{name:'Coverage result'});await waitFor(()=>expect(within(group).getByRole('button',{name:/View execution process|Processing complete|Execution records/}).getAttribute('aria-expanded')).toBe('false'));
    await waitFor(()=>expect(screen.queryByText('Checking native coverage')).toBeNull());expect(screen.getByRole('table').textContent).toContain('86%');
    const userMessage=screen.getByText('Find the requested imagery');
    expect(userMessage.closest('.agent-work-records')).toBeNull();expect(userMessage.closest('[data-from="user"]').querySelector('[data-slot="message-header"]')).toBeNull();
    expect(screen.queryByText('You',{exact:true})).toBeNull();expect(screen.getByRole('heading',{name:'Coverage result'}).closest('.agent-work-records')).toBeNull();
    expect(screen.queryByRole('status',{name:'Current activity'})).toBeNull();expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('ports the ordered GeoD Agent timeline and keeps safe native details inside its tool disclosure',async()=>{
    backend=registryBackend();
    backend.selected.entries=[{id:'request',type:'user',status:'completed',text:'Find imagery for Manhattan'},
      {id:'region',type:'tool',status:'completed',name:'geod_region_search',references:[],summary:{kind:'places',count:1,total:1},
        record:finishToolRecord(startToolRecord({query:'Manhattan',countryCode:'USA'},0),{candidates:[{name:'New York County',countryCode:'USA',adminLevel:2}],matchCount:1},750)},
      {id:'note',type:'assistant',status:'completed',text:'Internal progress note in another language'},
      {id:'coverage',type:'tool',status:'completed',name:'geod_scene_coverage',references:[],record:finishToolRecord(startToolRecord({searchId:projectId},0),{coverage:{status:'partial',coveredFraction:0.8,sceneCount:2}},100)},
      {id:'reply',type:'assistant',status:'completed',text:'More scenes are required before preparing a download.'}];
    mount();await userEvent.click(await screen.findByRole('button',{name:/View execution process|Processing complete|Execution records/}));
    expect(screen.getByText('Internal progress note in another language')).toBeTruthy();
    for(const group of document.querySelectorAll('.agent-activity-trigger'))await userEvent.click(group);
    const timeline=document.querySelector('.agent-work-flow');
    expect(timeline.textContent.indexOf('Find administrative area')).toBeLessThan(timeline.textContent.indexOf('Internal progress note'));
    expect(timeline.textContent.indexOf('Internal progress note')).toBeLessThan(timeline.textContent.indexOf('Check entire area coverage'));
    await userEvent.click(screen.getByRole('button',{name:/Find administrative area/}));
    expect(screen.getByRole('region',{name:'Input summary data'}).textContent).toContain('Manhattan');
    expect(screen.getByRole('region',{name:'Result excerpt data'}).textContent).toContain('New York County');
    expect(screen.queryByRole('button',{name:/Agent notes/})).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('uses one running activity group, preserves inspected details after completion and marks stopped tools honestly',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});
    backend=registryBackend();backend.busy=true;backend.selected.status='running';
    backend.selected.entries=[{id:'request',type:'user',status:'completed',text:'Find actual imagery'},
      {id:'read',type:'tool',status:'completed',name:'geod_sources_list',references:[],summary:{kind:'read'}},
      {id:'search',type:'tool',status:'running',name:'geod_scene_search',references:[],record:startToolRecord({bounds:[1,2,3,4]},0)}];
    mount();const records=await screen.findByRole('region',{name:'Execution records'});
    const group=within(records).getByRole('button',{name:'Checking 2 resources'});
    expect(screen.queryByRole('status',{name:'Current activity'})).toBeNull();
    await userEvent.click(group);await userEvent.click(await screen.findByRole('button',{name:/Search imagery/}));
    expect(screen.getByRole('region',{name:'Input summary data'})).toBeTruthy();
    backend.busy=false;backend.selected.status='completed';backend.selected.entries[2].status='completed';
    backend.selected.entries.push({id:'answer',type:'assistant',phase:'final',text:'Search complete',status:'completed'});backend.revision++;notify();
    await screen.findByText('Search complete');expect(records.querySelector('.agent-work-records-trigger').getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('region',{name:'Input summary data'})).toBeTruthy();
    cleanup();backend.selected.status='interrupted';backend.selected.entries=[{id:'stopped',type:'tool',name:'geod_scene_search',status:'running',references:[]}];mount();
    await toolButton(/Search imagery/);await screen.findByText('Tool stopped or could not finish.');expect(screen.queryByRole('status',{name:'Current activity'})).toBeNull();
    expect(document.querySelector('.agent-tool[data-status="interrupted"] .bui-spinner')).toBeNull();
  });
  it('keeps left conversations and right deliveries while revealing only the completed result task in its own activity',async()=>{
    const rect=vi.spyOn(HTMLElement.prototype,'getBoundingClientRect').mockReturnValue({width:1280,height:900,top:0,left:0,bottom:900,right:1280,x:0,y:0,toJSON:()=>({})});
    try{
      const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
      backend.selected.goal={id:'verified-goal',objective:'Deliver the requested crop',status:'complete',planIds:[planId],outputs:[{id:'crop',label:'Final crop',state:'verified',planId,verifiedIds:[projectId]}]};
      backend.selected.entries=[{id:'request',type:'user',status:'completed',text:'Crop the requested area'}, {id:'native',type:'tool',name:'geod_clip_plan',status:'completed',references:[{kind:'plan',id:planId}]}, {id:'summary',type:'assistant',status:'completed',text:'The requested crop has been delivered.'}];
      backend.plans=[{planId,planHash:'a'.repeat(64),kind:'clip',status:'submitted',source:'Verified local raster',bounds:[-74,40,-73,41],expectedBytes:null,files:[{itemId:'Native file',assetKey:'visual',bytes:null,width:100,height:200}],notes:[],jobs:[{id:projectId,status:'succeeded',settled:true,title:'Delivered crop',bytesDownloaded:2000,totalBytes:2000}]}];
      const p=mount({...props(),variant:'home',onOpenResult:vi.fn()});
      await userEvent.click(await screen.findByRole('button',{name:/First history/}));
      const history=screen.getByRole('complementary',{name:'Conversation list'}),results=screen.getByRole('region',{name:'Conversation results'});
      expect(history.closest('.agent-workspace-layout')).toBeTruthy();expect(results.closest('.agent-conversation')).toBeNull();
      expect(screen.queryByRole('region',{name:'Plan progress'})).toBeNull();
      await userEvent.click(screen.getByRole('button',{name:'Show plan progress'}));
      expect(screen.getByRole('region',{name:'Plan progress'})).toBeTruthy();
      await userEvent.click(screen.getAllByRole('button',{name:'Hide plan progress'})[0]);
      expect(screen.queryByRole('region',{name:'Plan progress'})).toBeNull();
      expect(screen.getByText('The requested crop has been delivered.').closest('.agent-work-records')).toBeNull();
      expect(screen.getByText('Crop the requested area')).toBeTruthy();expect(screen.queryByRole('button',{name:'Confirm crop'})).toBeNull();
      expect(within(results).getByRole('link',{name:'Open in workspace'}).getAttribute('href')).toBe('#Workspace?file='+projectId);
      await userEvent.click(within(results).getByRole('button',{name:'Preview result'}));expect(p.onOpenResult).toHaveBeenCalledWith({kind:'raster',id:projectId,verified:true});
      const viewport=screen.getByRole('region',{name:'Conversation messages'});viewport.scrollTo=vi.fn();
      await userEvent.click(within(results).getByRole('button',{name:'View result task'}));
      await screen.findByText('Crop the requested area');await waitFor(()=>expect(document.activeElement.dataset.planId).toBe(planId));
      expect(viewport.scrollTo).toHaveBeenCalled();expect(within(screen.getByRole('region',{name:'Conversation messages'})).queryByRole('link',{name:'Open in workspace'})).toBeNull();
      await userEvent.click(screen.getByRole('button',{name:/Processing complete|Execution records/}));
      await waitFor(()=>expect(document.querySelector('[data-plan-id]')).toBeNull());
      expect(screen.getByText('Crop the requested area')).toBeTruthy();
      expect(agentRequest.mock.calls.some(([op])=>['approvePlan','send','executionMode'].includes(op))).toBe(false);
    }finally{rect.mockRestore();}
  });
  it('clears previous deliveries on conversation switching and never substitutes another conversation\'s submitted plans',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',otherId='f1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries.unshift({id:'native',type:'tool',status:'completed',references:[{kind:'plan',id:planId}]});
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Native source',bounds:[-74,40,-73,41],expectedBytes:null,notes:[],files:[{itemId:'Native scene',assetKey:'visual',bytes:null}],jobs:[{id:projectId,status:'succeeded',settled:true,title:'First result',bytesDownloaded:2000}]}];
    backend.sessions.push({id:otherId,title:'Second history',compatible:true,status:'completed'});
    agentRequest.mockImplementation(async(operation,args)=>{if(operation==='select'&&args.id===otherId)backend.selected={id:otherId,status:'completed',entries:[{id:'reply',type:'assistant',status:'completed',text:'Second conversation, no files yet.'}]};return structuredClone(backend);});
    mount();await userEvent.click(await screen.findByRole('button',{name:'Show conversation results'}));await screen.findByRole('article',{name:'First result'});
    fireEvent.keyDown(screen.getByRole('complementary',{name:'Conversation results sidebar'}),{key:'Escape'});
    await userEvent.click(screen.getByRole('button',{name:'Conversation history'}));await userEvent.click(screen.getByRole('button',{name:/Second history/}));
    await screen.findByText('Second conversation, no files yet.');expect(screen.queryByRole('button',{name:'Show conversation results'})).toBeNull();expect(screen.queryByRole('article',{name:'First result'})).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('keeps new requests, pending choices and running tasks visible after an earlier completed delivery',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});backend=registryBackend();
    backend.selected.goal={status:'complete',objective:'Deliver a file',outputs:[{id:'file',state:'verified'}]};
    backend.selected.entries.unshift({id:'request',type:'user',status:'completed',text:'Earlier request'}, {id:'progress',type:'assistant',phase:'progress',turnId:'earlier',status:'completed',text:'Earlier execution progress'});
    mount();const earlier=await screen.findByRole('region',{name:'Execution records'});expect(screen.getByText('Earlier request')).toBeTruthy();
    expect(screen.getByText('First connection answer')).toBeTruthy();expect(earlier.querySelector('[data-slot="disclosure-trigger"]').getAttribute('aria-expanded')).toBe('false');
    backend.selected.entries.push({id:'followup',type:'user',status:'completed',text:'Now do the next area'});backend.busy=true;backend.selected.status='running';backend.revision++;notify();
    await screen.findByText('Now do the next area');expect(screen.getByText('Earlier request')).toBeTruthy();expect(screen.getByText('First connection answer')).toBeTruthy();
    expect(earlier.querySelector('[data-slot="disclosure-trigger"]').getAttribute('aria-expanded')).toBe('false');
  });
  it('preserves the greeting and all final replies, with independent disclosures in each request position',async()=>{
    backend=registryBackend();backend.selected.goal={status:'complete',objective:'Deliver a file',outputs:[{id:'file',state:'verified'}]};
    backend.selected.entries=[
      {id:'hello',type:'user',status:'completed',text:'Hello'},
      {id:'greeting',type:'assistant',turnId:'greeting',phase:'final',status:'completed',text:'Hello, what imagery do you need?'},
      {id:'request',type:'user',status:'completed',text:'Use the actual administrative boundary'},
      {id:'progress',type:'assistant',turnId:'first',phase:'progress',status:'completed',text:'The boundary is now resolved'},
      {id:'first-summary',type:'assistant',turnId:'first',phase:'final',status:'completed',text:'The first requested area is ready'},
      {id:'followup',type:'user',status:'completed',text:'Also keep the original resolution'},
      {id:'second-progress',type:'assistant',turnId:'second',phase:'progress',status:'completed',text:'Checking the original grid'},
      {id:'summary',type:'assistant',turnId:'second',phase:'final',status:'completed',text:'The requested file is ready'}
    ];
    mount();const groups=await screen.findAllByRole('region',{name:'Execution records'});
    const toggles=groups.map(group=>group.querySelector('[data-slot="disclosure-trigger"]'));
    expect(groups).toHaveLength(2);expect(toggles.map(button=>button.getAttribute('aria-expanded'))).toEqual(['false','false']);
    expect(screen.getByText('Hello, what imagery do you need?')).toBeTruthy();expect(screen.getByText('The first requested area is ready')).toBeTruthy();
    expect(screen.getByText('Use the actual administrative boundary')).toBeTruthy();
    expect(screen.getByText('Also keep the original resolution')).toBeTruthy();
    expect(screen.getByText('The requested file is ready')).toBeTruthy();
    expect(screen.queryByText('The boundary is now resolved')).toBeNull();
    await userEvent.click(toggles[0]);expect(toggles[1].getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('Checking the original grid')).toBeNull();
    await userEvent.click(toggles[1]);
    const messages=Array.from(document.querySelectorAll('.agent-conversation .agent-message')).map(node=>node.querySelector('.agent-message-body').textContent);
    expect(messages.map(text=>backend.selected.entries.findIndex(entry=>text===entry.text))).toEqual([0,1,2,4,5,7]);
    const displayed=document.querySelector('.agent-transcript-stage').textContent;
    expect(displayed.indexOf('The boundary is now resolved')).toBeLessThan(displayed.indexOf('The first requested area is ready'));
    expect(displayed.indexOf('Also keep the original resolution')).toBeLessThan(displayed.indexOf('Checking the original grid'));
    await userEvent.click(toggles[0]);
    expect(screen.queryByText('The boundary is now resolved')).toBeNull();
    expect(screen.getByText('Also keep the original resolution')).toBeTruthy();
    expect(screen.getByText('Checking the original grid')).toBeTruthy();expect(screen.getByText('Hello, what imagery do you need?')).toBeTruthy();
    expect(agentRequest.mock.calls.some(([op])=>['approvePlan','send','executionMode'].includes(op))).toBe(false);
  });
  it('keeps choices and approval cards visible when folded and restores their exact chronology when expanded',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'request',type:'user',text:'Prepare this area',status:'completed'},
      {id:'before',type:'assistant',turnId:'turn',phase:'progress',text:'Before the review',status:'completed'},
      {id:'plan',type:'tool',turnId:'turn',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId}]},
      {id:'decision',type:'tool',turnId:'turn',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'Choose the output',status:'pending',questions:[{id:'output',prompt:'Which output?',options:[{id:'visual',label:'Visual',description:'True color.'},{id:'scl',label:'SCL',description:'Quality classification.'}]}]}},
      {id:'after',type:'assistant',turnId:'turn',phase:'progress',text:'After the choice card',status:'completed'},
      {id:'final',type:'assistant',turnId:'turn',phase:'final',text:'Please review the prepared task.',status:'completed'}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Native source',bounds:[1,2,3,4],expectedBytes:null,files:[{itemId:'Native scene',assetKey:'visual',bytes:null}],notes:[],jobs:[]}];
    mount();const group=await screen.findByRole('region',{name:'Execution records'}),toggle=group.querySelector('[data-slot="disclosure-trigger"]');
    expect(toggle.getAttribute('aria-expanded')).toBe('false');expect(screen.getByRole('button',{name:'Submit choices'})).toBeTruthy();
    expect(screen.getByRole('button',{name:'Confirm download'}).disabled).toBe(true);expect(screen.queryByText('Before the review')).toBeNull();
    await userEvent.click(toggle);const text=group.textContent;
    expect(text.indexOf('Before the review')).toBeLessThan(text.indexOf('Complete download task'));
    expect(text.indexOf('Complete download task')).toBeLessThan(text.indexOf('Choose the output'));
    expect(text.indexOf('Choose the output')).toBeLessThan(text.indexOf('After the choice card'));
    await userEvent.click(toggle);expect(screen.queryByText('After the choice card')).toBeNull();
    expect(screen.getByText('Please review the prepared task.').closest('.agent-work-records')).toBeNull();
    expect(screen.getByRole('button',{name:'Submit choices'})).toBeTruthy();expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('keeps a pending review referenced by the current reply usable without expanding the process',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'request',type:'user',status:'completed',text:'Prepare this download'},
      {id:'reply',type:'assistant',phase:'final',status:'completed',text:'Please confirm this download.',references:[{kind:'plan',id:planId}]}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Native source',bounds:[1,2,3,4],expectedBytes:null,files:[{itemId:'Native scene',assetKey:'visual',bytes:null}],notes:[],jobs:[]}];
    mount();const confirmation=await screen.findByRole('button',{name:'Confirm download'});
    expect(confirmation.disabled).toBe(false);
    const group=screen.getByRole('region',{name:'Execution records'});
    expect(group.querySelector('[data-slot="disclosure-trigger"]').getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('Please confirm this download.').closest('.agent-work-records')).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('renders one final delivery across download, mosaic and verification continuations without losing the greeting or folding live background work',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});backend=registryBackend();
    backend.selected.entries=[{id:'hello',type:'user',status:'completed',text:'Hello'},
      {id:'greeting',type:'assistant',turnId:'hello',phase:'final',status:'completed',text:'Hello, what imagery do you need?'},
      {id:'request',type:'user',status:'completed',text:'Download and crop Shanghai'},
      {id:'download',type:'tool',turnId:'download-turn',name:'geod_scene_search',status:'completed',references:[]},
      {id:'submitted',type:'assistant',turnId:'download-turn',phase:'final',status:'completed',text:'Download tasks submitted.'}];
    backend.selected.goal={status:'waiting_jobs',requestText:'Download and crop Shanghai',objective:'Deliver Shanghai imagery',outputs:[{id:'image',state:'waiting',label:'Final imagery'}]};
    backend.selected.workflow={status:'waiting',waitingPlans:1,continuations:0,updatedAt:'2026-10-10T00:00:00Z'};backend.activity=null;
    mount();const group=await screen.findByRole('region',{name:'Execution records'}),toggle=group.querySelector('[data-slot="disclosure-trigger"]'),groupId=group.dataset.workId;
    expect(toggle.getAttribute('aria-expanded')).toBe('true');expect(screen.getByText('Download tasks submitted.').closest('.agent-work-records')).toBe(group);
    expect(screen.getByText('Hello, what imagery do you need?').closest('.agent-work-records')).toBeNull();expect(document.querySelectorAll('.agent-message-assistant')).toHaveLength(1);
    expect(toggle.hidden).toBe(false);
    backend.busy=true;backend.activity={sessionId:id,kind:'turn',turnId:'mosaic-turn'};backend.selected.status='running';
    backend.selected.entries.push({id:'wake',type:'system',origin:'desktop',status:'completed',text:'Background tasks settled. Checking results and continuing your request.'},
      {id:'mosaic',type:'tool',turnId:'mosaic-turn',name:'geod_project_mosaic_plan',status:'completed',references:[]},
      {id:'mosaic-submitted',type:'assistant',turnId:'mosaic-turn',phase:'final',status:'completed',text:'Mosaic task submitted.'});backend.revision++;notify();
    await screen.findByRole('status',{name:'Current activity'});expect(document.querySelectorAll('.agent-work-records')).toHaveLength(1);
    expect(group.dataset.workId).toBe(groupId);expect(toggle.getAttribute('aria-expanded')).toBe('true');expect(await screen.findByText('Mosaic task submitted.')).toBeTruthy();
    backend.busy=false;backend.activity=null;backend.selected.status='completed';backend.revision++;notify();
    await waitFor(()=>expect(screen.getByRole('status',{name:'Current activity'}).textContent).toContain('Waiting for background tasks'));expect(group.dataset.active).toBe('true');
    backend.selected.entries.push({id:'verify',type:'tool',turnId:'verify-turn',name:'geod_goal_check',status:'completed',references:[]},
      {id:'delivered',type:'assistant',turnId:'verify-turn',phase:'final',status:'completed',text:'Verified Shanghai delivery is ready.'});
    backend.selected.workflow.status='completed';backend.selected.goal.status='complete';backend.selected.goal.outputs[0].state='verified';backend.revision++;notify();
    await screen.findByText('Verified Shanghai delivery is ready.');expect(toggle.getAttribute('aria-expanded')).toBe('false');expect(group.dataset.active).toBeUndefined();
    expect(document.querySelectorAll('.agent-message-assistant')).toHaveLength(2);expect(screen.queryByText('Download tasks submitted.')).toBeNull();
    await userEvent.click(toggle);expect(screen.getByText('Download tasks submitted.').closest('.agent-work-records')).toBe(group);
    expect(screen.getByText('Mosaic task submitted.').closest('.agent-work-records')).toBe(group);expect(screen.getByText('Verified Shanghai delivery is ready.').closest('.agent-work-records')).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>['send','approvePlan'].includes(op))).toBe(false);
  });
  it('shows polygon coverage and prevents confirmation of a partial area even when its percentage rounds to 100',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'p',type:'tool',name:'geod_project_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}]};
    backend.sessions=[{id,title:'Area acquisition',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Earth Search',bounds:[115,39,118,42],polygon:{sha256:'b'.repeat(64),bounds:[115,39,118,42]},expectedBytes:2000,files:[{itemId:'Native tile',assetKey:'visual',bytes:2000}],notes:[],jobs:[],areaCoverage:{status:'partial',coveredFraction:0.99999,target:'polygon',basis:'catalog-footprints'}}];
    mount();await screen.findByText('Administrative / selected polygon · Area coverage incomplete');
    expect(screen.getByRole('button',{name:'Confirm download'}).disabled).toBe(true);
    expect(screen.queryByText('Footprint coverage does not establish valid pixels or cloud-free coverage.')).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:'Area and files'}));
    expect(within(screen.getByRole('dialog',{name:'Area and files'})).getByText('Footprint coverage does not establish valid pixels or cloud-free coverage.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Close',exact:true}));
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('clears a recovered snapshot validation error while retaining a failed user action across healthy refreshes',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});
    backend=registryBackend();backend.selected.threadId=projectId;
    agentRequest.mockRejectedValueOnce(Error('Agent returned invalid goal state.'));
    mount();await screen.findByText('Agent returned invalid goal state.');notify();
    await screen.findByText('First connection answer');await waitFor(()=>expect(screen.queryByText('Agent returned invalid goal state.')).toBeNull());
    agentRequest.mockImplementation(async operation=>{if(operation==='compact')throw Error('Context could not be organized. Your conversation history was retained.');return structuredClone(backend);});
    await userEvent.click(await moreOption('Organize context'));
    await screen.findByText('Context could not be organized. Your conversation history was retained.');
    notify();await waitFor(()=>expect(agentRequest.mock.calls.filter(([op])=>op==='snapshot').length).toBeGreaterThanOrEqual(3));
    expect(screen.getByText('Context could not be organized. Your conversation history was retained.')).toBeTruthy();
  });
  it('shows a superseded missing-boundary card as resolved without an answer, stale upload prompt or automatic permission',async()=>{
    backend=registryBackend();backend.selected.entries=[{id:'q',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'City crop boundary',status:'superseded',resolution:{reason:'source-boundary-ready',boundary:{id,sha256:'a'.repeat(64)},bounds:[-74,40,-73,41]},questions:[{id:'crop_area',prompt:'No verified polygon. Upload a boundary?',options:[{id:'rectangle',label:'Rectangle',description:'Outer extent.'},{id:'boundary',label:'Upload polygon',description:'Provide it.'}]}]}}];
    mount();await screen.findByText('Boundary ready');
    expect(screen.getByText('The source boundary is ready. The earlier missing-boundary question no longer applies. No choice was recorded; the complete task still needs confirmation.')).toBeTruthy();
    expect(screen.queryByText('No verified polygon. Upload a boundary?')).toBeNull();expect(screen.queryByText('Choices recorded')).toBeNull();
    expect(screen.queryByRole('button',{name:'Submit choices'})).toBeNull();expect(screen.queryByText('Automatic choices enabled')).toBeNull();
  });
  it('shows the full goal manifest and keeps unsupported outputs unfinished despite a completed reply',async()=>{
    backend=registryBackend();backend.selected.goal={objective:'Download imagery and deliver regional statistics.',status:'needs_attention',outputs:[{id:'image',label:'Original imagery',state:'verified'},{id:'csv',label:'Regional statistics CSV',state:'unavailable'}],reason:null};
    mount();await userEvent.click(await screen.findByRole('button',{name:'Show plan progress'}));await screen.findByText('Download imagery and deliver regional statistics.');
    expect(screen.getByRole('region',{name:'Plan progress'}).closest('.agent-conversation')).toBeNull();
    const progress=screen.getByRole('progressbar',{name:'Required output progress'});
    expect(progress.getAttribute('aria-valuenow')).toBe('1');expect(progress.getAttribute('aria-valuemax')).toBe('2');
    expect(screen.getByText('Regional statistics CSV')).toBeTruthy();expect(screen.getByText('Not supported yet')).toBeTruthy();
    expect(screen.queryByText('Goal verified complete')).toBeNull();expect(screen.queryByText('Workflow complete')).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:'Continue goal'}));expect(agentRequest).toHaveBeenCalledWith('goalControl',{sessionId:id,action:'resume'});
    expect(agentRequest.mock.calls.some(([operation])=>operation==='executionMode'||operation==='approvePlan')).toBe(false);
  });
  it('pauses or clears the selected goal through private controls without cancelling download jobs',async()=>{
    backend=registryBackend();backend.selected.goal={objective:'Deliver requested imagery.',status:'waiting_jobs',outputs:[{id:'image',label:'Original imagery',state:'waiting'}],reason:null};
    mount();await userEvent.click(await screen.findByRole('button',{name:'Show plan progress'}));await screen.findByText('Deliver requested imagery.');
    await userEvent.click(screen.getByRole('button',{name:'Pause goal'}));expect(agentRequest).toHaveBeenCalledWith('goalControl',{sessionId:id,action:'pause'});
    await waitFor(()=>expect(screen.getByRole('button',{name:'Clear goal'}).disabled).toBe(false));await userEvent.click(screen.getByRole('button',{name:'Clear goal'}));
    expect(agentRequest).toHaveBeenCalledWith('goalControl',{sessionId:id,action:'clear'});expect(runtimeRequest).not.toHaveBeenCalled();
  });
  it('tracks native steps in the sidebar and closes a narrow side panel when opening the selected review without approval',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.goal={objective:'Deliver imagery for the selected area.',status:'waiting_confirmation',planIds:[planId],outputs:[{id:'image',label:'Final imagery',state:'missing'}],reason:null};
    backend.selected.entries=[{id:'p',type:'tool',name:'geod_project_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Earth Search',bounds:[-74,40,-73,41],expectedBytes:2000,files:[{itemId:'Native scene',assetKey:'visual',bytes:2000}],notes:[],jobs:[]}];
    mount();await userEvent.click(await screen.findByRole('button',{name:'Show plan progress'}));await screen.findByRole('region',{name:'Plan progress'});
    const viewport=screen.getByRole('region',{name:'Conversation messages'});viewport.scrollTo=vi.fn();
    await userEvent.click(screen.getByRole('button',{name:'View task: Download source imagery'}));
    expect(viewport.scrollTo).toHaveBeenCalled();expect(document.activeElement.dataset.planId).toBe(planId);
    expect(screen.queryByRole('complementary',{name:'Plan sidebar'})).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>['approvePlan','send'].includes(op))).toBe(false);
  });
  it('updates the sidebar current action from a native running tool without inventing completed outputs',async()=>{
    let notify;subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return ()=>{};});backend=registryBackend();backend.busy=true;backend.selected.status='running';
    backend.selected.goal={objective:'Deliver imagery for the selected area.',status:'active',outputs:[{id:'image',label:'Final imagery',state:'missing'}],reason:null};
    backend.selected.entries=[{id:'tool',type:'tool',name:'geod_scene_coverage',status:'running',references:[]}];
    mount();await userEvent.click(await screen.findByRole('button',{name:'Show plan progress'}));await screen.findByText('Current action: Check entire area coverage');
    expect(screen.getByRole('progressbar',{name:'Required output progress'}).getAttribute('aria-valuenow')).toBe('0');
    backend.revision++;backend.selected.entries[0].name='geod_scene_search_more';notify();await screen.findByText('Current action: Continue imagery search');
    expect(screen.queryByText('Goal verified complete')).toBeNull();
  });
  it('keeps narrow conversations free of the plan panel until opened and restores focus when Escape closes it',async()=>{
    backend=registryBackend();backend.selected.goal={objective:'Deliver requested imagery.',status:'waiting_jobs',outputs:[{id:'image',label:'Original imagery',state:'waiting'}],reason:null};
    mount();const toggle=await screen.findByRole('button',{name:'Show plan progress'});
    expect(toggle.getAttribute('aria-expanded')).toBe('false');expect(screen.queryByRole('region',{name:'Plan progress'})).toBeNull();
    await userEvent.click(toggle);const sidebar=screen.getByRole('complementary',{name:'Plan sidebar'});
    expect(sidebar.getAttribute('data-overlay')).toBe('true');fireEvent.keyDown(sidebar,{key:'Escape'});
    expect(screen.queryByRole('complementary',{name:'Plan sidebar'})).toBeNull();expect(document.activeElement).toBe(toggle);
    expect(agentRequest.mock.calls.every(([operation])=>operation==='snapshot')).toBe(true);
  });
  it('refreshes actual streamed snapshots from native signals and unsubscribes when closed',async()=>{
    backend=registryBackend();backend.busy=true;backend.selected.status='running';
    backend.selected.entries=[{id:'stream',type:'assistant',status:'running',text:'First chunk'}];
    let notify;const dispose=vi.fn();subscribeAgentUpdates.mockImplementation(async callback=>{notify=callback;return dispose;});
    mount();await screen.findByText('First chunk');expect(document.querySelector('.bui-message-text').dataset.streaming).toBe('true');
    backend.revision++;backend.selected.entries[0].text+=' and next chunk';notify();
    await screen.findByText('First chunk and next chunk');
    backend.busy=false;backend.selected.status='interrupted';backend.selected.entries[0].status='interrupted';backend.revision++;notify();
    await screen.findByRole('button',{name:'Copy response'});expect(document.querySelector('.bui-message-text').hasAttribute('data-streaming')).toBe(false);
    cleanup();expect(dispose).toHaveBeenCalledOnce();const calls=agentRequest.mock.calls.length;notify();expect(agentRequest.mock.calls.length).toBe(calls);
  });
  it('copies the original completed answer without sending a model request',async()=>{
    backend=registryBackend();const writeText=vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText}});
    mount();fireEvent.click(await screen.findByRole('button',{name:'Copy response'}));
    await screen.findByRole('button',{name:'Response copied'});expect(writeText).toHaveBeenCalledWith('First connection answer');
    expect(agentRequest.mock.calls.every(([operation])=>operation==='snapshot')).toBe(true);
  });
  it('execution mode changes through native permission and a fresh conversation uses the draft default',async()=>{
    backend=registryBackend();backend.execution={mode:'confirm-each',defaultMode:'confirm-each',scope:'managed-projects-and-files',modelCanChangePermission:false};
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='executionMode')backend.execution.mode=args.mode;
      return structuredClone(backend);
    });mount();
    const mode=await screen.findByRole('button',{name:'Execution mode'});await userEvent.click(mode);await userEvent.click(screen.getByRole('button',{name:/Full access.*Run validated plans/}));
    expect(agentRequest).toHaveBeenCalledWith('executionMode',{sessionId:id,mode:'full-access'});
    await waitFor(()=>expect(screen.getByRole('button',{name:'Execution mode'}).textContent).toContain('Full access'));await userEvent.click(screen.getByRole('button',{name:'New conversation'}));
    expect(screen.getByRole('button',{name:'Execution mode'}).textContent).toContain('Confirm each plan');
    expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
  });
  it('native notices stay distinct from model replies and a verified map request opens only once',async()=>{
    const requestId='f1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'system-notice',type:'system',origin:'desktop',status:'completed',text:'Background tasks settled. Checking results and continuing your request.'}];
    backend.selected.workspaceView={requestId,id:projectId,kind:'raster',verified:true,acknowledged:false};
    const p={...props(),onOpenResult:vi.fn()};mount(p);
    await waitFor(()=>expect(p.onOpenResult).toHaveBeenCalledTimes(1));
    expect(p.onOpenResult).toHaveBeenCalledWith(backend.selected.workspaceView);
    expect(agentRequest).toHaveBeenCalledWith('acknowledgeView',{sessionId:id,requestId});
    expect(screen.queryByText('GeoD Agent')).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:/Processing complete|Execution records/}));
    expect(document.querySelector('.agent-message-system').textContent).toContain('Background tasks settled.');
    expect(document.querySelector('.agent-message-system').closest('.agent-work-records')).toBeTruthy();
  });
  it('pausing continuation preserves background tasks and uses the existing native stop action',async()=>{
    backend=registryBackend();backend.selected.workflow={status:'waiting',waitingPlans:1,continuations:0,updatedAt:'2026-10-06T00:00:00Z'};
    mount();await screen.findByText('Waiting for background tasks');await userEvent.click(screen.getByRole('button',{name:'Pause continuation'}));
    expect(agentRequest).toHaveBeenCalledWith('interrupt');expect(runtimeRequest).not.toHaveBeenCalled();
  });
  it('connection test checks the unsaved form, blocks duplicate clicks, shows results and clears stale proof',async()=>{
    backend=registryBackend();let release;
    agentRequest.mockImplementation(async operation=>operation==='testModel'?new Promise(resolve=>release=resolve):structuredClone(backend));
    mount();const picker=await screen.findByRole('combobox',{name:'Agent model selection'});
    expect(picker.textContent).toBe('first-model');expect(picker.title).toContain('OpenAI saved');
    await userEvent.click(picker);
    const option=screen.getByRole('option',{name:'DeepSeek saved · second-model'});
    expect(option.getAttribute('data-description-layout')).toBeNull();expect(option.textContent).toContain('DeepSeek saved');
    expect(screen.queryByText('Connection availability is not verified')).toBeNull();
    fireEvent.keyDown(screen.getByRole('listbox'),{key:'Escape'});
    await userEvent.click(await moreOption('Agent model connection'));
    fireEvent.change(screen.getByLabelText('Model ID'),{target:{value:'unsaved-model'}});
    const test=screen.getByRole('button',{name:'Test connection'});fireEvent.click(test);fireEvent.click(test);
    expect(agentRequest.mock.calls.filter(([operation])=>operation==='testModel')).toHaveLength(1);
    expect(agentRequest).toHaveBeenCalledWith('testModel',{request:{id:backend.model.id,provider:'openai',label:'OpenAI saved',protocol:'openai-compatible',baseUrl:'https://api.openai.com/v1',model:'unsaved-model',apiKey:'',contextWindow:null}});
    expect(screen.getByRole('button',{name:'Save connection'}).disabled).toBe(true);
    expect(screen.getByLabelText('Model ID').disabled).toBe(true);
    release({version:1,status:'passed',text:true,functionCalls:true,latencyMs:521,checkedAt:'2026-10-06T12:00:00Z'});
    await screen.findByText('Connection test passed');expect(screen.getByText('Text and tool calls · 521 ms')).toBeTruthy();
    expect(screen.getByLabelText('Model ID').value).toBe('unsaved-model');
    expect(backend.model.model).toBe('first-model');expect(agentRequest.mock.calls.some(([operation])=>operation==='saveModel')).toBe(false);
    fireEvent.change(screen.getByLabelText('API endpoint'),{target:{value:'https://changed.example/v1'}});
    expect(screen.queryByText('Connection test passed')).toBeNull();
    fireEvent.click(screen.getByRole('button',{name:'Test connection'}));
    release({version:1,status:'failed',text:true,functionCalls:false,latencyMs:301,checkedAt:'2026-10-06T12:01:00Z',message:'The model responded, but the test tool call was not valid.'});
    expect((await screen.findByRole('alert')).textContent).toContain('Text connected; tool test incomplete');
    expect(screen.queryByText('Connection test failed')).toBeNull();
  });
  it('submitted tasks refresh unknown progress, finalization, failure and settled output without automatic actions',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'download',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}]};
    backend.sessions=[{id,title:'Current download',status:'completed',compatible:true}];
    const job={id:projectId,title:'Current source raster',status:'running',settled:false,bytesDownloaded:65536,totalBytes:null};
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Selected catalog scene',bounds:[-123,37,-122,38],expectedBytes:null,files:[{itemId:'Current source',assetKey:'scl',bytes:null}],notes:[],jobs:[job]}];
    const p=mount();await openDownloadDetail();await screen.findByText('Current source raster');
    const header=()=>document.querySelector('.agent-plan-heading').textContent;
    const task=within(screen.getByRole('region',{name:'Complete download task'}));
    const progress=task.getByRole('progressbar',{name:'Download progress'});
    expect(progress.getAttribute('data-state')).toBe('indeterminate');expect(progress.getAttribute('aria-valuenow')).toBeNull();expect(header()).toContain('Tasks in progress');
    fireEvent.click(task.getByRole('button',{name:'Open task'}));expect(p.onOpenTasks).toHaveBeenCalledWith(projectId);
    job.status='succeeded';await waitFor(()=>expect(header()).toContain('Finalizing output'),{timeout:2500});
    expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
    job.status='failed';job.settled=true;await waitFor(()=>expect(header()).toContain('Needs attention'),{timeout:2500});expect(screen.getByText('Failed task')).toBeTruthy();
    expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
    job.status='succeeded';await waitFor(()=>expect(header()).toContain('Execution complete'),{timeout:2500});
    expect(screen.getByRole('link',{name:'Open in workspace'}).getAttribute('href')).toBe('#Workspace?file='+projectId);
    expect(agentRequest.mock.calls.every(([operation])=>operation==='snapshot')).toBe(true);
  },12000);
  it('video local preview keeps custom controls and blocks incompatible sends without losing the draft',async()=>{
    const record=videoRecords[0],preview={...record,dataUrl:'data:video/mp4;base64,owned-native-preview'};
    const load=vi.spyOn(HTMLMediaElement.prototype,'load').mockImplementation(()=>{});
    const pause=vi.spyOn(HTMLMediaElement.prototype,'pause').mockImplementation(()=>{});
    try{
      agentRequest.mockImplementation(async operation=>operation==='attachDocument'?record.document:operation==='documentPreview'?preview:structuredClone(backend));
      mount();await screen.findByRole('button',{name:'Attach files'});
      fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['owned-ui-fixture'],record.document.name,{type:record.document.mimeType})]}});
      await screen.findByText('Video files require a Google native connection.');expect(screen.getByRole('button',{name:'Send message'}).disabled).toBe(true);
      await userEvent.click(screen.getByRole('button',{name:'Preview file '+record.document.name}));await screen.findByRole('button',{name:'Play video'});
      const video=document.querySelector('.agent-video-preview video');expect(video.controls).toBe(false);expect(video.autoplay).toBe(false);
      for(const [key,value] of Object.entries({duration:3,videoWidth:320,videoHeight:180,readyState:2}))Object.defineProperty(video,key,{configurable:true,value});
      fireEvent.loadedData(video);expect(screen.getByRole('slider',{name:'Video position'})).toBeTruthy();
      expect(screen.getByRole('button',{name:'Play video'}).disabled).toBe(false);
      await userEvent.click(screen.getByRole('button',{name:'Mute video'}));expect(video.muted).toBe(true);
      await userEvent.click(screen.getByRole('button',{name:'Close'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
      expect(pause).toHaveBeenCalled();expect(document.querySelector('.agent-draft-documents')).toBeTruthy();
      expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
    }finally{cleanup();load.mockRestore();pause.mockRestore();}
  });
  it('audio local preview has custom controls without autoplay and blocks incompatible sends while keeping the draft',async()=>{
    const record=audioRecords[0],preview={...record,dataUrl:'data:audio/wav;base64,owned-native-preview'};
    const load=vi.spyOn(HTMLMediaElement.prototype,'load').mockImplementation(()=>{});
    const pause=vi.spyOn(HTMLMediaElement.prototype,'pause').mockImplementation(()=>{});
    try{
      agentRequest.mockImplementation(async operation=>operation==='attachDocument'?record.document:operation==='documentPreview'?preview:structuredClone(backend));
      mount();await screen.findByRole('button',{name:'Attach files'});
      fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['owned-ui-fixture'],record.document.name,{type:record.document.mimeType})]}});
      await screen.findByText('Audio files require a Google native connection.');expect(screen.getByRole('button',{name:'Send message'}).disabled).toBe(true);
      await userEvent.click(screen.getByRole('button',{name:'Preview file '+record.document.name}));
      await screen.findByRole('button',{name:'Play audio'});
      const audio=document.querySelector('.agent-audio-preview audio');expect(audio.controls).toBe(false);expect(audio.autoplay).toBe(false);
      Object.defineProperty(audio,'duration',{configurable:true,value:3});fireEvent.loadedMetadata(audio);
      expect(screen.getByRole('slider',{name:'Audio position'})).toBeTruthy();
      await userEvent.click(screen.getByRole('button',{name:'Close'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
      expect(pause).toHaveBeenCalled();expect(document.querySelector('.agent-draft-documents')).toBeTruthy();
      expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
    }finally{cleanup();load.mockRestore();pause.mockRestore();}
  });
  it('Office content can be previewed locally while unsupported protocol blocks sending and retains the draft',async()=>{
    const preview=officeRecords[0];
    agentRequest.mockImplementation(async operation=>operation==='attachDocument'?preview.document:operation==='documentPreview'?preview:structuredClone(backend));
    mount();await screen.findByRole('button',{name:'Attach files'});
    fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['owned-ui-fixture'],preview.document.name,{type:preview.document.mimeType})]}});
    await screen.findByText('Office files require an OpenAI Responses connection.');expect(screen.getByRole('button',{name:'Send message'}).disabled).toBe(true);
    await userEvent.click(screen.getByRole('button',{name:'Preview file '+preview.document.name}));
    await screen.findByText('Office content preview');expect(document.querySelector('.agent-document-text').textContent).toBe(preview.text);
    expect(window.officeInjected).toBeUndefined();expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(false);
  });
  it('text document previews escape content and reference-only sends preserve a failed draft before clearing on success',async()=>{
    const text='<script>window.documentInjected=true</script>\n北京 🌍';
    const document={id:'b'.repeat(64),name:'说明.md',mimeType:'text/plain',bytes:new TextEncoder().encode(text).length,characters:[...text].length};
    let refuse=true;
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='attachDocument'){expect(args.name).toBe(document.name);return document;}
      if(operation==='documentPreview')return {document,text};
      if(operation==='send'){
        expect(args).toEqual({sessionId:null,text:'',context:null,documents:[document.id]});
        if(refuse){refuse=false;throw Error('Owned send refusal');}
        backend.selected={id,status:'completed',entries:[{id:'file-message',type:'user',status:'completed',text:'',documents:[document]}]};
      }
      return structuredClone(backend);
    });
    mount();await screen.findByRole('button',{name:'Attach files'});
    fireEvent.change(window.document.querySelector('input[type=file]'),{target:{files:[new File([text],document.name,{type:'text/markdown'})]}});
    await screen.findByRole('button',{name:'Remove file 说明.md'});
    expect(agentRequest.mock.calls.some(([operation])=>operation==='documentPreview')).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Preview file 说明.md'}));
    await screen.findByText(text,{exact:true,normalizer:value=>value});expect(window.documentInjected).toBeUndefined();
    expect(window.document.querySelector('.agent-document-text').textContent).toBe(text);
    await userEvent.click(screen.getByRole('button',{name:'Close'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));await screen.findByText('Owned send refusal');
    expect(window.document.querySelector('.agent-draft-documents')).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));await waitFor(()=>expect(window.document.querySelector('.agent-draft-documents')).toBeNull());
    expect(screen.getByRole('button',{name:'Preview file 说明.md'})).toBeTruthy();
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('attachment storage is explicit, protects the current draft and retains it after cleanup refusal and retry',async()=>{
    const image={id:'a'.repeat(64),name:'draft.png',mimeType:'image/png',bytes:68,width:1,height:1};
    let refuse=true;
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='attachImage')return image;
      if(operation==='imagePreview')return {image,dataUrl:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jY1kAAAAASUVORK5CYII='};
      if(operation==='attachmentStorage'){
        expect(args.protectedImages).toEqual([image.id]);
        if(args.cleanup && refuse){refuse=false;throw Error('Cleanup refused');}
        return {images:{limitBytes:128*1024*1024,usedBytes:args.cleanup?512:1024,imageCount:args.cleanup?1:2,unusedBytes:args.cleanup?0:512,unusedCount:args.cleanup?0:1,removedBytes:args.cleanup?512:0,removedCount:args.cleanup?1:0},documents:{limitBytes:32*1024*1024,usedBytes:0,documentCount:0,unusedBytes:0,unusedCount:0,removedBytes:0,removedCount:0}};
      }
      return structuredClone(backend);
    });
    mount();await screen.findByRole('button',{name:'Attach files'});
    await userEvent.upload(document.querySelector('input[type=file]'),new File(['owned fixture'],'draft.png',{type:'image/png'}));
    await screen.findByRole('button',{name:'Remove image draft.png'});
    await userEvent.click(screen.getByRole('button',{name:'Conversation history'}));
    expect(agentRequest.mock.calls.some(([operation])=>operation==='attachmentStorage')).toBe(false);
    await userEvent.click(await moreOption('Attachment storage'));
    const clean=await screen.findByRole('button',{name:'Clean unused copies'});await waitFor(()=>expect(clean.disabled).toBe(false));
    expect(agentRequest.mock.calls.filter(([operation,args])=>operation==='attachmentStorage' && args.cleanup)).toHaveLength(0);
    await userEvent.click(clean);expect(await screen.findByRole('alert')).toHaveProperty('textContent','Cleanup refused');
    expect(document.querySelectorAll('.agent-draft-images .agent-image')).toHaveLength(1);
    await userEvent.click(clean);await screen.findByText(/Freed /);expect(clean.disabled).toBe(true);
    expect(document.querySelectorAll('.agent-draft-images .agent-image')).toHaveLength(1);
    expect(agentRequest.mock.calls.some(([operation])=>operation==='send' || operation==='approvePlan')).toBe(false);
  });
  it('organizes a selected conversation without a message and keeps the draft',async()=>{
    backend.selected={id,threadId:projectId,status:'completed',entries:[{id:'answer',type:'assistant',status:'completed',text:'Original saved response'}]};backend.sessions=[{id,title:'Existing',status:'completed',compatible:true}];
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='compact'){expect(args).toEqual({sessionId:id});backend.busy=true;backend.selected.contextState={status:'organizing',count:0,lastCompletedAt:null,usedTokens:20000,windowTokens:32768};}
      return structuredClone(backend);
    });mount();await screen.findByText('Original saved response');await userEvent.type(screen.getByRole('textbox',{name:'Message GeoD Agent'}),'Unsent draft');
    await userEvent.click(await moreOption('Organize context'));
    await screen.findByText('Organizing context…');expect(agentRequest).toHaveBeenCalledWith('compact',{sessionId:id});
    expect(screen.getByRole('textbox',{name:'Message GeoD Agent'}).value).toBe('Unsent draft');expect(screen.getByText('Original saved response')).toBeTruthy();
    expect(screen.getByRole('button',{name:'New conversation'}).disabled).toBe(true);expect(agentRequest.mock.calls.some(([op])=>op==='send'||op==='approvePlan')).toBe(false);
  });
  it('separates cumulative model usage from current context and leaves omitted provider details unknown',async()=>{
    backend.selected={id,threadId:projectId,status:'completed',entries:[],contextState:{usedTokens:1000,windowTokens:10000,count:0},usage:{requests:3,totals:{input:7000,output:200,total:7200,cachedInput:6000,reasoningOutput:null}}};
    backend.sessions=[{id,title:'Measured',status:'completed',compatible:true}];
    mount();await userEvent.click(await screen.findByRole('button',{name:'Context usage'}));
    expect(await screen.findByText('Recorded model usage')).toBeTruthy();
    expect(screen.getByText('3 requests')).toBeTruthy();expect(screen.getByText('7,200')).toBeTruthy();
    expect(screen.getByText('10.0%')).toBeTruthy();expect(screen.getByText('Unknown')).toBeTruthy();
    expect(agentRequest.mock.calls.some(([op])=>op==='send'||op==='approvePlan'||op==='compact')).toBe(false);
  });
  it('compaction refusal keeps saved entries and allows retry',async()=>{
    backend.selected={id,threadId:projectId,status:'completed',entries:[{id:'answer',type:'assistant',status:'completed',text:'Retained answer'}]};backend.sessions=[{id,title:'Existing',status:'completed',compatible:true}];
    agentRequest.mockImplementation(async operation=>{if(operation==='compact')throw Error('Context organization refused');return structuredClone(backend);});mount();await screen.findByText('Retained answer');
    await userEvent.click(await moreOption('Organize context'));await screen.findByText('Context organization refused');
    expect(screen.getByText('Retained answer')).toBeTruthy();expect((await moreOption('Organize context')).disabled).toBe(false);
  });
  it('previews images, sends only references, and shows persisted image-only messages',async()=>{
    const image={id:'a'.repeat(64),name:'map.png',mimeType:'image/png',bytes:68,width:1,height:1};
    const dataUrl='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jY1kAAAAASUVORK5CYII=';
    agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='attachImage'){expect(args.name).toBe('map.png');expect(args.encoded).toBeTruthy();return image;}
      if(operation==='imagePreview')return {image,dataUrl};
      if(operation==='send') {backend.selected={id,status:'completed',entries:[{id:'image-message',type:'user',status:'completed',text:'',images:[image]}]};}
      return structuredClone(backend);
    });mount();await screen.findByRole('button',{name:'Attach files'});
    fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['image-test'],'map.png',{type:'image/png'})]}});
    await screen.findByRole('img',{name:'map.png'});expect(screen.getByRole('button',{name:'Send message'}).disabled).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));
    expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:null,text:'',context:null,images:[image.id]});
    await waitFor(()=>expect(document.querySelector('.agent-draft-images')).toBeNull());
    expect(document.querySelector('.agent-message-user img').getAttribute('src')).toBe(dataUrl);
    expect(document.querySelector('.agent-message-user .bui-spinner')).toBeNull();
  });
  it('retains an image draft when send fails and removes it without another upload',async()=>{
    const image={id:'a'.repeat(64),name:'map.png',mimeType:'image/png',bytes:68,width:1,height:1};
    agentRequest.mockImplementation(async operation=>{
      if(operation==='attachImage')return image;
      if(operation==='imagePreview')return {image,dataUrl:'data:image/png;base64,test'};
      if(operation==='send')throw Error('Route unavailable');return structuredClone(backend);
    });mount();await screen.findByRole('button',{name:'Attach files'});
    fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['image-test'],'map.png',{type:'image/png'})]}});
    await screen.findByRole('button',{name:'Remove image map.png'});
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));await screen.findByText('Route unavailable');
    expect(document.querySelector('.agent-draft-images')).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Remove image map.png'}));expect(document.querySelector('.agent-draft-images')).toBeNull();
    expect(agentRequest.mock.calls.filter(([operation])=>operation==='attachImage').length).toBe(1);
  });
  it('rejects unsupported images before native upload',async()=>{
    mount();await screen.findByRole('button',{name:'Attach files'});
    fireEvent.change(document.querySelector('input[type=file]'),{target:{files:[new File(['<svg/>'],'image.svg',{type:'image/svg+xml'})]}});
    await screen.findByRole('alert');
    expect(agentRequest.mock.calls.some(([operation])=>operation==='attachImage')).toBe(false);
  });
  it.each([['SAFE ZIP','product','copernicus'],['HGT ZIP','srtm','nasa-earthdata'],['HDF5','viirs','nasa-earthdata'],['GeoTIFF','red','nasa-earthdata']])('protected %s review requires native authorization and displays unknown transfer size',async(format,assetKey,provider)=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'protected-plan',type:'tool',name:'geod_project_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Protected download'}]}]};
    backend.sessions=[{id,title:'Protected request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Protected source',bounds:[-123,37,-122,38],expectedBytes:null,notes:[],files:[{itemId:'Original native product',assetKey,bytes:null}],jobs:[],format,authorization:{provider,status:'not-connected',expiresAt:null,verifiedAt:null,downloadEnabled:false,entitlement:'not-checked'}}];
    mount(); const confirm=await screen.findByRole('button',{name:'Confirm download'});
    expect(confirm.disabled).toBe(true); expect(screen.getByText('Authorization required')).toBeTruthy();
    expect(screen.getByText('Size available after download')).toBeTruthy();
    expect(document.querySelector('.agent-plan').textContent).not.toContain('NaN');
    expect(document.querySelector('.agent-plan-facts').textContent).toContain(format);
    const setup=screen.getByRole('link',{name:provider==='copernicus'?'Manage Copernicus authorization':'Manage NASA Earthdata authorization'});
    expect(setup.getAttribute('href')).toBe(`#Settings?account=${provider}`);
    await userEvent.click(confirm);
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it.each([['2099-01-01T00:00:00Z',false],['2000-01-01T00:00:00Z',true]])('saved authorization is re-evaluated when displaying a review (%s)',async(expiresAt,blocked)=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'protected-plan',type:'tool',name:'geod_project_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Protected download'}]}]};
    backend.sessions=[{id,title:'Protected request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Protected source',bounds:[-123,37,-122,38],expectedBytes:null,notes:[],files:[{itemId:'Original native product',assetKey:'viirs',bytes:null}],jobs:[],format:'HDF5',authorization:{provider:'nasa-earthdata',status:'saved',expiresAt,verifiedAt:'2026-10-01T00:00:00Z',downloadEnabled:true,entitlement:'not-checked'}}];
    mount(); const confirm=await screen.findByRole('button',{name:'Confirm download'});
    expect(confirm.disabled).toBe(blocked);
    expect(screen.getByText(blocked?'Authorization required':'Authorization saved')).toBeTruthy();
    await userEvent.click(confirm);
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(!blocked);
  });
  it('coverage card reviews the actual grid and unknown size before native confirmation',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'wcs',type:'tool',name:'geod_wcs_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Coverage download'}]}]};
    backend.sessions=[{id,title:'Coverage request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'download',status:'pending',source:'Selected WCS coverage subsets',bounds:[2,53,2.05,53.05],expectedBytes:null,notes:['Service-generated subset on the declared native grid; not an original scene. Encoded size is unknown before download; the native 512 MiB limit applies. Grid edges align outward and intersect the coverage; no resampling or polygon clipping.'],files:[{itemId:'native:coverage',assetKey:'wcs_coverage',referenceId:'c'.repeat(64),coveragePlanId:'c'.repeat(64),bytes:null,width:48,height:48,crs:'EPSG:4326',requestedBounds:[2,53,2.05,53.05],alignedBounds:[2,53,2.05,53.05],selection:'bbox-native-grid'}],jobs:[],project:{id:projectId,name:'Native subset',sceneCount:1,mode:'existing',committed:false}}];
    mount();await screen.findByRole('button',{name:'Confirm download'});
    expect(document.querySelector('.agent-plan-facts').textContent).toContain('48 × 48 px');
    expect(document.querySelector('.agent-plan').textContent).not.toContain('NaN');
    expect(document.querySelector('.agent-plan').textContent).not.toContain('Original raster');
    expect(screen.queryByRole('dialog',{name:'Area and files'})).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:'Area and files'}));
    await screen.findByText('Encoded size available after download; maximum 512 MiB per file.');
    expect(screen.getByText(/Aligned coverage area/)).toBeTruthy();
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Close',exact:true}));
    await userEvent.click(screen.getByRole('button',{name:'Confirm download'}));
    expect(agentRequest).toHaveBeenCalledWith('approvePlan',{sessionId:id,planId,planHash});
  });
  it('vector acquisition retains the user request and choices with an explicit final extraction confirmation',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',choiceId='f1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'q',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:choiceId,title:'Choose coverage',status:'answered',answers:[{questionId:'coverage',optionId:'full'}],questions:[{id:'coverage',prompt:'Which features?',options:[{id:'full',label:'Full intersecting features',description:'Preserve complete geometry.'},{id:'clip',label:'Clipped features',description:'Clip to the area when supported.'}]}]}},
      {id:'p',type:'tool',name:'geod_vector_extract_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Vector extraction'}],taskContext:{requestText:'Save lakes in my study area.',choices:[{decisionId:choiceId,questionId:'coverage',prompt:'Which features?',answer:'Full intersecting features'}]}}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'vector',status:'pending',source:'Saved service',bounds:[1,2,3,4],expectedBytes:null,notes:[],jobs:[],vector:null,vectorReview:{serviceId:projectId,serviceSha256:'c'.repeat(64),collectionId:'lakes',collectionTitle:'Lakes',protocol:'OGC API Features',name:'Reviewed lakes',pageSize:2,responseFormat:null,selection:'bbox-full-features',liveAvailabilityChecked:false,clipped:false}}];
    mount();const card=await screen.findByRole('region',{name:'Vector extraction'}),task=within(card);
    expect(task.queryByText('Save lakes in my study area.')).toBeNull();
    await userEvent.click(task.getByRole('button',{name:'Area and collection'}));
    const details=within(screen.getByRole('dialog',{name:'Area and collection'}));
    expect(details.getByText('Save lakes in my study area.')).toBeTruthy();expect(details.getByText('Full intersecting features')).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Close',exact:true}));
    expect(task.getByText('Count available after extraction')).toBeTruthy();
    expect(task.getByRole('button',{name:'Confirm extraction'}).disabled).toBe(false);expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('vector extraction shows unknown counts, edits without executing, and confirms the revised hash once',async()=>{
    const old='c1234567-1234-1234-1234-123456789abc',next='f1234567-1234-1234-1234-123456789abc',hash='a'.repeat(64),newHash='b'.repeat(64);
    const review={serviceId:projectId,serviceSha256:'c'.repeat(64),collectionId:'lakes',collectionTitle:'Lakes',protocol:'OGC API Features',name:'Reviewed lakes',pageSize:2,responseFormat:null,selection:'bbox-full-features',liveAvailabilityChecked:false,clipped:false};
    backend.selected={id,status:'completed',entries:[{id:'vector-plan',type:'tool',name:'geod_vector_extract_plan',status:'completed',references:[{kind:'plan',id:old,label:'Vector extraction'}]}]};
    backend.sessions=[{id,title:'Extract vector',status:'completed',compatible:true}];
    backend.plans=[{planId:old,planHash:hash,kind:'vector',status:'pending',source:'Saved service',bounds:[-130,20,-60,65],boundsCrs:'EPSG:4326',expectedBytes:null,notes:[],jobs:[],vectorReview:review,vector:null}];
    const draft={planId:old,planHash:hash,kind:'vector',parameters:{kind:'vector',name:review.name,bounds:backend.plans[0].bounds,keepPolygon:false},fields:{name:true,bounds:true,polygon:false},boundsCrs:'EPSG:4326'};
    let release; const confirmation=new Promise(resolve=>release=resolve);
    agentRequest.mockImplementation(async operation=>{
      if(operation==='revisionDraft') return structuredClone(draft);
      if(operation==='revisePlan') {
        backend.plans[0]={...backend.plans[0],status:'superseded',replacedBy:next};
        backend.plans.push({...backend.plans[0],planId:next,planHash:newHash,status:'pending',replacedBy:null,bounds:[-129,20,-60,65],vectorReview:{...review,name:'Corrected lakes'}});
        backend.selected.entries.push({id:'corrected',type:'tool',name:'geod_plan_status',status:'completed',references:[{kind:'plan',id:next,label:'Vector extraction'}]});
      }
      if(operation==='approvePlan') return confirmation;
      return structuredClone(backend);
    });
    mount();
    expect(await screen.findByText('Count available after extraction')).toBeTruthy();
    expect(screen.queryByText(/GeoTIFF| px|Downloaded/)).toBeNull();
    expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:'Edit review parameters'}));
    fireEvent.change(screen.getByLabelText('Minimum X'),{target:{value:'-129'}});
    fireEvent.change(screen.getByLabelText('Output name'),{target:{value:'Corrected lakes'}});
    await userEvent.click(screen.getByRole('button',{name:'Save and review again'}));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    expect(agentRequest).toHaveBeenCalledWith('revisePlan',{sessionId:id,planId:old,planHash:hash,revision:{kind:'vector',name:'Corrected lakes',bounds:[-129,20,-60,65],keepPolygon:false}});
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
    const confirm=screen.getByRole('button',{name:'Confirm extraction'}); fireEvent.click(confirm);fireEvent.click(confirm);
    expect(agentRequest.mock.calls.filter(([op])=>op==='approvePlan')).toEqual([['approvePlan',{sessionId:id,planId:next,planHash:newHash}]]);
    backend.plans[1]={...backend.plans[1],status:'submitted',vector:{id:projectId,name:'Corrected lakes',format:'geojson',bytes:14842,featureCount:13,coordinateCount:253,sourceSha256:'c'.repeat(64),geojsonSha256:'d'.repeat(64),verified:true}};
    release(structuredClone(backend));
    const link=await screen.findByRole('link',{name:'Open in workspace'});
    expect(link.getAttribute('href')).toBe('#Workspace?vector='+projectId);
    expect(screen.getByText(/Verified vector file ·/)).toBeTruthy();
    expect(screen.queryByRole('button',{name:'Confirm extraction'})).toBeNull();
  });
  it('custom asset correction shows readable labels while sending only the selected immutable reference',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64),first='b'.repeat(64),second='c'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'custom',type:'tool',name:'geod_stac_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}]};backend.sessions=[{id,title:'Custom originals',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'download',status:'pending',source:'Selected custom raster assets',bounds:[13,52,14,53],expectedBytes:4000,files:['first-band','second-band'].map((originalAssetKey,index)=>({itemId:'Same native item',assetKey:'stac_asset',originalAssetKey,referenceId:index?second:first,bytes:2000})),notes:[],jobs:[],project:{id:projectId,name:'Saved custom project',mode:'existing',sceneCount:2,committed:false}}];
    const draft={planId,planHash,kind:'download',parameters:{kind:'download',itemIds:[first,second]},fields:{items:[{id:first,label:'Same native item · first-band',locked:false},{id:second,label:'Same native item · second-band',locked:false}]},boundsCrs:'EPSG:4326',projectId};
    agentRequest.mockImplementation(async operation=>operation==='revisionDraft'?structuredClone(draft):structuredClone(backend));
    mount();await userEvent.click(await screen.findByRole('button',{name:'Edit review parameters'}));
    expect(screen.getByRole('switch',{name:'Same native item · first-band'})).toBeTruthy();
    await userEvent.click(screen.getByRole('switch',{name:'Same native item · first-band'}));
    await userEvent.click(screen.getByRole('button',{name:'Save and review again'}));
    expect(agentRequest).toHaveBeenCalledWith('revisePlan',{sessionId:id,planId,planHash,revision:{kind:'download',itemIds:[second]}});
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('edits crop coordinates in their native CRS and requires a separate confirmation of the new hash',async()=>{
    const old='c1234567-1234-1234-1234-123456789abc',next='f1234567-1234-1234-1234-123456789abc',hash='a'.repeat(64),newHash='b'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'crop',type:'tool',name:'geod_clip_plan',status:'completed',references:[{kind:'plan',id:old,label:'Crop plan'}]}]};backend.sessions=[{id,title:'Crop',status:'completed',compatible:true}];
    backend.plans=[{planId:old,planHash:hash,kind:'clip',status:'pending',source:'Verified local SCL',bounds:[500000,4199940,500090,4200000],boundsCrs:'EPSG:32610',expectedBytes:null,files:[{itemId:'Native SCL',assetKey:'scl',bytes:null,width:3,height:2}],notes:[],jobs:[]}];
    const draft={planId:old,planHash:hash,kind:'clip',parameters:{kind:'clip',name:'Original crop',bounds:backend.plans[0].bounds,keepPolygon:false},fields:{name:true,bounds:true,polygon:false},boundsCrs:'EPSG:32610'};
    let release;const saved=new Promise(resolve=>release=resolve);
    agentRequest.mockImplementation(async operation=>operation==='revisionDraft'?structuredClone(draft):operation==='revisePlan'?saved:structuredClone(backend));
    mount();await userEvent.click(await screen.findByRole('button',{name:'Edit review parameters'}));
    expect(agentRequest).toHaveBeenCalledWith('revisionDraft',{sessionId:id,planId:old,planHash:hash});
    expect(screen.getByRole('group',{name:'Area coordinates · EPSG:32610'})).toBeTruthy();
    expect(screen.getByLabelText('Minimum X').value).toBe('500000');
    fireEvent.change(screen.getByLabelText('Minimum X'),{target:{value:'500030'}});fireEvent.change(screen.getByLabelText('Output name'),{target:{value:'Corrected crop'}});
    const save=screen.getByRole('button',{name:'Save and review again'});fireEvent.click(save);fireEvent.click(save);
    expect(agentRequest.mock.calls.filter(([op])=>op==='revisePlan')).toEqual([['revisePlan',{sessionId:id,planId:old,planHash:hash,revision:{kind:'clip',name:'Corrected crop',bounds:[500030,4199940,500090,4200000],keepPolygon:false}}]]);
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
    backend.plans[0]={...backend.plans[0],status:'superseded',replacedBy:next};backend.plans.push({...backend.plans[0],planId:next,planHash:newHash,status:'pending',replacedBy:null,bounds:[500030,4199940,500090,4200000]});
    backend.selected.entries.push({id:'correction',type:'tool',name:'geod_plan_status',status:'completed',references:[{kind:'plan',id:next,label:'Crop plan'}],summary:{kind:'plan',status:'pending',revisionOf:old}});release(structuredClone(backend));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(screen.getByText('Replaced by revised review')).toBeTruthy();
    expect(screen.getAllByRole('button',{name:'Confirm crop'})).toHaveLength(1);
    await userEvent.click(screen.getByRole('button',{name:'Confirm crop'}));expect(agentRequest).toHaveBeenCalledWith('approvePlan',{sessionId:id,planId:next,planHash:newHash});
  });
  it('native append forms keep locked scenes and omit the saved project name and area',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'project',type:'tool',name:'geod_project_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Project selection'}]}]};backend.sessions=[{id,title:'Append',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'project',status:'pending',source:'Selected catalog scenes',bounds:[-122,37,-121,38],expectedBytes:null,files:['old','new-a','new-b'].map(itemId=>({itemId,assetKey:'scene',bytes:null})),notes:[],jobs:[],project:{id:projectId,name:'Saved native project',mode:'append',sceneCount:3,committed:false}}];
    const draft={planId,planHash,kind:'project',parameters:{kind:'project',name:null,bounds:null,itemIds:['old','new-a','new-b'],keepPolygon:false},fields:{name:false,bounds:false,polygon:false,items:['old','new-a','new-b'].map(itemId=>({id:itemId,date:'2026-10-01',locked:itemId==='old'}))},boundsCrs:'EPSG:4326',projectId};
    agentRequest.mockImplementation(async operation=>operation==='revisionDraft'?structuredClone(draft):structuredClone(backend));
    mount();await userEvent.click(await screen.findByRole('button',{name:'Edit review parameters'}));
    expect(screen.queryByLabelText('Output name')).toBeNull();expect(screen.queryByLabelText('Minimum X')).toBeNull();
    expect(screen.getByRole('switch',{name:'old'}).disabled).toBe(true);await userEvent.click(screen.getByRole('switch',{name:'new-a'}));
    await userEvent.click(screen.getByRole('button',{name:'Save and review again'}));
    expect(agentRequest).toHaveBeenCalledWith('revisePlan',{sessionId:id,planId,planHash,revision:{kind:'project',keepPolygon:false,itemIds:['old','new-b']}});
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('RGB form uses only native quality options, retains failed edits, and cancellation never confirms',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'rgb',type:'tool',name:'geod_scientific_rgb_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Scientific RGB'}]}]};backend.sessions=[{id,title:'RGB',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'rgb',status:'pending',source:'Verified local reflectance bands',bounds:[500000,4199940,500090,4200000],expectedBytes:null,files:['red','green','blue'].map(assetKey=>({itemId:'RGB source',assetKey,bytes:null,width:3,height:2})),notes:[],jobs:[]}];
    const draft={planId,planHash,kind:'rgb',parameters:{kind:'rgb',name:'RGB native',qualityPolicy:'cloud_free',excludeSnow:false},fields:{name:true,qualityPolicies:['cloud_free','cloud_free_conservative'],snow:true}};
    agentRequest.mockImplementation(async operation=>{if(operation==='revisionDraft')return structuredClone(draft);if(operation==='revisePlan')throw Error('Synthetic changed native source');return structuredClone(backend);});
    mount();await userEvent.click(await screen.findByRole('button',{name:'Edit review parameters'}));
    await userEvent.click(screen.getByRole('combobox',{name:'Quality policy'}));expect(screen.queryByRole('option',{name:'VI good observations'})).toBeNull();
    await userEvent.click(screen.getByRole('option',{name:'Landsat conservative QA'}));await userEvent.click(screen.getByRole('switch',{name:'Exclude snow'}));
    await userEvent.click(screen.getByRole('button',{name:'Save and review again'}));
    expect(await screen.findByRole('alert')).toHaveProperty('textContent','Synthetic changed native source');
    expect(screen.getByRole('switch',{name:'Exclude snow'}).getAttribute('aria-checked')).toBe('true');
    expect(agentRequest).toHaveBeenCalledWith('revisePlan',{sessionId:id,planId,planHash,revision:{kind:'rgb',name:'RGB native',qualityPolicy:'cloud_free_conservative',excludeSnow:true}});
    await userEvent.click(screen.getByRole('button',{name:'Cancel'}));expect(screen.queryByRole('dialog')).toBeNull();expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('labels saved connections and starts a new conversation after a native connection switch',async()=>{
    backend=registryBackend();agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='saveModel') {backend.registry.selectedId=args.request.id;backend.model=backend.registry.connections[1];backend.sessions[0].compatible=false;}
      return structuredClone(backend);
    });
    mount();await userEvent.click(await screen.findByRole('combobox',{name:'Agent model selection'}));
    expect(screen.getByRole('option',{name:'OpenAI saved · first-model'})).toBeTruthy();
    await userEvent.click(screen.getByRole('option',{name:'DeepSeek saved · second-model'}));
    await waitFor(()=>expect(agentRequest).toHaveBeenCalledWith('saveModel',{request:{action:'select',id:backend.registry.connections[1].id}}));
    await waitFor(()=>expect(screen.queryByText('First connection answer')).toBeNull());
    const input=screen.getByRole('textbox',{name:'Message GeoD Agent'});await userEvent.type(input,'Read workspace');
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));
    expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:null,text:'Read workspace',context:null});
    await userEvent.click(screen.getByRole('button',{name:'Conversation history'}));expect(screen.getByText('OpenAI · OpenAI saved · first-model')).toBeTruthy();
  });
  it('keeps current configuration on save failure, prevents double save and clears keys when changing the editing target',async()=>{
    backend=registryBackend();let rejectSave;agentRequest.mockImplementation(async operation=>operation==='saveModel'?new Promise((_,reject)=>rejectSave=reject):structuredClone(backend));
    mount();await userEvent.click(await moreOption('Agent model connection'));
    expect(screen.getByLabelText('API key').value).toBe('');
    await userEvent.type(screen.getByLabelText('API key'),'synthetic-replacement');
    const save=screen.getByRole('button',{name:'Save connection'});fireEvent.click(save);fireEvent.click(save);
    expect(agentRequest.mock.calls.filter(([op])=>op==='saveModel')).toHaveLength(1);
    rejectSave(Error('Synthetic registry write failure'));expect(await screen.findByRole('alert')).toHaveProperty('textContent','Synthetic registry write failure');
    expect(screen.getByLabelText('Connection name').value).toBe('OpenAI saved');expect(screen.getByLabelText('API key').value).toBe('synthetic-replacement');
    await userEvent.click(screen.getByRole('combobox',{name:'Saved connection'}));
    await userEvent.click(screen.getByRole('option',{name:'second-model'}));
    expect(screen.getByLabelText('API key').value).toBe('');expect(screen.getByLabelText('API endpoint').value).toBe('https://api.deepseek.com');
    await userEvent.click(screen.getByRole('button',{name:'Connection details'}));expect(screen.getByText(/The connection test checks text and a harmless tool call/)).toBeTruthy();await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('button',{name:'Cancel'}));expect(screen.getByRole('combobox',{name:'Agent model selection'}).textContent).toBe('first-model');
  });
  it('retains saved native protocols and clears keys when changing provider or custom protocol',async()=>{
    backend=registryBackend();backend.model={...backend.model,provider:'anthropic',protocol:'anthropic-messages',baseUrl:'https://api.anthropic.com/v1',model:'claude-haiku-4-5'};backend.registry.connections[0]=backend.model;
    mount();await userEvent.click(await moreOption('Agent model connection'));
    expect(screen.getByRole('combobox',{name:'Agent model protocol'}).textContent).toContain('Anthropic Messages');
    expect(screen.getByLabelText('API endpoint').value).toBe('https://api.anthropic.com/v1');
    await userEvent.type(screen.getByLabelText('API key'),'synthetic-old-key');
    await userEvent.click(screen.getByRole('combobox',{name:'Agent model provider'}));await userEvent.click(screen.getByRole('option',{name:'Google',exact:true}));
    expect(screen.getByLabelText('API key').value).toBe('');expect(screen.getByLabelText('API endpoint').value).toBe('https://generativelanguage.googleapis.com/v1beta');
    expect(screen.getByRole('combobox',{name:'Agent model protocol'}).textContent).toContain('Google Generative AI');
    await userEvent.click(screen.getByRole('combobox',{name:'Agent model provider'}));await userEvent.click(screen.getByRole('option',{name:'Custom connection',exact:true}));
    await userEvent.type(screen.getByLabelText('API key'),'synthetic-new-key');
    await userEvent.click(screen.getByRole('combobox',{name:'Agent model protocol'}));await userEvent.click(screen.getByRole('option',{name:'Anthropic Messages',exact:true}));
    expect(screen.getByLabelText('API key').value).toBe('');
  });
  it('defaults new OpenAI connections to Responses while keeping saved Chat Completions editable',async()=>{
    backend=registryBackend();mount();await userEvent.click(await moreOption('Agent model connection'));
    const protocol=screen.getByRole('combobox',{name:'Agent model protocol'});
    expect(protocol.textContent).toContain('OpenAI-compatible Chat Completions');expect(protocol.disabled).toBe(false);
    await userEvent.type(screen.getByLabelText('API key'),'synthetic-key');
    await userEvent.click(protocol);await userEvent.click(screen.getByRole('option',{name:'OpenAI Responses',exact:true}));
    expect(screen.getByLabelText('API key').value).toBe('');
    await userEvent.click(screen.getByRole('button',{name:'Connection details'}));expect(screen.getByText(/OpenAI Responses keeps encrypted reasoning/)).toBeTruthy();await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('combobox',{name:'Saved connection'}));await userEvent.click(screen.getByRole('option',{name:'Add model connection',exact:true}));
    expect(screen.getByRole('combobox',{name:'Agent model protocol'}).textContent).toContain('OpenAI Responses');
  });
  it('connection removal requires its own click and sends only the selected ID while retaining history',async()=>{
    backend=registryBackend();agentRequest.mockImplementation(async(operation,args)=>{
      if(operation==='saveModel') {backend.registry.connections=backend.registry.connections.filter(connection=>connection.id!==args.request.id);backend.model=backend.registry.connections[0];backend.registry.selectedId=backend.model.id;backend.sessions[0].compatible=false;}
      return structuredClone(backend);
    });
    mount();await userEvent.click(await moreOption('Agent model connection'));
    await userEvent.click(screen.getByRole('button',{name:'Remove connection'}));expect(agentRequest.mock.calls.some(([op])=>op==='saveModel')).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Confirm removal'}));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    expect(agentRequest).toHaveBeenCalledWith('saveModel',{request:{action:'delete',id:'d1234567-1234-1234-1234-123456789abc'}});
    await userEvent.click(screen.getByRole('button',{name:'Conversation history'}));expect(screen.getByText('First history')).toBeTruthy();
  });
  it('an active turn disables connection switching and its manager',async()=>{
    backend=registryBackend();backend.busy=true;backend.selected.status='running';mount();
    expect((await screen.findByRole('combobox',{name:'Agent model selection'})).disabled).toBe(true);
    expect((await moreOption('Agent model connection')).disabled).toBe(true);
    expect(agentRequest.mock.calls.some(([op])=>op==='saveModel')).toBe(false);
  });
  it('shows observed native account status and fixed authorization links without login actions',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'sources',type:'tool',name:'geod_sources_list',status:'completed',references:[],summary:{kind:'sources',count:9,checkedAt:new Date().toISOString(),accounts:[
      {provider:'nasa-earthdata',status:'not-connected',expiresAt:null,verifiedAt:null},
      {provider:'copernicus',status:'connected',expiresAt:'2020-01-01T00:00:00Z',verifiedAt:'2019-12-01T00:00:00Z'},
    ]}}]};backend.sessions=[{id,title:'Account check',status:'completed',compatible:true}];
    mount();await userEvent.click(await toolButton(/Check data source capabilities/));
    expect(screen.getByText('Not connected')).toBeTruthy();expect(screen.getByText('Authorization expired')).toBeTruthy();
    expect(screen.queryByText('Connected',{exact:true})).toBeNull();expect(screen.getByText(/Account status at/)).toBeTruthy();
    expect(screen.getByRole('link',{name:'Manage NASA Earthdata authorization'}).getAttribute('href')).toBe('#Settings?account=nasa-earthdata');
    expect(screen.getByRole('link',{name:'Manage Copernicus authorization'}).getAttribute('href')).toBe('#Settings?account=copernicus');
    expect(runtimeRequest).not.toHaveBeenCalled();expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('scientific RGB shows original type and quality policy and waits for native confirmation',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.mode='review-first';backend.selected={id,status:'completed',entries:[{id:'rgb',type:'tool',name:'geod_scientific_rgb_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Scientific RGB'}]}]};
    backend.sessions=[{id,title:'RGB request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'rgb',status:'pending',source:'Verified local reflectance bands',bounds:[500000,4199940,500090,4200000],boundsCrs:'EPSG:32610',files:['red','green','blue'].map(assetKey=>({itemId:'Actual native scene',assetKey,bytes:1000,width:3,height:2})),expectedBytes:null,notes:[],jobs:[],processing:{product:'landsat-c2-l2',dataType:'UInt16',channels:3,rawBytes:36,requiredDiskBytes:8388700,quality:{product:'landsat-c2-l2',policy:'cloud_free_conservative',excludeSnow:true,coupled:false,sceneCount:1,sourceCount:5,sourceSha256:'c'.repeat(64)}}}];
    mount();const confirm=await screen.findByRole('button',{name:'Confirm RGB'});
    expect(screen.getByText('Original values: UInt16')).toBeTruthy();expect(screen.getByText('Landsat conservative QA · Exclude snow')).toBeTruthy();
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Area and files'}));expect(screen.getByText(/Required workspace space/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Close',exact:true}));
    await userEvent.click(confirm);expect(agentRequest).toHaveBeenCalledWith('approvePlan',{sessionId:id,planId,planHash});
  });
  it('only a human delivery action packages the exact settled RGB result and reveals it separately',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'rgb',type:'tool',name:'geod_scientific_rgb_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Scientific RGB'}]}]};
    backend.sessions=[{id,title:'RGB result',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'rgb',status:'submitted',source:'Verified local reflectance bands',bounds:[500000,4199940,500090,4200000],expectedBytes:null,files:['red','green','blue'].map(assetKey=>({itemId:'Native scene',assetKey,bytes:1000,width:3,height:2})),notes:[],jobs:[{id:projectId,status:'succeeded',settled:true,title:'Verified RGB',bytesDownloaded:2000,totalBytes:2000}]}];
    let release;const prepared=new Promise(resolve=>release=resolve);
    runtimeRequest.mockImplementation(operation=>operation==='package'?prepared:Promise.resolve(undefined));
    mount();const prepare=await screen.findByRole('button',{name:'Prepare delivery package'});
    expect(runtimeRequest).not.toHaveBeenCalled();expect(screen.queryByRole('dialog')).toBeNull();
    fireEvent.click(prepare);fireEvent.click(prepare);
    expect(runtimeRequest.mock.calls.filter(([op])=>op==='package')).toHaveLength(1);
    expect(runtimeRequest.mock.calls[0].slice(0,2)).toEqual(['package',{id:projectId}]);
    release({jobId:projectId,filename:`geod-rgb-${projectId}.zip`,path:'Managed verification folder',bytes:4000,sha256:'b'.repeat(64),files:[`${projectId}.tif`,'preview.png','checksums.sha256']});
    await screen.findByRole('dialog',{name:'Verified delivery package'});
    expect(runtimeRequest.mock.calls.some(([op])=>op==='revealPackage')).toBe(false);
    await userEvent.click(screen.getByRole('button',{name:'Show package in folder'}));
    expect(runtimeRequest.mock.calls.at(-1).slice(0,2)).toEqual(['revealPackage',{id:projectId}]);
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('original downloads never expose the derived delivery action',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'download',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}]};
    backend.sessions=[{id,title:'Source result',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'submitted',source:'Earth Search',bounds:[-122.5,37.7,-122.4,37.8],expectedBytes:2000,files:[{itemId:'Native source',assetKey:'scl',bytes:2000}],notes:[],jobs:[{id:projectId,status:'succeeded',settled:true,title:'Verified original',bytesDownloaded:2000,totalBytes:2000}]}];
    mount();await openDownloadDetail();await screen.findByText('Output ready');
    expect(screen.queryByRole('button',{name:'Prepare delivery package'})).toBeNull();
    expect(runtimeRequest).not.toHaveBeenCalled();
  });
  it('saves and uses a user-owned model without activating Global login or hosted models',async()=>{
    const previous=window.__TAURI__,invoke=vi.fn().mockRejectedValue(Error('Account service unavailable'));
    window.__TAURI__={core:{invoke}};
    try {
    backend.configured=false;backend.model=null;
    render(<I18nProvider><IdentityProvider><IdentityEntry onOpen={vi.fn()}/><AgentPanel {...props()}/></IdentityProvider></I18nProvider>);
    await userEvent.click(await screen.findByRole('button',{name:'Connect model'}));
    await userEvent.type(screen.getByLabelText('Connection name'),'My connection');await userEvent.type(screen.getByLabelText('Model ID'),'test-model');
    await userEvent.click(screen.getByRole('button',{name:'Hosted models'}));
    const hosted=screen.getByRole('dialog',{name:'Hosted models'});
    expect(within(hosted).queryByRole('button',{name:/Sign in/})).toBeNull();
    await userEvent.click(within(hosted).getByRole('button',{name:'Close',exact:true}));
    expect(screen.getByLabelText('Connection name').value).toBe('My connection');
    await userEvent.type(screen.getByLabelText('API key'),'synthetic-unit-key');backend.configured=true;backend.model=base().model;
    await userEvent.click(screen.getByRole('button',{name:'Save connection'}));
    await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
    expect(agentRequest).toHaveBeenCalledWith('saveModel',{request:expect.objectContaining({apiKey:'synthetic-unit-key',model:'test-model'})});
    expect(document.body.textContent).not.toContain('synthetic-unit-key');
    const input=screen.getByRole('textbox',{name:'Message GeoD Agent'});
    await userEvent.type(input,'Show saved projects');await userEvent.click(screen.getByRole('button',{name:'Send message'}));
    await waitFor(()=>expect(agentRequest.mock.calls.some(([operation])=>operation==='send')).toBe(true));
    expect(invoke.mock.calls.some(([command])=>command.startsWith('identity_')||command==='official_request')).toBe(false);
    expect(screen.queryByRole('button',{name:'Sign in'})).toBeNull();
    } finally {window.__TAURI__=previous;}
  });
  it('Enter submits while IME composition and Shift+Enter do not',async()=>{
    mount();const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});await waitFor(()=>expect(input.disabled).toBe(false));
    fireEvent.change(input,{target:{value:'查工程'}});fireEvent.compositionStart(input);fireEvent.keyDown(input,{key:'Enter'});
    expect(agentRequest.mock.calls.some(([op])=>op==='send')).toBe(false);
    fireEvent.compositionEnd(input);fireEvent.keyDown(input,{key:'Enter',shiftKey:true});expect(agentRequest.mock.calls.some(([op])=>op==='send')).toBe(false);
    fireEvent.keyDown(input,{key:'Enter'});await waitFor(()=>expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:null,text:'查工程',context:null}));
  });
  it('closing the sidebar hides it without interrupting the response or background tasks',async()=>{
    backend.busy=true;backend.selected={id,status:'running',entries:[]};backend.sessions=[{id,title:'Saved conversation',status:'running',compatible:true}];
    const p=mount();await waitFor(()=>expect(screen.getByRole('button',{name:'Stop response'})).toBeTruthy());
    await userEvent.click(screen.getByRole('button',{name:'Close Agent'}));expect(p.onClose).toHaveBeenCalledTimes(1);expect(agentRequest.mock.calls.some(([op])=>op==='interrupt')).toBe(false);
    expect(agentRequest.mock.calls.every(([operation])=>operation!=='cancelJob')).toBe(true);
  });
  it('shows native tool results and opens their exact project IDs',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'read',type:'tool',name:'geod_projects_list',status:'completed',summary:{kind:'projects',count:1},references:[{kind:'project',id:projectId,label:'Actual saved project'}]}]};
    backend.sessions=[{id,title:'Saved conversation',status:'completed',compatible:true}];const p=mount();
    await userEvent.click(await toolButton(/Read projects/));await userEvent.click(await screen.findByRole('button',{name:'Actual saved project'}));expect(p.onOpenProject).toHaveBeenCalledWith(projectId);
  });
  it.each([
    ['en','record-storage','Could not save this query. Local storage needs attention.'],
    ['zh-CN','record-storage','查询结果保存失败，需要修复本地存储。'],
    ['en','city-scope','The requested city has not been located. Imagery search was stopped.'],
    ['zh-CN','city-scope','尚未定位到指定城市，已停止影像检索。'],
    ['en','source-network','The data service could not be reached. Try again after the connection recovers.'],
    ['zh-CN','source-network','暂时无法连接数据服务，连接恢复后可重试。'],
  ])('shows a localized safe tool failure for %s / %s',async(locale,failureCode,message)=>{
    window.localStorage.setItem('geod-global-locale',locale);
    backend.selected={id,status:'completed',entries:[{id:'failed-lookup',type:'tool',name:'geod_place_search',status:'failed',references:[],failureCode,error:'C:\\Private\\secret-token'}]};
    backend.sessions=[{id,title:'Failed lookup',status:'completed',compatible:true}];mount();
    const tool=await toolButton(locale==='en'?/Find requested place/:/查找指定地点/);
    await userEvent.click(tool);expect(await screen.findByText(message)).toBeTruthy();
    expect(document.body.textContent).not.toContain('secret-token');
    expect(agentRequest.mock.calls.every(([operation])=>operation==='snapshot')).toBe(true);
  });
  it('opens verified vector results through their native workspace identity',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'vector-read',type:'tool',name:'geod_vector_inspect',status:'completed',summary:{kind:'vector',count:25,verified:true,sha256:'a'.repeat(64)},references:[{kind:'vector',id:projectId,label:'Actual public lakes'}]}]};
    backend.sessions=[{id,title:'Vector read',status:'completed',compatible:true}];const p=mount();
    await userEvent.click(await toolButton(/Verify vector file/));
    const link=await screen.findByRole('link',{name:'Actual public lakes'});
    expect(link.getAttribute('href')).toBe(`#Workspace?vector=${projectId}`);
    expect(screen.getByText('25 features verified')).toBeTruthy();
    expect(p.onOpenTasks).not.toHaveBeenCalled();expect(p.onOpenProject).not.toHaveBeenCalled();
  });
  it('preserves the typed message if submission fails',async()=>{
    agentRequest.mockImplementation(async operation=>{if(operation==='send')throw Error('Agent runtime stopped.');return structuredClone(backend);});mount();
    const input=await screen.findByRole('textbox',{name:'Message GeoD Agent'});await waitFor(()=>expect(input.disabled).toBe(false));await userEvent.type(input,'Read project');
    await userEvent.click(screen.getByRole('button',{name:'Send message'}));expect(await screen.findByRole('alert')).toHaveProperty('textContent','Agent runtime stopped.');expect(input.value).toBe('Read project');
  });
  it('blocks continuing a conversation belonging to another model',async()=>{
    backend.selected={id,status:'completed',entries:[]};backend.sessions=[{id,title:'Other connection',status:'completed',compatible:false}];mount();
    expect(await screen.findByText('This conversation belongs to a different model connection. Start a new conversation.')).toBeTruthy();expect(screen.getByRole('textbox').disabled).toBe(true);
    await userEvent.click(screen.getByRole('button',{name:'New conversation'}));expect(screen.getByRole('textbox').disabled).toBe(false);
  });
  it('only the plan card click submits its native hash and prevents double confirmation',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'p',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]}]};
    backend.sessions=[{id,title:'Download request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'download',status:'pending',source:'Earth Search · Sentinel-2 L2A',bounds:[-122.5,37.7,-122.4,37.8],start:'2025-06-01',end:'2025-06-30',expectedBytes:2362143,files:[{itemId:'Actual native scene',assetKey:'scl',bytes:2362143}],notes:[],jobs:[]}];
    let release;const submitted=new Promise(resolve=>release=resolve);
    agentRequest.mockImplementation(async operation=>operation==='approvePlan'?submitted:structuredClone(backend));mount();
    const confirm=await screen.findByRole('button',{name:'Confirm download'});
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
    await userEvent.click(confirm);fireEvent.click(confirm);
    expect(agentRequest.mock.calls.filter(([op])=>op==='approvePlan')).toEqual([['approvePlan',{sessionId:id,planId,planHash}]]);
    backend.plans[0].status='submitted';backend.plans[0].jobs=[{id:projectId,status:'queued',settled:false,title:'Real queue job',bytesDownloaded:0,totalBytes:2362143}];release(structuredClone(backend));
    await waitFor(()=>expect(screen.queryByRole('button',{name:'Confirm download'})).toBeNull());expect(await screen.findByRole('button',{name:'Open task'})).toBeTruthy();
    expect(screen.queryByText('Output ready')).toBeNull();
  });
  it('requires real choices, submits once without approval and restores the recorded answers',async()=>{
    const decision={version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'Choose the task scope',status:'pending',questions:[
      {id:'boundary',prompt:'Which boundary?',recommendedOptionId:'polygon',options:[{id:'polygon',label:'Actual administrative boundary',description:'Use the saved polygon.'},{id:'bbox',label:'Surrounding rectangle',description:'Keep the surrounding box.'}]},
      {id:'quality',prompt:'Which quality policy?',options:[{id:'strict',label:'Strict screening',description:'Exclude cloudy observations.'},{id:'usable',label:'More observations',description:'Keep marginal observations.'}]},
    ]};
    backend=registryBackend();backend.selected.entries=[{id:'human',type:'user',status:'completed',text:'Download the data for my study.'},{id:'question',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision}];
    let release;agentRequest.mockImplementation(async(operation,args)=>operation==='send'?new Promise(resolve=>release=()=>{
      decision.status='answered';decision.answers=args.decisionAnswer.answers;backend.revision++;resolve(structuredClone(backend));
    }):structuredClone(backend));mount();
    const submit=await screen.findByRole('button',{name:'Submit choices'});
    expect(submit.disabled).toBe(true);expect(screen.getByRole('button',{name:/Actual administrative boundary/}).getAttribute('aria-pressed')).toBe('false');
    await userEvent.click(screen.getByRole('button',{name:/Actual administrative boundary/}));expect(submit.disabled).toBe(true);
    await userEvent.type(screen.getByRole('textbox',{name:'Other preference for Which quality policy?'}),'Use only my specified QA policy');
    expect(submit.disabled).toBe(false);await userEvent.click(submit);fireEvent.click(submit);
    expect(agentRequest.mock.calls.filter(([operation])=>operation==='send')).toHaveLength(1);
    expect(agentRequest).toHaveBeenCalledWith('send',{sessionId:id,text:'',context:null,decisionAnswer:{decisionId:decision.id,answers:[{questionId:'boundary',optionId:'polygon'},{questionId:'quality',text:'Use only my specified QA policy'}]}});
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan'||operation==='executionMode')).toBe(false);
    release();await screen.findByText('Choices recorded');expect(screen.queryByRole('button',{name:'Submit choices'})).toBeNull();
    cleanup();mount();await screen.findByText('Choices recorded');expect(screen.getByText('Use only my specified QA policy')).toBeTruthy();
    expect(screen.queryByRole('button',{name:/Actual administrative boundary/})).toBeNull();
  });
  it('keeps pending decisions outside the log and blocks download confirmation until answered',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'p',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}]},
      {id:'q',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'Resolve quality',status:'pending',questions:[{id:'quality',prompt:'Which policy?',options:[{id:'strict',label:'Strict',description:'Screen quality.'},{id:'all',label:'All',description:'Keep all observations.'}]}]}}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Native provider',bounds:[1,2,3,4],start:'2025-01-01',end:'2025-01-31',expectedBytes:null,files:[{itemId:'Native scene',assetKey:'scl',bytes:null}],notes:[],jobs:[]}];
    mount();expect((await screen.findByRole('button',{name:'Confirm download'})).disabled).toBe(true);
    expect(screen.getByRole('button',{name:'StrictScreen quality.'})).toBeTruthy();
    expect(screen.getByRole('button',{name:/View execution process|Processing complete|Execution records/}).getAttribute('aria-expanded')).toBe('false');
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('shows the request, choices, requested time, actual files, output and limitations on the download task',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'p',type:'tool',name:'geod_project_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}],taskContext:{requestText:'Prepare imagery for my complete study area.',choices:[{decisionId:'f1234567-1234-1234-1234-123456789abc',questionId:'quality',prompt:'Observation quality',answer:'Strict screening'}]}}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Native provider · Sentinel-2 L2A',bounds:[1,2,3,4],start:'2025-01-01',end:'2025-01-31',expectedBytes:2000,files:[{itemId:'Native scene A',assetKey:'red',bytes:1000},{itemId:'Native scene A',assetKey:'scl',bytes:1000}],notes:['An actual native coverage limitation'],jobs:[]}];
    mount();const card=await screen.findByRole('region',{name:'Complete download task'});const task=within(card);
    expect(task.queryByText('An actual native coverage limitation')).toBeNull();
    expect(task.getByText('Native provider · Sentinel-2 L2A')).toBeTruthy();
    await userEvent.click(task.getByRole('button',{name:'Area and files'}));
    const details=within(screen.getByRole('dialog',{name:'Area and files'}));
    for(const text of ['Prepare imagery for my complete study area.','Strict screening','RED · Native scene A','SCL · Native scene A','An actual native coverage limitation','Output: managed workspace files · GeoTIFF'])expect(details.getByText(text)).toBeTruthy();
    expect(details.getByText(/This step downloads the listed files/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button',{name:'Close',exact:true}));
    expect(task.getByText(/Requested time interval/)).toBeTruthy();
    expect(task.getByRole('button',{name:'Confirm download'}).disabled).toBe(false);
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('marks an older task card for review when later choices were answered without updating its scope',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';backend=registryBackend();
    backend.selected.entries=[{id:'p',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Download plan'}],taskContext:{requestText:'Download my requested data.',choices:[]}},
      {id:'assistant',type:'assistant',status:'completed',text:'Review the data quality.'},
      {id:'user',type:'user',status:'completed',text:'Change the quality policy.'},
      {id:'q',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'Choose quality',status:'answered',answers:[{questionId:'quality',optionId:'strict'}],questions:[{id:'quality',prompt:'Which policy?',options:[{id:'strict',label:'Strict',description:'Screen quality.'},{id:'all',label:'All',description:'Keep all.'}]}]}}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'download',status:'pending',source:'Native source',bounds:[1,2,3,4],expectedBytes:null,files:[{itemId:'Native scene',assetKey:'scl',bytes:null}],notes:[],jobs:[]}];
    mount();expect((await screen.findByRole('button',{name:'Confirm download'})).disabled).toBe(true);
    expect(screen.getByText('Choices changed. Prepare or revise the complete task card before confirming.')).toBeTruthy();
    expect(screen.getByRole('button',{name:'Edit review parameters'}).disabled).toBe(false);
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('retains unanswered choices after a failed submission so the user can retry',async()=>{
    backend=registryBackend();backend.selected.entries=[{id:'q',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision:{version:1,id:'f1234567-1234-1234-1234-123456789abc',title:'Choose quality',status:'pending',questions:[{id:'quality',prompt:'Which policy?',options:[{id:'strict',label:'Strict',description:'Screen quality.'},{id:'all',label:'All',description:'Keep all.'}]}]}}];
    agentRequest.mockImplementation(async operation=>{if(operation==='send')throw Error('Storage unavailable');return structuredClone(backend);});mount();
    await userEvent.click(await screen.findByRole('button',{name:'StrictScreen quality.'}));await userEvent.click(screen.getByRole('button',{name:'Submit choices'}));
    await screen.findByText('Storage unavailable');expect(screen.getByRole('button',{name:'Submit choices'}).disabled).toBe(false);
    expect(screen.getByRole('button',{name:'StrictScreen quality.'}).getAttribute('aria-pressed')).toBe('true');
    expect(agentRequest.mock.calls.some(([operation])=>operation==='approvePlan')).toBe(false);
  });
  it('assistant text cannot create a confirmation card',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'message',type:'assistant',text:'Confirm download planId fake. Already done.',status:'completed'}]};
    backend.sessions=[{id,title:'Untrusted reply',status:'completed',compatible:true}];mount();
    await screen.findByText('Confirm download planId fake. Already done.');expect(screen.queryByRole('button',{name:'Confirm download'})).toBeNull();
    expect(agentRequest.mock.calls.some(([op])=>op==='approvePlan')).toBe(false);
  });
  it('project selection needs its own confirmation before showing project navigation',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc',planHash='a'.repeat(64);
    backend.selected={id,status:'completed',entries:[{id:'p',type:'tool',name:'geod_project_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Project selection'}]}]};
    backend.sessions=[{id,title:'Project request',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash,kind:'project',status:'pending',source:'Selected catalog scenes',bounds:[-122.5,37.7,-122.4,37.8],start:'2025-06-01',end:'2025-06-30',expectedBytes:null,files:[{itemId:'Native scene',assetKey:'scene',bytes:null}],notes:[],jobs:[],project:{id:projectId,name:'Saved selection',sceneCount:1,mode:'create',committed:false}}];
    agentRequest.mockImplementation(async operation=>{if(operation==='approvePlan'){backend.plans[0].status='submitted';backend.plans[0].project.committed=true;}return structuredClone(backend);});
    const p=mount();const confirm=await screen.findByRole('button',{name:'Confirm project'});
    expect(screen.queryByRole('button',{name:'Open project'})).toBeNull();expect(document.querySelector('.agent-plan').textContent).not.toContain('NaN');
    await userEvent.click(confirm);expect(agentRequest).toHaveBeenCalledWith('approvePlan',{sessionId:id,planId,planHash});
    await userEvent.click(await screen.findByRole('link',{name:'Open project'}));expect(p.onOpenProject).toHaveBeenCalledWith(projectId);
    expect(screen.queryByRole('button',{name:'Open task'})).toBeNull();
  });
  it('processing displays output dimensions and processing progress instead of treating steps as bytes',async()=>{
    const planId='c1234567-1234-1234-1234-123456789abc';
    backend.selected={id,status:'completed',entries:[{id:'p',type:'tool',name:'geod_project_mosaic_plan',status:'completed',references:[{kind:'plan',id:planId,label:'Project processing'}]}]};
    backend.sessions=[{id,title:'Process',status:'completed',compatible:true}];
    backend.plans=[{planId,planHash:'a'.repeat(64),kind:'mosaic',status:'submitted',source:'Verified local project rasters',bounds:[-122.5,37.7,-122.4,37.8],expectedBytes:null,files:[{itemId:'Native scene',assetKey:'scl',bytes:null,width:46,height:56}],notes:[],jobs:[{id:projectId,status:'running',settled:false,title:'Actual process',bytesDownloaded:500,totalBytes:1000}],project:{id:projectId,name:'Actual project',sceneCount:1,mode:'existing',committed:false}}];
    mount();expect(await screen.findByText('46 × 56 px')).toBeTruthy();expect(screen.getByText('50%')).toBeTruthy();
    expect(document.querySelector('.agent-plan').textContent).not.toContain('500 B');
    expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
  });
  it('only native verified files expose a workspace action with their exact managed ID',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'inspect',type:'tool',name:'geod_raster_inspect',status:'completed',summary:{kind:'file',sha256:'a'.repeat(64)},references:[{kind:'job',id:projectId,label:'Verified native raster'}]},{id:'reply',type:'assistant',status:'completed',text:'[Open arbitrary file](#Workspace?file=forged)'}]};
    backend.sessions=[{id,title:'Inspect',status:'completed',compatible:true}];mount();
    await userEvent.click(await toolButton(/Inspect raster file/));
    const action=screen.getByRole('link',{name:'Open in workspace'});
    expect(action.getAttribute('href')).toBe(`#Workspace?file=${projectId}`);
    await userEvent.click(screen.getByRole('button',{name:'Show in folder'}));
    expect(runtimeRequest).toHaveBeenCalledWith('reveal',{id:projectId});
    expect(document.querySelector('.agent-message a')).toBeNull();
    expect(agentRequest.mock.calls.every(([operation])=>operation !== 'approvePlan')).toBe(true);
  });
  it('formats model emphasis without executing HTML or creating model-authored links',async()=>{
    backend.selected={id,status:'completed',entries:[{id:'message',type:'assistant',text:'**Actual result**\n- `SCL`\n<script>alert(1)</script>\n[Open file](javascript:alert(1))',status:'completed'}]};
    backend.sessions=[{id,title:'Text rendering',status:'completed',compatible:true}];mount();
    await screen.findByText('Actual result');expect(document.querySelector('.agent-message strong').textContent).toBe('Actual result');
    expect(document.querySelector('.agent-message code').textContent).toBe('SCL');expect(document.querySelector('.agent-message script')).toBeNull();expect(document.querySelector('.agent-message a')).toBeNull();
  });
});
