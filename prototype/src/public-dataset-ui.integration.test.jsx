import React from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PublicDatasetDialog, PublicDatasetExplorer } from './public-dataset-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { stacRequest } from './stac-client.js';
import { SOURCE_DIRECTORY } from './source-directory.js';
import { publicDataset, datasetPayload, datasetPlan, datasetConnected, datasetSnapshot, datasetProject, datasetJob, catalogDataset, catalogPayload, catalogQuery, catalogPlan, catalogConnected, catalogSnapshot } from './public-dataset-fixtures.js';
import { snapshotId } from './stac-fixtures.js';
import { worldcerealDataset, worldcerealRequest, worldcerealPlan, worldcerealConnection, worldcerealSnapshot, worldcerealConfidenceCase } from './public-dataset-worldcereal-fixtures.js';
import { deafricaDatasets, deafricaCases } from './public-dataset-deafrica-fixtures.js';
import publicDatasetChinese from './locales/public-dataset.zh-CN.js';

const language=vi.hoisted(()=>({value:'en'}));
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (text, values = {}) => (language.value==='zh-CN'?publicDatasetChinese[text]||text:text).replace(/\{(\w+)\}/g, (_, key) => values[key] ?? ''), number: String }) }));
vi.mock('./public-dataset-client.js', async original => ({ ...await original(), publicDatasetRequest: vi.fn() }));
vi.mock('./stac-client.js', async original => ({ ...await original(), stacRequest: vi.fn() }));
const refresh = vi.fn(async () => {});
const mount = props => render(<RuntimeContext.Provider value={{ projects: [datasetProject], refresh }}><PublicDatasetDialog datasetId="worldcover" areaBounds={[10,20,11,21]} onClose={vi.fn()} {...props}/></RuntimeContext.Provider>);
// These older tests use example.com transport doubles, not publisher records.
// Match the actual direct-file response shape: one raster and an explicit client
// preset bound to the reviewed request, without creating publisher STAC claims.
const directTransportSnapshot = plan => {
  const source=plan.sources[0],proof={schemaVersion:2,dataset:plan.dataset,request:plan.request,
    layer:plan.dataset.layers.find(layer=>layer.id===plan.request.layer),sourceId:source.id,
    sourceTitle:source.title,sourceBounds:source.bounds??null,
    declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  return {...datasetSnapshot,collectionId:null,datetime:null,startDatetime:null,endDatetime:null,temporalStatus:'missing',bbox:source.bounds??null,
    properties:{datasetPreset:proof},provenance:{...datasetSnapshot.provenance,documentUrl:source.stac.url},
    assets:[{...datasetSnapshot.assets[0],key:'raster',href:source.stac.url,metadata:{kind:'direct-raster',datasetPreset:proof}}]};
};
beforeEach(() => {
  language.value='en';
  vi.clearAllMocks();
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [publicDataset] : operation === 'resolve' ? datasetPlan : datasetConnected);
  stacRequest.mockImplementation(async operation => {
    if(operation==='snapshot') { const connection=await publicDatasetRequest.mock.results.at(-1).value;return directTransportSnapshot(connection.plan); }
    return operation==='project'?{...datasetProject,canonicalSelections:[{snapshotId,assetKey:'raster'}]}:{jobs:[datasetJob]};
  });
});

it.each(['en','zh-CN'])('only colliding selected SPI item/asset names gain their original longitude range in %s',async locale=>{
  // UI transport doubles reproduce the two author hemisphere titles; they
  // are not claimed publisher snapshots or original-download acceptance.
  language.value=locale;
  const label=text=>locale==='zh-CN'?publicDatasetChinese[text]||text:text;
  const dataset={...publicDataset,id:'jaxa-gsmap-spi',title:'SPI title fixture',discovery:'stac-items',years:[2026],defaultYear:2026,months:[9],defaultMonth:9,
    layers:[{id:'spi',title:'SPI original DN',unit:'-',valueInterpretation:'Original float samples',scale:1,offset:0,nodata:-999}],defaultLayer:'spi'};
  const request={datasetId:dataset.id,bounds:[-1,-1,1,1],year:2026,month:9,layer:'spi'};
  const sources=[[-180,-90,0,90],[0,-90,180,90]].map((bounds,index)=>({id:`spi-${index}`,title:`SPI hemisphere ${index}`,bounds,
    stac:{name:`SPI ${index}`,url:`https://example.com/spi-${index}.json`,kind:'item'},startDatetime:'2026-09-01T00:00:00Z',endDatetime:'2026-09-30T23:59:59Z'}));
  const plan={dataset,request,sources,warnings:[]};
  const items=sources.map((source,index)=>{
    const proof={schemaVersion:2,dataset,request,layer:dataset.layers[0],sourceId:source.id,sourceTitle:source.title,sourceBounds:source.bounds,
      declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
    return {...datasetSnapshot,id:String(index+6).repeat(64),connectionId:`f${index+1}${'1'.repeat(6)}-1111-4111-8111-111111111111`,itemId:'S90.00-N90.00',title:'S90.00-N90.00',bbox:source.bounds,
      datetime:null,startDatetime:source.startDatetime,endDatetime:source.endDatetime,temporalStatus:'interval',
      properties:{start_datetime:source.startDatetime,end_datetime:source.endDatetime},
      provenance:{...datasetSnapshot.provenance,documentUrl:source.stac.url,reviewedDataset:proof},
      assets:[{...datasetSnapshot.assets[0],key:'SPI',title:'SPI',href:`https://example.com/spi-${index}.tif`,metadata:{'je:rasters':{value:{unit:'-'},dn2value:{slope:1,offset:0},dn:{data_type:'float32',min:null,max:null,nodata:-999}}}}]};
  });
  const connection={plan,failures:[],complete:true,wcsConnections:[],stacConnections:items.map((item,index)=>({...datasetConnected.stacConnections[0],
    id:item.connectionId,name:sources[index].stac.name,url:sources[index].stac.url,kind:'item',snapshotIds:[item.id]}))};
  const original=JSON.stringify({plan,items});
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?[dataset]:operation==='resolve'?plan:connection);
  const pins=items.map(item=>({snapshotId:item.id,assetKey:'SPI'}));
  stacRequest.mockImplementation(async(operation,payload)=>operation==='snapshot'?items.find(item=>item.id===payload.id):operation==='project'?{...datasetProject,canonicalSelections:pins}:{jobs:[datasetJob]});
  const user=userEvent.setup();
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={dataset.id} areaBounds={request.bounds} autoSearch/></RuntimeContext.Provider>);
  await waitFor(()=>expect(screen.getAllByRole('checkbox',{name:`${label('Select asset')} · S90.00-N90.00 · SPI`})).toHaveLength(2));
  const selectors=screen.getAllByRole('checkbox',{name:`${label('Select asset')} · S90.00-N90.00 · SPI`});
  await user.click(selectors[0]);await user.click(screen.getByRole('button',{name:label('Load selected files to map')}));
  const region=screen.getByRole('region',{name:label('Product preview layers')});
  expect(within(region).getByText('S90.00-N90.00 · SPI original DN',{exact:true})).toBeTruthy();
  await user.click(selectors[1]);await user.click(screen.getByRole('button',{name:label('Load selected files to map')}));
  for(const [west,east] of [[-180,0],[0,180]]){
    const longitude=label('Longitude {west}° to {east}°').replace('{west}',String(west)).replace('{east}',String(east));
    expect(within(region).getByText(`${longitude} · S90.00-N90.00 · SPI original DN`,{exact:true})).toBeTruthy();
  }
  expect(within(region).queryByText('S90.00-N90.00 · SPI original DN',{exact:true})).toBeNull();
  await user.click(within(screen.getByRole('region',{name:label('Selected product actions')})).getByRole('button',{name:label('Save and download')}));
  expect(stacRequest.mock.calls.some(([operation])=>['project','downloads'].includes(operation))).toBe(false);
  await user.click(screen.getByRole('button',{name:label('Confirm and download')}));
  await screen.findByRole('link',{name:label('View download tasks')});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='project')[1].selections).toEqual(pins);
  expect(stacRequest.mock.calls.find(([operation])=>operation==='downloads')[1].selections).toEqual(pins);
  expect(JSON.stringify({plan,items})).toBe(original);
});
async function resolveAndConnect(user) {
  await waitFor(() => expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(false));
  await user.click(screen.getByRole('button', { name: 'Find published files' }));
  await user.click(await screen.findByRole('button', { name: 'Check source files' }));
  await screen.findByRole('checkbox', { name: 'Select asset · model-42 · raster' });
}

it.each(deafricaDatasets)('reviews actual $id original assets in the existing explorer without region or season guesses', async dataset => {
  const c=deafricaCases.find(c=>c.datasetId===dataset.id&&c.request.year===dataset.defaultYear&&c.layer===dataset.defaultLayer),user=userEvent.setup(),requestChanged=vi.fn();
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?deafricaDatasets:operation==='resolve'?c.plan:c.connection);
  stacRequest.mockResolvedValue(c.snapshot);
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={dataset.id} areaBounds={c.request.bounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  await screen.findByRole('button',{name:'Review published items'});
  expect(screen.queryByRole('combobox',{name:'Published region'})).toBeNull();expect(screen.queryByRole('combobox',{name:'Published season'})).toBeNull();
  expect(screen.queryByRole('combobox',{name:/month/i})).toBeNull();expect(screen.queryByRole('textbox',{name:'Dataset country code'})).toBeNull();
  expect(requestChanged.mock.calls.at(-1)[0]).toEqual(c.request);
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe(c.request.bounds.join(', '));
  await user.click(screen.getByRole('button',{name:'Review published items'}));
  expect(await screen.findByText(`Actual product period: ${c.snapshot.startDatetime.slice(0,10)} – ${c.snapshot.endDatetime.slice(0,10)}`)).toBeTruthy();
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();
  await user.click(screen.getByRole('button',{name:'Read published item metadata'}));
  const asset=c.snapshot.assets.find(asset=>asset.eligible&&(asset.key===c.layer||dataset.id==='wsf-settlements'||dataset.id==='cgls-landcover'));
  const selection=await screen.findByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · ${asset.key}`});
  await user.click(within(selection.closest('article')).getByRole('button',{name:'Asset source details'}));
  expect(screen.getByRole('dialog',{name:'Asset source details'}).textContent).toContain(`Declared unit: ${dataset.layers.find(layer=>layer.id===c.layer).unit}`);
  await user.keyboard('{Escape}');
  await user.click(selection);expect(selection.getAttribute('aria-checked')).toBe('true');
  if(dataset.id==='cgls-landcover') {
    const unavailable=c.snapshot.assets.filter(a=>!a.eligible);
    expect(unavailable).toHaveLength(13);
    for(const other of unavailable) {
      const disabled=screen.getByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · ${other.key}`});
      expect(disabled.getAttribute('data-disabled')).not.toBeNull();
      await user.click(within(disabled.closest('article')).getByRole('button',{name:'Asset source details'}));
      const details=screen.getByRole('dialog',{name:'Asset source details'});
      expect(details.textContent).toContain('This layer is not connected. Currently only permanent-water cover fraction is supported.');
      expect(details.textContent).not.toContain(other.reason);
      await user.keyboard('{Escape}');
      expect(other.reason).toBe('Use a public HTTPS data service without credentials or fragments');
    }
  }
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['snapshot']);
  await user.clear(screen.getByRole('textbox',{name:'Dataset search bounds'}));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();expect(screen.queryByRole('button',{name:'Save asset selection'})).toBeNull();
});

it('annual regional metadata retains its actual interval and gives each simultaneous original asset its own component and unit', async () => {
  const c=deafricaCases.find(value=>value.datasetId==='deafrica-fractional-cover'),user=userEvent.setup(),requestChanged=vi.fn();
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?deafricaDatasets:operation==='resolve'?c.plan:c.connection);
  stacRequest.mockResolvedValue(c.snapshot);
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={c.datasetId} areaBounds={c.request.bounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  await screen.findByRole('button',{name:'Review published items'});
  expect(screen.queryByRole('combobox',{name:'Published region'})).toBeNull();expect(screen.queryByRole('combobox',{name:'Published season'})).toBeNull();
  expect(screen.getByRole('combobox',{name:'Product layer'}).textContent).toBe('Green vegetation · annual percentile 50');
  expect(requestChanged.mock.calls.at(-1)[0]).toEqual(c.request);
  await user.click(screen.getByRole('button',{name:'Review published items'}));
  expect(await screen.findByText('Actual product period: 2025-01-01 – 2025-12-31')).toBeTruthy();
  expect(screen.getByText('Published region: x218y075')).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Read published item metadata'}));
  const checkbox=key=>screen.getByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · ${key}`});
  await screen.findByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · pv_pc_50`});
  for(const [key,unit,title] of [['npv_pc_50','modelled percentage','Non-green vegetation · annual percentile 50'],['count_valid','observation count','Clear valid observation count'],['qa','quality code','Annual summary quality code']]) {
    const card=checkbox(key).closest('article');
    expect(card.textContent).toContain(title);
    await user.click(within(card).getByRole('button',{name:'Asset source details'}));
    const details=screen.getByRole('dialog',{name:'Asset source details'});
    expect(details.textContent).toContain(`Declared unit: ${unit}`);
    if(key==='count_valid'||key==='qa') expect(details.textContent).toContain('have no embedded TIFF NoData tag, although the publisher table declares 255');
    await user.keyboard('{Escape}');
    await user.click(checkbox(key));expect(checkbox(key).getAttribute('aria-checked')).toBe('true');
  }
  expect(screen.getByText('3 original assets selected')).toBeTruthy();
  const green=checkbox('pv_pc_50').closest('article');await user.click(within(green).getByRole('button',{name:'Asset source details'}));
  const original=c.snapshot.assets.find(asset=>asset.key==='pv_pc_50');
   expect(within(screen.getByRole('dialog',{name:'Asset source details'})).getByText('Original asset reference').nextElementSibling.textContent).toBe(original.metadata.href);
   expect(within(screen.getByRole('dialog',{name:'Asset source details'})).getByText('Public download URL').nextElementSibling.textContent).toBe(original.href);
  expect(screen.getByText(`Original published item bounds: ${c.snapshot.bbox.join(', ')}`)).toBeTruthy();
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['snapshot']);
   await user.keyboard('{Escape}');
   await user.click(screen.getByRole('combobox',{name:'Product layer'}));await user.click(screen.getByRole('option',{name:'Clear valid observation count',exact:true}));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();expect(requestChanged.mock.calls.at(-1)[0].layer).toBe('count_valid');
});

it('regional themes automatically select their actual published season without replacing the map, and archive real standalone source pins', async () => {
  const user=userEvent.setup(), requestChanged=vi.fn();
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [worldcerealDataset] : operation === 'resolve' ? worldcerealPlan : worldcerealConnection);
  stacRequest.mockImplementation(async operation => operation === 'snapshot' ? worldcerealSnapshot : datasetProject);
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId="worldcereal" areaBounds={worldcerealRequest.bounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  const area=await screen.findByRole('textbox',{name:'Dataset search bounds'});
  expect(area.value).toBe(worldcerealRequest.bounds.join(', '));
  expect(screen.getByRole('combobox',{name:'Published region'}).textContent).toContain('Match current map area automatically');
  expect(requestChanged.mock.calls.at(-1)[0]).toEqual({...worldcerealRequest,region:undefined,layer:'temporarycrops',season:'tc-annual'});
  expect(screen.queryByRole('combobox',{name:/month/i})).toBeNull();
  expect(screen.queryByRole('textbox',{name:'Dataset country code'})).toBeNull();
  await user.click(screen.getByRole('combobox',{name:'Product layer'}));
  await user.click(screen.getByRole('option',{name:'Active cropland · winter-cereals window',exact:true}));
  await user.click(screen.getByRole('combobox',{name:'Published region'}));
  await user.click(screen.getByRole('option',{name:'Mauritius / Réunion · AEZ 9026',exact:true}));
  await user.click(screen.getByRole('button',{name:'Review published items'}));
  const check=await screen.findByRole('button',{name:'Read published item metadata'});
  expect(publicDatasetRequest.mock.calls.find(([operation])=>operation==='resolve')[1]).toEqual(worldcerealRequest);
  expect(screen.getByText('Actual product period: 2021-04-03 – 2021-11-09')).toBeTruthy();
  expect(stacRequest).not.toHaveBeenCalled();
  await user.click(check);
  const asset=await screen.findByRole('checkbox',{name:`Select asset · ${worldcerealSnapshot.itemId} · classification`});
  await user.click(screen.getByRole('button',{name:'Asset source details'}));
  expect(screen.getByText('Original publisher URL')).toBeTruthy();
  expect(screen.getByText(worldcerealSnapshot.assets[0].originalHref)).toBeTruthy();
  expect(screen.getByText('Public download URL')).toBeTruthy();
  expect(screen.queryByRole('button',{name:'Load more items'})).toBeNull();
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['snapshot']);
   await user.keyboard('{Escape}');
   await user.click(asset);await user.click(screen.getByRole('button',{name:'Save asset selection'}));
  await screen.findByRole('button',{name:'Download selected originals'});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='project')[1].selections).toEqual([{snapshotId:worldcerealSnapshot.id,assetKey:'classification'}]);
  await user.click(screen.getByRole('combobox',{name:'Product layer'}));
  await user.click(screen.getByRole('option',{name:'Maize crop type · main season',exact:true}));
  expect(screen.queryByRole('button',{name:'Download selected originals'})).toBeNull();
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();
  expect(requestChanged.mock.calls.at(-1)[0].season).toBe('tc-maize-main');
  expect(area.value).toBe(worldcerealRequest.bounds.join(', '));
});

it.each(worldcerealDataset.layers)('selects $id with its only published season while keeping automatic regional matching and the original AOI', async option => {
  const user=userEvent.setup(), requestChanged=vi.fn();
  publicDatasetRequest.mockResolvedValue([worldcerealDataset]);
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId="worldcereal" areaBounds={worldcerealRequest.bounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  await screen.findByRole('combobox',{name:'Product layer'});
  await user.click(screen.getByRole('combobox',{name:'Product layer'}));await user.click(screen.getByRole('option',{name:option.title,exact:true}));
  expect(requestChanged.mock.calls.at(-1)[0].season).toBe(option.seasons[0]);
  expect(requestChanged.mock.calls.at(-1)[0].region).toBeUndefined();
  expect(screen.getByRole('combobox',{name:'Published season'}).textContent).toBe(worldcerealDataset.seasons.find(season=>season.id===option.seasons[0]).title);
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe(worldcerealRequest.bounds.join(', '));
});

it('keeps the actual confidence legend conflict visible beside asset selection and does not inherit the classification grid', async () => {
  const c=worldcerealConfidenceCase,user=userEvent.setup();
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?[worldcerealDataset]:operation==='resolve'?c.plan:c.connection);
  stacRequest.mockResolvedValue(c.snapshot);
  mount({datasetId:'worldcereal',areaBounds:c.request.bounds});
  await screen.findByRole('button',{name:'Review published items'});
  await user.click(screen.getByRole('combobox',{name:'Published region'}));await user.click(screen.getByRole('option',{name:'Mauritius / Réunion · AEZ 9026',exact:true}));
  await user.click(screen.getByRole('button',{name:'Review published items'}));await user.click(await screen.findByRole('button',{name:'Read published item metadata'}));
  await screen.findByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · confidence`});
   await user.click(within(screen.getByRole('checkbox',{name:`Select asset · ${c.snapshot.itemId} · confidence`}).closest('article')).getByRole('button',{name:'Asset source details'}));
   expect(within(screen.getByRole('dialog',{name:'Asset source details'})).getByText(worldcerealDataset.warnings.find(warning=>warning.startsWith('The reviewed annual confidence original')))).toBeTruthy();
  const assets=c.snapshot.assets;
  expect(assets.find(asset=>asset.key==='confidence').metadata['proj:shape']).not.toEqual(assets.find(asset=>asset.key==='classification').metadata['proj:shape']);
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['snapshot']);
});

it('inline product exploration keeps the selected product and current AOI and discards an old late source check after an area change', async () => {
  let settle; const signals=[];
  publicDatasetRequest.mockImplementation(async (operation, params, signal) => {
    if (operation === 'connect') { signals.push(signal); return new Promise(resolve => {settle=resolve;}); }
    return operation === 'list' ? [publicDataset] : datasetPlan;
  });
  const user=userEvent.setup(),requestChanged=vi.fn();
  const content=bounds=><RuntimeContext.Provider value={{projects:[datasetProject],refresh}}><PublicDatasetExplorer datasetId="worldcover" areaBounds={bounds} currentProject={{...datasetProject,bounds:[0,0,1,1]}} onRequestChange={requestChanged}/></RuntimeContext.Provider>;
  const view=render(content([10,20,11,21]));
  const bounds=await screen.findByRole('textbox',{name:'Dataset search bounds'});
  expect(bounds.value).toBe('10, 20, 11, 21');
  expect(screen.queryByRole('combobox',{name:'Data product'})).toBeNull();expect(screen.queryByRole('dialog')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Find published files'}));
  await user.click(await screen.findByRole('button',{name:'Check source files'}));
  view.rerender(content([12,22,13,23]));
  await waitFor(()=>expect(signals[0].aborted).toBe(true));
  await act(async()=>settle(datasetConnected));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe('12, 22, 13, 23');
  expect(stacRequest).not.toHaveBeenCalled();
  expect(requestChanged.mock.calls.at(-1)[0].bounds).toEqual([12,22,13,23]);
  expect(screen.getByRole('button',{name:'Find published files'}).disabled).toBe(false);
});

it('reviews a named product and its metadata, then requires separate file check, selection save and download actions', async () => {
  const user = userEvent.setup(); mount();
  await screen.findByText('Land-cover classes, not optical reflectance.');
  expect(screen.getByRole('link', { name: 'CC-BY-4.0' }).getAttribute('href')).toContain('creativecommons.org');
  expect(publicDatasetRequest.mock.calls.map(([operation]) => operation)).toEqual(['list']);
  expect(stacRequest).not.toHaveBeenCalled(); expect(screen.queryByRole('textbox', { name: 'Source URL' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Find published files' }));
  const check = await screen.findByRole('button', { name: 'Check source files' });
  expect(publicDatasetRequest.mock.calls[1][1]).toEqual(datasetPayload); expect(stacRequest).not.toHaveBeenCalled();
  const conditions = screen.getByRole('button', {name:'File scope and usage conditions'});
  expect(conditions.getAttribute('aria-expanded')).toBe('false');
  expect(screen.queryByText(datasetPlan.warnings[0])).toBeNull();
  await user.click(conditions);
  expect(conditions.getAttribute('aria-expanded')).toBe('true');
  expect(screen.getByText(datasetPlan.warnings[0])).toBeTruthy();
  await user.keyboard('{Escape}');
  await user.click(check); const asset = await screen.findByRole('checkbox', { name: 'Select asset · model-42 · raster' });
  expect(asset.getAttribute('aria-checked')).toBe('false');
  expect(screen.queryByRole('checkbox', { name: 'Select asset · model-42 · readme' })).toBeNull();
  expect(stacRequest.mock.calls.map(([operation]) => operation)).toEqual(['snapshot']);
  await user.click(asset); await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  const download = await screen.findByRole('button', { name: 'Download selected originals' });
  expect(stacRequest.mock.calls.find(([operation]) => operation === 'project')[1]).toEqual({ name: 'ESA WorldCover', bounds: datasetPayload.bounds, selections: [{ snapshotId, assetKey: 'raster' }] });
  expect(stacRequest.mock.calls.some(([operation]) => operation === 'downloads')).toBe(false);
  await user.click(download); await screen.findByRole('button', { name: 'Downloads queued' });
  expect(stacRequest.mock.calls.find(([operation]) => operation === 'downloads')[1]).toEqual({ projectId: datasetProject.id, selections: [{ snapshotId, assetKey: 'raster' }] });
  expect(screen.getByRole('link', { name: 'Open project' }).getAttribute('href')).toBe(`#My%20Data?project=${datasetProject.id}`);
});

it('changing the region removes old selections, source checks and saved download actions', async () => {
  const user = userEvent.setup(); mount(); await resolveAndConnect(user);
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · raster' }));
  await user.click(screen.getByRole('button', { name: 'Save asset selection' })); await screen.findByRole('button', { name: 'Download selected originals' });
  await user.clear(screen.getByRole('textbox', { name: 'Dataset search bounds' }));
  expect(screen.queryByRole('button', { name: 'Download selected originals' })).toBeNull();
  expect(screen.queryByRole('checkbox', { name: 'Select asset · model-42 · raster' })).toBeNull();
  expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(true);
  expect(stacRequest.mock.calls.some(([operation]) => operation === 'downloads')).toBe(false);
});

it('whole-country products require a country code and disclose that the bbox does not crop the country raster', async () => {
  const countryDataset = { ...publicDataset, id: 'worldpop', title: 'WorldPop historical country grids', requiresCountry: true, years: [2019,2020], defaultYear: 2020 };
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [countryDataset] : { ...datasetPlan, dataset: countryDataset });
  const user = userEvent.setup(); mount({ datasetId: 'worldpop' });
  const country = await screen.findByRole('textbox', { name: 'Dataset country code' });
  expect(screen.getByText('The country code selects the complete published country raster. The area organizes the project; it does not crop the downloaded file.')).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(true);
  await user.type(country, 'niu'); expect(country.value).toBe('NIU');
  await user.click(screen.getByRole('button', { name: 'Find published files' }));
  expect(publicDatasetRequest.mock.calls[1][1]).toEqual({ ...datasetPayload, datasetId: 'worldpop', year: 2020, country: 'NIU' });
});

it('monthly products publish the selected month to Agent context and invalidate January pins before reviewing February', async () => {
  const monthly = { ...publicDataset, id: 'chirps', title: 'CHIRPS v3 monthly precipitation', years: [2024], defaultYear: 2024,
    months: Array.from({length:12}, (_, index) => index + 1), defaultMonth: 1,
    layers:[{...publicDataset.layers[0],valueInterpretation:'Monthly values are millimetres; -9999 is missing, even without an embedded TIFF NoData tag.'}] };
  const plans = month => ({ ...datasetPlan, dataset: monthly, request: { ...datasetPayload, datasetId: 'chirps', year: 2024, month } });
  publicDatasetRequest.mockImplementation(async (operation, payload) => operation === 'list' ? [monthly]
    : operation === 'resolve' ? plans(payload.month) : { ...datasetConnected, plan: plans(payload.month) });
  const requestChanged = vi.fn(), user = userEvent.setup();
  render(<RuntimeContext.Provider value={{ projects: [datasetProject], refresh }}><PublicDatasetExplorer datasetId="chirps" areaBounds={datasetPayload.bounds} onRequestChange={requestChanged}/></RuntimeContext.Provider>);
  const month = await screen.findByRole('combobox', { name: 'Reference month' });
  expect(month.textContent).toBe('1');
  expect(screen.getByRole('combobox', { name: 'Reference year' }).textContent).toBe('2024');
  expect(screen.getByText('The selected month downloads one complete global file covering 60°S to 60°N. The area organizes the project; it does not clip, subset or resample the original.')).toBeTruthy();
  expect(screen.queryByText('The area selects intersecting published tiles. Downloads retain each complete file; no clipping or resampling is applied.')).toBeNull();
  expect(requestChanged.mock.calls.at(-1)[0]).toMatchObject({datasetId:'chirps',year:2024,month:1});
  await resolveAndConnect(user);
  expect(screen.getByRole('button', {name:'File scope and usage conditions'}).getAttribute('aria-expanded')).toBe('false');
  await user.click(screen.getByRole('button', {name:'Product details'}));
  expect(screen.getByText(monthly.layers[0].valueInterpretation)).toBeTruthy();
  expect(screen.getByText('The selected month downloads one complete global file covering 60°S to 60°N. The area organizes the project; it does not clip, subset or resample the original.')).toBeTruthy();
  await user.keyboard('{Escape}');
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · raster' }));
  await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  await screen.findByRole('button', { name: 'Download selected originals' });
  await user.click(month); await user.click(screen.getByRole('option', { name: '2', exact: true }));
  expect(screen.queryByRole('button', { name: 'Download selected originals' })).toBeNull();
  expect(screen.queryByRole('checkbox', { name: /Select asset/ })).toBeNull();
  expect(requestChanged.mock.calls.at(-1)[0]).toMatchObject({datasetId:'chirps',year:2024,month:2});
  expect(stacRequest.mock.calls.some(([operation]) => operation === 'downloads')).toBe(false);
  await resolveAndConnect(user);
  const recipes = publicDatasetRequest.mock.calls.filter(([operation]) => operation !== 'list');
  expect(recipes.map(([operation,payload]) => [operation,payload.month])).toEqual([['resolve',1],['connect',1],['resolve',2],['connect',2]]);
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · raster' }));
  await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  await user.click(await screen.findByRole('button', { name: 'Download selected originals' }));
  await screen.findByRole('button', { name: 'Downloads queued' });
  expect(stacRequest.mock.calls.filter(([operation]) => operation === 'downloads')).toHaveLength(1);
});

it('switching from a monthly product to an older annual definition drops the month from the request', async () => {
  const monthly = { ...publicDataset, id: 'chirps', title: 'CHIRPS v3 monthly precipitation', years: [2024], defaultYear: 2024, months: [1,2], defaultMonth: 1 };
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [monthly,publicDataset] : datasetPlan);
  const user = userEvent.setup(); mount({datasetId:'chirps'});
  await screen.findByRole('combobox', {name:'Reference month'});
  await user.click(screen.getByRole('combobox', {name:'Data product'}));
  await user.click(screen.getByRole('option', {name:'ESA WorldCover'}));
  expect(screen.queryByRole('combobox', {name:'Reference month'})).toBeNull();
  await user.click(screen.getByRole('button', {name:'Find published files'}));
  expect(publicDatasetRequest.mock.calls.at(-1)[1]).toEqual(datasetPayload);
});

it('catalog products use observation-month labels and explicit same-query pagination, preserve original pins, then separately save and download', async () => {
  const first = catalogSnapshot;
  const laterQuery = {...first.provenance.search,cursor:null};
  const duplicate = {...first,id:'f'.repeat(64),provenance:{...first.provenance,search:laterQuery}};
  const second = {...first,id:'e'.repeat(64),itemId:'CBERS-test-observation-2',title:'CBERS MUX scene 2',provenance:{...first.provenance,search:laterQuery}};
  const canonicalSelections = [{snapshotId:first.id,assetKey:'BAND5'}];
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [catalogDataset] : operation === 'resolve' ? catalogPlan : catalogConnected);
  stacRequest.mockImplementation(async operation => operation === 'search' ? {items:[duplicate,second],nextCursor:null,complete:true,limitReached:false}
    : operation === 'project' ? {...datasetProject,canonicalSelections} : operation === 'downloads' ? {jobs:[datasetJob]} : Promise.reject(new Error('Catalog snapshots must come from this search page.')));
  const user=userEvent.setup(),contextChanged=vi.fn();
  mount({datasetId:'cbers-sr',onRequestChange:contextChanged});
  expect((await screen.findByRole('combobox',{name:'Observation month'})).textContent).toBe('9');
  expect(screen.getByRole('combobox',{name:'Observation year'}).textContent).toBe('2026');
  expect(screen.queryByRole('combobox',{name:'Reference month'})).toBeNull();
  await user.click(screen.getByRole('button',{name:'Review catalog search'}));
  expect(screen.getByText('Catalog sources · 1')).toBeTruthy();
  await user.click(await screen.findByRole('button',{name:'Search product catalog'}));
  const firstCheckbox=await screen.findByRole('checkbox',{name:'Select asset · CBERS-test-observation-1 · BAND5'});
  expect(screen.getByText('CBERS-test-observation-1 · 2026-09-02')).toBeTruthy();
  expect(screen.getByText('1 items loaded · more available')).toBeTruthy();
  expect(stacRequest).not.toHaveBeenCalled();
  expect(contextChanged.mock.calls.at(-1)[0]).toEqual(catalogPayload);
  await user.click(firstCheckbox);
  await user.click(screen.getByRole('button',{name:'Load more items'}));
  await screen.findByRole('checkbox',{name:'Select asset · CBERS-test-observation-2 · BAND5'});
  expect(stacRequest.mock.calls[0][0]).toBe('search');
  expect(stacRequest.mock.calls[0][1]).toEqual({connectionId:catalogConnected.stacConnections[0].id,...catalogQuery,cursor:'test-next-page'});
  expect(screen.getAllByRole('checkbox',{name:/Select asset/})).toHaveLength(2);
  expect(screen.getByRole('checkbox',{name:'Select asset · CBERS-test-observation-1 · BAND5'}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByText('2 items loaded · search complete')).toBeTruthy();
  expect(screen.queryByRole('button',{name:'Load more items'})).toBeNull();
  expect(screen.getByText('Catalog completion only describes this metadata search. Item footprints and selected band files do not prove complete, valid-pixel or cloud-free area coverage.')).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Save asset selection'}));
  await screen.findByRole('button',{name:'Download selected originals'});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='project')[1].selections).toEqual(canonicalSelections);
  expect(stacRequest.mock.calls.some(([operation])=>operation==='downloads')).toBe(false);
  await user.click(screen.getByRole('button',{name:'Download selected originals'}));
  await screen.findByRole('button',{name:'Downloads queued'});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='downloads')[1].selections).toEqual(canonicalSelections);
});

it('a catalog continuation from another month is rejected while current items and retry remain usable', async () => {
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [catalogDataset] : operation === 'resolve' ? catalogPlan : catalogConnected);
  stacRequest.mockResolvedValue({items:[{...catalogSnapshot,id:'e'.repeat(64),itemId:'wrong-month-item',provenance:{...catalogSnapshot.provenance,search:{...catalogSnapshot.provenance.search,datetime:'2026-08-01T00:00:00Z/2026-08-31T23:59:59.999999Z',cursor:null}}}],nextCursor:null,complete:true,limitReached:false});
  const user=userEvent.setup();mount({datasetId:'cbers-sr'});
  await waitFor(()=>expect(screen.getByRole('button',{name:'Review catalog search'}).disabled).toBe(false));
  await user.click(screen.getByRole('button',{name:'Review catalog search'}));await user.click(await screen.findByRole('button',{name:'Search product catalog'}));
  await user.click(await screen.findByRole('button',{name:'Load more items'}));
  expect((await screen.findByRole('alert')).textContent).toBe('Raster search returned an item from a different query.');
  expect(screen.getAllByRole('checkbox',{name:/Select asset/})).toHaveLength(1);
  expect(screen.queryByRole('checkbox',{name:/wrong-month-item/})).toBeNull();
  expect(screen.getByRole('button',{name:'Load more items'}).disabled).toBe(false);
  await user.click(screen.getByRole('combobox',{name:'Observation month'}));await user.click(screen.getByRole('option',{name:'10',exact:true}));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();expect(screen.queryByRole('button',{name:'Load more items'})).toBeNull();
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['search']);
});

it('WPM keeps the oversize PAN visible but batch selection and saved pins contain only the four reviewed multispectral originals', async () => {
  // Transport doubles exercise the user workflow, not publisher/download acceptance.
  const dataset={...catalogDataset,id:'cbers-wpm',title:'CBERS WPM test transport',layers:[{id:'wpm-ms',title:'8 m multispectral DN',unit:'DN',valueInterpretation:'Original Int16 values'}],defaultLayer:'wpm-ms'};
  const request={...catalogPayload,datasetId:dataset.id,layer:'wpm-ms'};
  const query={...catalogQuery,collectionId:'CB4A-WPM-L4-DN-1'};
  const plan={...catalogPlan,dataset,request,sources:[{...catalogPlan.sources[0],id:query.collectionId,search:query}]};
  const sizeReason='The declared original exceeds the 512 MiB download and preview limit.';
  const template=catalogSnapshot.assets[0];
  const item={...catalogSnapshot,collectionId:query.collectionId,provenance:{...catalogSnapshot.provenance,search:{...catalogSnapshot.provenance.search,...query}},assets:[
    {...template,key:'BAND0',eligible:false,reason:sizeReason,metadata:{'bdc:size':2639842927}},
    ...[1,2,3,4].map(band=>({...template,key:`BAND${band}`,title:`Original band ${band}`,eligible:true,metadata:{'bdc:size':174662589,'raster:bands':[{data_type:'int16',nodata:0,scale:1}]}})),
  ]};
  const connection={...catalogConnected,plan,stacConnections:[{...catalogConnected.stacConnections[0],collections:[{...catalogConnected.stacConnections[0].collections[0],id:query.collectionId}]}],searches:[{...catalogConnected.searches[0],sourceId:query.collectionId,page:{items:[item],nextCursor:null,complete:true,limitReached:false}}]};
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?[dataset]:operation==='resolve'?plan:connection);
  const pins=[1,2,3,4].map(band=>({snapshotId:item.id,assetKey:`BAND${band}`}));
  stacRequest.mockImplementation(async operation=>operation==='project'?{...datasetProject,canonicalSelections:pins}:{jobs:[datasetJob]});
  const user=userEvent.setup(),openProject=vi.fn();
  render(<RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId={dataset.id} areaBounds={request.bounds} autoSearch onOpenProject={openProject}/></RuntimeContext.Provider>);
  const pan=await screen.findByRole('checkbox',{name:`Select asset · ${item.itemId} · BAND0`});
  expect(screen.getByText('CBERS WPM preview uses the downloaded original band. Save and download first, then load its local preview.')).not.toBeNull();
  expect(pan.getAttribute('data-disabled')).not.toBeNull();
  await user.click(within(pan.closest('article')).getByRole('button',{name:'Asset source details'}));
  expect(screen.getByRole('dialog',{name:'Asset source details'}).textContent).toContain(sizeReason);
  await user.keyboard('{Escape}');
  await user.click(screen.getByRole('button',{name:'Select reviewed files'}));
  expect(pan.getAttribute('aria-checked')).toBe('false');
  for(const band of [1,2,3,4]) expect(screen.getByRole('checkbox',{name:`Select asset · ${item.itemId} · BAND${band}`}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('button',{name:'Load selected files to map'}).disabled).toBe(true);
  expect(screen.queryByRole('region',{name:'Product preview layers'})).toBeNull();
  expect(screen.getByText('Download originals before previewing.')).not.toBeNull();
  await user.click(within(screen.getByRole('region',{name:'Selected product actions'})).getByRole('button',{name:'Save and download'}));
  expect(stacRequest.mock.calls.some(([operation])=>['project','downloads'].includes(operation))).toBe(false);
  await user.click(screen.getByRole('button',{name:'Confirm and download'}));
  await screen.findByRole('link',{name:'View download tasks'});
  expect(stacRequest.mock.calls.find(([operation])=>operation==='project')[1].selections).toEqual(pins);
  expect(stacRequest.mock.calls.find(([operation])=>operation==='downloads')[1].selections).toEqual(pins);
  expect(openProject).not.toHaveBeenCalled();
  await user.click(screen.getByRole('link',{name:'Open project',exact:true}));
  expect(openProject).toHaveBeenCalledExactlyOnceWith(datasetProject.id);
});

it('annual catalog products retain reference-year meaning and an empty search never invents files or downloads', async () => {
  const annual={...catalogDataset,id:'io-lulc',title:'Annual land-cover classes',years:[2024],defaultYear:2024,months:undefined,defaultMonth:undefined};
  const request={...catalogPayload,datasetId:'io-lulc',year:2024,month:undefined};
  const plan={...catalogPlan,dataset:annual,request,sources:catalogPlan.sources.map(source=>({...source,search:{...source.search,datetime:'2024-01-01T00:00:00Z/2024-12-31T23:59:59.999999Z'}}))};
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?[annual]:operation==='resolve'?plan:{...catalogConnected,plan,stacConnections:[{...catalogConnected.stacConnections[0],snapshotIds:[]}],searches:[{...catalogConnected.searches[0],page:{items:[],nextCursor:null,complete:true,limitReached:false}}]});
  const user=userEvent.setup();mount({datasetId:'io-lulc'});
  expect((await screen.findByRole('combobox',{name:'Reference year'})).textContent).toBe('2024');
  expect(screen.queryByRole('combobox',{name:'Observation year'})).toBeNull();expect(screen.queryByRole('combobox',{name:'Observation month'})).toBeNull();
  await user.click(screen.getByRole('button',{name:'Review catalog search'}));await user.click(await screen.findByRole('button',{name:'Search product catalog'}));
  await screen.findByText('No product items match this search.');
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();expect(screen.queryByRole('button',{name:'Save asset selection'})).toBeNull();
  expect(screen.queryByRole('button',{name:'Download selected originals'})).toBeNull();expect(stacRequest).not.toHaveBeenCalled();
});

it('a next page cannot use a matching saved query to admit an observation outside the actual requested month', async () => {
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [catalogDataset] : operation === 'resolve' ? catalogPlan : catalogConnected);
  const foreign = {...catalogSnapshot,id:'e'.repeat(64),itemId:'outside-month-item',datetime:'2026-08-15T12:00:00Z'};
  stacRequest.mockResolvedValue({items:[foreign],nextCursor:null,complete:true,limitReached:false});
  const user=userEvent.setup();mount({datasetId:'cbers-sr'});
  await waitFor(()=>expect(screen.getByRole('button',{name:'Review catalog search'}).disabled).toBe(false));
  await user.click(screen.getByRole('button',{name:'Review catalog search'}));await user.click(await screen.findByRole('button',{name:'Search product catalog'}));
  await user.click(await screen.findByRole('checkbox',{name:'Select asset · CBERS-test-observation-1 · BAND5'}));
  await user.click(screen.getByRole('button',{name:'Load more items'}));
  expect((await screen.findByRole('alert')).textContent).toBe('Raster search returned an item outside the requested area or time.');
  expect(screen.queryByRole('checkbox',{name:/outside-month-item/})).toBeNull();
  expect(screen.getByRole('checkbox',{name:'Select asset · CBERS-test-observation-1 · BAND5'}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('button',{name:'Load more items'}).disabled).toBe(false);
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['search']);
});

it('changing the exploration area aborts a pending catalog page and drops its late items and old source selections', async () => {
  let settle; const signals=[];
  publicDatasetRequest.mockImplementation(async operation=>operation==='list'?[catalogDataset]:operation==='resolve'?catalogPlan:catalogConnected);
  stacRequest.mockImplementation(async (operation,query,signal)=>{signals.push(signal);return new Promise(resolve=>{settle=resolve;});});
  const content=bounds=><RuntimeContext.Provider value={{projects:[],refresh}}><PublicDatasetExplorer datasetId="cbers-sr" areaBounds={bounds}/></RuntimeContext.Provider>;
  const view=render(content(catalogPayload.bounds)),user=userEvent.setup();
  await waitFor(()=>expect(screen.getByRole('button',{name:'Review catalog search'}).disabled).toBe(false));
  await user.click(screen.getByRole('button',{name:'Review catalog search'}));await user.click(await screen.findByRole('button',{name:'Search product catalog'}));
  await user.click(await screen.findByRole('button',{name:'Load more items'}));
  view.rerender(content([12,22,13,23]));
  await waitFor(()=>expect(signals[0].aborted).toBe(true));
  await act(async()=>settle({items:[{...catalogSnapshot,id:'e'.repeat(64),itemId:'late-old-area'}],nextCursor:null,complete:true,limitReached:false}));
  expect(screen.queryByRole('checkbox',{name:/Select asset/})).toBeNull();expect(screen.queryByRole('button',{name:'Load more items'})).toBeNull();
  expect(screen.getByRole('textbox',{name:'Dataset search bounds'}).value).toBe('12, 22, 13, 23');
  expect(screen.getByRole('button',{name:'Review catalog search'}).disabled).toBe(false);
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['search']);
});

it('a source check failure stays recoverable without presenting files or a queued download', async () => {
  publicDatasetRequest.mockImplementation(async operation => { if (operation === 'connect') throw new Error('Published tile returned HTTP 404'); return operation === 'list' ? [publicDataset] : datasetPlan; });
  const user = userEvent.setup(); mount(); await waitFor(() => expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(false));
  await user.click(screen.getByRole('button', { name: 'Find published files' })); await user.click(await screen.findByRole('button', { name: 'Check source files' }));
  expect((await screen.findByRole('alert')).textContent).toContain('404');
  expect(screen.getByRole('button', { name: 'Check source files' }).disabled).toBe(false);
  expect(screen.queryByRole('button', { name: 'Download selected originals' })).toBeNull(); expect(stacRequest).not.toHaveBeenCalled();
});

it('keeps successfully checked tiles usable while explicitly exposing missing files and never claiming full coverage', async () => {
  const second = { ...datasetPlan.sources[0], id: 'missing-tile', title: 'Missing published tile', stac: { ...datasetPlan.sources[0].stac, url: 'https://example.com/missing.tif' } };
  const partial = { ...datasetConnected, plan: { ...datasetPlan, sources: [...datasetPlan.sources, second] }, failures: [{ sourceId: second.id, error: 'Provider returned HTTP 404' }], complete: false };
  publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [publicDataset] : operation === 'resolve' ? partial.plan : partial);
  const user = userEvent.setup(); mount(); await resolveAndConnect(user);
  expect(screen.getByText('Some source files could not be checked')).toBeTruthy(); expect(screen.getByText('Provider returned HTTP 404')).toBeTruthy();
  expect(screen.getByText('The available files can be selected below. Failed files are excluded; this is not a complete coverage claim.')).toBeTruthy();
  expect(screen.getAllByRole('checkbox', { name: /Select asset/ })).toHaveLength(1);
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · raster' }));
  expect(screen.getByRole('button', { name: 'Save asset selection' }).disabled).toBe(false);
  expect(stacRequest.mock.calls.some(([operation]) => operation === 'downloads')).toBe(false);
});

it('allows closing while waiting for metadata and does not continue to file pins after the request is aborted', async () => {
  let release; publicDatasetRequest.mockImplementation(async operation => operation === 'list' ? [publicDataset] : operation === 'resolve' ? datasetPlan : new Promise(resolve => { release = resolve; }));
  const close = vi.fn(), user = userEvent.setup(); const { unmount } = mount({ onClose: close });
  await waitFor(() => expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(false));
  await user.click(screen.getByRole('button', { name: 'Find published files' })); await user.click(await screen.findByRole('button', { name: 'Check source files' }));
  expect(screen.getByRole('button', { name: 'Close' }).disabled).toBe(false); await user.click(screen.getByRole('button', { name: 'Close' })); await waitFor(() => expect(close).toHaveBeenCalledOnce());
  unmount(); await act(async () => release(datasetConnected));
  // Real request validation rejects aborted responses before returning; this mock
  // deliberately ignores cancellation, so component must guard the continuation.
  expect(stacRequest).not.toHaveBeenCalled();
});

it('does not invent a sample geography or substitute an unavailable named product', async () => {
  mount({ areaBounds: undefined }); await screen.findByRole('button', { name: 'Find published files' });
  expect(screen.getByRole('textbox', { name: 'Dataset search bounds' }).value).toBe('');
  expect(screen.getByRole('button', { name: 'Find published files' }).disabled).toBe(true);
});

it('an unavailable named connector does not silently choose another product from the running catalog', async () => {
  mount({ datasetId: 'soilgrids' });
  expect((await screen.findByRole('alert')).textContent).toBe('This product connector is not available in the running app.');
  expect(screen.getByRole('combobox', { name: 'Data product' }).textContent).toBe('Choose a data product');
  expect(screen.queryByRole('button', { name: 'Find published files' })).toBeNull(); expect(stacRequest).not.toHaveBeenCalled();
});

it('source directory accepts the ten original backlog products once while broader portals stay pending', () => {
  const ids = SOURCE_DIRECTORY.map(source => source.id); expect(new Set(ids).size).toBe(ids.length);
  expect(SOURCE_DIRECTORY.filter(source => source.status === 'ready' && source.action?.kind === 'dataset').map(source => source.id).sort()).toEqual(['worldcover', 'worldcover-composites', 'worldpop', 'jrc-water', 'glad-forest', 'etopo', 'chirps', 'nasadem', 'alos-aw3d30', 'srtm-gl1', 'hydrosheds', 'cbers-sr', 'io-lulc', 'hilda-land-use', 'worldcereal', 'deafrica-fractional-cover', 'deafrica-wofs-annual', 'wsf-settlements', 'gmw-mangroves', 'cgls-landcover', 'usgs-3dep', 'chelsa-climate', 'jaxa-amsr2-soil-moisture', 'jaxa-modis-ndvi', 'jaxa-sgli-lst', 'jaxa-palsar-fnf', 'jaxa-sgli-chla', 'jaxa-gsmap-rainfall', 'jaxa-hrlulc-japan', 'jaxa-modis-swr', 'jaxa-sgli-sst', 'jaxa-sgli-et', 'jaxa-sgli-et-index', 'jaxa-sgli-aerosol', 'jaxa-modis-aerosol', 'jaxa-amsr2-wind', 'jaxa-amsr2-sst', 'jaxa-gsmap-spi', 'srtm-gl3', 'gedi-l3-canopy', 'srtm15plus-bathymetry', 'openlandmap-nightlights', 'amazonia-wfi-dn', 'cbers-4a', 'usgs-3dep-dem', 'ghsl', 'glad-landcover-2020', 'maxar-open', 'ghsl-built-surface', 'ghsl-built-volume', 'ghsl-population', 'eth-canopy-height', 'hydrosheds-accumulation', 'etopo-geoid', 'pc-hrea', 'pc-biodiversity', 'noaa-ohc', 'noaa-nclimgrid','worldclim','fabdem','glc-fcs10','glc-fcs30d','soilgrids','vantor-open','sinolc-1','clcd','enmap'].sort());
  for(const id of ['worldclim','fabdem','glc-fcs10','glc-fcs30d','soilgrids','vantor-open']) expect(SOURCE_DIRECTORY.find(source=>source.id===id)).toMatchObject({status:'ready',action:{kind:'dataset',datasetId:id}});
  expect(SOURCE_DIRECTORY.find(source=>source.id==='earth-search-more')).toMatchObject({status:'planned',partialAction:true,action:{kind:'dataset',datasetId:'earth-search-more'}});
  expect(SOURCE_DIRECTORY.find(source=>source.id==='overture-maps')).toMatchObject({status:'ready',action:{kind:'overture'}});
  const regional=SOURCE_DIRECTORY.find(source=>source.id==='worldcereal');
  expect(regional.name).toContain('Mauritius / Réunion');expect(regional.detail).toContain('AEZ 9026');expect(regional.detail).toContain('seven');
  for (const id of ['deafrica-fractional-cover','deafrica-wofs-annual','wsf-settlements','gmw-mangroves','cgls-landcover']) {
    const source=SOURCE_DIRECTORY.find(source=>source.id===id);expect(source.status).toBe('ready');expect(source.action).toEqual({kind:'dataset',datasetId:id});
  }
  for (const [id,datasetId] of [['ghsl','ghsl-smod'],['maxar-open','maxar-open-legacy'],['glad-landcover-2020','glad-landcover-2020']]) {
    const source=SOURCE_DIRECTORY.find(source=>source.id===id);expect(source.status).toBe('ready');expect(source.action).toEqual({kind:'dataset',datasetId});
  }
  const gl3=SOURCE_DIRECTORY.filter(source=>source.id==='srtm-gl3');
  expect(gl3).toHaveLength(1);expect(gl3[0].status).toBe('ready');
  expect(gl3[0].action).toEqual({kind:'dataset',datasetId:'srtm-gl3'});
  expect(gl3[0].name).toBe('SRTM GL3 · 3 arc-second elevation');
  expect(gl3[0].notes).toContain('point');expect(gl3[0].notes).toContain('Not Provided license');
  const gscloud=SOURCE_DIRECTORY.find(item=>item.id==='gscloud');expect(gscloud.status).toBe('auth');expect(gscloud.action).toEqual({kind:'gscloud'});
  for (const id of ['dynamic-world', 'eog-nightlights']) { const source = SOURCE_DIRECTORY.find(item => item.id === id); expect(source.status).toBe('planned'); expect(source.action).toBeUndefined(); }
});

it('public product cards group distinct scientific families without implying classification, population or carbon delivery', () => {
  const source=id=>SOURCE_DIRECTORY.find(source=>source.id===id);
  expect(source('deafrica-fractional-cover').group).toBe('vegetation');
  expect(source('deafrica-wofs-annual').group).toBe('hydrology');
  expect(source('wsf-settlements').group).toBe('population');expect(source('wsf-settlements').detail).toContain('settlement mask');
  expect(source('gmw-mangroves').group).toBe('vegetation');expect(source('gmw-mangroves').detail).toContain('coastal tiles');
  expect(source('cgls-landcover').group).toBe('land-cover');expect(source('cgls-landcover').detail).toContain('permanent-water fraction only');
});

it('directory separates public COG elevation archives and limited DIR tiles from protected original delivery', () => {
  for (const id of ['nasadem','alos-aw3d30','srtm-gl1']) {
    const source = SOURCE_DIRECTORY.find(item => item.id === id);
    expect(source.group).toBe('elevation'); expect(source.platform).toBe('OpenTopography'); expect(source.name).toContain('Public COG archive');
  }
  expect(SOURCE_DIRECTORY.find(source => source.id === 'alos-aw3d30').detail).toContain('2021 V3.2');
  const direction = SOURCE_DIRECTORY.find(source => source.id === 'hydrosheds');
  expect(direction.group).toBe('hydrology'); expect(direction.detail).toBe('v2r0 DIR · 1 arc-second · published Americas tiles only');
  expect(direction.coverage).toContain('100 officially published Americas tiles');
  expect(direction.formats).toBe('Original numerical GeoTIFF / COG tiles');
  expect(SOURCE_DIRECTORY.find(source => source.id === 'chirps').name).toBe('CHIRPS v3 · Monthly precipitation');
  const protectedOriginal = SOURCE_DIRECTORY.find(source => source.id === 'nasa-srtm');
  expect(protectedOriginal.status).toBe('ready'); expect(protectedOriginal.action).toEqual({kind:'provider',providerId:'nasa-srtm'});
});
