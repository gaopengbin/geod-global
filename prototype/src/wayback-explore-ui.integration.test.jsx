import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {act,render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {WaybackExplorer} from './wayback-explore-ui.jsx';
import {fetchWaybackCatalog,fetchWaybackPointMetadata,parseWaybackCatalog,WAYBACK_CATALOG_URL} from './wayback-client.js';

vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),number:String,locale:'en-US'})}));
vi.mock('./wayback-client.js',async load=>({...await load(),fetchWaybackCatalog:vi.fn(),fetchWaybackPointMetadata:vi.fn()}));
vi.mock('./explore-source-workspace.jsx',()=>({ExploreSourceFrame:({children,actions,map})=><main><aside>{children}{actions}</aside><section>{map}</section></main>}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:({entries,onInspectWaybackPoint,onRemove})=><div><output data-testid="wayback-layers">{entries.map(entry=>entry.key).join('|')}</output><button onClick={()=>onInspectWaybackPoint([116.4,39.9],14,entries.filter(entry=>entry.visible!==false).map(entry=>entry.data.version))}>Inspect map point</button>{entries.map(entry=><button key={entry.key} onClick={()=>onRemove(entry.key)}>Remove {entry.title}</button>)}</div>}));

const bounds=[116.3,39.8,116.5,40];
const record=(release,date,year,revision)=>({itemID:'ab83ada1bd3145978eecfb115aaa10c3',itemTitle:`World Imagery (Wayback ${date})`,itemURL:`https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/${release}/{level}/{row}/{col}`,metadataLayerUrl:`https://metadata.maptiles.arcgis.com/arcgis/rest/services/World_Imagery_Metadata_${year}_r${revision}/MapServer`,metadataLayerItemID:'f2696faf5a3643fe9f9ee097b09caa55',layerIdentifier:`WB_${year}_R${revision}`});
const raw=JSON.stringify({63116:record(63116,'2026-09-24',2026,'09'),26334:record(26334,'2026-08-05',2026,'07')});
beforeEach(()=>{vi.clearAllMocks();fetchWaybackCatalog.mockResolvedValue({raw,url:WAYBACK_CATALOG_URL,document:JSON.parse(raw),versions:parseWaybackCatalog(JSON.parse(raw))});fetchWaybackPointMetadata.mockImplementation(async(version,point,zoom)=>({versionId:version.id,point,zoom,raw:'{}',records:[{acquisitionDate:'2026-01-26T00:00:00.000Z',provider:'Vantor',source:'LG06',resolutionMeters:.3,accuracyMeters:8.47}]}));});

it('keeps archive publication versions in the shared region and creates only online basemap entries',async()=>{
  const user=userEvent.setup();render(<WaybackExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));
  expect(fetchWaybackPointMetadata).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Preview selected'}));
  expect(screen.getByTestId('wayback-layers').textContent).toBe('wayback-63116|wayback-26334');
  await user.click(screen.getByRole('button',{name:'Preview details'}));
  expect(screen.getByText('Online rendered basemap. Publication dates identify archive versions, not imagery acquisition dates.')).toBeTruthy();
  await user.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Preview details'})).toBeNull());
  const links=screen.getAllByRole('link',{name:'Authorized export in ArcGIS'});expect(links).toHaveLength(2);expect(links[0].getAttribute('href')).toContain('active=63116');expect(links[0].getAttribute('href')).toContain('116.3%2C39.8%2C116.5%2C40');
  expect(screen.queryByRole('button',{name:/Export result/})).toBeNull();
});

it('reads point acquisition metadata for the visible versions without replacing publication dates',async()=>{
  const user=userEvent.setup();render(<WaybackExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Select visible results'}));await user.click(screen.getByRole('button',{name:'Preview selected'}));await user.click(screen.getByRole('button',{name:'Inspect map point'}));
  await waitFor(()=>expect(fetchWaybackPointMetadata).toHaveBeenCalledTimes(2));
  await waitFor(()=>expect(screen.queryByText('Reading point imagery metadata')).toBeNull());
  await user.click(screen.getAllByRole('button',{name:'Point imagery metadata'})[0]);
  expect(screen.getByText('2026-01-26')).toBeTruthy();expect(screen.getByText('Vantor · LG06')).toBeTruthy();expect(screen.getByText('0.3 / 8.47')).toBeTruthy();
  expect(screen.getByText(/2026-09-24/,{selector:'small'})).toBeTruthy();
});

it('AOI changes abort point requests, discard old previews and ignore late metadata',async()=>{
  let finish,signal;fetchWaybackPointMetadata.mockImplementation((version,point,zoom,requestSignal)=>{signal=requestSignal;return new Promise(resolve=>{finish=()=>resolve({versionId:version.id,point,zoom,records:[{acquisitionDate:'2020-01-01T00:00:00.000Z'}]});});});
  const user=userEvent.setup(),view=render(<WaybackExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('checkbox',{name:'Select result World Imagery (Wayback 2026-09-24)'}));await user.click(screen.getByRole('button',{name:'Preview selected'}));await user.click(screen.getByRole('button',{name:'Inspect map point'}));
  view.rerender(<WaybackExplorer areaBounds={[-74,40,-73.9,40.1]}/>);expect(signal.aborted).toBe(true);await act(async()=>finish());
  expect(screen.getByTestId('wayback-layers').textContent).toBe('');expect(screen.queryByRole('button',{name:'Point imagery metadata'})).toBeNull();
});

it('missing AOI disables preview and metadata-reference saving while catalog remains readable',async()=>{
  const user=userEvent.setup();render(<WaybackExplorer/>);await user.click(await screen.findByRole('button',{name:'Select visible results'}));
  expect(screen.getByRole('button',{name:'Preview selected'}).disabled).toBe(true);expect(screen.getAllByRole('button',{name:/Save reference/}).every(button=>button.disabled)).toBe(true);
  expect(screen.queryByRole('link',{name:'Authorized export in ArcGIS'})).toBeNull();
});

it('a directory failure is visible and refresh retries the real directory request',async()=>{
  fetchWaybackCatalog.mockRejectedValueOnce(new Error('Wayback returned HTTP 503'));
  const user=userEvent.setup();render(<WaybackExplorer areaBounds={bounds}/>);expect((await screen.findByRole('alert')).textContent).toContain('HTTP 503');
  await user.click(screen.getByRole('button',{name:'Refresh Wayback directory'}));expect(await screen.findByRole('checkbox',{name:'Select result World Imagery (Wayback 2026-09-24)'})).toBeTruthy();expect(fetchWaybackCatalog).toHaveBeenCalledTimes(2);
});
