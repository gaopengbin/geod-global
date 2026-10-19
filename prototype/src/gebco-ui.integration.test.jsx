import React from 'react';
import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {I18nProvider} from './i18n.jsx';
import {GebcoExplorer} from './gebco-ui.jsx';
import {gebcoRequest,saveGebcoFile} from './gebco-client.js';
const map=vi.hoisted(()=>({current:null}));
vi.mock('./gebco-client.js',async original=>({...await original(),gebcoRequest:vi.fn(),saveGebcoFile:vi.fn()}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:props=>{map.current=props;return <section aria-label="GEBCO map"/>;}}));
const bounds=[-30.1,40,-30,40.1];
const records=Array.from({length:5},(_,i)=>({id:`region-${i}`,name:`Region ${i}`,bounds,bytes:8992,sha256:String(i).repeat(64),archiveSha256:'a'.repeat(64),memberName:'official numerical subset.tif',attribution:'GEBCO',limitations:['Generated subset']}));
const raster=id=>({jobId:id,sha256:records.find(r=>r.id===id).sha256,width:24,height:24,bands:[{index:1,dataType:'Int16',nodata:-32767}],crs:'EPSG:4326',transform:[1/240,0,-30.1,0,-1/240,40.1],bounds,pixelInterpretation:'area',previewDataUrl:'data:image/png;base64,AA==',previewWidth:24,previewHeight:24,displayBand:1,displayRange:[-1941,-1209],limitations:[]});
const component=props=><I18nProvider><GebcoExplorer areaBounds={bounds} {...props}/></I18nProvider>;
beforeEach(()=>{Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});map.current=null;saveGebcoFile.mockReset();gebcoRequest.mockReset().mockImplementation(async(op,p)=>op==='list'?records:op==='grids'?{grid:{name:'gebco_2026_global'}}:op==='inspect'?raster(p.id):{});});
const check=i=>screen.getByRole('checkbox',{name:`Select result Region ${i}`});
it('library reopens saved regions without metadata network or new queue and loads up to four explicit layers',async()=>{
  render(component({libraryOnly:true}));await screen.findByRole('checkbox',{name:'Select result Region 0'});expect(gebcoRequest.mock.calls.map(([op])=>op)).toEqual(['list']);expect(screen.queryByLabelText('Subset name')).toBeNull();expect(map.current.areaBounds).toBeUndefined();
  for(let i=0;i<4;i++)fireEvent.click(check(i));expect(check(4).disabled).toBe(true);fireEvent.click(screen.getByRole('button',{name:'Preview selected'}));await waitFor(()=>expect(map.current.entries).toHaveLength(4));
  await act(async()=>map.current.onPatch(records[0].id,{visible:false,opacity:0.35}));expect(map.current.entries[0]).toMatchObject({visible:false,opacity:0.35});await act(async()=>map.current.onRemove(records[0].id));expect(map.current.entries).toHaveLength(3);expect(check(0).getAttribute('aria-checked')).toBe('true');expect(gebcoRequest.mock.calls.some(([op])=>op==='create')).toBe(false);
});
it('AOI changes and late old-region inspection cannot attach stale layers or old selection',async()=>{
  let finish;const pending=new Promise(resolve=>{finish=resolve;});gebcoRequest.mockImplementation(async op=>op==='list'?records:op==='grids'?{}:pending);
  const view=render(component());await screen.findByRole('checkbox',{name:'Select result Region 0'});fireEvent.click(check(0));fireEvent.click(screen.getByRole('button',{name:'Preview selected'}));await waitFor(()=>expect(gebcoRequest.mock.calls.some(([op])=>op==='inspect')).toBe(true));view.rerender(component({areaBounds:[-31,40,-30.9,40.1]}));await act(async()=>finish(raster(records[0].id)));expect(map.current.entries).toEqual([]);expect(screen.queryByRole('checkbox',{name:'Select result Region 0'})).toBeNull();expect(gebcoRequest.mock.calls.some(([op])=>op==='create')).toBe(false);
});
it('saved details read exact signed raw pixels and export TIFF and ZIP separately without a new subset',async()=>{
  gebcoRequest.mockImplementation(async(op,p)=>op==='list'?[records[0]]:op==='inspect'?raster(p.id):op==='pixel'?{sha256:records[0].sha256,values:[-1705],noData:[false]}:{});
  render(component({libraryOnly:true}));await screen.findByRole('checkbox',{name:'Select result Region 0'});fireEvent.click(screen.getByRole('button',{name:'Details and pixels'}));await screen.findByRole('button',{name:'Save numerical GeoTIFF'});fireEvent.click(screen.getByRole('button',{name:'Read a numerical pixel'}));fireEvent.change(screen.getByLabelText('Pixel column'),{target:{value:'12'}});fireEvent.change(screen.getByLabelText('Pixel row'),{target:{value:'12'}});fireEvent.click(screen.getByRole('button',{name:'Read original numerical value'}));await screen.findByText(/-1705/);expect(gebcoRequest).toHaveBeenCalledWith('pixel',{id:records[0].id,column:12,row:12},expect.any(AbortSignal));
  fireEvent.click(screen.getByRole('button',{name:'Save numerical GeoTIFF'}));await waitFor(()=>expect(saveGebcoFile).toHaveBeenCalledWith(records[0].id));fireEvent.click(screen.getByRole('button',{name:'Save complete official ZIP'}));await waitFor(()=>expect(saveGebcoFile).toHaveBeenCalledWith(records[0].id,true));expect(gebcoRequest.mock.calls.some(([op])=>op==='create')).toBe(false);
});
it('Chinese library explains generated numerical scope and release year while reopening without a queue',async()=>{
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'zh-CN',setItem:vi.fn()}});
  render(component({libraryOnly:true}));await screen.findByText('已保存的 GEBCO 区域');
  expect(screen.getByText(/完整发布原件仍待接入/)).toBeTruthy();expect(screen.getByText(/2026 是网格发布版本，不代表单次观测日期/)).toBeTruthy();
  expect(gebcoRequest.mock.calls.map(([operation])=>operation)).toEqual(['list']);expect(screen.queryByLabelText('区域子集名称')).toBeNull();
});
