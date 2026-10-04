import React from 'react';
import { readFileSync } from 'node:fs';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { VegetationQualityControls } from './vegetation-quality-ui.jsx';
import { VI_SELECTION_KEYS } from './vegetation-quality.js';
import { normalizeScene } from './catalog.js';
import { projectRequest } from './projects-client.js';
import { MODIS_SCIENCE } from './modis-science-layers.js';
// Captured catalogue records + simulated jobs check UI dependencies, not files.
const feature=JSON.parse(readFileSync('prototype/public/samples/modis-vegetation-response.json','utf8')).features[0];
const item=structuredClone(feature);
for(const key of ['vi_quality','vi_reliability']) {
  const layer=MODIS_SCIENCE[key];
  item.assets[layer.asset]={type:'image/tiff; application=geotiff',href:item.assets['250m_16_days_NDVI'].href.replace('250m_16_days_NDVI',layer.asset),'raster:bands':[{data_type:layer.dataType,scale:layer.scale,nodata:layer.nodata,spatial_resolution:250,...(layer.catalogUnit?{unit:layer.catalogUnit}:{})}]};
}
const scene=normalizeScene(item,'planetary-vegetation');
const project={...projectRequest({scenes:[scene],bounds:[-122.55,37.68,-122.32,37.84],name:'Quality indices'}),id:'quality-project'};
const jobs=VI_SELECTION_KEYS.map(key=>({id:'quality-'+key,kind:'download',itemId:scene.id,assetKey:key,href:scene.assets[key].href,mediaType:scene.assets[key].type,status:'succeeded',sha256:'b'.repeat(64),bytesDownloaded:1000}));
beforeEach(()=>Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}}));
it('requires and downloads four layers for quality processing, without proposing RGB',async()=>{
  const user=userEvent.setup(),action=vi.fn().mockResolvedValue(true);
  render(<I18nProvider><VegetationQualityControls project={project} jobs={jobs.slice(0,2)} health={{}} onAction={action}/></I18nProvider>);
  expect(screen.getByRole('button',{name:'Quality-screen and clip'}).disabled).toBe(true);
  await user.click(screen.getByRole('button',{name:'Download indices and QA'}));
  await waitFor(()=>expect(action).toHaveBeenCalledTimes(2));
  expect(action.mock.calls).toEqual([['vi_quality','downloadProject'],['vi_reliability','downloadProject']]);
  expect(screen.queryByRole('button',{name:/RGB/})).toBeNull();
});
it('submits the chosen index and policy and keeps unmasked processing available',async()=>{
  const user=userEvent.setup(),action=vi.fn();
  render(<I18nProvider><VegetationQualityControls project={project} jobs={jobs} health={{}} onAction={action}/></I18nProvider>);
  await user.click(screen.getByRole('combobox',{name:'Vegetation index'}));await user.click(screen.getByRole('option',{name:'EVI'}));
  await user.click(screen.getByRole('combobox',{name:'Pixel quality'}));await user.click(screen.getByRole('option',{name:'Good or marginal pixels'}));
  await user.click(screen.getByRole('button',{name:'Quality-screen and clip'}));
  expect(action).toHaveBeenCalledWith('evi','mosaicProject',{viQuality:{policy:'usable'}});
  await user.click(screen.getByRole('combobox',{name:'Pixel quality'}));await user.click(screen.getByRole('option',{name:'Keep original values'}));
  await user.click(screen.getByRole('button',{name:'Clip or mosaic index'}));
  expect(action).toHaveBeenLastCalledWith('evi','mosaicProject',{});
});
