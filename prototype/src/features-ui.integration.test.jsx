import React from 'react';import {it,expect,vi,beforeEach} from 'vitest';
import {render,screen,waitFor} from '@testing-library/react';import userEvent from '@testing-library/user-event';
import {FeatureServiceDialog,FeatureProvenanceDetails,OsmProvenanceDetails} from './features-ui.jsx';import {featureRequest} from './features-client.js';
import {OVERPASS_PRESETS,OVERPASS_DESCRIPTION,buildOverpassQuery} from './vector-client.js';
const translationState=vi.hoisted(()=>({values:{}}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(s,v={})=>(translationState.values[s]??s).replace(/\{(\w+)\}/g,(_,k)=>v[k]??''),number:String})}));
vi.mock('./features-client.js',async importOriginal=>({...await importOriginal(),featureRequest:vi.fn()}));
const id='735f227b-5f95-473f-967b-077f3419bc68';
const service={id,name:'Public lakes',url:'https://example.com/api',collections:[{id:'lakes',title:'Lakes',description:'Full polygons with attributes',licenseLinks:[]}]};
const arcgisLayer={objectIdField:'OBJECTID',geometryType:'esriGeometryPoint',spatialReference:{wkid:102100,latestWkid:3857},fields:[{name:'OBJECTID',alias:'Object ID',fieldType:'esriFieldTypeOID'},{name:'NAME',alias:'Name',fieldType:'esriFieldTypeString'}],maxRecordCount:1000,metadataSha256:'a'.repeat(64),copyrightText:'<img src=x onerror=alert(1)> Data provider'};
const arcgisService={...service,name:'Earthquakes',url:'https://example.com/arcgis/rest/services/Earthquakes/FeatureServer',arcgis:{copyrightText:'Service provider',excludedLayers:[{id:'2',name:'Elevation',reason:'Z, M and declared curve layers need a separate geometry adapter'}]},collections:[{id:'0',title:'Earthquakes',description:'Point features',licenseLinks:[],arcgis:arcgisLayer}]};
const osmService={...service,name:'Authorized OSM service',url:'https://example.com/overpass/api/interpreter',overpass:{generator:'Overpass API 0.7.62',apiVersion:0.6,metadataSha256:'a'.repeat(64),copyrightText:'Data from openstreetmap.org under ODbL.'},collections:Object.entries(OVERPASS_PRESETS).map(([id,title])=>({id,title,description:OVERPASS_DESCRIPTION,licenseLinks:['https://www.openstreetmap.org/copyright']}))};
beforeEach(()=>{vi.clearAllMocks();translationState.values={};featureRequest.mockImplementation(async op=>op==='list'?[service]:op==='connect'?service:{id:'file'});});
it('does not query without an explicit region and describes full feature geometry',async()=>{
  render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText('Full polygons with attributes');expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(true);
  expect(screen.getByText('Select a query region on Explore first.')).toBeTruthy();expect(screen.getByText(/Full feature geometry and attributes/)).toBeTruthy();
});
it('records the polygon snapshot but explicitly submits its bounding rectangle',async()=>{
  const user=userEvent.setup(),onImported=vi.fn(),onClose=vi.fn();
  const geometry={type:'Polygon',coordinates:[[[0,0],[10,0],[10,10],[0,10],[0,0]]]};
  render(<FeatureServiceDialog areaBounds={[0,0,10,10]} areaPolygon={{geometry}} onClose={onClose} onImported={onImported}/>);
  await screen.findByText('The polygon is recorded; selection uses its bounding rectangle.');await user.click(screen.getByRole('button',{name:'Get features and save'}));
  expect(featureRequest).toHaveBeenCalledWith('query',{serviceId:id,collectionId:'lakes',bounds:[0,0,10,10],areaGeometry:geometry});expect(onImported).toHaveBeenCalledWith({id:'file'});expect(onClose).toHaveBeenCalled();
});
it('failed extraction stays open, exposes the error and never reports an imported file',async()=>{
  const user=userEvent.setup(),onImported=vi.fn(),onClose=vi.fn();featureRequest.mockImplementation(async op=>{if(op==='query')throw new Error('Pagination incomplete');return [service];});
  render(<FeatureServiceDialog areaBounds={[0,0,10,10]} onClose={onClose} onImported={onImported}/>);await screen.findByText('Full polygons with attributes');await user.click(screen.getByRole('button',{name:'Get features and save'}));
  expect((await screen.findByRole('alert')).textContent).toBe('Pagination incomplete');expect(onImported).not.toHaveBeenCalled();expect(onClose).not.toHaveBeenCalled();
});
it('public demo only fills the connection form and connection discovery remains explicit',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[]:service);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByRole('button',{name:'Use public demo'});await user.click(screen.getByRole('button',{name:'Use public demo'}));
  expect(screen.getByRole('textbox',{name:'Service landing URL'}).value).toBe('https://demo.pygeoapi.io/stable');expect(featureRequest).not.toHaveBeenCalledWith('connect',expect.anything());
  await user.click(screen.getByRole('button',{name:'Connect service'}));await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:'pygeoapi · Natural Earth',url:'https://demo.pygeoapi.io/stable'}));
});
it('ArcGIS demo sets the protocol and connects only after explicit submission',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[]:arcgisService);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await user.click(await screen.findByRole('button',{name:'Use ArcGIS demo'}));
  expect(screen.getByRole('combobox',{name:'Service protocol'}).textContent).toBe('ArcGIS Feature Service');
  expect(screen.getByRole('textbox',{name:'Service landing URL'}).value).toBe('https://sampleserver6.arcgisonline.com/arcgis/rest/services/Earthquakes_Since1970/FeatureServer');
  expect(featureRequest.mock.calls.filter(([op])=>op==='connect')).toHaveLength(0);
  await user.click(screen.getByRole('button',{name:'Connect service'}));
  await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:'Esri · Earthquakes since 1970',url:'https://sampleserver6.arcgisonline.com/arcgis/rest/services/Earthquakes_Since1970/FeatureServer',protocol:'ArcGIS'}));
  await screen.findByText('Point features');
});
it('protocol selection works through the shared select and OGC demo resets it',async()=>{
  featureRequest.mockResolvedValue([]);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await user.click(await screen.findByRole('combobox',{name:'Service protocol'}));await user.click(screen.getByRole('option',{name:'ArcGIS Feature Service'}));
  expect(screen.getByRole('combobox',{name:'Service protocol'}).textContent).toBe('ArcGIS Feature Service');
  await user.click(screen.getByRole('button',{name:'Use public demo'}));expect(screen.getByRole('combobox',{name:'Service protocol'}).textContent).toBe('OGC API Features');
});
it('refresh preserves ArcGIS protocol and exposes excluded layers only on demand',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[arcgisService]:arcgisService);const user=userEvent.setup();
  render(<FeatureServiceDialog areaBounds={[0,0,10,10]} onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText('Point features');expect(screen.getByText(/Up to 25 batches, 5,000 features/)).toBeTruthy();
  const excluded=screen.getByRole('button',{name:'1 unavailable layers'});expect(excluded.getAttribute('aria-expanded')).toBe('false');expect(screen.queryByText(/Z, M and declared/)).toBeNull();
  await user.click(excluded);expect(screen.getByText('Z, M and declared curve layers need a separate geometry adapter')).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Refresh service collections'}));
  await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:arcgisService.name,url:arcgisService.url,protocol:'ArcGIS'}));
  expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(false);
});
it('ArcGIS metadata displays fields, source CRS and copyright as inert text',async()=>{
  featureRequest.mockResolvedValue([arcgisService]);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await user.click(await screen.findByRole('button',{name:'Collection source and license'}));
  expect(screen.getByText('2 fields · object ID: OBJECTID')).toBeTruthy();expect(screen.getByText(JSON.stringify(arcgisLayer.spatialReference))).toBeTruthy();
  expect(screen.getByText(arcgisLayer.metadataSha256)).toBeTruthy();expect(screen.getByText(arcgisLayer.copyrightText,{exact:false})).toBeTruthy();expect(document.querySelector('img')).toBeNull();
  expect(screen.getByText('Copyright attribution is not a dataset license.',{exact:false})).toBeTruthy();expect(screen.getByText('No dataset license link declared; check collection metadata before reuse.')).toBeTruthy();
});
it('a service containing only excluded ArcGIS layers remains inspectable and cannot be queried',async()=>{
  featureRequest.mockResolvedValue([{...arcgisService,collections:[]}]);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[0,0,10,10]} onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText('This service has no supported GeoJSON feature collections.');expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(true);
  await user.click(screen.getByRole('button',{name:'1 unavailable layers'}));expect(screen.getByText('Elevation · 2')).toBeTruthy();
  expect(featureRequest.mock.calls.filter(([op])=>op==='query')).toHaveLength(0);
});
it('empty ArcGIS provenance shows zero data batches and separate successful before and after checks',()=>{
  const source={serviceName:'Earthquakes',serviceUrl:arcgisService.url,collectionTitle:'Earthquakes',collectionId:'0',requestedBounds:[0,0,10,10],requestedAt:'2026-10-02T00:00:00Z',featureCount:0,pages:[],licenseLinks:[],arcgis:{layer:arcgisLayer,objectIds:[],idReceipts:[{returned:0},{returned:0}],countReceipts:[{returned:0},{returned:0}]}};
  render(<dl><FeatureProvenanceDetails source={source}/></dl>);
  expect(screen.getByText('Batches: 0 · 0 features; initial and final IDs and counts match',{exact:false})).toBeTruthy();
  expect(screen.getByText('Initial: 0 IDs · 0 features',{exact:false})).toBeTruthy();expect(screen.getByText('Final: 0 IDs · 0 features',{exact:false})).toBeTruthy();
  expect(screen.getByText('Checks detect ID and count changes; this is not a transaction snapshot.',{exact:false})).toBeTruthy();expect(screen.getByText('Service-converted WGS84 GeoJSON (EPSG:4326).')).toBeTruthy();
});
it('Overpass has no default endpoint or demo and sends only the endpoint explicitly entered by the user',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[]:osmService);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await user.click(await screen.findByRole('button',{name:'Use ArcGIS demo'}));await user.click(screen.getByRole('combobox',{name:'Service protocol'}));await user.click(screen.getByRole('option',{name:'OSM Overpass'}));
  expect(screen.getByRole('textbox',{name:'Overpass endpoint URL'}).value).toBe('');expect(screen.queryByRole('button',{name:'Use public demo'})).toBeNull();expect(screen.queryByRole('button',{name:'Use ArcGIS demo'})).toBeNull();
  expect(screen.getByText(/endpoint you operate or are authorized/)).toBeTruthy();expect(screen.getByRole('button',{name:'Connect service'}).disabled).toBe(true);
  await user.clear(screen.getByRole('textbox',{name:'Connection name'}));await user.type(screen.getByRole('textbox',{name:'Connection name'}),'My OSM service');await user.type(screen.getByRole('textbox',{name:'Overpass endpoint URL'}),osmService.url);
  expect(featureRequest.mock.calls.filter(([op])=>op==='connect')).toHaveLength(0);await user.click(screen.getByRole('button',{name:'Connect service'}));
  await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:'My OSM service',url:osmService.url,protocol:'Overpass'}));
});
it('Overpass refresh preserves protocol while an oversized region cannot be submitted',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[osmService]:osmService);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[0,0,0.1,0.1]} onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText(OVERPASS_DESCRIPTION);expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(true);expect((await screen.findByRole('alert')).textContent).toContain('at most 100 km²');
  await user.click(screen.getByRole('button',{name:'Refresh service collections'}));await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:osmService.name,url:osmService.url,protocol:'Overpass'}));
  expect(featureRequest.mock.calls.filter(([op])=>op==='query')).toHaveLength(0);
});
it('OSM presets are translated and remain searchable in the selected language',async()=>{
  translationState.values={Buildings:'建筑',Roads:'道路',Water:'水体','Land use':'土地利用','Points of interest':'兴趣点'};featureRequest.mockResolvedValue([osmService]);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[13.4,52.5,13.41,52.51]} onClose={()=>{}} onImported={()=>{}}/>);
  const collection=await screen.findByRole('combobox',{name:'Collection'});await user.click(collection);
  for(const name of Object.values(translationState.values))expect(screen.getByRole('option',{name,exact:true})).toBeTruthy();await user.keyboard('{Escape}');
  await user.type(screen.getByRole('searchbox',{name:'Find a collection'}),'建筑');await user.click(collection);expect(screen.getByRole('option',{name:'建筑'})).toBeTruthy();expect(screen.queryByRole('option',{name:'道路'})).toBeNull();
});
it('OSM query retains the area snapshot and submits one chosen preset without pagination',async()=>{
  const user=userEvent.setup(),onImported=vi.fn(),onClose=vi.fn(),bounds=[13.4,52.5,13.41,52.51],geometry={type:'Polygon',coordinates:[[[13.4,52.5],[13.41,52.5],[13.41,52.51],[13.4,52.5]]]};
  featureRequest.mockImplementation(async op=>op==='list'?[osmService]:{id:'osm-file'});render(<FeatureServiceDialog areaBounds={bounds} areaPolygon={{geometry}} onClose={onClose} onImported={onImported}/>);
  await screen.findByText(/One bounded OSM query/);expect(screen.getByText('OSM bounding-box selection; full geometry retained. Not an exact polygon clip.')).toBeTruthy();
  await user.click(screen.getByRole('combobox',{name:'Collection'}));await user.click(screen.getByRole('option',{name:'Roads'}));await user.click(screen.getByRole('button',{name:'Get features and save'}));
  expect(featureRequest).toHaveBeenCalledWith('query',{serviceId:id,collectionId:'roads',bounds,areaGeometry:geometry});expect(featureRequest.mock.calls.filter(([op])=>op==='query')).toHaveLength(1);expect(onImported).toHaveBeenCalledWith({id:'osm-file'});expect(onClose).toHaveBeenCalled();
});
it('Overpass rate limiting stays visible without a retry or a registered result',async()=>{
  featureRequest.mockImplementation(async op=>{if(op==='query')throw new Error('Overpass is busy or rate limited; wait before trying again. No automatic retry was started.');return [osmService];});const user=userEvent.setup(),onImported=vi.fn();render(<FeatureServiceDialog areaBounds={[13.4,52.5,13.41,52.51]} onClose={()=>{}} onImported={onImported}/>);
  await screen.findByText(OVERPASS_DESCRIPTION);await user.click(screen.getByRole('button',{name:'Get features and save'}));expect((await screen.findByRole('alert')).textContent).toContain('No automatic retry');expect(featureRequest.mock.calls.filter(([op])=>op==='query')).toHaveLength(1);expect(onImported).not.toHaveBeenCalled();
});
it('OSM provenance separates selected elements and dependencies, original response and derived GeoJSON',()=>{
  const source={serviceUrl:osmService.url,serviceName:osmService.name,preset:'buildings',presetTitle:'Buildings',requestedBounds:[13.4,52.5,13.41,52.51],requestedAt:'2026-10-02T00:00:00Z',dataTimestamp:'2026-10-01T23:59:00Z',elementCounts:{nodes:0,ways:1,relations:1,total:2},dependencyCounts:{nodes:10,ways:2,relations:0,total:12},query:buildOverpassQuery('buildings',[13.4,52.5,13.41,52.51]),bytes:4000,responseSha256:'a'.repeat(64),generator:osmService.overpass.generator,apiVersion:0.6,copyrightText:'<img src=x> openstreetmap.org ODbL'};
  render(<dl><OsmProvenanceDetails source={source} geojsonSha256={'b'.repeat(64)}/></dl>);
  expect(screen.getByText('Nodes: 0 · ways: 1 · relations: 1',{exact:false})).toBeTruthy();expect(screen.getByText('Nodes: 10 · ways: 2 · relations: 0',{exact:false})).toBeTruthy();expect(screen.getByText('2 elements converted to features',{exact:false})).toBeTruthy();
  expect(screen.getByText('Original OSM response')).toBeTruthy();expect(screen.getByText('Derived GeoJSON SHA-256')).toBeTruthy();expect(screen.getByText('OSM database time')).toBeTruthy();expect(screen.getByText(source.query)).toBeTruthy();expect(document.querySelector('img')).toBeNull();expect(screen.queryByText(/complete matching query/)).toBeNull();
});
const wfsLayer={typeName:'demo:lakes',namespace:'demo',defaultCrs:'EPSG:3857',otherCrs:[],formats:[{id:'gml32',mime:'application/gml+xml; version=3.2'},{id:'geojson',mime:'application/json'}],defaultFormat:'gml32'};
const wfsService={...service,name:'WFS lake service',url:'https://example.com/wfs',wfs:{version:'2.0.0',capabilitiesSha256:'c'.repeat(64),fees:'NONE',accessConstraints:'<img src=x> Consult the provider',pagingSupported:true,excludedLayers:[{id:'demo:complex',title:'Complex data',reason:'Nested properties are not supported'}]},collections:[{id:'demo:lakes',title:'WFS lakes',description:'WFS lake polygons',licenseLinks:[],wfs:wfsLayer}]};
it('WFS has no default endpoint or sample and preserves its explicit protocol when connecting',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[]:wfsService);const user=userEvent.setup();render(<FeatureServiceDialog onClose={()=>{}} onImported={()=>{}}/>);
  await user.click(await screen.findByRole('button',{name:'Use public demo'}));await user.click(screen.getByRole('combobox',{name:'Service protocol'}));await user.click(screen.getByRole('option',{name:'WFS 2.0'}));
  expect(screen.getByRole('textbox',{name:'WFS service URL'}).value).toBe('');expect(screen.queryByRole('button',{name:'Use public demo'})).toBeNull();expect(screen.queryByRole('button',{name:'Use ArcGIS demo'})).toBeNull();
  await user.clear(screen.getByRole('textbox',{name:'Connection name'}));await user.type(screen.getByRole('textbox',{name:'Connection name'}),'My WFS');await user.type(screen.getByRole('textbox',{name:'WFS service URL'}),wfsService.url);await user.click(screen.getByRole('button',{name:'Connect service'}));
  await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:'My WFS',url:wfsService.url,protocol:'WFS2'}));
});
it('WFS prefers advertised GML and submits a separately chosen GeoJSON service response',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[wfsService]:{id:'wfs-file'});const user=userEvent.setup(),onImported=vi.fn();render(<FeatureServiceDialog areaBounds={[10,20,11,21]} onClose={()=>{}} onImported={onImported}/>);
  const select=await screen.findByRole('combobox',{name:'Service response format'});await waitFor(()=>expect(select.textContent).toBe('GML 3.2'));expect(screen.getByText(/service returns GML 3.2/)).toBeTruthy();
  await user.click(select);await user.click(screen.getByRole('option',{name:'GeoJSON'}));expect(screen.getByText(/service returns GeoJSON/)).toBeTruthy();await user.click(screen.getByRole('button',{name:'Get features and save'}));
  expect(featureRequest).toHaveBeenCalledWith('query',{serviceId:id,collectionId:'demo:lakes',bounds:[10,20,11,21],areaGeometry:null,responseFormat:'geojson'});expect(onImported).toHaveBeenCalledWith({id:'wfs-file'});
});
it('WFS refresh preserves protocol and presents unsupported types and access declarations as text',async()=>{
  featureRequest.mockImplementation(async op=>op==='list'?[wfsService]:wfsService);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[10,20,11,21]} onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText('WFS lake polygons');expect(screen.getByText(/Up to 250 pages, 50,000 features/)).toBeTruthy();const disclosure=screen.getByRole('button',{name:'1 unavailable feature types'});expect(disclosure.getAttribute('aria-expanded')).toBe('false');expect(screen.queryByText('Nested properties are not supported')).toBeNull();
  await user.click(disclosure);expect(screen.getByText('Nested properties are not supported')).toBeTruthy();await user.click(screen.getByRole('button',{name:'Collection source and license'}));expect(screen.getByText(wfsService.wfs.accessConstraints)).toBeTruthy();expect(document.querySelector('img')).toBeNull();expect(screen.getByText('NONE')).toBeTruthy();expect(screen.getByText(/No dataset license link declared/)).toBeTruthy();
  await user.click(screen.getByRole('button',{name:'Refresh service collections'}));await waitFor(()=>expect(featureRequest).toHaveBeenCalledWith('connect',{name:wfsService.name,url:wfsService.url,protocol:'WFS2'}));
});
it('an entirely excluded WFS service remains inspectable without claiming GeoJSON support',async()=>{
  featureRequest.mockResolvedValue([{...wfsService,collections:[]}]);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[10,20,11,21]} onClose={()=>{}} onImported={()=>{}}/>);
  await screen.findByText('This service has no supported WFS feature types.');expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(true);expect(screen.queryByRole('combobox',{name:'Service response format'})).toBeNull();await user.click(screen.getByRole('button',{name:'1 unavailable feature types'}));expect(screen.getByText('Complex data · demo:complex')).toBeTruthy();
});
it('long WFS collection abstracts stay available without taking over the query form',async()=>{
  const description='A complete original provider abstract. '.repeat(30),source={...wfsService,collections:[{...wfsService.collections[0],description}]};featureRequest.mockResolvedValue([source]);const user=userEvent.setup();render(<FeatureServiceDialog areaBounds={[10,20,11,21]} onClose={()=>{}} onImported={()=>{}}/>);
  const disclosure=await screen.findByRole('button',{name:'Collection description'});expect(disclosure.getAttribute('aria-expanded')).toBe('false');expect(screen.queryByText(description,{exact:true})).toBeNull();expect(screen.getByRole('button',{name:'Get features and save'}).disabled).toBe(false);await user.click(disclosure);expect(screen.getByText(description.trim(),{exact:true})).toBeTruthy();
});
it('WFS provenance separates original GML responses and the converted GeoJSON export without inferring a license',()=>{
  const source={serviceUrl:wfsService.url,serviceName:wfsService.name,collectionId:'demo:lakes',collectionTitle:'Lakes',requestedBounds:[10,20,11,21],requestedAt:'2026-10-02T00:00:00Z',featureCount:2,pages:[{}],licenseLinks:[],wfs:{...wfsService.wfs,layer:wfsLayer,format:wfsLayer.formats[0],requestCrs:'urn:ogc:def:crs:EPSG::4326',responseCrs:'urn:ogc:def:crs:EPSG::4326',sortField:null,verificationPages:[{}],schema:{fields:[{name:'name'}],geometryField:'geom',geometryType:'Polygon',sha256:'a'.repeat(64)}}};
  render(<dl><FeatureProvenanceDetails source={source} sourceSha256={'d'.repeat(64)} sourceBytes={2500} geojsonSha256={'e'.repeat(64)}/></dl>);
  expect(screen.getByText(/GML geometry is converted locally/)).toBeTruthy();expect(screen.getByText(/Original schema, count and feature responses/)).toBeTruthy();expect(screen.getByText(/not a transaction snapshot/)).toBeTruthy();expect(screen.getByText('Original response archive SHA-256')).toBeTruthy();expect(screen.getByText('Derived GeoJSON SHA-256')).toBeTruthy();expect(screen.getByText('NONE')).toBeTruthy();expect(screen.queryByText('Service-converted WGS84 GeoJSON (EPSG:4326).')).toBeNull();
});
