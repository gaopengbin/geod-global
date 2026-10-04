import React from 'react';
import {readFileSync} from 'node:fs';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {DownloadAssetButton} from './runtime-ui.jsx';
import {CatalogFilters} from './catalog-filters.jsx';
import {ProjectsLibrary} from './projects-ui.jsx';
import {runtimeRequest} from './runtime-client.js';
import {normalizeScene} from './catalog.js';
import {projectRequest} from './projects-client.js';
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:vi.fn()}));
const scene=normalizeScene(JSON.parse(readFileSync('prototype/public/samples/cop-dem-response.json','utf8')).features[0],'copernicus-dem');
const scene90=normalizeScene(JSON.parse(readFileSync('prototype/public/samples/cop-dem-90-response.json','utf8')).features[0],'copernicus-dem-90');
const project={...projectRequest({scenes:[scene],bounds:[-122.55,37.68,-122.32,37.84],name:'DEM tiles'}),id:'dem-project'};
const wrap=(children,extra={})=>render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],projects:[project],act:vi.fn(),refresh:vi.fn(),...extra}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('DEM filtering submits a geographic area without optical date or cloud controls',async()=>{
  const user=userEvent.setup(),apply=vi.fn();
  wrap(<CatalogFilters initialValues={{provider:'copernicus-dem',bbox:project.bounds.join(','),start:'',end:'',cloud:NaN}} onApply={apply} onClose={vi.fn()}/>);
  expect(screen.queryByLabelText('Search start date')).toBeNull();
  expect(screen.queryByLabelText('Live maximum cloud cover')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Search catalog'}));expect(apply).toHaveBeenCalledOnce();
});
it.each([[scene,'GLO-30 Public'],[scene90,'GLO-90']])('DEM download creates a named %s project and queues the original elevation asset without authorization or optical substitutes',async(scene,productLabel)=>{
  const user=userEvent.setup();runtimeRequest.mockImplementation(async operation=>operation==='createProject'?project:{jobs:[{id:'dem-job'}]});
  wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="DEM"/>);
  await user.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByRole('textbox',{name:'Project name'}).value).toBe(`DEM · ${productLabel}`);
  const combo=screen.getByRole('combobox',{name:'Download content'});expect(combo.textContent).toContain('Float32');
  await user.click(combo);expect(screen.getAllByRole('option')).toHaveLength(1);await user.keyboard('{Escape}');
  expect(screen.queryByText('Manage authorization')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.map(([op])=>op)).toEqual(['createProject','downloadProject']);
  expect(runtimeRequest.mock.calls[1][1].assetKey).toBe('elevation');
});
it('restored DEM projects show tile counts and original height download and project clipping without optical metadata',async()=>{
  runtimeRequest.mockResolvedValue([project]);wrap(<ProjectsLibrary focusedProjectId={project.id}/>);
  await screen.findByRole('heading',{name:project.name});
  expect(screen.getByRole('button',{name:'Download elevation tiles'})).toBeTruthy();
  expect(screen.getByText('1 elevation tiles')).toBeTruthy();
  expect(screen.queryByText(/2021/)).toBeNull();
  expect(screen.getByRole('button',{name:'Clip elevation to project area'}).disabled).toBe(true);
  expect(screen.queryByRole('button',{name:/Mosaic and clip|Download SCL|Download true-color/})).toBeNull();
});

it('completed elevation originals enable the native project clip action',async()=>{
  const user=userEvent.setup(),act=vi.fn().mockResolvedValue({});
  const source={id:'dem-file',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'elevation',href:scene.assets.elevation.href,sha256:'a'.repeat(64),mediaType:'image/tiff'};
  runtimeRequest.mockImplementation(async operation=>operation==='projects'?[project]:operation==='thumbnail'?{dataUrl:'data:image/png;base64,AAAA'}:[]);
  wrap(<ProjectsLibrary focusedProjectId={project.id}/>,{jobs:[source],act});
  const clip=await screen.findByRole('button',{name:'Clip elevation to project area'});
  expect(clip.disabled).toBe(false);await user.click(clip);
  expect(act).toHaveBeenCalledWith('mosaicProject',{id:project.id,assetKey:'elevation'});
});
