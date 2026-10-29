import React,{useState} from 'react';
import {afterEach,expect,it,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import fixture from '../qa/public-preview-catalog.json';
import radarFixture from '../qa/sentinel-1d-rtc-catalog.json';
import {normalizeScene} from './catalog.js';
import {I18nProvider} from './i18n.jsx';
import {createCatalogPreviewSource} from './explore-preview-source.js';
import {elevationPreviewURL} from './elevation-preview.js';
import {CatalogPreviewControls,CatalogPreviewLegend} from './catalog-preview-ui.jsx';
import {modisPreviewGrid} from './modis-preview.js';

const loader=vi.hoisted(()=>vi.fn());
vi.mock('geotiff',async importOriginal=>({...await importOriginal(),fromCustomClient:loader}));
afterEach(()=>{vi.restoreAllMocks();loader.mockReset();});

it('radar polarization changes by keyboard and keeps source identity and scientific units in the legend',async()=>{
 Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'zh-CN',setItem:vi.fn()}});
 const scene=normalizeScene(radarFixture.features[0],'planetary-radar');
 function Preview(){const [channel,setChannel]=useState('vv');return <I18nProvider><CatalogPreviewControls scene={scene} kind="radar" value={channel} onValueChange={setChannel}/><CatalogPreviewLegend scene={scene} channel={channel}/></I18nProvider>;}
 const user=userEvent.setup();render(<Preview/>);
 expect(screen.getByRole('radio',{name:'HH'}).hasAttribute('disabled')).toBe(true);
 expect(screen.getByRole('region',{name:'地图预览图例'}).textContent).toContain('-30 dB');
 await user.tab();await user.keyboard('{ArrowRight} ');
 expect(screen.getByRole('radio',{name:'VH'}).getAttribute('aria-checked')).toBe('true');
 expect(screen.getByRole('region',{name:'地图预览图例'}).textContent).toContain('VH');
});

it.each(['copernicus-dem','copernicus-dem-90'])('real OpenLayers applies the half-pixel shift to the %s original grid and keeps zero as valid height',async provider=>{
 const scene=normalizeScene(fixture.catalogs[provider][0],provider),[height,width]=scene.grid.shape,t=scene.grid.transform;
 const x=t[2]+t[0]/2,y=t[5]+t[4]/2;
 const image={getWidth:()=>width,getHeight:()=>height,getSamplesPerPixel:()=>1,getTileWidth:()=>256,getTileHeight:()=>256,
  getGDALNoData:()=>null,getGDALMetadata:()=>null,getOrigin:()=>[x,y,0],getResolution:()=>[t[0],t[4],0],
  getBoundingBox:()=>[x,y+t[4]*height,x+t[0]*width,y],
  getGeoKeys:()=>({GTModelTypeGeoKey:2,GTRasterTypeGeoKey:2,GeographicTypeGeoKey:4326,GeogAngularUnitsGeoKey:9102}),
  fileDirectory:{getValue:key=>({BitsPerSample:[32],SampleFormat:[3],PhotometricInterpretation:1})[key]}};
 loader.mockResolvedValue({getImageCount:async()=>1,getImage:async()=>image});
 const preview=createCatalogPreviewSource(scene,'height');
 try {
  const view=await preview.source.getView();
  expect(view.extent).toEqual(scene.bbox);expect(preview.source.bandCount).toBe(1);expect(preview.source.hasAlpha).toBe(false);
  expect(preview.source.normalize_).toBe(false);expect(loader.mock.calls[0][0].url).toBe(elevationPreviewURL(scene));
 } finally {preview.dispose();}
});

it('an original elevation transport failure rejects metadata promptly and leaves no permanent loading promise',async()=>{
 vi.spyOn(console,'error').mockImplementation(()=>{});
 loader.mockRejectedValue(new Error('transport failure'));
 const scene=normalizeScene(fixture.catalogs['copernicus-dem'][0],'copernicus-dem');
 const preview=createCatalogPreviewSource(scene,'height');
 try {await expect(preview.source.getView()).rejects.toThrow('Preview network request failed');expect(preview.source.getState()).toBe('error');}
 finally {preview.dispose();}
});

it('MODIS reads original COGs with a finite sinusoidal view and a normalized NoData alpha channel',async()=>{
 const scene=normalizeScene(fixture.catalogs['planetary-modis'][0],'planetary-modis'),t=modisPreviewGrid(scene);
 const keys={GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjCoordTransGeoKey:24,ProjLinearUnitsGeoKey:9001,GeogAngularUnitsGeoKey:9102,GeogSemiMajorAxisGeoKey:6371007.181,GeogSemiMinorAxisGeoKey:6371007.181,ProjCenterLongGeoKey:0,ProjFalseEastingGeoKey:0,ProjFalseNorthingGeoKey:0};
 const image={getWidth:()=>2400,getHeight:()=>2400,getSamplesPerPixel:()=>1,getTileWidth:()=>512,getTileHeight:()=>512,
   getGDALNoData:()=>-28672,getGDALMetadata:()=>({}),getOrigin:()=>[t[2],t[5],0],getResolution:()=>[t[0],t[4],0],
   getBoundingBox:()=>[t[2],t[5]+2400*t[4],t[2]+2400*t[0],t[5]],getGeoKeys:()=>keys,
   fileDirectory:{getValue:key=>({BitsPerSample:[16],SampleFormat:[2],PhotometricInterpretation:1})[key]}};
 loader.mockResolvedValue({getImageCount:async()=>1,getImage:async()=>image});
 const preview=createCatalogPreviewSource(scene,'rgb');
 try{
   const view=await preview.source.getView();expect(view.extent.every(Number.isFinite)).toBe(true);
   expect(view.projection.getCode()).toBe('MODIS:Sinusoidal');expect(preview.extent.every(Number.isFinite)).toBe(true);
   expect(preview.source.bandCount).toBe(4);expect(preview.source.normalize_).toBe(false);
   expect(preview.getStyle().color.at(-1)).toEqual(['/',['band',4],255]);
   expect(loader.mock.calls.map(([client])=>client.url)).toEqual(['red','green','blue'].map(k=>scene.assets[k].href));
 }finally{preview.dispose();}
});
