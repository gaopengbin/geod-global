import React from 'react';import {it,expect,vi,beforeEach} from 'vitest';
import {render,screen,waitFor} from '@testing-library/react';import userEvent from '@testing-library/user-event';
import {VectorLibrary} from './vector-library.jsx';
import {vectorRequest,exportVector} from './vector-client.js';import {desktopAvailable} from './runtime-client.js';
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(s,v)=>s.replace('{count}',v?.count??''),number:String})}));
vi.mock('./runtime-client.js',()=>({desktopAvailable:vi.fn(()=>true)}));
vi.mock('./vector-client.js',()=>({vectorRequest:vi.fn(),importVectorFile:vi.fn(),exportVector:vi.fn()}));
const id='735f227b-5f95-473f-967b-077f3419bc68';
const asset={id,name:'Local Berlin <script>.json',featureCount:247,coordinateCount:5400,format:'overpass-json',storageMode:'reference',crs:'EPSG:4326',sourceSha256:'a'.repeat(64),licenseUrl:'https://www.openstreetmap.org/copyright',attribution:'© OpenStreetMap contributors',dataTimestamp:'2026-10-02T00:00:00Z'};
beforeEach(()=>{vi.clearAllMocks();desktopAvailable.mockReturnValue(true);vectorRequest.mockImplementation(async op=>op==='list'?[asset]:null);});
it('desktop opening references the selected source by default and keeps destructive actions separate from source files',async()=>{
  const user=userEvent.setup();render(<VectorLibrary/>);
  await screen.findByText(asset.name);expect(document.querySelector('script')).toBeNull();
  expect(screen.getByRole('link',{name:'Open in workspace'}).getAttribute('href')).toBe(`#Workspace?vector=${id}`);
  await user.click(screen.getByRole('button',{name:'Open vector file'}));
  await waitFor(()=>expect(vectorRequest).toHaveBeenCalledWith('open',{managed:false}));
  await user.click(screen.getByRole('button',{name:'Export GeoJSON'}));
  expect(exportVector).toHaveBeenCalledWith(id);
  await user.click(screen.getByRole('button',{name:'Remove registration'}));
  expect(vectorRequest).toHaveBeenCalledWith('forget',{id});
});
it('development browser offers an explicit managed copy and displays service failures without claiming a completed import',async()=>{
  desktopAvailable.mockReturnValue(false);vectorRequest.mockRejectedValue(new Error('Runtime unavailable'));
  render(<VectorLibrary/>);expect(screen.getByRole('button',{name:'Import a vector copy'})).toBeTruthy();
  expect(screen.queryByRole('combobox')).toBeNull();expect((await screen.findByRole('alert')).textContent).toContain('Runtime unavailable');
});
it('Shapefile originals export as a bundle while converted GeoJSON remains a separate action',async()=>{
  vectorRequest.mockResolvedValue([{...asset,name:'local.shp',format:'shapefile',shapefile:{container:'sidecars',layers:[],files:[]}}]);
  const user=userEvent.setup();render(<VectorLibrary/>);await screen.findByText('local.shp');
  await user.click(screen.getByRole('button',{name:'Export original Shapefile bundle'}));expect(exportVector).toHaveBeenCalledWith(id,true);
  await user.click(screen.getByRole('button',{name:'Export GeoJSON'}));expect(exportVector).toHaveBeenCalledWith(id);
  await user.click(screen.getByRole('button',{name:'Vector source details'}));expect(screen.getByText('Exact companion files in a reproducible ZIP bundle',{exact:false})).toBeTruthy();
});
it('original OSM export stays distinct and file timestamps are not invented',async()=>{
  const localOsm={encoding:'pbf',objectCounts:{node:2,way:1,relation:0},generator:'independent parser',datasetTimestamp:null,declaredBounds:null,replicationSequence:null,requiredFeatures:['OsmSchema-V0.6','DenseNodes'],ignoredBlockTypes:[]};
  vectorRequest.mockResolvedValue([{...asset,name:'local.osm.pbf',format:'osm-pbf',dataTimestamp:null,localOsm}]);
  const user=userEvent.setup();render(<VectorLibrary/>);await screen.findByText('local.osm.pbf');
  await user.click(screen.getByRole('button',{name:'Export original OSM file'}));expect(exportVector).toHaveBeenCalledWith(id,true);
  await user.click(screen.getByRole('button',{name:'Export GeoJSON'}));expect(exportVector).toHaveBeenCalledWith(id);
  await user.click(screen.getByRole('button',{name:'Vector source details'}));expect(screen.getByText('Not recorded in this file')).toBeTruthy();expect(screen.getByText('OsmSchema-V0.6, DenseNodes')).toBeTruthy();
});
it('online OSM source details retain the endpoint, raw response and derived export identities',async()=>{
  const osmSource={serviceName:'Authorized OSM service',serviceUrl:'https://example.com/overpass/api/interpreter',preset:'buildings',presetTitle:'Buildings',requestedBounds:[13.4,52.5,13.41,52.51],requestedAt:'2026-10-02T00:00:00Z',dataTimestamp:'2026-10-01T23:59:00Z',elementCounts:{nodes:0,ways:1,relations:0,total:1},dependencyCounts:{nodes:4,ways:0,relations:0,total:4},bytes:1200,responseSha256:'a'.repeat(64),generator:'Overpass API 0.7.62',apiVersion:0.6,copyrightText:'OpenStreetMap data under ODbL.',query:'<script>inert query text</script>'};
  vectorRequest.mockResolvedValue([{...asset,storageMode:'managed',osmSource,geojsonSha256:'b'.repeat(64)}]);const user=userEvent.setup();render(<VectorLibrary/>);
  await user.click(await screen.findByRole('button',{name:'Vector source details'}));expect(screen.getByText(osmSource.serviceUrl,{exact:false})).toBeTruthy();expect(screen.getByText(osmSource.responseSha256)).toBeTruthy();expect(screen.getByText('b'.repeat(64))).toBeTruthy();
  expect(screen.getByText('OSM database time')).toBeTruthy();expect(screen.getByText('Geometry dependencies')).toBeTruthy();expect(screen.getByText(osmSource.query)).toBeTruthy();expect(document.querySelector('script')).toBeNull();expect(screen.queryByText('OSM dataset timestamp')).toBeNull();
});
