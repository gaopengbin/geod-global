import React from 'react';
import {beforeEach,it,expect,vi} from 'vitest';
import {act,render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {ScientificRgbDialog} from './scientific-rgb-ui.jsx';
import {RuntimeJobRows} from './runtime-ui.jsx';
import {RuntimeContext} from './runtime-context.js';
import {runtimeRequest} from './runtime-client.js';

const sources=['red','green','blue'].map((band,i)=>({id:`00000000-0000-4000-8000-00000000000${i}`,assetKey:band,sha256:String(i+1).repeat(64)}));
const group={id:'rgb:test',sourceJobs:sources};
const plan={spec:{name:'Test RGB',grid:{width:3,height:2,crs:'EPSG:32610',bounds:[500000,4199940,500090,4200000],pixelSize:[30,30],pixelInterpretation:'PixelIsArea'},profile:{product:'hls-l30-v2',signed:true,scale:.0001,offset:0,nodata:-9999},sources:sources.map(s=>({jobId:s.id,band:s.assetKey,sha256:s.sha256,href:'https://example.org/'+s.assetKey,attribution:'QA'})),schemaVersion:'geod-scientific-rgb/v1'},requiredDiskBytes:8000000};
const saved={id:'00000000-0000-4000-8000-000000000009',kind:'raster_rgb',itemId:'MYD09A1.A2025177.h08v05.061.2025189031924',title:'Test RGB',assetKey:'reflectance_rgb',status:'succeeded',mediaType:'image/tiff',bytesDownloaded:1000,sha256:'9'.repeat(64),rgbSpec:plan.spec};
beforeEach(()=>{vi.restoreAllMocks();delete window.__TAURI__;Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});

it('reviews the real grid and queues a named native RGB task using only source IDs',async()=>{
  const invoke=vi.fn().mockImplementation(command=>Promise.resolve(command==='plan_scientific_rgb'?plan:saved));window.__TAURI__={core:{invoke}};
  const queued=vi.fn();render(<I18nProvider><ScientificRgbDialog group={group} projectId="project-one" onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  const name=await screen.findByRole('textbox',{name:'Result name'});expect(screen.getByText('Int16 · RGB · EPSG:32610')).toBeTruthy();
  const user=userEvent.setup();await user.clear(name);await user.type(name,'Research RGB');await user.click(screen.getByRole('button',{name:'Create RGB file'}));
  await waitFor(()=>expect(queued).toHaveBeenCalledWith(saved));expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:sources.map(s=>s.id),projectId:'project-one',name:'Research RGB'}});
});
it('failed preflight has no enabled create action or queued callback',async()=>{
  window.__TAURI__={core:{invoke:vi.fn().mockRejectedValue('RGB bands use different scene selections or processing areas')}};const queued=vi.fn();
  render(<I18nProvider><ScientificRgbDialog group={group} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  expect(await screen.findByRole('alert')).toBeTruthy();expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();expect(queued).not.toHaveBeenCalled();
});
it('saved RGB uses the workspace reader and a result badge, with no clip recipe requirement',async()=>{
  render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[saved],projects:[]}}><RuntimeJobRows jobs={[saved]} library/></RuntimeContext.Provider></I18nProvider>);
  expect(screen.getByText('Test RGB')).toBeTruthy();
  expect(screen.getByText('Scientific RGB · GeoTIFF')).toBeTruthy();expect(screen.getByText('Scientific RGB')).toBeTruthy();
  expect(screen.getByRole('link',{name:'Open in workspace'}).hash).toBe('#Workspace?file='+saved.id);
  await userEvent.setup().click(screen.getByRole('button',{name:'File details and provenance'}));
  expect(screen.getByRole('button',{name:'Prepare delivery package'})).toBeTruthy();expect(screen.queryByText('Crop recipe')).toBeNull();
});
it('browser and native scientific RGB operations use matching shared contracts',async()=>{
  const invoke=vi.fn().mockResolvedValue(plan);window.__TAURI__={core:{invoke}};const payload={jobIds:sources.map(s=>s.id)};
  await runtimeRequest('planRgb',payload);expect(invoke).toHaveBeenLastCalledWith('plan_scientific_rgb',{request:payload});delete window.__TAURI__;
  const fetch=vi.spyOn(globalThis,'fetch').mockResolvedValue({ok:true,json:async()=>saved});await runtimeRequest('runRgb',payload);
  expect(fetch).toHaveBeenLastCalledWith('http://127.0.0.1:4318/rasters/rgb',expect.objectContaining({method:'POST',body:JSON.stringify(payload)}));
});

const modisItem='MYD09A1.A2025177.h08v05.061.2025189031924';
const modisLayers={red:'sur_refl_b01',green:'sur_refl_b04',blue:'sur_refl_b03',modis_qc:'sur_refl_qc_500m',modis_state:'sur_refl_state_500m'};
const modisJobs=Object.keys(modisLayers).map((assetKey,i)=>({id:`00000000-0000-4000-8000-00000000000${i}`,assetKey,kind:'download',status:'succeeded',itemId:modisItem,sha256:String(i+1).repeat(64),href:`https://modiseuwest.blob.core.windows.net/modis-061-cogs/MYD09A1/08/05/2025177/${modisItem}_${modisLayers[assetKey]}.tif`}));
const modisGroup={id:'rgb:modis-test',product:'modis-09a1-v061',sourceJobs:modisJobs.slice(0,3)};
it('quality selection replans with pinned local flags, preserves the edited name and queues the selected snow rule',async()=>{
  const invoke=vi.fn().mockImplementation((command,{request})=>{
    if (!request) return Promise.resolve();
    const qualityMask=request.qualityMask?{...request.qualityMask,schemaVersion:'geod-modis-rgb-mask/v1',sources:[{jobId:request.qualityMask.qcJobId},{jobId:request.qualityMask.stateJobId}]}:undefined;
    return Promise.resolve(command==='plan_scientific_rgb'?{...plan,spec:{...plan.spec,name:qualityMask?'Masked RGB':'Original RGB',qualityMask}}:saved);
  });window.__TAURI__={core:{invoke}};const queued=vi.fn();
  render(<I18nProvider><ScientificRgbDialog group={modisGroup} jobs={modisJobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  const user=userEvent.setup(), name=await screen.findByRole('textbox',{name:'Result name'});await user.clear(name);await user.type(name,'Clear area');
  await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Clear pixels with best RGB quality'}));
  await screen.findByText('Accepted pixels keep original DN. Rejected pixels become NoData; the rules and source checksums are saved with the result.');
  await user.click(screen.getByRole('checkbox',{name:'Also exclude snow and ice'}));
  await waitFor(()=>expect(invoke).toHaveBeenLastCalledWith('plan_scientific_rgb',{request:{jobIds:modisJobs.slice(0,3).map(j=>j.id),qualityMask:{qcJobId:modisJobs[3].id,stateJobId:modisJobs[4].id,policy:'clear_best',excludeSnow:true}}}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create RGB file'}).disabled).toBe(false));
  expect(screen.getByRole('textbox',{name:'Result name'}).value).toBe('Clear area');
  await user.click(screen.getByRole('button',{name:'Create RGB file'}));
  await waitFor(()=>expect(queued).toHaveBeenCalledWith(saved));expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:modisJobs.slice(0,3).map(j=>j.id),qualityMask:{qcJobId:modisJobs[3].id,stateJobId:modisJobs[4].id,policy:'clear_best',excludeSnow:true},name:'Clear area'}});
});
it('missing local quality layers explains availability and keeps unmasked creation usable',async()=>{
  window.__TAURI__={core:{invoke:vi.fn().mockResolvedValue(plan)}};
  render(<I18nProvider><ScientificRgbDialog group={modisGroup} jobs={modisJobs.slice(0,3)} onClose={vi.fn()} onQueued={vi.fn()}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});expect(screen.getByText('Download the matching MODIS QC and state files to enable screening.')).toBeTruthy();
  const user=userEvent.setup();await user.click(screen.getByRole('combobox',{name:'Quality screening'}));expect(screen.getByRole('option',{name:'Clear pixels with best RGB quality'}).getAttribute('aria-disabled')).toBe('true');
});
it('a server that omits the chosen quality rules cannot expose a create action',async()=>{
  window.__TAURI__={core:{invoke:vi.fn().mockResolvedValue(plan)}};const queued=vi.fn();
  render(<I18nProvider><ScientificRgbDialog group={modisGroup} jobs={modisJobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Exclude cloud and shadow flags'}));
  await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();expect(queued).not.toHaveBeenCalled();
});

function multiModisFixture() {
  const olderItem='MYD09A1.A2025169.h08v05.061.2025178155736';
  const older=modisJobs.map((j,i)=>({...j,id:`older-${i}`,itemId:olderItem,href:j.href.replace('2025177/','2025169/').replace(modisItem,olderItem)}));
  const outputs=modisJobs.map((j,i)=>({...j,id:`multi-${i}`,kind:'raster_mosaic',itemId:'project:multi',
    mosaic:{projectId:'multi',assetKey:j.assetKey,sources:[older[i],j].map(s=>({jobId:s.id,sha256:s.sha256}))},
    mosaicOutput:{width:8,height:5,crs:'MODIS:Sinusoidal',bounds:[0,0,3706.50173222334,2316.56358263959],pixelSize:[463.312716527918,463.312716527918],bandCount:1,
      ...(i<3?{calibration:{product:'modis-09a1-v061'}}:{quality:{product:'modis-09a1-v061'}})}}));
  return {jobs:[...older,...modisJobs,...outputs],outputs,group:{...modisGroup,derived:true,sourceJobs:outputs.slice(0,3)}};
}
it('matched multi-scene quality reviews coherent selection and queues only the existing local IDs',async()=>{
  const fixture=multiModisFixture(),queued=vi.fn();
  const invoke=vi.fn().mockImplementation((command,{request})=>Promise.resolve(command==='plan_scientific_rgb'?{...plan,spec:{...plan.spec,
    qualityMask:request.qualityMask?{...request.qualityMask,schemaVersion:'geod-modis-rgb-mask/v2',sources:fixture.outputs.slice(3).map(j=>({jobId:j.id})),coupled:{scenes:[{},{}]}}:undefined}}:saved));
  window.__TAURI__={core:{invoke}};
  render(<I18nProvider><ScientificRgbDialog group={fixture.group} jobs={fixture.jobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();
  await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Clear pixels with best RGB quality'}));
  expect(await screen.findByText('Overlaps use the newest qualified complete RGB scene. An older qualified scene fills flagged or incomplete newer pixels.')).toBeTruthy();
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create RGB file'}).disabled).toBe(false));
  await user.click(screen.getByRole('button',{name:'Create RGB file'}));await waitFor(()=>expect(queued).toHaveBeenCalledWith(saved));
  expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:fixture.outputs.slice(0,3).map(j=>j.id),name:'Test RGB',
    qualityMask:{qcJobId:'multi-3',stateJobId:'multi-4',policy:'clear_best',excludeSnow:false}}});
});
it('a multi-scene preflight with only independent QA cannot enable creation',async()=>{
  const fixture=multiModisFixture(),queued=vi.fn();
  window.__TAURI__={core:{invoke:vi.fn().mockImplementation((command,{request})=>Promise.resolve({...plan,spec:{...plan.spec,
    qualityMask:request.qualityMask?{...request.qualityMask,schemaVersion:'geod-modis-rgb-mask/v1',sources:fixture.outputs.slice(3).map(j=>({jobId:j.id}))}:undefined}}))}};
  render(<I18nProvider><ScientificRgbDialog group={fixture.group} jobs={fixture.jobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();
  await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Exclude cloud and shadow flags'}));
  await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();expect(queued).not.toHaveBeenCalled();
});

const landsatItem='LC09_L2SP_044034_20250628_02_T1', landsatProduct='LC09_L2SP_044034_20250628_20250629_02_T1';
const landsatJobs=['red','green','blue','qa_pixel','qa_radsat'].map((assetKey,i)=>({...modisJobs[i],assetKey,itemId:landsatItem,
  href:`https://landsateuwest.blob.core.windows.net/landsat-c2/level-2/standard/oli-tirs/2025/044/034/${landsatProduct}/${landsatProduct}_${['SR_B4','SR_B3','SR_B2','QA_PIXEL','QA_RADSAT'][i]}.TIF`}));
const landsatGroup={id:'rgb:landsat',product:'landsat-c2-l2',sourceJobs:landsatJobs.slice(0,3)};
it('Landsat selection sends its own five-layer pins and reviews the conservative snow rule',async()=>{
  const queued=vi.fn(),invoke=vi.fn().mockImplementation((command,{request})=>Promise.resolve(command==='plan_scientific_rgb'?{...plan,spec:{...plan.spec,
    qualityMask:request.qualityMask?{...request.qualityMask,schemaVersion:'geod-landsat-rgb-mask/v1',sources:[{jobId:request.qualityMask.qaPixelJobId},{jobId:request.qualityMask.qaRadsatJobId}]}:undefined}}:saved));
  window.__TAURI__={core:{invoke}};
  render(<I18nProvider><ScientificRgbDialog group={landsatGroup} jobs={landsatJobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();
  await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Conservative cloud-free flags'}));
  await screen.findByText('Require clear and explicit low cloud, shadow and cirrus confidence. Unset confidence and unused saturation bits are excluded; water remains eligible.');
  await user.click(screen.getByRole('checkbox',{name:'Also exclude snow and ice'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create RGB file'}).disabled).toBe(false));
  await user.click(screen.getByRole('button',{name:'Create RGB file'}));await waitFor(()=>expect(queued).toHaveBeenCalledWith(saved));
  expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:landsatJobs.slice(0,3).map(j=>j.id),name:'Test RGB',
    qualityMask:{qaPixelJobId:landsatJobs[3].id,qaRadsatJobId:landsatJobs[4].id,policy:'cloud_free_conservative',excludeSnow:true}}});
});
it('missing Landsat QA explains the required files while keeping unmasked creation available',async()=>{
  window.__TAURI__={core:{invoke:vi.fn().mockResolvedValue(plan)}};
  render(<I18nProvider><ScientificRgbDialog group={landsatGroup} jobs={landsatJobs.slice(0,4)} onClose={vi.fn()} onQueued={vi.fn()}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});
  expect(screen.getByText('Download matching Landsat QA_PIXEL and QA_RADSAT files to enable screening.')).toBeTruthy();
  await userEvent.setup().click(screen.getByRole('combobox',{name:'Quality screening'}));
  expect(screen.getByRole('option',{name:'Exclude cloud, shadow and RGB saturation'}).getAttribute('aria-disabled')).toBe('true');
});

function multiLandsatFixture(){
  const older=landsatJobs.map((j,i)=>({...j,id:`older-landsat-${i}`,itemId:j.itemId.replace('20250628','20250612'),href:j.href.replaceAll('20250628','20250612').replaceAll('20250629','20250613')}));
  const outputs=landsatJobs.map((j,i)=>({...j,id:`landsat-multi-${i}`,kind:'raster_mosaic',itemId:'project:landsat-multi',
    mosaic:{projectId:'landsat-multi',sources:[older[i],j].map(s=>({jobId:s.id,sha256:s.sha256}))},
    mosaicOutput:{width:8,height:5,crs:'EPSG:32610',bounds:[0,0,240,150],pixelSize:[30,30],bandCount:1,
      ...(i<3?{calibration:{product:'landsat-c2-l2'}}:{landsatQuality:{product:'landsat-c2-l2'}})}}));
  return {jobs:[...older,...landsatJobs,...outputs],outputs,group:{...landsatGroup,derived:true,sourceJobs:outputs.slice(0,3)}};
}
it('Landsat multi-scene selection reviews whole-triplet fallback and uses the Landsat v2 plan',async()=>{
  const fixture=multiLandsatFixture(),queued=vi.fn(),invoke=vi.fn().mockImplementation((command,{request})=>Promise.resolve(command==='plan_scientific_rgb'?{...plan,spec:{...plan.spec,
    qualityMask:request.qualityMask?{...request.qualityMask,schemaVersion:'geod-landsat-rgb-mask/v2',sources:fixture.outputs.slice(3).map(j=>({jobId:j.id})),coupled:{scenes:[{},{}]}}:undefined}}:saved));
  window.__TAURI__={core:{invoke}};
  render(<I18nProvider><ScientificRgbDialog group={fixture.group} jobs={fixture.jobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();
  await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Conservative cloud-free flags'}));
  expect(await screen.findByText('Overlaps use the newest qualified complete RGB scene. An older qualified scene fills flagged or incomplete newer pixels.')).toBeTruthy();
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create RGB file'}).disabled).toBe(false));await user.click(screen.getByRole('button',{name:'Create RGB file'}));
  await waitFor(()=>expect(queued).toHaveBeenCalledWith(saved));
  expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:fixture.outputs.slice(0,3).map(j=>j.id),name:'Test RGB',
    qualityMask:{qaPixelJobId:fixture.outputs[3].id,qaRadsatJobId:fixture.outputs[4].id,policy:'cloud_free_conservative',excludeSnow:false}}});
});
it('a Landsat multi-scene preflight without coherent scene evidence keeps creation unavailable',async()=>{
  const fixture=multiLandsatFixture(),queued=vi.fn();window.__TAURI__={core:{invoke:vi.fn().mockImplementation((command,{request})=>Promise.resolve({...plan,spec:{...plan.spec,
    qualityMask:request.qualityMask?{...request.qualityMask,schemaVersion:'geod-landsat-rgb-mask/v1',sources:fixture.outputs.slice(3).map(j=>({jobId:j.id}))}:undefined}}))}};
  render(<I18nProvider><ScientificRgbDialog group={fixture.group} jobs={fixture.jobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  await screen.findByRole('button',{name:'Create RGB file'});const user=userEvent.setup();await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Conservative cloud-free flags'}));
  await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();expect(queued).not.toHaveBeenCalled();
});

it('a busy raster read waits and resumes preflight without queuing a task',async()=>{
  vi.useFakeTimers();
  const invoke=vi.fn().mockRejectedValueOnce('Raster inspection is busy; try again shortly').mockResolvedValue(plan),queued=vi.fn();
  window.__TAURI__={core:{invoke}};
  const view=render(<I18nProvider><ScientificRgbDialog group={group} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  try {
    await act(async()=>{});
    expect(screen.getByRole('status').textContent).toContain('Waiting for the current raster read…');
    expect(screen.queryByRole('alert')).toBeNull();expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();
    await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
    expect(screen.getByRole('button',{name:'Create RGB file'}).disabled).toBe(false);
    expect(invoke.mock.calls.some(([command])=>command==='run_scientific_rgb')).toBe(false);expect(queued).not.toHaveBeenCalled();
  } finally {view.unmount();vi.useRealTimers();}
});

it('changing quality rules cancels obsolete retries and preserves the edited result name',async()=>{
  const attempts=new Map(),queued=vi.fn();
  const invoke=vi.fn().mockImplementation((command,{request})=>{
    if(!request)return Promise.resolve();
    if(command==='run_scientific_rgb')return Promise.resolve(saved);
    const mask=request.qualityMask;
    if(mask){
      const key=JSON.stringify(mask),count=(attempts.get(key)||0)+1;attempts.set(key,count);
      if(count===1)return Promise.reject('Raster inspection is busy; try again shortly');
    }
    return Promise.resolve({...plan,spec:{...plan.spec,qualityMask:mask?{...mask,schemaVersion:'geod-landsat-rgb-mask/v1',sources:[{jobId:mask.qaPixelJobId},{jobId:mask.qaRadsatJobId}]}:undefined}});
  });window.__TAURI__={core:{invoke}};
  const view=render(<I18nProvider><ScientificRgbDialog group={landsatGroup} jobs={landsatJobs} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  const name=await screen.findByRole('textbox',{name:'Result name'}),initialUser=userEvent.setup();
  await initialUser.clear(name);await initialUser.type(name,'Retained name');
  try {
    const user=userEvent.setup();
    await user.click(screen.getByRole('combobox',{name:'Quality screening'}));await user.click(screen.getByRole('option',{name:'Conservative cloud-free flags'}));
    await user.click(screen.getByRole('checkbox',{name:'Also exclude snow and ice'}));
    await screen.findByRole('button',{name:'Create RGB file'});
    expect(attempts.get(JSON.stringify({qaPixelJobId:landsatJobs[3].id,qaRadsatJobId:landsatJobs[4].id,policy:'cloud_free_conservative',excludeSnow:false}))).toBe(1);
    expect(screen.getByRole('textbox',{name:'Result name'}).value).toBe('Retained name');
    await user.click(screen.getByRole('button',{name:'Create RGB file'}));await act(async()=>{});
    expect(queued).toHaveBeenCalledWith(saved);
    expect(invoke).toHaveBeenLastCalledWith('run_scientific_rgb',{request:{jobIds:landsatJobs.slice(0,3).map(j=>j.id),name:'Retained name',qualityMask:{qaPixelJobId:landsatJobs[3].id,qaRadsatJobId:landsatJobs[4].id,policy:'cloud_free_conservative',excludeSnow:true}}});
  } finally {view.unmount();}
});

it('closing the dialog cancels busy preflight retries',async()=>{
  vi.useFakeTimers();const invoke=vi.fn().mockRejectedValue('Raster inspection is busy; try again shortly');window.__TAURI__={core:{invoke}};
  const view=render(<I18nProvider><ScientificRgbDialog group={group} onClose={vi.fn()} onQueued={vi.fn()}/></I18nProvider>);
  try {
    await act(async()=>{});view.unmount();const calls=invoke.mock.calls.length;
    await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});expect(invoke).toHaveBeenCalledTimes(calls);
  } finally {view.unmount();vi.useRealTimers();}
});

it('continued raster contention stops after the bounded wait and cannot submit a task',async()=>{
  vi.useFakeTimers();const queued=vi.fn(),invoke=vi.fn().mockRejectedValue('Raster inspection is busy; try again shortly');window.__TAURI__={core:{invoke}};
  const view=render(<I18nProvider><ScientificRgbDialog group={group} onClose={vi.fn()} onQueued={queued}/></I18nProvider>);
  try {
    await act(async()=>{await vi.advanceTimersByTimeAsync(60000);});
    expect(screen.getByRole('alert').textContent).toContain('Raster reading is taking longer than expected. Try again shortly.');
    expect(screen.queryByRole('button',{name:'Create RGB file'})).toBeNull();expect(queued).not.toHaveBeenCalled();
    const calls=invoke.mock.calls.length;await act(async()=>{await vi.advanceTimersByTimeAsync(30000);});expect(invoke).toHaveBeenCalledTimes(calls);
  } finally {view.unmount();vi.useRealTimers();}
});
