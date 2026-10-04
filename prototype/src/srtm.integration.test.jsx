import React from 'react';
import {readFileSync} from 'node:fs';
import {beforeEach,afterEach,expect,it,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
// Deferred authorized-download workflow; the candidate wrapper is tested separately.
import {OriginalDownloadWorkflow as DownloadAssetButton,RuntimeJobRows} from './runtime-ui.jsx';
import {CatalogFilters} from './catalog-filters.jsx';
import {ProjectsLibrary} from './projects-ui.jsx';
import {runtimeRequest} from './runtime-client.js';
import {normalizeScene} from './catalog.js';
import {projectRequest} from './projects-client.js';
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:vi.fn()}));
const scene=normalizeScene(JSON.parse(readFileSync('prototype/public/samples/srtm-response.json','utf8')).features[0],'nasa-srtm');
const project={...projectRequest({scenes:[scene],bounds:[-122.6,37.5,-122.3,37.8],name:'SRTM tiles'}),id:'srtm-project'};
const wrap=children=>render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],act:vi.fn(),refresh:vi.fn()}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
afterEach(()=>{delete window.__TAURI__;});
it('SRTM search uses geographic area without optical date or cloud controls',async()=>{
  const user=userEvent.setup(),apply=vi.fn();
  wrap(<CatalogFilters initialValues={{provider:'nasa-srtm',bbox:project.bounds.join(','),start:'',end:'',cloud:NaN}} onApply={apply} onClose={vi.fn()}/>);
  expect(screen.queryByLabelText('Search start date')).toBeNull();
  expect(screen.queryByLabelText('Live maximum cloud cover')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Search catalog'}));expect(apply).toHaveBeenCalledOnce();
});
it('original HGT file cards open in Workspace and missing Earthdata tasks link to the same account',()=>{
  const job={id:'srtm-original',kind:'download',status:'succeeded',itemId:scene.id,href:scene.assets.srtm.href,assetKey:'srtm',mediaType:'application/zip',sha256:'a'.repeat(64)};
  wrap(<RuntimeJobRows library jobs={[job]}/>);
  expect(screen.getByRole('link',{name:'Open in workspace'}).hash).toBe('#Workspace?file=srtm-original');
  wrap(<RuntimeJobRows jobs={[{...job,id:'srtm-failed',status:'failed',error:'Connect this data source in Settings before downloading protected files.'}]}/>);
  expect(screen.getByRole('link',{name:'Manage authorization'}).hash).toBe('#Settings?account=nasa-earthdata');
});
it('SRTM browser download review clearly requires desktop authorization and preserves HGT product format',async()=>{
  const user=userEvent.setup();wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="SRTM"/>);
  await user.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByRole('textbox',{name:'Project name'}).value).toBe('SRTM · SRTMGL1 v003');
  const combo=screen.getByRole('combobox',{name:'Download content'});expect(combo.textContent).toContain('Int16 HGT ZIP');
  expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);
  expect(screen.getByRole('link',{name:'Manage authorization'}).hash).toBe('#Settings?account=nasa-earthdata');
  expect(runtimeRequest).not.toHaveBeenCalled();
});
it('SRTM native review uses the existing Earthdata account and queues the SRTM original key',async()=>{
  window.__TAURI__={core:{invoke:vi.fn()}};
  runtimeRequest.mockImplementation(async operation=>operation==='accounts'?[{provider:'nasa-earthdata',status:'connected',expiresAt:'2099-01-01T00:00:00Z'}]:operation==='createProject'?project:{jobs:[{id:'srtm-job'}]});
  const user=userEvent.setup();wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="SRTM"/>);
  await user.click(screen.getByRole('button',{name:'Create project and download'}));
  await screen.findByText('Earthdata authorization is available. File access will be checked when the task starts.');
  await user.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.map(([op])=>op)).toEqual(['accounts','createProject','downloadProject']);
  expect(runtimeRequest.mock.calls[2][1].assetKey).toBe('srtm');
});
it('SRTM restored project reports EGM96 and requires every original before processing',async()=>{
  runtimeRequest.mockResolvedValue([project]);wrap(<ProjectsLibrary focusedProjectId={project.id}/>);
  await screen.findByRole('heading',{name:project.name});
  expect(screen.getByRole('button',{name:'Download elevation tiles'})).toBeTruthy();
  expect(screen.getByText('1 elevation tiles')).toBeTruthy();
  await userEvent.setup().click(screen.getByRole('button',{name:'Review selected scenes · 1'}));
  expect(screen.getByText('SRTMGL1 v003 · Int16 · EGM96')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Clip elevation to project area'}).disabled).toBe(true);
  expect(screen.queryByRole('button',{name:/Download SCL|Download true-color/})).toBeNull();
  expect(screen.queryByText(/EGM2008/)).toBeNull();
});
it('completed SRTM sources enable the project clip and dispatch the original SRTM key',async()=>{
  const act=vi.fn(),job={id:'srtm-source',kind:'download',status:'succeeded',itemId:scene.id,href:scene.assets.srtm.href,assetKey:'srtm',sha256:'a'.repeat(64)};
  runtimeRequest.mockImplementation(async operation=>operation==='projects'?[project]:{id:'srtm-clip',kind:'raster_mosaic',assetKey:'srtm'});
  render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[job],act,refresh:vi.fn()}}><ProjectsLibrary focusedProjectId={project.id}/></RuntimeContext.Provider></I18nProvider>);
  const button=await screen.findByRole('button',{name:'Clip elevation to project area'});
  expect(button.disabled).toBe(false);
  await userEvent.setup().click(button);
  expect(act).toHaveBeenCalledWith('mosaicProject',{id:project.id,assetKey:'srtm'});
  expect(screen.queryByText(/EGM2008|not available yet/)).toBeNull();
});
