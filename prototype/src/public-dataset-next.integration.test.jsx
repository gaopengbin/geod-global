import React from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PublicDatasetExplorer } from './public-dataset-ui.jsx';
import { StacRasterInspection } from './stac-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { stacRequest } from './stac-client.js';
import { nextPublicDatasets, nextPublicCases } from './public-dataset-next-fixtures.js';
import publicDatasetChinese from './locales/public-dataset.zh-CN.js';

const language=vi.hoisted(()=>({value:'en'}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({
  t:(text,values={})=>(language.value==='zh-CN'?publicDatasetChinese[text]||text:text).replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),
  number:String,date:value=>value||'—',locale:language.value,
})}));
vi.mock('./public-dataset-client.js',async original=>({...await original(),publicDatasetRequest:vi.fn()}));
vi.mock('./stac-client.js',async original=>({...await original(),stacRequest:vi.fn()}));
// This test inspects delivery/UI behavior; georeferencing is covered separately
// by the real native receipt and map model tests, not by this test double.
vi.mock('./stac-map.jsx',()=>({default:({data})=><div aria-label="Georeferenced original raster" data-original-bounds={data.bounds.join(',')}/>}));
const refresh=vi.fn(async()=>{});
const byId=id=>nextPublicCases.find(value=>value.datasetId===id);
const operations=()=>stacRequest.mock.calls.map(([operation])=>operation);
const mount=(value,props={})=>render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={value.datasetId} areaBounds={value.request.bounds} {...props}/></RuntimeContext.Provider>);
const resolveLabel=value=>value.plan.dataset.discovery==='catalog-raster'?'Review catalog search':value.plan.dataset.discovery==='stac-items'?'Review published items':'Find published files';
const connectLabel=value=>value.plan.dataset.discovery==='catalog-raster'?'Search product catalog':value.plan.dataset.discovery==='stac-items'?'Read published item metadata':'Check source files';
function playback(value) {
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?nextPublicDatasets:operation==='resolve'?value.resolvedPlan:value.connection);
  stacRequest.mockImplementation(async(operation,payload)=>operation==='snapshot'?value.snapshot:operation==='project'?value.project:operation==='downloads'?{jobs:[value.job]}:operation==='inspect'?value.inspection:value.pixels.find(pixel=>pixel.column===payload.column&&pixel.row===payload.row));
}
beforeEach(()=>{vi.clearAllMocks();language.value='en';});

it.each(nextPublicCases)('$datasetId preserves the requested map area and requires separate review, connect, save and download actions',async value=>{
  playback(value);const user=userEvent.setup(),requestChanged=vi.fn();mount(value,{onRequestChange:requestChanged});
  const review=await screen.findByRole('button',{name:resolveLabel(value)});
  await waitFor(()=>expect(review.disabled).toBe(false));
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe(value.request.bounds.join(', '));
  expect(requestChanged.mock.calls.at(-1)[0]).toEqual(value.request);
  expect(screen.queryByRole('textbox',{name:'Dataset country code'})).toBeNull();
  expect(screen.queryByRole('combobox',{name:'Published region'})).toBeNull();
  expect(screen.queryByRole('combobox',{name:'Published season'})).toBeNull();
  if(value.datasetId.startsWith('jaxa-')) {
    expect(screen.getByRole('combobox',{name:'Observation year'}).textContent).toBe('2025');
    expect(screen.getByRole('combobox',{name:'Observation month'}).textContent).toBe('12');
  } else {
    expect(screen.queryByRole('combobox',{name:/year|month/i})).toBeNull();
  }
  expect(operations()).toEqual([]);
  await user.click(review);
  await screen.findByRole('button',{name:connectLabel(value)});
  expect(publicDatasetRequest.mock.calls.find(([operation])=>operation==='resolve')[1]).toEqual(value.request);
  expect(operations()).toEqual([]);
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();
  if(value.datasetId==='usgs-3dep') {
    expect(screen.getByText('Catalog sources · 1')).toBeTruthy();
    expect(screen.getByText('USGS 3DEP · published catalog query')).toBeTruthy();
    expect(screen.queryByText(value.plan.sources[0].title)).toBeNull();
  }
  await user.click(screen.getByRole('button',{name:connectLabel(value)}));
  const selection=await screen.findByRole('checkbox',{name:`Select asset · ${value.snapshot.itemId} · ${value.assetKey}`});
  expect(operations()).toEqual(['snapshot']);
  expect(publicDatasetRequest.mock.calls.find(([operation])=>operation==='connect')[1]).toEqual(value.request);
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe(value.request.bounds.join(', '));
  if(value.datasetId==='usgs-3dep') {
    expect(screen.queryByText('USGS 3DEP · published catalog query')).toBeNull();
    expect(screen.getByText('Publisher release date: 2026-09-17 · Release date is not the terrain observation date.')).toBeTruthy();
    expect(screen.queryByText(/Actual product period:/)).toBeNull();
  } else if(value.datasetId.startsWith('jaxa-')) {
    expect(screen.getAllByText(`Actual product period: ${value.snapshot.startDatetime.slice(0,10)} – ${value.snapshot.endDatetime.slice(0,10)}`).length).toBeGreaterThan(0);
  }
  expect(screen.getByRole('button',{name:'Save asset selection'}).disabled).toBe(true);
  await user.click(selection);
  expect(selection.getAttribute('aria-checked')).toBe('true');
  const card=selection.closest('article');
  await user.click(within(card).getByRole('button',{name:'Asset source details'}));
  const details=screen.getByRole('dialog',{name:'Asset source details'});
  expect(details.textContent).toContain(value.plan.dataset.layers[0].unit);
  expect(details.textContent).toContain(value.plan.dataset.layers[0].valueInterpretation);
  await user.keyboard('{Escape}');
  expect(operations()).toEqual(['snapshot']);
  await user.click(screen.getByRole('button',{name:'Save asset selection'}));
  await screen.findByRole('button',{name:'Download selected originals'});
  const saveCall=stacRequest.mock.calls.find(([operation])=>operation==='project')[1];
  expect(saveCall.bounds).toEqual(value.request.bounds);
  expect(saveCall.selections).toEqual([{snapshotId:value.snapshot.id,assetKey:value.assetKey}]);
  expect(operations()).toEqual(['snapshot','project']);
  await user.click(screen.getByRole('button',{name:'Download selected originals'}));
  await screen.findByRole('button',{name:'Original files ready'});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='downloads')[1]).toEqual({projectId:value.project.id,selections:saveCall.selections});
  expect(screen.getByRole('link',{name:'Open project'}).getAttribute('href')).toBe(`#My%20Data?project=${value.project.id}`);
});

it('changing an exploration AOI clears a reviewed USGS selection and does not query or download until reviewed again',async()=>{
  const value=byId('usgs-3dep'),user=userEvent.setup(),requestChanged=vi.fn();playback(value);
  const view=mount(value,{onRequestChange:requestChanged});
  await user.click(await screen.findByRole('button',{name:resolveLabel(value)}));
  await user.click(await screen.findByRole('button',{name:connectLabel(value)}));
  await user.click(await screen.findByRole('checkbox',{name:`Select asset · ${value.snapshot.itemId} · raster`}));
  const nextBounds=[-73.999,40.711,-73.995,40.715];
  view.rerender(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={value.datasetId} areaBounds={nextBounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  await waitFor(()=>expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe(nextBounds.join(', ')));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();
  expect(screen.queryByRole('button',{name:'Save asset selection'})).toBeNull();
  expect(requestChanged.mock.calls.at(-1)[0]).toEqual({...value.request,bounds:nextBounds});
  expect(operations()).toEqual(['snapshot']);
  expect(publicDatasetRequest.mock.calls.map(([operation])=>operation)).toEqual(['list','resolve','connect']);
});

it.each(nextPublicCases)('$datasetId inspection shows real typed NoData and unconverted original raw values',async value=>{
  playback(value);const user=userEvent.setup();render(<StacRasterInspection job={value.job} onClose={vi.fn()}/>);
  await user.click(await screen.findByRole('button',{name:'File metadata and display'}));
  const row=screen.getByRole('table').querySelector('tbody tr'),band=value.inspection.bands[0];
  expect(within(row).getAllByRole('cell').map(cell=>cell.textContent)).toEqual(['1',band.dataType,String(band.nodata)]);
  expect(screen.getByText('CRS from file').nextElementSibling.textContent).toBe(value.inspection.crs);
  expect(screen.getByRole('img',{name:'First-band display preview'}).getAttribute('src')).toBe(value.inspection.previewDataUrl);
  expect(screen.getByLabelText('Georeferenced original raster').getAttribute('data-original-bounds')).toBe(value.inspection.bounds.join(','));
  const negative=value.datasetId==='usgs-3dep'?value.pixels.find(pixel=>pixel.values[0]<0&&!pixel.noData[0]):value.datasetId==='chelsa-climate'?value.pixels.find(pixel=>pixel.values[0]===-323):value.datasetId==='jaxa-amsr2-soil-moisture'?value.pixels.find(pixel=>pixel.values[0]===-32767):value.pixels.find(pixel=>pixel.values[0]===13785);
  const missing=value.pixels.find(pixel=>pixel.noData[0]);
  await user.keyboard('{Escape}');
  for(const pixel of [negative,missing]) {
    await user.clear(screen.getByRole('textbox',{name:'Pixel column'}));await user.type(screen.getByRole('textbox',{name:'Pixel column'}),String(pixel.column));
    await user.clear(screen.getByRole('textbox',{name:'Pixel row'}));await user.type(screen.getByRole('textbox',{name:'Pixel row'}),String(pixel.row));
    await user.click(screen.getByRole('button',{name:'Read raw pixel'}));
    await waitFor(()=>expect(document.querySelector('.stac-pixel-result strong').textContent).toBe(`${pixel.values[0]}${pixel.noData[0]?' · NoData':''}`));
    expect(stacRequest.mock.calls.filter(([operation])=>operation==='pixel').at(-1)[1]).toEqual({id:value.job.id,column:pixel.column,row:pixel.row});
  }
});

it('Chinese CHELSA details explain the fixed normal, signed samples and 32767 mask without adding year/month inputs',async()=>{
  const value=byId('chelsa-climate'),user=userEvent.setup();language.value='zh-CN';playback(value);mount(value);
  await screen.findByRole('heading',{name:'CHELSA 气候水分指数 · 1981–2010 气候平均场'});
  expect(screen.queryByRole('combobox',{name:/年份|月份|year|month/i})).toBeNull();
  expect(screen.getByText('所选区域用于组织一份完整的近全球 CMI 均值原件，固定代表 1981–2010 气候平均场；不裁剪、重采样或改变原始气候时段。')).toBeTruthy();
  await user.click(screen.getByRole('button',{name:publicDatasetChinese['Product details']}));
  const details=document.querySelector('.public-dataset-definition');
  expect(details.textContent).toContain('负值表示模型中的气候水分亏缺');
  expect(details.textContent).toContain('32767');expect(details.textContent).toContain('kg·m⁻²·月⁻¹');
  expect(operations()).toEqual([]);
});

it.each(nextPublicCases.filter(value=>value.datasetId.startsWith('jaxa-')))('$datasetId Chinese scientific terms retain proprietary, original error codes and advance commercial notice',async value=>{
  language.value='zh-CN';playback(value);const user=userEvent.setup();mount(value);
  await screen.findByRole('heading',{name:publicDatasetChinese[value.plan.dataset.title]});
  await user.click(screen.getByRole('button',{name:publicDatasetChinese['Product details']}));
  const details=document.querySelector('.public-dataset-definition');
  expect(details.textContent).toContain('proprietary');
  expect(within(details).getByRole('heading',{name:'产品限制'})).toBeTruthy();
  expect(details.textContent).toContain('商业使用须事先通知 JAXA');
  expect(details.textContent).toContain('不等于 Creative Commons 许可');
  expect(details.textContent).toContain('不是任务原始 HDF');
  expect(details.textContent).toContain(value.datasetId==='jaxa-amsr2-soil-moisture'?'-32767 至 -32761':'65535');
  expect(operations()).toEqual([]);
});
