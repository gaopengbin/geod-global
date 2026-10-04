import React from 'react';
import { readFileSync } from 'node:fs';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { DownloadAssetButton, RuntimeJobRows } from './runtime-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';
import { runtimeRequest } from './runtime-client.js';
import { normalizeScene } from './catalog.js';
import { projectRequest } from './projects-client.js';

vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn() }));
const scene = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/landsat-response.json','utf8')).features[0], 'planetary-landsat');
const project = { ...projectRequest({ scenes: [scene], bounds: [-122.55,37.68,-122.32,37.84], name: 'Landsat original bands' }), id: 'landsat-project' };
const jobs = ['red','green','blue'].map(key=>({ id:key,kind:'download',itemId:scene.id,assetKey:key,href:scene.assets[key].href,mediaType:scene.assets[key].type,status:'succeeded',bytesDownloaded:1000,sha256:'a'.repeat(64),updatedAt:scene.date }));
const wrap = (children, extra={}) => render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],projects:[project],refresh:vi.fn(),act:vi.fn(),...extra}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{
  vi.resetAllMocks();
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});
});
it('downloads all original channels into one named project without offering an 8-bit true-color or SCL substitute', async()=>{
  const user=userEvent.setup();
  runtimeRequest.mockImplementation(async (operation,payload)=>operation==='createProject'?project:{jobs:[{id:payload.assetKey}]});
  wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="Landsat"/>);
  await user.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('Red + green + blue');
  await user.click(screen.getByRole('combobox',{name:'Download content'}));
  expect(screen.getAllByRole('option')).toHaveLength(4);
  expect(screen.queryByRole('option',{name:/True-color|SCL/})).toBeNull();
  await user.keyboard('{Escape}');
  await user.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.map(([operation])=>operation)).toEqual(['createProject','downloadProject','downloadProject','downloadProject']);
  expect(runtimeRequest.mock.calls.slice(1).map(([,payload])=>payload.assetKey)).toEqual(['red','green','blue']);
  expect(runtimeRequest.mock.calls[0][1].scenes[0].assets.red.rasterBand.dataType).toBe('uint16');
});
it('restored Landsat projects process each original band into the same project area', async()=>{
  const user=userEvent.setup();
  const act=vi.fn();
  runtimeRequest.mockResolvedValue([project]);
  wrap(<ProjectsLibrary focusedProjectId={project.id}/>,{jobs,act});
  await screen.findByRole('heading',{name:project.name});
  expect(screen.getAllByRole('button',{name:'Download source band'})).toHaveLength(3);
  expect(screen.getAllByRole('button',{name:'Download source band'}).every(button=>button.disabled)).toBe(true);
  const clips=screen.getAllByRole('button',{name:'Clip band to project area'});
  expect(clips).toHaveLength(3);
  expect(clips.every(button=>!button.disabled)).toBe(true);
  await user.click(clips[0]);
  expect(act).toHaveBeenCalledWith('mosaicProject',{id:project.id,assetKey:'red'});
  expect(screen.getAllByRole('link',{name:'Open local RGB'})).toHaveLength(3);
  expect(screen.getByText(/Band processing preserves original DN/)).toBeTruthy();
});
it('original reflectance file details show the persisted conversion and inspection',async()=>{
  const user=userEvent.setup();
  wrap(<RuntimeJobRows jobs={[jobs[0]]} library/>,{jobs});
  await user.click(screen.getByRole('button',{name:'File details and provenance'}));
  expect(screen.getByText('DN × 0.0000275 + (-0.2)')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Inspect raster'})).toBeTruthy();
  expect(screen.queryByText('Preview image · JPEG')).toBeNull();
});

it.each(['planetary-landsat', 'nasa-earthdata'])('band inspection for %s shows calibration without SCL classes', async provider => {
  const user=userEvent.setup();
  const signed=provider==='nasa-earthdata';
  const sourceScene=signed?normalizeScene(JSON.parse(readFileSync('prototype/public/samples/nasa-response.json','utf8')).features[0],provider):scene;
  const job={...jobs[0], itemId:sourceScene.id,href:sourceScene.assets.red.href};
  const metadata={width:3,height:2,bandCount:1,dataType:signed?'Int16':'UInt16',crs:'EPSG:32610',
    bounds:[500000,4199940,500090,4200000],pixelSize:[30,30],nodata:signed?-9999:0,
    previewWidth:3,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',classes:[],sha256:job.sha256,
    reflectance:{product:signed?'hls-l30-v2':'landsat-c2-l2',band:'red',scale:signed?0.0001:0.0000275,offset:signed?0:-0.2,pixelInterpretation:'PixelIsArea',displayRange:[1000,10000],sampleCount:6,validSampleCount:5}};
  runtimeRequest.mockResolvedValue(metadata);
  wrap(<RuntimeJobRows jobs={[job]} library/>,{jobs:[job]});
  await user.click(screen.getByRole('button',{name:'File details and provenance'}));
  await user.click(screen.getByRole('button',{name:'Inspect raster'}));
  await screen.findByRole('heading',{name:'Reflectance display'});
  expect(screen.getByText(signed?'Int16':'UInt16')).toBeTruthy();
  expect(screen.getByText(/Sampled 2–98 percentiles/)).toBeTruthy();
  expect(screen.queryByRole('heading',{name:'Scene classes'})).toBeNull();
  expect(screen.getByAltText('Verified local reflectance band in grayscale')).toBeTruthy();
});
