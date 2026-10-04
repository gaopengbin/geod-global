import React from 'react';
import { readFileSync } from 'node:fs';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
// Protocol coverage for the deferred protected-source adapter, not release UI availability.
import { OriginalDownloadWorkflow as DownloadAssetButton, RuntimeJobRows } from './runtime-ui.jsx';
import { runtimeRequest, desktopAvailable } from './runtime-client.js';
import { normalizeScene } from './catalog.js';
import { projectRequest } from './projects-client.js';
import { ProjectsLibrary } from './projects-ui.jsx';

vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn(), desktopAvailable: vi.fn(() => true) }));
const scene = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/nasa-response.json','utf8')).features[0], 'nasa-earthdata');
const request = projectRequest({ scenes: [scene], bounds: [-122.55,37.68,-122.32,37.84], name: 'HLS original bands' });
const project = { ...request, id: 'hls-project' };
const state = status => ({ provider: 'nasa-earthdata', status, expiresAt: ['connected','saved'].includes(status) ? '2099-01-01T00:00:00Z' : null, verifiedAt: status === 'connected' ? '2026-10-01T00:00:00Z' : null });
let account;
beforeEach(() => {
  account = state('not-connected');
  desktopAvailable.mockReturnValue(true);
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});
  runtimeRequest.mockReset();
  runtimeRequest.mockImplementation(async (operation,payload) => operation === 'accounts' ? [account,{provider:'copernicus',status:'not-connected',expiresAt:null,verifiedAt:null}] : operation === 'createProject' ? { ...payload, id:project.id } : {jobs:[{id:payload.assetKey}]});
});
afterEach(() => vi.restoreAllMocks());
const wrap = (element, extra = {}) => render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],projects:[project],refresh:vi.fn(),act:vi.fn(),busyJobs:{},...extra}}>{element}</RuntimeContext.Provider></I18nProvider>);
const download = () => wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="HLS"/>);

it('blocks unconnected HLS original downloads and links to the matching authorization entry', async () => {
  download();
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  await screen.findByText('Connect NASA Earthdata before downloading protected files.');
  expect(screen.getByRole('link',{name:'Manage authorization'}).hash).toBe('#Settings?account=nasa-earthdata');
  expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);
  expect(runtimeRequest.mock.calls.map(([operation])=>operation)).toEqual(['accounts']);
  expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('Int16');
  expect(screen.queryByText(/UInt16/)).toBeNull();
});

it('saved native authorization queues three calibrated originals into one project', async () => {
  account = state('saved');
  download();
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(false));
  await userEvent.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.map(([operation])=>operation)).toEqual(['accounts','createProject','downloadProject','downloadProject','downloadProject']);
  const saved = runtimeRequest.mock.calls.find(([operation])=>operation==='createProject')[1];
  expect(saved.scenes[0].assets.red.rasterBand).toEqual({dataType:'int16',scale:0.0001,offset:0,nodata:-9999,spatialResolution:30});
  expect(JSON.stringify(saved)).not.toMatch(/token|Authorization|Signature/);
});

it('a browser preview cannot queue protected downloads or request a credential', async () => {
  desktopAvailable.mockReturnValue(false);
  account = state('connected');
  download();
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByText('Open the desktop app to authorize and download protected NASA files.')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);
  expect(runtimeRequest).not.toHaveBeenCalled();
  expect(screen.queryByLabelText('Earthdata user token')).toBeNull();
});

it('file details preserve the HLS conversion and failed access offers a matching authorization action', async () => {
  const source = {id:'red',kind:'download',itemId:scene.id,assetKey:'red',href:scene.assets.red.href,mediaType:scene.assets.red.type,status:'succeeded',bytesDownloaded:1000,sha256:'a'.repeat(64),updatedAt:scene.date};
  const { unmount } = wrap(<RuntimeJobRows jobs={[source]} library/>,{jobs:[source]});
  await userEvent.click(screen.getByRole('button',{name:'File details and provenance'}));
  expect(screen.getByText('DN × 0.0001 + (0)')).toBeTruthy();
  expect(screen.getByText('-9999')).toBeTruthy();
  expect(screen.queryByText(/UInt16/)).toBeNull();
  expect(screen.getByRole('button',{name:'Inspect raster'})).toBeTruthy();
  unmount();
  wrap(<RuntimeJobRows jobs={[{...source,status:'failed',error:'Connect this data source in Settings before downloading protected files.'}]}/>);
  expect(screen.getByRole('link',{name:'Manage authorization'}).hash).toBe('#Settings?account=nasa-earthdata');
  expect(screen.getByRole('button',{name:'Retry download'})).toBeTruthy();
});

it('Copernicus resolves a complete original product before saving or queuing a download', async () => {
  const cdse = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/copernicus-response.json','utf8')).features[0], 'copernicus');
  const product = {itemId:cdse.id,href:'https://download.dataspace.copernicus.eu/odata/v1/Products(0d695b42-4b24-4954-ba09-f2a44303fdd8)/$value',mediaType:'application/zip',bytes:1128466167};
  let resolve;
  runtimeRequest.mockImplementation(async (operation,payload) => operation === 'accounts' ? [state('not-connected'),{...state('connected'),provider:'copernicus'}] : operation === 'resolveProducts' ? new Promise(done=>{resolve=done;}) : operation === 'createProject' ? {...payload,id:'safe-project'} : {jobs:[{id:'product-job'}]});
  wrap(<DownloadAssetButton scene={cdse} scenes={[cdse]} areaBounds={cdse.bbox} areaName="SAFE"/>);
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByText('Resolving original products from the official catalogue…')).toBeTruthy();
  expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);
  expect(screen.queryByText(/no common supported/)).toBeNull();
  resolve([product]);
  await waitFor(()=>expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(false));
  expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('SAFE product · ZIP');
  expect(screen.getByText(/1.1 GiB/)).toBeTruthy();
  await userEvent.click(screen.getByRole('button',{name:'Create project and start download'}));
  await screen.findByRole('button',{name:'Open project'});
  const saved = runtimeRequest.mock.calls.find(([operation])=>operation==='createProject')[1];
  expect(saved.scenes[0].assets).toEqual({product:{href:product.href,mediaType:'application/zip'}});
  expect(runtimeRequest.mock.calls.filter(([operation])=>operation==='downloadProject').map(([,payload])=>payload.assetKey)).toEqual(['product']);
});

it('Copernicus lookup failure keeps the download disabled and does not save an empty project', async () => {
  const cdse = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/copernicus-response.json','utf8')).features[0], 'copernicus');
  runtimeRequest.mockImplementation(async operation=>{
    if(operation==='accounts') return [state('not-connected'),{...state('connected'),provider:'copernicus'}];
    if(operation==='resolveProducts') throw new Error('This Copernicus product is offline. Choose another scene or check its availability in Copernicus Browser.');
  });
  wrap(<DownloadAssetButton scene={cdse} scenes={[cdse]} areaBounds={cdse.bbox} areaName="SAFE"/>);
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  await screen.findByText('Original products could not be resolved. Close and retry this download.');
  expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);
  expect(runtimeRequest.mock.calls.some(([operation])=>operation==='createProject')).toBe(false);
});

it('restored SAFE project offers a product download without pretending the ZIP is a GeoTIFF or SCL raster', async () => {
  const cdse = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/copernicus-response.json','utf8')).features[0], 'copernicus');
  const asset = {href:'https://download.dataspace.copernicus.eu/odata/v1/Products(0d695b42-4b24-4954-ba09-f2a44303fdd8)/$value',mediaType:'application/zip'};
  const saved = {id:'safe-project',name:'SAFE project',bounds:cdse.bbox,scenes:[{itemId:cdse.id,date:cdse.date,bbox:cdse.bbox,assets:{product:asset}}]};
  runtimeRequest.mockResolvedValue([saved]);
  wrap(<ProjectsLibrary focusedProjectId={saved.id}/>,{projects:[saved]});
  await screen.findByRole('heading',{name:saved.name});
  expect(screen.getByRole('button',{name:'Download original product'})).toBeTruthy();
  expect(screen.getByRole('button',{name:'Prepare SCL'}).disabled).toBe(true);
  expect(screen.getByRole('button',{name:'Clip SCL to project area'}).disabled).toBe(true);
  expect(screen.getByText(/SAFE archives remain the original source/)).toBeTruthy();
});

it('completed SAFE originals prepare rasters before enabling project clips and label the files correctly', async () => {
  const cdse = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/copernicus-response.json','utf8')).features[0], 'copernicus');
  const href = 'https://download.dataspace.copernicus.eu/odata/v1/Products(0d695b42-4b24-4954-ba09-f2a44303fdd8)/$value';
  const saved = {id:'safe-project',name:'SAFE project',bounds:cdse.bbox,scenes:[{itemId:cdse.id,date:cdse.date,bbox:cdse.bbox,assets:{product:{href,mediaType:'application/zip'}}}]};
  const original = {id:'zip-job',kind:'download',assetKey:'product',itemId:cdse.id,href,mediaType:'application/zip',status:'succeeded',bytesDownloaded:1000,sha256:'a'.repeat(64)};
  const act = vi.fn(); runtimeRequest.mockResolvedValue([saved]);
  const {unmount}=wrap(<ProjectsLibrary focusedProjectId={saved.id}/>,{projects:[saved],jobs:[original],act});
  await screen.findByRole('heading',{name:saved.name});
  expect(screen.getByRole('button',{name:'Prepare SCL'}).disabled).toBe(false);
  expect(screen.getByRole('button',{name:'Clip SCL to project area'}).disabled).toBe(true);
  await userEvent.click(screen.getByRole('button',{name:'Prepare SCL'}));
  expect(act).toHaveBeenCalledWith('prepareProject',{id:saved.id,assetKey:'scl'}); unmount();
  const prepared = {id:'scl-job',kind:'raster_prepare',parentId:original.id,safe:{sourceJobId:original.id,sourceSha256:original.sha256},assetKey:'scl',itemId:cdse.id,href,mediaType:'image/tiff',status:'succeeded',bytesDownloaded:1000,sha256:'b'.repeat(64)};
  wrap(<ProjectsLibrary focusedProjectId={saved.id}/>,{projects:[saved],jobs:[original,prepared],act});
  await screen.findByRole('heading',{name:saved.name});
  expect(screen.getByRole('button',{name:'Prepare SCL'}).disabled).toBe(true);
  expect(screen.getByRole('button',{name:'Clip SCL to project area'}).disabled).toBe(false);
  expect(screen.getByText('Prepared source')).toBeTruthy();
});
