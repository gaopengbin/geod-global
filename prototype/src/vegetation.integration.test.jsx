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
import { VEGETATION_PIXEL } from './vegetation.js';

vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn() }));
// Captured catalogue fields and simulated API replies test UI contracts only.
// Actual files and native processing have separate verification receipts.
const scene = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/modis-vegetation-response.json','utf8')).features[0], 'planetary-vegetation');
const project = { ...projectRequest({ scenes: [scene], bounds: [-122.55,37.68,-122.32,37.84], name: 'NDVI and EVI' }), id: 'vegetation-project' };
const jobs = ['ndvi','evi'].map(key=>({ id:`vegetation-${key}`,kind:'download',itemId:scene.id,assetKey:key,href:scene.assets[key].href,mediaType:scene.assets[key].type,status:'succeeded',bytesDownloaded:1000,sha256:'b'.repeat(64),updatedAt:scene.date }));
const wrap = (children, extra={}) => render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],projects:[project],refresh:vi.fn(),act:vi.fn(),...extra}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{
  vi.resetAllMocks();
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});
});

it('defaults to both original indices in one named project and keeps scientific source pins', async()=>{
  const user=userEvent.setup();
  runtimeRequest.mockImplementation(async (operation,payload)=>operation==='createProject'?project:{jobs:[{id:payload.assetKey}]});
  wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="MODIS"/>);
  await user.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('NDVI + EVI');
  await user.click(screen.getByRole('combobox',{name:'Download content'}));
  expect(screen.getAllByRole('option')).toHaveLength(3);
  expect(screen.queryByRole('option',{name:/True-color|SCL|reflectance/i})).toBeNull();
  await user.keyboard('{Escape}');
  expect(screen.getByText(/full NASA HDF and QA layers are not included/)).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.map(([operation])=>operation)).toEqual(['createProject','downloadProject','downloadProject']);
  expect(runtimeRequest.mock.calls.slice(1).map(([,payload])=>payload.assetKey)).toEqual(['ndvi','evi']);
  const asset=runtimeRequest.mock.calls[0][1].scenes[0].assets.ndvi;
  expect(asset.rasterBand).toEqual({dataType:'int16',scale:0.0001,offset:0,nodata:-3000,spatialResolution:250});
});

it('restores both indices and processes each in the project area without proposing RGB',async()=>{
  const user=userEvent.setup(),act=vi.fn();
  runtimeRequest.mockResolvedValue([project]);
  wrap(<ProjectsLibrary focusedProjectId={project.id}/>,{jobs,act});
  await screen.findByRole('heading',{name:project.name});
  const buttons=screen.getAllByRole('button',{name:'Clip band to project area'});
  expect(buttons).toHaveLength(2);expect(buttons.every(button=>!button.disabled)).toBe(true);
  await user.click(buttons[1]);
  expect(act).toHaveBeenCalledWith('mosaicProject',{id:project.id,assetKey:'evi'});
  expect(screen.queryByRole('button',{name:/RGB/})).toBeNull();
});

function metadata(job){
  const size=VEGETATION_PIXEL*4800;
  return {sha256:job.sha256,width:4800,height:4800,bandCount:1,dataType:'Int16',crs:'MODIS:Sinusoidal',nodata:-3000,
    bounds:[-10*size,3*size,-9*size,4*size],pixelSize:[VEGETATION_PIXEL,VEGETATION_PIXEL],classes:[],previewWidth:160,previewHeight:160,previewDataUrl:'data:image/png;base64,AAAA',
    vegetation:{product:'modis-13q1-v061',index:job.assetKey,scale:0.0001,offset:0,validRange:[-2000,10000],displayRange:[-2000,10000],palette:'modis-vi-v1',sampleCount:25600,validSampleCount:20000,outOfRangeSampleCount:10,pixelInterpretation:'PixelIsArea'}};
}
it.each(['ndvi','evi'])('inspects %s as an index with signed values and a fixed display range',async key=>{
  const user=userEvent.setup(),job=jobs.find(job=>job.assetKey===key);
  runtimeRequest.mockResolvedValue(metadata(job));
  wrap(<RuntimeJobRows jobs={[job]} library/>,{jobs:[job]});
  await user.click(screen.getByRole('button',{name:'File details and provenance'}));
  await user.click(screen.getByRole('button',{name:'Inspect raster'}));
  await screen.findByRole('heading',{name:`${key.toUpperCase()} vegetation index`});
  expect(screen.getByText('DN × 0.0001')).toBeTruthy();
  expect(screen.getByText('Int16')).toBeTruthy();
  expect(screen.getByText(/No QA or cloud mask is applied/)).toBeTruthy();
  expect(screen.queryByRole('heading',{name:'Scene classes'})).toBeNull();
  expect(screen.queryByRole('heading',{name:'Reflectance display'})).toBeNull();
  expect(screen.queryByText('Preview image · JPEG')).toBeNull();
});

it.each(['index','reflectance'])('rejects a mismatched %s response before showing an index preview',async fault=>{
  const user=userEvent.setup(),job=jobs[0],data=metadata(job);
  if(fault==='index')data.vegetation.index='evi';else data.reflectance={scale:0.0001};
  runtimeRequest.mockResolvedValue(data);
  wrap(<RuntimeJobRows jobs={[job]} library/>,{jobs:[job]});
  await user.click(screen.getByRole('button',{name:'File details and provenance'}));
  await user.click(screen.getByRole('button',{name:'Inspect raster'}));
  await screen.findByRole('button',{name:'Retry inspection'});
  expect(screen.queryByAltText('Verified local vegetation index preview')).toBeNull();
  expect(screen.queryByRole('heading',{name:'NDVI vegetation index'})).toBeNull();
});
