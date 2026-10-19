import React from 'react';
import { expect, it, vi, beforeEach } from 'vitest';
import { act, render, screen, waitFor, fireEvent, within } from '@testing-library/react';
import { I18nProvider } from './i18n.jsx';
import { EarthEngineDialog, EarthEngineExplorer } from './earth-engine-ui.jsx';
import { earthEngineRequest, saveEarthEngineFile } from './earth-engine-client.js';
import { stacInspection } from './stac-fixtures.js';
const map=vi.hoisted(()=>({current:null}));
vi.mock('./earth-engine-client.js', async original => ({...await original(),earthEngineRequest:vi.fn(),saveEarthEngineFile:vi.fn()}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:props=>{map.current=props;return <section aria-label="GEE map boundary"/>;}}));
const url='https://earthengine.googleapis.com/v1/projects/example-project/thumbnails/0123456789abcdef-0123456789abcdef:getPixels';
beforeEach(()=>{
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'zh-CN',setItem:vi.fn()}});
  earthEngineRequest.mockReset().mockResolvedValue([]);
  saveEarthEngineFile.mockReset();map.current=null;
});

// The adapter's actual metadata/grid validation and UI run here; only native
// transport and the canvas are replaced. These fixtures do not assert a new
// GEE network download or decoded pixels.
const records=Array.from({length:5},(_,index)=>({id:`export-${index}`,name:`Export ${index}`,projectId:'example-project',bytes:1024,downloadSeconds:1,sha256:String(index).repeat(64)}));
const area=[10,20,11,21];
const geometry={type:'Polygon',coordinates:[[[10,20],[11,20],[11,21],[10,20]]]};
const differentGeometry={type:'Polygon',coordinates:[[[10,20],[11,20],[10.5,21],[10,20]]]};
const component=props=><I18nProvider><EarthEngineExplorer areaBounds={area} areaGeometry={geometry} {...props}/></I18nProvider>;
const select=index=>screen.getByRole('checkbox',{name:`选择结果 Export ${index}`});
const checked=index=>select(index).getAttribute('aria-checked')==='true';
const load=()=>screen.getByRole('button',{name:'预览所选'});
const operationNames=()=>earthEngineRequest.mock.calls.map(([operation])=>operation);
async function confirmDownload(){
  const dialog=screen.getByRole('dialog',{name:'确认下载'});
  expect(operationNames().includes('download')).toBe(false);
  fireEvent.click(within(dialog).getByRole('button',{name:'确认并下载'}));
  await waitFor(()=>expect(within(dialog).getAllByRole('button',{name:'关闭'})[0].disabled).toBe(false));
  fireEvent.click(within(dialog).getAllByRole('button',{name:'关闭'})[0]);
  await waitFor(()=>expect(screen.queryByRole('dialog',{name:'确认下载'})).toBeNull());
}
function inspectPlayback(){
  earthEngineRequest.mockImplementation(async(operation,payload)=>operation==='list'?records:operation==='inspect'?{...structuredClone(stacInspection),exportId:payload.id}:[]);
}

it('inline lists saved exports without reading raster previews or downloading, limits selection to four and places them only after explicit Load',async()=>{
  inspectPlayback();render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  expect(screen.queryByRole('dialog')).toBeNull();expect(screen.getByRole('region',{name:'数据源筛选与结果'})).toBeTruthy();
  expect(operationNames()).toEqual(['list']);expect(map.current.entries).toEqual([]);expect(load().disabled).toBe(true);
  for(let index=0;index<4;index++)fireEvent.click(select(index));
  expect(select(4).disabled).toBe(true);fireEvent.click(select(4));expect(checked(4)).toBe(false);
  expect(operationNames()).toEqual(['list']);expect(map.current.entries).toEqual([]);
  fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(4));
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect').map(([,payload])=>payload)).toEqual(records.slice(0,4).map(record=>({id:record.id})));
  expect(map.current.entries.map(entry=>entry.key)).toEqual(records.slice(0,4).map(record=>record.id));
  expect(map.current.entries.every(entry=>entry.visible&&entry.opacity===1&&entry.kind==='map')).toBe(true);
  expect(map.current.entries[0].data.asset).toMatchObject({bounds:stacInspection.bounds,crs:stacInspection.crs,imageExtent:stacInspection.bounds});
  expect(map.current.areaBounds).toEqual(area);expect(map.current.areaGeometry).toEqual(geometry);
  expect(operationNames().includes('download')).toBe(false);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it('map visibility, opacity and removal preserve the explicit GEE export selections',async()=>{
  inspectPlayback();render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(1));
  await act(async()=>map.current.onPatch(records[0].id,{visible:false,opacity:0.35}));
  expect(map.current.entries[0]).toMatchObject({visible:false,opacity:0.35});expect(checked(0)).toBe(true);
  await act(async()=>map.current.onRemove(records[0].id));
  expect(map.current.entries).toEqual([]);expect(checked(0)).toBe(true);
  expect(operationNames()).toEqual(['list','inspect']);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it('one failed inspection does not discard another selected export and its row retries only that export',async()=>{
  let failFirst=true;
  earthEngineRequest.mockImplementation(async(operation,payload)=>{
    if(operation==='list')return records;
    if(operation==='inspect'){
      if(payload.id===records[0].id&&failFirst)throw new Error('Preview unavailable for Export 0');
      return {...structuredClone(stacInspection),exportId:payload.id};
    }
    return [];
  });
  render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(select(1));fireEvent.click(load());
  await waitFor(()=>expect(map.current.entries.map(entry=>entry.key)).toEqual([records[1].id]));
  const failure=await screen.findByRole('alert');
  expect(failure.textContent).toBe('Preview unavailable for Export 0');
  expect(failure.closest('article').textContent).toContain('Export 0');
  expect(screen.queryAllByRole('alert')).toHaveLength(1);
  expect(checked(0)).toBe(true);expect(checked(1)).toBe(true);
  const successfulEntry=map.current.entries[0];failFirst=false;
  fireEvent.click(within(failure.closest('article')).getByRole('button',{name:/重新预览|重试预览/}));
  await waitFor(()=>expect(map.current.entries.map(entry=>entry.key)).toEqual([records[1].id,records[0].id]));
  expect(map.current.entries[0]).toBe(successfulEntry);expect(screen.queryByRole('alert')).toBeNull();
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect').map(([,payload])=>payload.id)).toEqual([records[0].id,records[1].id,records[0].id]);
  expect(operationNames().includes('download')).toBe(false);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it('the four-layer limit includes loaded exports that are no longer selected and removal restores capacity',async()=>{
  inspectPlayback();render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  for(let index=0;index<4;index++)fireEvent.click(select(index));
  fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(4));
  fireEvent.click(select(0));fireEvent.click(select(4));
  expect(checked(0)).toBe(false);expect(checked(4)).toBe(true);expect(load().disabled).toBe(true);
  fireEvent.click(load());
  expect(map.current.entries.map(entry=>entry.key)).toEqual(records.slice(0,4).map(record=>record.id));
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(4);
  await act(async()=>map.current.onRemove(records[0].id));
  expect(load().disabled).toBe(false);expect(checked(4)).toBe(true);
  fireEvent.click(load());await waitFor(()=>expect(map.current.entries.map(entry=>entry.key)).toEqual(records.slice(1,5).map(record=>record.id)));
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect').map(([,payload])=>payload.id)).toEqual(records.map(record=>record.id));
  expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it('loading an already loaded selection neither repeats inspection nor resets its map state while new exports append',async()=>{
  inspectPlayback();render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(1));
  await act(async()=>map.current.onPatch(records[0].id,{visible:false,opacity:0.35}));
  const originalEntry=map.current.entries[0];
  fireEvent.click(load());await waitFor(()=>expect(load().disabled).toBe(false));
  expect(map.current.entries).toHaveLength(1);expect(map.current.entries[0]).toBe(originalEntry);
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(1);
  fireEvent.click(select(1));fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(2));
  expect(map.current.entries[0]).toBe(originalEntry);expect(map.current.entries[0]).toMatchObject({visible:false,opacity:0.35});
  expect(map.current.entries[1]).toMatchObject({key:records[1].id,visible:true,opacity:1});
  expect(earthEngineRequest.mock.calls.filter(([operation])=>operation==='inspect').map(([,payload])=>payload.id)).toEqual([records[0].id,records[1].id]);
  expect(checked(0)).toBe(true);expect(checked(1)).toBe(true);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it.each(['bounds','polygon'])('changing AOI %s clears selected exports and loaded previews without a new download',async(change)=>{
  inspectPlayback();const view=render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(load());await waitFor(()=>expect(map.current.entries).toHaveLength(1));
  view.rerender(component(change==='bounds'?{areaBounds:[11,21,12,22]}:{areaGeometry:differentGeometry}));
  await waitFor(()=>expect(map.current.entries).toEqual([]));
  expect(checked(0)).toBe(false);expect(load().disabled).toBe(true);
  expect(operationNames()).toEqual(['list','inspect']);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});

it('late inspection for the previous AOI cannot restore old previews or old selections',async()=>{
  let finish;const pending=new Promise(resolve=>{finish=resolve;});
  earthEngineRequest.mockImplementation(async operation=>operation==='list'?records:pending);
  const view=render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(load());await waitFor(()=>expect(operationNames()).toEqual(['list','inspect']));
  view.rerender(component({areaBounds:[11,21,12,22]}));
  expect(map.current.entries).toEqual([]);expect(checked(0)).toBe(false);
  await act(async()=>finish(structuredClone(stacInspection)));
  await waitFor(()=>expect(select(0).disabled).toBe(false));
  expect(map.current.entries).toEqual([]);expect(checked(0)).toBe(false);expect(map.current.areaBounds).toEqual([11,21,12,22]);
  expect(operationNames()).toEqual(['list','inspect']);
});

it('an invalid grid is reported without inventing a preview or starting any GEE download',async()=>{
  earthEngineRequest.mockImplementation(async operation=>operation==='list'?records:{...structuredClone(stacInspection),bounds:[10,20,12,22]});
  render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(select(0));fireEvent.click(load());
  await screen.findByRole('alert');expect(map.current.entries).toEqual([]);
  expect(operationNames()).toEqual(['list','inspect']);
});

it('inline quota text remains measured-usage unknown in Chinese and does not imply transfer time is EECU consumption',async()=>{
  inspectPlayback();render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.click(screen.getByRole('button',{name:'GEE 计算用量'}));
  const quota=screen.getByText(/EECU 用量：尚未测得。下载耗时不代表计算消耗/);
  expect(quota.textContent).toContain('Completed EECU-seconds');
  expect(operationNames()).toEqual(['list']);
});

it('a new signed link is downloaded only after the explicit action and does not automatically select or load its new saved export',async()=>{
  const newRecord={...records[0],id:'new-export',name:'New export'};
  earthEngineRequest.mockImplementation(async operation=>operation==='list'?records:operation==='download'?{export:newRecord,raster:structuredClone(stacInspection)}:[]);
  render(component());await screen.findByRole('checkbox',{name:'选择结果 Export 0'});
  fireEvent.change(screen.getByLabelText('GEE 下载链接'),{target:{value:url}});
  expect(operationNames()).toEqual(['list']);expect(map.current.entries).toEqual([]);
  fireEvent.click(screen.getByRole('button',{name:'下载并校验'}));
  await confirmDownload();
  await screen.findByRole('checkbox',{name:'选择结果 New export'});
  expect(operationNames()).toEqual(['list','download']);expect(screen.getByLabelText('GEE 下载链接').value).toBe('');
  expect(screen.getByRole('checkbox',{name:'选择结果 New export'}).getAttribute('aria-checked')).toBe('false');
  expect(map.current.entries).toEqual([]);expect(saveEarthEngineFile).not.toHaveBeenCalled();
});
it('download stays disabled for invalid URLs, clears the signed link and displays real result with unknown usage',async()=>{
  render(<I18nProvider><EarthEngineDialog areaBounds={[116.25,39.8,116.5,40]} onClose={vi.fn()}/></I18nProvider>);
  await screen.findByText('还没有已校验的 GEE 成果。');
  const input=screen.getByLabelText('GEE 下载链接'), button=screen.getByRole('button',{name:'下载并校验'});
  expect(button.disabled).toBe(true);
  fireEvent.change(input,{target:{value:'https://evil.test/file.tif'}});expect(button.disabled).toBe(true);
  fireEvent.change(input,{target:{value:url}});expect(button.disabled).toBe(false);
  const result={export:{id:'abc',name:'Real export',projectId:'example-project',bytes:917329,downloadSeconds:1.95,sha256:'a'.repeat(64)},raster:{width:719,height:746,crs:'EPSG:32650',bands:[{dataType:'UInt16'}],limitations:[]}};
  earthEngineRequest.mockResolvedValueOnce(result);
  fireEvent.click(button);
  await confirmDownload();
  await screen.findByRole('button',{name:'另存为 GeoTIFF…'});
  expect(input.value).toBe('');
  expect(earthEngineRequest).toHaveBeenCalledWith('download',{name:'Landsat · GEE',url});
  expect(screen.getByText('719 × 746 · EPSG:32650 · UInt16')).toBeTruthy();
  expect(screen.getByText(/EECU 用量：尚未测得/,{selector:'small'})).toBeTruthy();
});
it('an expired link produces no success or preview and can be retried',async()=>{
  render(<I18nProvider><EarthEngineDialog onClose={vi.fn()}/></I18nProvider>);
  await screen.findByText('还没有已校验的 GEE 成果。');
  fireEvent.change(screen.getByLabelText('GEE 下载链接'),{target:{value:url}});
  earthEngineRequest.mockRejectedValueOnce(new Error('GEE link expired or access was refused; generate a new link in your project'));
  fireEvent.click(screen.getByRole('button',{name:'下载并校验'}));
  await confirmDownload();
  await waitFor(()=>expect(screen.getByRole('alert').textContent).toContain('链接已过期'));
  expect(screen.queryByRole('button',{name:'另存为 GeoTIFF…'})).toBeNull();
  expect(screen.getByLabelText('GEE 下载链接').disabled).toBe(false);
});
