import React from 'react';
import {readFileSync} from 'node:fs';
import {beforeEach,it,expect,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {DownloadAssetButton,RuntimeJobRows} from './runtime-ui.jsx';
import {CatalogFilters} from './catalog-filters.jsx';
import {ProjectsLibrary} from './projects-ui.jsx';
import {normalizeScene} from './catalog.js';
import {projectRequest} from './projects-client.js';
import {runtimeRequest} from './runtime-client.js';
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:vi.fn()}));
const scene=normalizeScene(JSON.parse(readFileSync('prototype/qa/sentinel-1-rtc-catalog.json','utf8')).features[0],'planetary-radar');
const project={...projectRequest({name:'Radar RTC',bounds:[-122.55,37.65,-122.4,37.8],scenes:[scene]}),id:'radar-project'};
const job={id:'0b95755d-70cd-4ccd-a90a-cd0eb6fe483a',kind:'download',status:'succeeded',itemId:scene.id,assetKey:'vv',href:scene.assets.vv.href,mediaType:scene.assets.vv.type,sha256:'a'.repeat(64),bytesDownloaded:1840806210};
const wrap=children=>render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[job],projects:[project],refresh:vi.fn(),act:vi.fn()}}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{vi.resetAllMocks();delete window.__TAURI__;Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('radar filters provide orbit and polarization controls without cloud sliders',()=>{
  wrap(<CatalogFilters initialValues={{provider:'planetary-radar',bbox:project.bounds,start:'2025-06-01',end:'2025-06-30'}} onClose={vi.fn()} onApply={vi.fn()}/>);
  expect(screen.getByRole('combobox',{name:'Orbit direction'})).toBeTruthy(); expect(screen.getByRole('combobox',{name:'Polarization'})).toBeTruthy();
  expect(screen.queryByLabelText('Minimum cloud cover')).toBeNull();
});
it('radar download defaults to all available polarizations and describes originals without optical promises',async()=>{
  wrap(<DownloadAssetButton scene={scene} areaBounds={project.bounds} areaName="Radar"/>);
  await userEvent.click(screen.getByRole('button',{name:'Create project and download'}));
  expect(screen.getByRole('combobox',{name:'Download content'}).textContent).toContain('VV / VH');
  expect(screen.getByText(/2 source files to download/)).toBeTruthy(); expect(screen.getByText(/up to 4 GiB/)).toBeTruthy();
  expect(screen.queryByText(/True-color GeoTIFF/)).toBeNull();
});
it('radar project offers verified polarization processing only when all original sources are ready',async()=>{
  runtimeRequest.mockImplementation(async op=>op==='projects'?[project]:[]);
  wrap(<ProjectsLibrary focusedProjectId={project.id}/>); await screen.findByRole('heading',{name:project.name});
  expect(screen.getByText(/No averaging, calibration or speckle filtering is applied/)).toBeTruthy();
  const buttons=screen.getAllByRole('button',{name:'Clip radar to project area'});
  expect(buttons).toHaveLength(2);
  expect(buttons[0].disabled).toBe(false);
  expect(buttons[1].disabled).toBe(true);
  expect(screen.queryByRole('button',{name:'Mosaic and clip SCL'})).toBeNull();
  expect(screen.getByRole('link',{name:'Open in workspace'})).toBeTruthy();
});
it('local files from the same radar scene distinguish their polarizations in the title',()=>{
  const vh={...job,id:'0b95755d-70cd-4ccd-a90a-cd0eb6fe483b',assetKey:'vh',href:scene.assets.vh.href};
  wrap(<RuntimeJobRows jobs={[job,vh]} library/>);
  expect(screen.getByText(/Sentinel-1[ABC] · VV$/)).toBeTruthy();
  expect(screen.getByText(/Sentinel-1[ABC] · VH$/)).toBeTruthy();
});
