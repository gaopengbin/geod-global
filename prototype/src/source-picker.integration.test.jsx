import React, { useRef, useState } from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Button, Modal } from './ui/index.jsx';
import { SourcePickerDialog } from './source-picker.jsx';
import { SOURCE_DIRECTORY, SOURCE_GROUPS } from './source-directory.js';
import sourceDirectoryChinese from './locales/source-directory.zh-CN.js';
import publicDatasetChinese from './locales/public-dataset.zh-CN.js';
import pickerChinese from './locales/source-picker.zh-CN.js';

const language=vi.hoisted(()=>({value:'en'}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({locale:language.value,t:(text,values={})=>{
  const translated=language.value==='zh-CN'?(pickerChinese[text]||publicDatasetChinese[text]||sourceDirectoryChinese[text]||({'Close':'关闭','Reset filters':'重置筛选','Catalog only':'仅目录检索','Downloadable':'可下载','Product connector':'产品已接入'}[text])||text):text;
  return translated.replace(/\{(\w+)\}/g,(_,key)=>values[key]??'');
}})}));
beforeEach(()=>{language.value='en';vi.clearAllMocks();const values=new Map();const storage={getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)};Object.defineProperty(window,'localStorage',{configurable:true,value:storage});vi.stubGlobal('localStorage',storage);});

it('offers the accepted whole Shanghai original in the default connected list',async()=>{
  language.value='zh-CN';
  const source=SOURCE_DIRECTORY.find(source=>source.id==='sinolc-1'),user=userEvent.setup(),entry=vi.fn();
  expect(source.status).toBe('ready');expect(source.partialAction).toBeUndefined();
  expect(source.action).toEqual({kind:'dataset',datasetId:'sinolc-1'});
  render(<Harness entry={entry}/>);await open(user);
  expect(screen.getByRole('radio',{name:'已接入 · 111'}).getAttribute('aria-checked')).toBe('true');
  await user.type(screen.getByRole('searchbox',{name:'搜索数据源'}),'SinoLC');
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId)).toEqual(['sinolc-1']);
  expect(within(card('sinolc-1')).queryByText('原件核验中',{exact:true})).toBeNull();
  await user.click(card('sinolc-1').querySelector('.source-picker-select'));
  await waitFor(()=>expect(entry).toHaveBeenCalledWith('sinolc-1',screen.getByRole('button',{name:'Data source trigger'})));
});

it('opens the official GEBCO regional workflow without claiming whole-grid completion',async()=>{
  language.value='zh-CN';
  const source=SOURCE_DIRECTORY.find(source=>source.id==='gebco'),user=userEvent.setup(),entry=vi.fn();
  expect(source.status).toBe('planned');expect(source.partialAction).toBe(true);expect(source.action).toEqual({kind:'gebco',datasetId:'gebco-tid'});
  render(<Harness entry={entry}/>);await open(user);
  await user.click(screen.getByRole('radio',{name:'全部 · 132'}));
  await user.type(screen.getByRole('searchbox',{name:'搜索数据源'}),'GEBCO');
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId)).toEqual(['gebco']);
  expect(within(card('gebco')).getByText('区域高程与全球 TID · 整幅高程待接入',{exact:true})).toBeTruthy();
  expect(card('gebco').querySelector('.source-picker-select').disabled).toBe(false);
  await user.click(card('gebco').querySelector('.source-picker-select'));
  await waitFor(()=>expect(entry).toHaveBeenCalledWith('gebco',screen.getByRole('button',{name:'Data source trigger'})));
  expect(source.status).toBe('planned');
});

it.each([
  {query:'微软',id:'ms-buildings',label:'微软 · 已核查建筑分区'},
  {query:'摩纳哥 免费 GIS',id:'geofabrik',label:'Geofabrik · 摩纳哥免费 GIS 文件包'},
  {query:'尼泊尔 30 角秒',id:'worldclim',label:'WorldClim 2.1 · 尼泊尔气候平均值'},
])('finds the one refined original card using its displayed Chinese title/detail: $query',async({query,id,label})=>{
  language.value='zh-CN';const user=userEvent.setup(),entry=vi.fn();render(<Harness entry={entry}/>);await open(user);
  await user.click(screen.getByRole('radio',{name:'已接入 · 111'}));
  await user.type(screen.getByRole('searchbox',{name:'搜索数据源'}),query);
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId)).toEqual([id]);
  expect(within(card(id)).getByText(label,{exact:true})).toBeTruthy();
  expect(card(id).querySelector('.source-picker-select').getAttribute('aria-label')).toContain(label);
  await user.click(card(id).querySelector('.source-picker-select'));await waitFor(()=>expect(entry).toHaveBeenCalledWith(id,screen.getByRole('button',{name:'Data source trigger'})));
});
it('uses native Chinese card fields for presentation and search while still supporting Chinese input in English',async()=>{
  const source=SOURCE_DIRECTORY.find(s=>s.id==='ms-buildings'),user=userEvent.setup();render(<Harness/>);await open(user);
  await user.click(screen.getByRole('radio',{name:'Connected · 111'}));
  await user.type(screen.getByRole('searchbox'),'微软');expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId)).toEqual(['ms-buildings']);
  expect(within(card('ms-buildings')).getByText(source.name,{exact:true})).toBeTruthy();
});

function Harness({ selectedSourceId='earth-search', disabled=false, choose=vi.fn(), entry=vi.fn(), closed=vi.fn(), nextDialog=false }) {
  const trigger=useRef(null),[open,setOpen]=useState(false),[next,setNext]=useState(false);
  return <><Button ref={trigger} onClick={()=>setOpen(true)}>Data source trigger</Button>
    {open&&<SourcePickerDialog selectedSourceId={selectedSourceId} disabled={disabled} returnFocusRef={trigger}
      onClose={()=>{closed(document.activeElement);setOpen(false);}}
      onChoose={id=>{choose(id,document.activeElement);if(nextDialog)setNext(true);}}
      onOpenEntry={id=>{entry(id,document.activeElement);if(nextDialog)setNext(true);}}/>}
    {next&&<Modal title="Selected service" onClose={()=>setNext(false)}><Button>Service action</Button></Modal>}
  </>;
}
async function open(user) { await user.click(screen.getByRole('button',{name:'Data source trigger'}));return screen.findByRole('dialog',{name:language.value==='en'?'Choose a data source':'选择数据源'}); }
const card=id=>document.querySelector(`[data-source-id="${id}"]`);
it('shows the current channel ahead of older preferences and changes nothing until a different channel is chosen',async()=>{
  localStorage.setItem('geod-global-product-preferences-v1',JSON.stringify({preferred:{'sentinel-2-l2a-cog':'planetary-computer'}}));
  const user=userEvent.setup(),choose=vi.fn();render(<Harness selectedSourceId="earth-search" choose={choose}/>);await open(user);
  expect(card('earth-search')).not.toBeNull();expect(card('planetary-computer')).toBeNull();
  await user.click(screen.getByRole('combobox',{name:'Data channel'}));await user.click(screen.getByRole('option',{name:'Planetary Computer',exact:true}));
  expect(choose).not.toHaveBeenCalled();
  await user.click(card('planetary-computer').querySelector('.source-picker-select'));
  await waitFor(()=>expect(choose).toHaveBeenCalledWith('planetary-computer',screen.getByRole('button',{name:'Data source trigger'})));
});

it('defaults to all connected entry types while catalog-only providers never claim downloadable originals', async()=>{
  const user=userEvent.setup();render(<Harness selectedSourceId="dataset-cgls-landcover"/>);await open(user);
  expect(document.activeElement).toBe(screen.getByRole('searchbox',{name:'Search data sources'}));
  expect(document.querySelectorAll('.source-picker-card')).toHaveLength(111);
  expect(screen.getByRole('radio',{name:'Connected · 111'}).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('navigation',{name:'Product categories'})).toBeTruthy();
  expect(within(screen.getByRole('navigation')).getAllByRole('button')).toHaveLength(SOURCE_GROUPS.length+3);
  for(const source of SOURCE_DIRECTORY.filter(source=>source.status==='auth')) {
    const element=card(source.id);expect(element).not.toBeNull();
    expect(within(element).getByText(source.verifiedOriginalProducts?.length?'Some originals available':'Catalog only',{exact:true})).toBeTruthy();
    expect(within(element).getByRole('button',{name:'Product and channel details'})).toBeTruthy();
    expect(within(element).queryByText('Downloadable',{exact:true})).toBeNull();
  }
  expect(card('cgls-landcover').getAttribute('data-selected')).toBe('true');
  expect(card('cgls-landcover').querySelector('.source-picker-select').getAttribute('aria-pressed')).toBe('true');
  expect(card('dynamic-world')).toBeNull();
});

it('searches Chinese product and mission aliases in an English interface, then platform and description across categories', async()=>{
  const user=userEvent.setup();render(<Harness/>);await open(user);const input=screen.getByRole('searchbox',{name:'Search data sources'});
  await user.type(input,'哨兵');expect(card('earth-search')).not.toBeNull();expect(card('planetary-radar')).not.toBeNull();expect(card('worldpop')).toBeNull();
  await user.clear(input);await user.type(input,'永久水体');expect(card('cgls-landcover')).not.toBeNull();expect(card('planetary-radar')).toBeNull();
  await user.clear(input);await user.type(input,'Planetary reflectance');expect(card('planetary-landsat')).not.toBeNull();expect(card('planetary-radar')).toBeNull();
  await user.clear(input);await user.type(input,'population');expect(card('worldpop')).not.toBeNull();
});

it('filters by product family, displays planned entries disabled, and resets an empty search without invoking anything', async()=>{
  const user=userEvent.setup(),choose=vi.fn(),entry=vi.fn();render(<Harness choose={choose} entry={entry}/>);await open(user);
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:/^Water and hydrology products/}));
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId).sort()).toEqual(SOURCE_DIRECTORY.filter(source=>source.group==='hydrology'&&source.status!=='planned').map(source=>source.id).sort());
  await user.click(screen.getByRole('radio',{name:'All · 132'}));
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:/^All categories/}));expect(document.querySelectorAll('.source-picker-card')).toHaveLength(132);
  const planned=card('dynamic-world').querySelector('.source-picker-select');expect(planned.disabled).toBe(true);await user.click(planned);
  expect(choose).not.toHaveBeenCalled();expect(entry).not.toHaveBeenCalled();
  const pending=SOURCE_DIRECTORY.filter(source=>source.status==='planned');
  await user.click(screen.getByRole('radio',{name:'Pending · '+pending.length}));expect(document.querySelectorAll('.source-picker-card')).toHaveLength(pending.length);
  for(const source of pending)expect(card(source.id).querySelector('.source-picker-select').disabled).toBe(!source.partialAction);
  await user.type(screen.getByRole('searchbox'), 'no-published-source-with-this-name');expect(screen.getByText('No sources match these filters.')).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Reset filters'}));expect(document.querySelectorAll('.source-picker-card')).toHaveLength(111);
  expect(screen.getByRole('searchbox').value).toBe('');
});

it.each([
  {id:'earth-search',name:'Choose Sentinel-2 L2A · COG',kind:'provider'},
  {id:'cgls-landcover',name:'Choose Copernicus land cover · Permanent-water fraction',kind:'entry'},
  {id:'usgs-3dep',name:'Choose USGS 3DEP · Current 1 arc-second elevation',kind:'entry'},
  {id:'chelsa-climate',name:'Choose CHELSA · Mean climate moisture index',kind:'entry'},
  {id:'jaxa-amsr2-soil-moisture',name:'Choose JAXA AMSR2 · Monthly soil moisture',kind:'entry'},
  {id:'jaxa-sgli-lst',name:'Choose JAXA SGLI · Monthly land-surface temperature',kind:'entry'},
  {id:'jaxa-modis-ndvi',name:'Choose JAXA MODIS · Monthly vegetation index',kind:'entry'},
  {id:'jaxa-palsar-fnf',name:'Choose JAXA PALSAR-2 · Forest / non-forest',kind:'entry'},
  {id:'jaxa-sgli-chla',name:'Choose JAXA SGLI · Monthly ocean chlorophyll',kind:'entry'},
  {id:'jaxa-gsmap-rainfall',name:'Choose JAXA GSMaP · Precipitation rate',kind:'entry'},
  {id:'jaxa-hrlulc-japan',name:'Choose JAXA HRLULC · Japan land cover overview',kind:'entry'},
  {id:'jaxa-modis-swr',name:'Choose JAXA MODIS · Shortwave radiation',kind:'entry'},
  {id:'jaxa-sgli-sst',name:'Choose JAXA SGLI · Sea-surface temperature',kind:'entry'},
  {id:'openlandmap-nightlights',name:'Choose OpenLandMap · Annual nighttime-light DN',kind:'entry'},
  {id:'amazonia-wfi-dn',name:'Choose AMAZÔNIA-1 WFI · L4 multispectral DN',kind:'entry'},
  {id:'jaxa-sgli-et',name:'Choose JAXA SGLI · Evapotranspiration',kind:'entry'},
  {id:'jaxa-sgli-et-index',name:'Choose JAXA SGLI · Evapotranspiration index',kind:'entry'},
  {id:'jaxa-sgli-aerosol',name:'Choose JAXA SGLI · Aerosol optical depth',kind:'entry'},
  {id:'jaxa-modis-aerosol',name:'Choose JAXA MODIS · Aerosol optical depth',kind:'entry'},
  {id:'jaxa-amsr2-wind',name:'Choose JAXA AMSR2 · Sea-surface wind speed',kind:'entry'},
  {id:'jaxa-amsr2-sst',name:'Choose JAXA AMSR2 · Microwave sea-surface temperature',kind:'entry'},
  {id:'jaxa-gsmap-spi',name:'Choose JAXA GSMaP · Standardized precipitation index',kind:'entry'},
  {id:'srtm-gl3',name:'Choose SRTM GL3 · 3 arc-second elevation',kind:'entry'},
  {id:'gedi-l3-canopy',name:'Choose GEDI L3 · Mean canopy height',kind:'entry'},
  {id:'srtm15plus-bathymetry',name:'Choose SRTM15+ · Land and ocean elevation',kind:'entry'},
  {id:'ghsl-built-surface',name:'Choose GHSL · Built-up surface',kind:'entry'},
  {id:'ghsl-built-volume',name:'Choose GHSL · Built-up volume',kind:'entry'},
  {id:'ghsl-population',name:'Choose GHSL · Residential population',kind:'entry'},
  {id:'eth-canopy-height',name:'Choose ETH · Canopy top height',kind:'entry'},
  {id:'hydrosheds-accumulation',name:'Choose HydroSHEDS v2 · Upstream accumulation',kind:'entry'},
  {id:'etopo-geoid',name:'Choose NOAA ETOPO · Geoid separation',kind:'entry'},
  {id:'pc-hrea',name:'Choose HREA · Settlement electricity access',kind:'entry'},
  {id:'pc-biodiversity',name:'Choose Impact Observatory · Biodiversity intactness',kind:'entry'},
  {id:'noaa-ohc',name:'Choose NOAA · Ocean heat-content anomaly',kind:'entry'},
  {id:'noaa-nclimgrid',name:'Choose NOAA NClimGrid · Monthly climate',kind:'entry'},
  {id:'cbers-4a',name:'Choose CBERS-4A WPM · L4 multispectral DN',kind:'entry'},
  {id:'usgs-3dep-dem',name:'Choose USGS 3DEP · NJ 1 m DEM',kind:'entry'},
  {id:'geoboundaries',name:'Choose geoBoundaries',kind:'entry'},
  {id:'ghsl',name:'Choose GHSL · Settlement classes 2020',kind:'entry'},
  {id:'glad-landcover-2020',name:'Choose GLAD · Land cover and use 2020',kind:'entry'},
  {id:'maxar-open',name:'Choose Maxar Open Data · Maui analytic imagery',kind:'entry'},
  {id:'wcs-service',name:'Choose WCS 2.0.1',kind:'entry'},
])('selects $id once after closing and restores the persistent trigger before another dialog opens', async({id,name,kind})=>{
  const user=userEvent.setup(),choose=vi.fn(),entry=vi.fn(),closed=vi.fn();render(<Harness choose={choose} entry={entry} closed={closed} nextDialog/>);
  const trigger=screen.getByRole('button',{name:'Data source trigger'});await open(user);
  await user.click(screen.getByRole('button',{name}));
  await screen.findByRole('dialog',{name:'Selected service'});
  expect(screen.queryByRole('dialog',{name:'Choose a data source'})).toBeNull();
  const action=kind==='provider'?choose:entry,other=kind==='provider'?entry:choose;
  expect(action).toHaveBeenCalledExactlyOnceWith(id,trigger);expect(other).not.toHaveBeenCalled();
  expect(closed).toHaveBeenCalledExactlyOnceWith(trigger);
  await user.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(document.activeElement).toBe(trigger);
});

it('Escape closes without a selection and returns keyboard focus', async()=>{
  const user=userEvent.setup(),choose=vi.fn(),entry=vi.fn(),closed=vi.fn();render(<Harness choose={choose} entry={entry} closed={closed}/>);await open(user);
  await user.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
  expect(document.activeElement).toBe(screen.getByRole('button',{name:'Data source trigger'}));expect(closed).toHaveBeenCalledOnce();expect(choose).not.toHaveBeenCalled();expect(entry).not.toHaveBeenCalled();
});

it('a newly locked picker disables every action and still closes normally', async()=>{
  const user=userEvent.setup(),choose=vi.fn(),entry=vi.fn(),closed=vi.fn(),view=render(<Harness choose={choose} entry={entry} closed={closed}/>);await open(user);
  view.rerender(<Harness disabled choose={choose} entry={entry} closed={closed}/>);
  expect(card('earth-search').querySelector('.source-picker-select').disabled).toBe(true);expect(card('wcs-service').querySelector('.source-picker-select').disabled).toBe(true);
  fireEvent.click(card('earth-search').querySelector('.source-picker-select'));expect(choose).not.toHaveBeenCalled();
  await user.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(closed).toHaveBeenCalledOnce();expect(entry).not.toHaveBeenCalled();
});

it('shows translated filters and searches English names within Chinese source presentation', async()=>{
  language.value='zh-CN';const user=userEvent.setup();render(<Harness/>);await open(user);
  expect(screen.getByRole('radio',{name:'已接入 · 111'})).toBeTruthy();
  await user.type(screen.getByRole('searchbox',{name:'搜索数据源'}),'permanent-water');
  expect(card('cgls-landcover')).not.toBeNull();expect(card('cgls-landcover').querySelector('.source-picker-select').getAttribute('aria-label')).toContain('永久水体');
});

it('soil and climate categories expose the specific reviewed measurements without enabling the broader JAXA portal', async()=>{
  const user=userEvent.setup();render(<Harness/>);await open(user);
  const navigation=screen.getByRole('navigation',{name:'Product categories'});
  await user.click(within(navigation).getByRole('button',{name:/^Soil products/}));
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId).sort()).toEqual(['jaxa-amsr2-soil-moisture','soilgrids']);
  await user.click(within(navigation).getByRole('button',{name:/^Climate and precipitation products/}));
  expect([...document.querySelectorAll('.source-picker-card')].map(node=>node.dataset.sourceId).sort()).toEqual(['chelsa-climate','chirps','jaxa-gsmap-rainfall','jaxa-gsmap-spi','jaxa-modis-aerosol','jaxa-modis-swr','jaxa-sgli-aerosol','jaxa-sgli-et','jaxa-sgli-lst','noaa-nclimgrid','worldclim']);
  expect(card('jaxa-earth')).toBeNull();expect(card('worldclim')).not.toBeNull();
  await user.click(screen.getByRole('radio',{name:'Pending · '+SOURCE_DIRECTORY.filter(s=>s.status==='planned').length}));
  await user.click(within(navigation).getByRole('button',{name:/^All categories/}));
  for(const id of ['jaxa-earth'])expect(card(id).querySelector('.source-picker-select').disabled).toBe(true);
  expect(card('soilgrids')).toBeNull();
  expect(card('worldclim')).toBeNull();
  await user.click(screen.getByRole('radio',{name:'Connected · 111'}));
  expect(card('worldclim').querySelector('.source-picker-select').disabled).toBe(false);expect(within(card('worldclim')).getByText('Product connector')).toBeTruthy();
});

it('Chinese search distinguishes monthly soil water from surface temperature while retaining the exact dataset entry',async()=>{
  language.value='zh-CN';const user=userEvent.setup();render(<Harness/>);await open(user);
  const input=screen.getByRole('searchbox',{name:'搜索数据源'});
  await user.type(input,'土壤水分');expect(card('jaxa-amsr2-soil-moisture')).not.toBeNull();expect(card('jaxa-sgli-lst')).toBeNull();
  await user.clear(input);await user.type(input,'地表温度');expect(card('jaxa-sgli-lst')).not.toBeNull();expect(card('jaxa-amsr2-soil-moisture')).toBeNull();
  expect(card('jaxa-sgli-lst').querySelector('.source-picker-select').getAttribute('aria-label')).toContain('JAXA SGLI · 月度地表温度');
});

it('GHSL opens the accepted 2020 settlement-class connector through its existing directory identity',async()=>{
  const source=SOURCE_DIRECTORY.find(source=>source.id==='ghsl');
  expect(source.name).toBe('GHSL · Settlement classes 2020');
  expect(source.group).toBe('population');
  expect(source.period).toContain('Only the modeled reference epoch 2020');
  expect(source.notes).toContain('not population counts');
  expect(source.notes).toContain('Raw code 0 is undocumented and is not NoData');
  expect(source.status).toBe('ready');expect(source.action).toEqual({kind:'dataset',datasetId:'ghsl-smod'});
  const user=userEvent.setup(),choose=vi.fn(),entry=vi.fn();render(<Harness choose={choose} entry={entry}/>);await open(user);
  await user.click(screen.getByRole('radio',{name:'All · 132'}));
  await user.type(screen.getByRole('searchbox'),'GHS-SMOD');
  expect(document.querySelectorAll('.source-picker-card')).toHaveLength(1);
  expect(within(card('ghsl')).getByText('GHS-SMOD V2 · 30 arc-second class grid · reference epoch 2020')).toBeTruthy();
  const button=card('ghsl').querySelector('.source-picker-select');expect(button.disabled).toBe(false);await user.click(button);
  expect(choose).not.toHaveBeenCalled();expect(entry).toHaveBeenCalledExactlyOnceWith('ghsl',screen.getByRole('button',{name:'Data source trigger'}));
});

it('GHSL science labels retain actual Int16 NoData and undocumented code zero in Chinese',()=>{
  expect(publicDatasetChinese['Degree of Urbanisation class code']).toBe('城市化程度分类代码');
  const value=publicDatasetChinese["Preserve the original signed Int16 level-2 class codes: 10 water grid cell; 11 very low density rural; 12 low density rural; 13 rural cluster; 21 suburban or peri-urban; 22 semi-dense urban cluster; 23 dense urban cluster; 30 urban centre. Original typed NoData is -200. The reviewed original also contains code 0, which is not explained by the publisher's legend: preserve it as an unclassified/undocumented raw code, without assigning a settlement class or replacing its mask. These categories are not continuous quantities or population counts."];
  expect(value).toContain('10 水体');expect(value).toContain('NoData 为 -200');expect(value).toContain('代码 0');expect(value).toContain('不是连续数值或人口数量');
  const dtype=publicDatasetChinese["The publisher's machine-readable product table describes int8, while this actual WGS84 V2 original is signed Int16 with representable -200 NoData. The physical TIFF is authoritative for raw storage and mask inspection. No in-file palette is present; a grayscale preview is not the publisher's categorical legend."];
  expect(dtype).toContain('int8');expect(dtype).toContain('有符号 Int16');expect(dtype).toContain('没有内嵌调色表');
});

it('Maui analytic, Vantor visual, processed nightlights and combined land-cover remain separate products',async()=>{
  const byId=Object.fromEntries(SOURCE_DIRECTORY.map(source=>[source.id,source]));
  expect(new Set(SOURCE_DIRECTORY.map(source=>source.id)).size).toBe(133);
  expect(SOURCE_DIRECTORY.filter(source=>source.status==='ready')).toHaveLength(110);
  expect(SOURCE_DIRECTORY.filter(source=>source.status==='auth')).toHaveLength(2);
  expect(SOURCE_DIRECTORY.filter(source=>source.status==='planned')).toHaveLength(21);
  expect(SOURCE_DIRECTORY.filter(source=>source.status==='verification')).toHaveLength(0);
  expect(byId['maxar-open'].name).toBe('Maxar Open Data · Maui analytic imagery');
  expect(byId['maxar-open'].notes).toContain('Visual YCbCr');
  expect(byId['vantor-open'].status).toBe('ready');expect(byId['vantor-open'].action).toEqual({kind:'dataset',datasetId:'vantor-open'});
  expect(byId['openlandmap-nightlights'].group).toBe('nightlights');
  expect(byId['openlandmap-nightlights'].detail).toContain('calibration declarations remain unresolved');
  expect(byId['eog-nightlights'].status).toBe('planned');expect(byId['eog-nightlights'].action).toBeUndefined();
  expect(byId['glad-landcover-2020'].group).toBe('land-cover');
  expect(byId['glad-forest'].group).toBe('vegetation');
  expect(byId['maxar-open'].status).toBe('ready');expect(byId['maxar-open'].action).toEqual({kind:'dataset',datasetId:'maxar-open-legacy'});
  expect(byId['glad-landcover-2020'].status).toBe('ready');expect(byId['glad-landcover-2020'].action).toEqual({kind:'dataset',datasetId:'glad-landcover-2020'});
  expect(byId['openlandmap-nightlights'].status).toBe('ready');expect(byId['openlandmap-nightlights'].action).toEqual({kind:'dataset',datasetId:'openlandmap-nightlights'});
  const user=userEvent.setup(),entry=vi.fn();render(<Harness entry={entry}/>);await open(user);
  await user.click(screen.getByRole('radio',{name:'All · 132'}));
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:/^Nighttime light products/}));
  await user.type(screen.getByRole('searchbox'),'nighttime');
  expect([...document.querySelectorAll('.source-picker-card')].map(n=>n.dataset.sourceId).sort()).toEqual(['eog-nightlights','openlandmap-nightlights']);
  expect(card('eog-nightlights').querySelector('.source-picker-select').disabled).toBe(true);
  await user.click(card('eog-nightlights').querySelector('.source-picker-select'));expect(entry).not.toHaveBeenCalled();
  await user.click(card('openlandmap-nightlights').querySelector('.source-picker-select'));expect(entry).toHaveBeenCalledExactlyOnceWith('openlandmap-nightlights',screen.getByRole('button',{name:'Data source trigger'}));
});

it('Chinese scientific labels distinguish GLAD zero from missing values and preserve unresolved nightlight calibration',()=>{
  expect(publicDatasetChinese['combined land-cover / land-use legend index']).toBe('土地覆盖 / 利用组合图例索引');
  expect(publicDatasetChinese['original processed Int16 DN; physical radiance calibration unresolved']).toContain('校准尚未明确');
  expect(publicDatasetChinese['Stored UInt16 image samples']).toBe('UInt16 影像存储值');
  const glad=Object.entries(publicDatasetChinese).find(([key])=>key.startsWith('Preserve original UInt8 annual-map indices'))?.[1];
  expect(glad).toContain('0 是有效');expect(glad).toContain('255');expect(glad).toContain('没有单一 NoData');expect(glad).toContain('不是');
  const lights=Object.entries(publicDatasetChinese).find(([key])=>key.startsWith('Scientific declarations conflict:'))?.[1];
  expect(lights).toContain('缩放 10');expect(lights).toContain('0.1');expect(lights).toContain('不能独立证明');expect(lights).toContain('proportion');
  const physical=Object.entries(publicDatasetChinese).find(([key])=>key.startsWith('Keep original signed Int16 processed DN')&&key.includes('Physical radiance calibration remains unresolved.'))?.[1];
  expect(physical).toContain('物理辐亮度校准仍未明确');expect(physical).toContain('不把 DN 换算成校准后的');
});

it('the accepted JAXA vegetation measurement and national boundary connector have distinct identities, categories and actions',async()=>{
  const byId=Object.fromEntries(SOURCE_DIRECTORY.map(source=>[source.id,source]));
  expect(byId['jaxa-modis-ndvi'].name).toBe('JAXA MODIS · Monthly vegetation index');
  expect(byId['jaxa-modis-ndvi'].status).toBe('ready');
  expect(byId['jaxa-modis-ndvi'].group).toBe('vegetation');
  expect(byId['jaxa-modis-ndvi'].category).toBe('thematic');
  expect(byId['jaxa-modis-ndvi'].detail).toBe('September 2026 monthly maximum NDVI · original DN');
  expect(byId['jaxa-modis-ndvi'].action).toEqual({kind:'dataset',datasetId:'jaxa-modis-ndvi'});
  expect(byId.geoboundaries.name).toBe('geoBoundaries');
  expect(byId.geoboundaries.status).toBe('ready');
  expect(byId.geoboundaries.group).toBe('vectors');
  expect(byId.geoboundaries.category).toBe('vectors');
  expect(byId.geoboundaries.action).toEqual({kind:'vector',protocol:'geoBoundaries'});
  expect(byId.geoboundaries.license).toContain('gbOpen CC BY 4.0');
  const user=userEvent.setup();render(<Harness/>);await open(user);
  const categories=screen.getByRole('navigation',{name:'Product categories'});
  await user.click(within(categories).getByRole('button',{name:/^Vegetation and forest products/}));
  expect(card('jaxa-modis-ndvi')).not.toBeNull();expect(card('geoboundaries')).toBeNull();
  await user.click(within(categories).getByRole('button',{name:/^Vector datasets and services/}));
  expect(card('geoboundaries')).not.toBeNull();expect(card('jaxa-modis-ndvi')).toBeNull();
  expect(card('geoboundaries').querySelector('.source-picker-select').disabled).toBe(false);
});
