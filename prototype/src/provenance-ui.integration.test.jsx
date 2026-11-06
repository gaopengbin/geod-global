import React from 'react';
import {beforeEach,it,expect,vi} from 'vitest';
import {render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RasterProvenanceButton} from './provenance-ui.jsx';
const job={id:'00000000-0000-4000-8000-000000000001'};
const file={jobId:job.id,filename:`${job.id}.tif.provenance.json`,bytes:1200,sha256:'a'.repeat(64),sourceCount:1,capturedStacItems:1,missingStacItems:0};
beforeEach(()=>{vi.restoreAllMocks();delete window.__TAURI__;Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('reviews captured context before a native paired save, preserves cancellation and permits another save',async()=>{
  let attempts=0;const invoke=vi.fn().mockImplementation(command=>Promise.resolve(command==='set_desktop_locale'?null:command==='prepare_raster_provenance'?file:++attempts>1));window.__TAURI__={core:{invoke}};
  render(<I18nProvider><RasterProvenanceButton job={job}/></I18nProvider>);const user=userEvent.setup();await user.click(screen.getByRole('button',{name:'Save raster with provenance'}));
  await screen.findByRole('dialog',{name:'Raster and provenance'});expect(screen.getByText('1 of 1 source assets have original STAC context.')).toBeTruthy();
  expect(invoke.mock.calls.filter(([name])=>name.includes('provenance'))).toHaveLength(1);await user.click(screen.getByRole('button',{name:'Save GeoTIFF + JSON'}));await waitFor(()=>expect(invoke.mock.calls.filter(([name])=>name.includes('provenance'))).toHaveLength(2));expect(screen.queryByText('The raster and its provenance sidecar were saved together.')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Save GeoTIFF + JSON'}));await screen.findByText('The raster and its provenance sidecar were saved together.');expect(invoke).toHaveBeenLastCalledWith('export_raster_provenance',{id:job.id});
});
it('missing original context is disclosed before export and technical errors stay in an information window',async()=>{
  let prepares=0;window.__TAURI__={core:{invoke:vi.fn().mockImplementation(command=>command==='prepare_raster_provenance'?Promise.resolve(++prepares===1?{...file,capturedStacItems:0,missingStacItems:1}:file):Promise.reject('A raster or provenance sidecar already exists; choose a new filename'))}};
  render(<I18nProvider><RasterProvenanceButton job={job}/></I18nProvider>);const user=userEvent.setup();await user.click(screen.getByRole('button',{name:'Save raster with provenance'}));await screen.findByText('Some original catalog records could not be captured. The JSON marks them as unavailable; it retains known source identities and processing records.');
  await user.click(screen.getByRole('button',{name:'Retry catalog context'}));await screen.findByText('1 of 1 source assets have original STAC context.');expect(screen.queryByRole('button',{name:'Retry catalog context'})).toBeNull();
  await user.click(screen.getByRole('button',{name:'Save GeoTIFF + JSON'}));await screen.findByRole('alert');expect(screen.queryByText('A raster or provenance sidecar already exists; choose a new filename')).toBeNull();await user.click(screen.getByRole('button',{name:'Technical details'}));await screen.findByText('A raster or provenance sidecar already exists; choose a new filename');
});
it('preparation failure leaves no download or paired-save action',async()=>{
  window.__TAURI__={core:{invoke:vi.fn().mockRejectedValue('Raster output checksum changed before provenance export')}};
  render(<I18nProvider><RasterProvenanceButton job={job}/></I18nProvider>);await userEvent.setup().click(screen.getByRole('button',{name:'Save raster with provenance'}));await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Save GeoTIFF + JSON'})).toBeNull();
});
