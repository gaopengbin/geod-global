import React from 'react';
import {readFileSync} from 'node:fs';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {ModisScienceControls,SciencePixelReadout} from './modis-science-ui.jsx';
import {DownloadAssetButton} from './runtime-ui.jsx';
import {runtimeRequest} from './runtime-client.js';
import {normalizeScene} from './catalog.js';
import {projectRequest} from './projects-client.js';
import {MODIS_SCIENCE,MODIS_SCIENCE_KEYS} from './modis-science-layers.js';
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:vi.fn()}));
// UI protocol fixtures only; real original COGs have independent receipts.
const base=JSON.parse(readFileSync('prototype/public/samples/modis-vegetation-response.json','utf8')).features[0];
const item=structuredClone(base);
for(const key of MODIS_SCIENCE_KEYS){const l=MODIS_SCIENCE[key];item.assets[l.asset]={type:'image/tiff; application=geotiff',href:item.assets['250m_16_days_NDVI'].href.replace('250m_16_days_NDVI',l.asset),'raster:bands':[{data_type:l.dataType,scale:l.scale,nodata:l.nodata,spatial_resolution:250,...(l.catalogUnit?{unit:l.catalogUnit}:{})}]};}
const scene=normalizeScene(item,'planetary-vegetation'),project={...projectRequest({name:'science',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]}),id:'science-project'};
const jobs=MODIS_SCIENCE_KEYS.map(key=>({id:key,kind:'download',itemId:scene.id,assetKey:key,href:scene.assets[key].href,status:'succeeded',sha256:'a'.repeat(64)}));
const wrap=(component,extra={})=>render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs,projects:[project],act:vi.fn(),refresh:vi.fn(),...extra}}>{component}</RuntimeContext.Provider></I18nProvider>);
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('Offers one compact layer selector and dispatches the selected original layer only',async()=>{
  const user=userEvent.setup(),action=vi.fn();wrap(<ModisScienceControls project={project} jobs={jobs} health={{}} onAction={action}/>);
  expect(screen.getAllByRole('combobox')).toHaveLength(1);expect(screen.getByRole('button',{name:'Download layer'}).disabled).toBe(true);
  await user.click(screen.getByRole('combobox',{name:'Scientific layer'}));expect(screen.getAllByRole('option')).toHaveLength(10);
  await user.click(screen.getByRole('option',{name:'Pixel reliability'}));await user.click(screen.getByRole('button',{name:'Clip layer'}));
  expect(action).toHaveBeenCalledExactlyOnceWith('vi_reliability','mosaicProject');
});
it('Prevents observation-day mosaics that would discard differing source years',async()=>{
  const user=userEvent.setup(),mixed={...project,scenes:[project.scenes[0],{...project.scenes[0],date:'2024-06-26T00:00:00Z'}]};
  wrap(<ModisScienceControls project={mixed} jobs={jobs} health={{}} onAction={vi.fn()}/>);
  await user.click(screen.getByRole('combobox',{name:'Scientific layer'}));await user.click(screen.getByRole('option',{name:'Pixel observation day'}));
  expect(screen.getByRole('button',{name:'Mosaic and clip layer'}).disabled).toBe(true);expect(screen.getByText('Process observation-day layers separately for each calendar year.')).toBeTruthy();
});
it('Download entry can queue all twelve COGs with persisted source calibration',async()=>{
  const user=userEvent.setup();runtimeRequest.mockImplementation(async(op,p)=>op==='createProject'?project:{jobs:[{id:p.assetKey}]});
  wrap(<DownloadAssetButton scene={scene} scenes={[scene]} areaBounds={project.bounds} areaName="MODIS"/>,{jobs:[],projects:[]});
  await user.click(screen.getByRole('button',{name:'Create project and download'}));await user.click(screen.getByRole('combobox',{name:'Download content'}));
  await user.click(screen.getByRole('option',{name:'All MODIS science layers · 12 COGs'}));expect(screen.getByText(/no quality mask is applied/)).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Create project and start download'}));await screen.findByRole('button',{name:'Open project'});
  expect(runtimeRequest.mock.calls.filter(([op])=>op==='downloadProject').map(([,p])=>p.assetKey)).toEqual(['ndvi','evi',...MODIS_SCIENCE_KEYS]);
  expect(runtimeRequest.mock.calls[0][1].scenes[0].assets.vi_reliability.rasterBand).toEqual({dataType:'int8',scale:1,offset:0,nodata:-1,spatialResolution:250});
});
it('Pixel readout shows a per-pixel observation date and the retained DN',()=>{
  wrap(<SciencePixelReadout metadata={{science:{band:'vi_doy',kind:'date',calendarYear:2025}}} pixel={{value:185,isNoData:false,pixel:[5,7],color:'#808080',science:{withinRange:true,date:'2025-07-04'}}}/>);
  expect(screen.getByRole('status').textContent).toContain('Jul 4, 2025');expect(screen.getByRole('status').textContent).toContain('DN 185');expect(screen.getByRole('status').textContent).toContain('Column 5, row 7');
});
