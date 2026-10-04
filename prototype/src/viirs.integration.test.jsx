import React from 'react';
import {readFileSync} from 'node:fs';
import {beforeEach,it,expect,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
// Deferred authorized-download workflow; the candidate wrapper is tested separately.
import {OriginalDownloadWorkflow as DownloadAssetButton,RuntimeJobRows} from './runtime-ui.jsx';
import {CatalogFilters} from './catalog-filters.jsx';
import {ProjectsLibrary} from './projects-ui.jsx';
import {normalizeScene} from './catalog.js';
import {projectRequest} from './projects-client.js';
import {runtimeRequest} from './runtime-client.js';
import {viirsIdentity} from './viirs.js';
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:vi.fn()}));
const item=JSON.parse(readFileSync('prototype/qa/viirs-catalog-items.json','utf8'))[0];
const scene=normalizeScene(item,'nasa-viirs-noaa21'),project={...projectRequest({name:'VIIRS project',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]}),id:'viirs-project'};
const job={id:'viirs-source',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'viirs',href:scene.assets.viirs.href,mediaType:'application/x-hdf5',sha256:'a'.repeat(64),bytesDownloaded:1000,outputPath:'test.h5'};
const wrap=(children,runtimeJob=job,act=vi.fn())=>render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:Array.isArray(runtimeJob)?runtimeJob:[runtimeJob],projects:[project],refresh:vi.fn(),act}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{vi.resetAllMocks();delete window.__TAURI__;Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('VIIRS filters use composite dates and omit the optical cloud filter',()=>{wrap(<CatalogFilters initialValues={{provider:scene.provider,bbox:project.bounds,start:'2025-06-01',end:'2025-06-30'}} onClose={vi.fn()} onApply={vi.fn()}/>);expect(screen.getByRole('button',{name:'Search start date'})).toBeTruthy();expect(screen.queryByLabelText('Minimum cloud cover')).toBeNull();});
it('VIIRS download selects the full HDF5 and requires software authorization',async()=>{wrap(<DownloadAssetButton scene={scene} areaBounds={project.bounds} areaName="VIIRS"/>);await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('original HDF5');expect(screen.getByRole('link',{name:'Manage authorization'}).getAttribute('href')).toBe('#Settings?account=nasa-earthdata');expect(screen.getByRole('button',{name:'Create project and start download'}).disabled).toBe(true);expect(screen.getByText(/prepare M5, M4 and M3 in the project/i)).toBeTruthy();});

it('verified native fixture evidence shows precise science checks without enabling unimplemented map actions',async()=>{
  const s=JSON.parse(readFileSync('prototype/qa/viirs-science-summary-fixtures.json','utf8')).summaries[0];
  const checked={...job,itemId:s.itemId,href:viirsIdentity(s.itemId).href,sha256:s.sourceSha256,viirsScience:s};
  wrap(<RuntimeJobRows jobs={[checked]} library/>,checked);
  await userEvent.click(screen.getByRole('button',{name:'File details and provenance'}));
  expect(screen.getByText('M5 / M4 / M3 · Int16')).toBeTruthy();
  expect(screen.getByText('1200 × 1200 · VIIRS:Sinusoidal')).toBeTruthy();
  expect(screen.getByText(/Other science and QA layers remain/)).toBeTruthy();
  expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
});

it('failed VIIRS tasks link directly to the Earthdata authorization entry',()=>{
  const failed={...job,status:'failed',error:'Connect Earthdata in Settings before downloading'};
  wrap(<RuntimeJobRows jobs={[failed]}/>,failed);
  expect(screen.getByRole('link',{name:'Manage authorization'}).getAttribute('href')).toBe('#Settings?account=nasa-earthdata');
});
it('old VIIRS originals retain HDF5 provenance while candidate downloads remain deferred',async()=>{
  const act=vi.fn();runtimeRequest.mockResolvedValue([project]);wrap(<ProjectsLibrary focusedProjectId={project.id}/>,job,act);
  await screen.findByRole('heading',{name:project.name});
  const download=screen.getByRole('button',{name:'Download original product'});expect(download.disabled).toBe(true);
  expect(screen.getByText(/An older VIIRS download has no science validation/)).toBeTruthy();
  expect(screen.getAllByRole('button',{name:'Clip band to project area'}).every(b=>b.disabled)).toBe(true);
  for(const band of ['M5','M4','M3'])expect(screen.getByRole('button',{name:`Prepare ${band}`}).disabled).toBe(true);
  expect(screen.queryByRole('link',{name:'Open in workspace'})).toBeNull();
  expect(screen.getByText('VIIRS original product · HDF5')).toBeTruthy();
  await userEvent.click(download);expect(act).not.toHaveBeenCalled();
  expect(screen.getByText(/Original downloads for this source are pending real-account verification/)).toBeTruthy();
});

it('checked originals enable native band preparation in their own project',async()=>{
  const native=JSON.parse(readFileSync('prototype/qa/viirs-prepared-fixture.json','utf8')),act=vi.fn();
  runtimeRequest.mockResolvedValue([native.project]);wrap(<ProjectsLibrary focusedProjectId={native.project.id}/>,native.source,act);
  const button=await screen.findByRole('button',{name:'Prepare M5'});expect(button.disabled).toBe(false);
  await userEvent.click(button);expect(act).toHaveBeenCalledWith('prepareProject',{id:native.project.id,assetKey:'red'});
});

it('native prepared bands expose thumbnails, workspace and their actual Int16 provenance',async()=>{
  const native=JSON.parse(readFileSync('prototype/qa/viirs-prepared-fixture.json','utf8'));
  wrap(<RuntimeJobRows jobs={native.jobs} library/>,[native.source,...native.jobs]);
  expect(screen.getAllByRole('link',{name:'Open local RGB'})).toHaveLength(3);
  expect(screen.queryByText('VIIRS original product · HDF5')).toBeNull();
  expect(screen.getAllByText(/GeoTIFF · Int16/)).toHaveLength(3);
  await userEvent.click(screen.getAllByRole('button',{name:'File details and provenance'})[0]);
  expect(screen.getByText(/every prepared Int16 sample checked/)).toBeTruthy();
  expect(screen.queryByText(/Science layers are not decoded/)).toBeNull();
});
