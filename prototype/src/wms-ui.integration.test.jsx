import React from 'react';import {it,expect,vi,beforeEach} from 'vitest';import{render,screen,waitFor}from'@testing-library/react';import userEvent from'@testing-library/user-event';import{MapServiceDialog}from'./wms-ui.jsx';import{mapRequest}from'./wms-client.js';
import {defaultTileConfig,gibsXyzExample,dlrTmsExample,xyzPlan} from './xyz-client.js';
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(s,v={})=>s.replace(/\{(\w+)\}/g,(_,k)=>v[k]??''),number:String,locale:'en-US'})}));
vi.mock('./wms-client.js',async load=>({...await load(),mapRequest:vi.fn()}));
const id='735f227b-5f95-473f-967b-077f3419bc68',layer={name:'land',title:'Land imagery',styles:[],time:{default:'2025-06-27',values:'2025-06-01/2025-06-30/P1D'}};
const service={id,name:'NASA GIBS',url:'https://maps.example.com/wms',layers:[layer],maxWidth:2048,maxHeight:2048};
beforeEach(()=>{vi.clearAllMocks();mapRequest.mockImplementation(async op=>op==='services'?[service]:{id:'image'});});
it('WMTS capability documents retain their discovery method when connecting and refreshing',async()=>{
 const url='https://gibs.earthdata.nasa.gov/wmts/epsg4326/best/1.0.0/WMTSCapabilities.xml';
 const m={id:'opaque-level',scaleDenominator:(Math.PI*6378137/180)/.00028,topLeft:[-180,90],tileWidth:16,tileHeight:16,matrixWidth:32,matrixHeight:16};
 const saved={...service,url,wmts:{capabilitiesDocument:true,restOnly:true,matrixSets:[{id:'regional',crs:'EPSG:4326',declaredCrs:'CRS:84',matrices:[m]}]},layers:[{...layer,styles:['default'],wmts:{resourceUrl:'https://maps.example.com/{Time}/{TileMatrix}/{TileRow}/{TileCol}.png',format:'image/png',defaultStyle:'default',timeIdentifier:'Time',links:[{matrixSet:'regional',limits:[]}]}}]};
 mapRequest.mockImplementation(async op=>op==='services'?[]:saved);const user=userEvent.setup();render(<MapServiceDialog areaBounds={[-125,30,-110,43]} onClose={()=>{}} onSaved={()=>{}}/>);
 await screen.findByRole('combobox',{name:'Map service type'});await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'WMTS',exact:true}));
 await user.click(screen.getByRole('combobox',{name:'WMTS address type'}));await user.click(await screen.findByRole('option',{name:'Capabilities document (XML)'}));await user.click(screen.getByRole('button',{name:'Use NASA GIBS'}));expect(screen.getByRole('textbox',{name:'WMTS endpoint URL'}).value).toBe(url);
 await user.click(screen.getByRole('button',{name:'Connect service'}));expect(mapRequest).toHaveBeenCalledWith('connect',{name:'NASA GIBS',url,protocol:'WMTS',wmtsDocument:true});
 await screen.findByRole('button',{name:'Refresh map layers'});await user.click(screen.getByRole('button',{name:'Refresh map layers'}));expect(mapRequest).toHaveBeenLastCalledWith('connect',{name:'NASA GIBS',url,protocol:'WMTS',wmtsDocument:true});
});
it('XYZ example supplies the explicit NASA grid and saving passes configuration without capability discovery',async()=>{
 mapRequest.mockResolvedValue([]);const user=userEvent.setup();render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);
 await screen.findByRole('combobox',{name:'Map service type'});await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'XYZ',exact:true}));
 await user.click(screen.getByRole('button',{name:'Use NASA GIBS'}));expect(screen.getByRole('textbox',{name:'Tile URL template'}).value).toBe(gibsXyzExample.url);expect(screen.getByRole('spinbutton',{name:'Maximum level'}).value).toBe('9');expect(screen.getByText(/Rows from top/)).toBeTruthy();
 expect(mapRequest.mock.calls.every(([op])=>op==='services')).toBe(true);mapRequest.mockResolvedValue({...service,xyz:{scheme:'XYZ',urlTemplate:gibsXyzExample.url,grid:gibsXyzExample.tileConfig},layers:[{name:'tiles',title:'NASA imagery',styles:[],time:null}]});
 await user.click(screen.getByRole('button',{name:'Save connection'}));expect(mapRequest).toHaveBeenCalledWith('connect',{name:gibsXyzExample.name,url:gibsXyzExample.url,protocol:'XYZ',tileConfig:gibsXyzExample.tileConfig});
});
it('TMS automatically chooses its sole layer and preserves grid settings on refresh and get',async()=>{
 const c={scheme:'TMS',urlTemplate:'https://example.com/{z}/{x}/{y}.png',grid:{...defaultTileConfig(),tileSize:512,minZoom:6,maxZoom:6,zoomOffset:-1}};
 const tileService={...service,xyz:c,layers:[{name:'tiles',title:'Configured tiles',styles:[],time:null}]};mapRequest.mockImplementation(async op=>op==='get'?{id:'tile-image'}:op==='connect'?tileService:[tileService]);
 const user=userEvent.setup(),bounds=[-2,-2,2,2],saved=vi.fn();render(<MapServiceDialog areaBounds={bounds} onClose={()=>{}} onSaved={saved}/>);
 await screen.findByRole('combobox',{name:'Tile level'});expect(screen.queryByRole('combobox',{name:'Image size'})).toBeNull();expect(screen.queryByRole('combobox',{name:'Map layer'})).toBeNull();
 await user.click(screen.getByRole('button',{name:'Refresh map layers'}));expect(mapRequest).toHaveBeenCalledWith('connect',{name:service.name,url:c.urlTemplate,protocol:'TMS',tileConfig:c.grid});
 await waitFor(()=>expect(screen.getByRole('button',{name:'Save map image'}).disabled).toBe(false));await user.click(screen.getByRole('button',{name:'Save map image'}));const p=xyzPlan(bounds,c,6);
 expect(mapRequest).toHaveBeenCalledWith('get',{serviceId:id,layerName:'tiles',style:'',time:null,bounds,width:p.width,height:p.height,areaGeometry:null,tileMatrixSet:'WebMercator',tileMatrix:'6'});expect(saved).toHaveBeenCalledWith({id:'tile-image'});
});
it('TMS example retains the protocol and fills the verified DLR grid without connecting',async()=>{
 mapRequest.mockResolvedValue([]);const user=userEvent.setup();render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);
 await screen.findByRole('combobox',{name:'Map service type'});await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'TMS',exact:true}));
 await user.click(screen.getByRole('button',{name:'Use DLR Basemap'}));
 expect(screen.getByRole('combobox',{name:'Map service type'}).textContent).toBe('TMS');
 expect(screen.getByRole('textbox',{name:'Connection name'}).value).toBe(dlrTmsExample.name);
 expect(screen.getByRole('textbox',{name:'Tile URL template'}).value).toBe(dlrTmsExample.url);
 expect(screen.getByRole('spinbutton',{name:'Maximum level'}).value).toBe('16');
 expect(screen.getByText(/Rows from bottom/)).toBeTruthy();expect(screen.getByRole('combobox',{name:'Tile format'}).textContent).toBe('PNG');
 expect(mapRequest.mock.calls.every(([op])=>op==='services')).toBe(true);
 mapRequest.mockResolvedValue({...service,name:dlrTmsExample.name,xyz:{scheme:'TMS',urlTemplate:dlrTmsExample.url,grid:dlrTmsExample.tileConfig},layers:[{name:'tiles',title:'Configured tiles',styles:[],time:null}]});
 await user.click(screen.getByRole('button',{name:'Save connection'}));
 expect(mapRequest).toHaveBeenCalledWith('connect',{name:dlrTmsExample.name,url:dlrTmsExample.url,protocol:'TMS',tileConfig:dlrTmsExample.tileConfig});
});
it('an official encoded TMS layer address can be pasted and saved without changing its path',async()=>{
 const url='https://tiles.geoservice.dlr.de/service/tms/1.0.0/eoc%3Abasemap@EPSG%3A3857@png/{z}/{x}/{y}.png';
 mapRequest.mockResolvedValue([]);const user=userEvent.setup();render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);
 await screen.findByRole('combobox',{name:'Map service type'});await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'TMS',exact:true}));
 await user.type(screen.getByRole('textbox',{name:'Connection name'}),'DLR EOC Basemap');
 await user.click(screen.getByRole('textbox',{name:'Tile URL template'}));await user.paste(url);
 expect(screen.queryByRole('alert')).toBeNull();expect(screen.getByRole('button',{name:'Save connection'}).disabled).toBe(false);
 mapRequest.mockResolvedValue({...service,name:'DLR EOC Basemap',url,xyz:{scheme:'TMS',urlTemplate:url,grid:defaultTileConfig()},layers:[{name:'tiles',title:'Configured tiles',styles:[],time:null}]});
 await user.click(screen.getByRole('button',{name:'Save connection'}));
 expect(mapRequest).toHaveBeenCalledWith('connect',{name:'DLR EOC Basemap',url,protocol:'TMS',tileConfig:defaultTileConfig()});
});
it('ArcGIS example fills its root address and refresh preserves the protocol',async()=>{
 mapRequest.mockResolvedValue([]);const user=userEvent.setup();const view=render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);
 await screen.findByRole('combobox',{name:'Map service type'});await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'ArcGIS MapServer / ImageServer'}));await user.click(screen.getByRole('button',{name:'Use USGS NAIP'}));expect(screen.getByRole('textbox',{name:'ArcGIS service URL'}).value).toBe('https://imagery.nationalmap.gov/arcgis/rest/services/USGSNAIPImagery/ImageServer');expect(mapRequest.mock.calls.every(([op])=>op==='services')).toBe(true);
 view.unmount();mapRequest.mockImplementation(async op=>op==='services'?[{...service,arcgis:{serviceType:'MapServer'}}]:{...service,arcgis:{serviceType:'MapServer'}});render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);await screen.findByRole('button',{name:'Refresh map layers'});await user.click(screen.getByRole('button',{name:'Refresh map layers'}));expect(mapRequest).toHaveBeenCalledWith('connect',{name:service.name,url:service.url,protocol:'ArcGIS'});
});
it('requires a chosen layer and explicit region before saving',async()=>{render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);await screen.findByRole('combobox',{name:'Map layer'});expect(screen.getByRole('button',{name:'Save map image'}).disabled).toBe(true);expect(screen.getByText('Select a query region on Explore first.')).toBeTruthy();});
it('saves an actual bounded image with explicit date, grid and polygon snapshot',async()=>{const user=userEvent.setup(),saved=vi.fn(),close=vi.fn();const geometry={type:'Polygon',coordinates:[[[0,0],[4,0],[4,2],[0,2],[0,0]]]};render(<MapServiceDialog areaBounds={[0,0,4,2]} areaPolygon={{geometry}} onClose={close} onSaved={saved}/>);await screen.findByRole('combobox',{name:'Map layer'});await user.click(screen.getByRole('combobox',{name:'Map layer'}));await user.click(await screen.findByRole('option',{name:'Land imagery'}));expect(screen.getByRole('button',{name:'Service time (UTC)'})).toBeTruthy();await user.click(screen.getByRole('button',{name:'Save map image'}));expect(mapRequest).toHaveBeenCalledWith('get',{serviceId:id,layerName:'land',style:'',time:'2025-06-27',bounds:[0,0,4,2],width:1024,height:512,areaGeometry:geometry});expect(saved).toHaveBeenCalledWith({id:'image'});expect(close).toHaveBeenCalled();});
it('a service exception keeps the dialog open and does not register success',async()=>{mapRequest.mockImplementation(async op=>{if(op==='get')throw new Error('WMS exception');return[service];});const user=userEvent.setup(),saved=vi.fn(),close=vi.fn();render(<MapServiceDialog areaBounds={[0,0,4,2]} onClose={close} onSaved={saved}/>);await screen.findByRole('combobox',{name:'Map layer'});await user.click(screen.getByRole('combobox',{name:'Map layer'}));await user.click(await screen.findByRole('option',{name:'Land imagery'}));await user.click(screen.getByRole('button',{name:'Save map image'}));expect((await screen.findByRole('alert')).textContent).toBe('WMS exception');expect(saved).not.toHaveBeenCalled();expect(close).not.toHaveBeenCalled();});
it('NASA example only fills the form and does not silently connect',async()=>{mapRequest.mockResolvedValue([]);const user=userEvent.setup();render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);await screen.findByRole('button',{name:'Use NASA GIBS'});await user.click(screen.getByRole('button',{name:'Use NASA GIBS'}));expect(screen.getByRole('textbox',{name:'WMS endpoint URL'}).value).toBe('https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi');expect(mapRequest).not.toHaveBeenCalledWith('connect',expect.anything());});
it('WMTS uses advertised grid and opaque level selection in the existing map flow',async()=>{
 const matrix={id:'coarse-grid',scaleDenominator:(Math.PI*6378137/180)/.00028,topLeft:[0,4],tileWidth:2,tileHeight:2,matrixWidth:4,matrixHeight:4};
 const wmtsLayer={...layer,styles:['default'],wmts:{format:'image/png',defaultStyle:'default',timeIdentifier:'Time',links:[{matrixSet:'regional',limits:[]}]}};
 const wmtsService={...service,layers:[wmtsLayer],wmts:{matrixSets:[{id:'regional',crs:'EPSG:4326',declaredCrs:'CRS:84',matrices:[matrix,{...matrix,id:'fine-grid',scaleDenominator:matrix.scaleDenominator/10,matrixWidth:40,matrixHeight:40}]}]}};
 mapRequest.mockImplementation(async op=>op==='services'?[wmtsService]:{id:'wmts-image'});const user=userEvent.setup(),saved=vi.fn();
 render(<MapServiceDialog areaBounds={[0,0,4,2]} onClose={()=>{}} onSaved={saved}/>);
 await screen.findByRole('combobox',{name:'Map layer'});await user.click(screen.getByRole('combobox',{name:'Map layer'}));await user.click(await screen.findByRole('option',{name:'Land imagery'}));
 expect(screen.queryByRole('combobox',{name:'Image size'})).toBeNull();await user.click(screen.getByRole('combobox',{name:'Tile level'}));await user.click(await screen.findByRole('option',{name:'coarse-grid · 4 × 2 px'}));
 await user.click(screen.getByRole('button',{name:'Save map image'}));expect(mapRequest).toHaveBeenCalledWith('get',{serviceId:id,layerName:'land',style:'default',time:'2025-06-27',bounds:[0,0,4,2],width:4,height:2,areaGeometry:null,tileMatrixSet:'regional',tileMatrix:'coarse-grid'});expect(saved).toHaveBeenCalledWith({id:'wmts-image'});
});
it('the service type selector fills the WMTS example without issuing a request',async()=>{
 mapRequest.mockResolvedValue([]);const user=userEvent.setup();render(<MapServiceDialog onClose={()=>{}} onSaved={()=>{}}/>);await screen.findByRole('combobox',{name:'Map service type'});
 await user.click(screen.getByRole('combobox',{name:'Map service type'}));await user.click(await screen.findByRole('option',{name:'WMTS',exact:true}));await user.click(screen.getByRole('button',{name:'Use NASA GIBS'}));expect(screen.getByRole('textbox',{name:'WMTS endpoint URL'}).value).toBe('https://gibs.earthdata.nasa.gov/wmts/epsg4326/best/wmts.cgi');expect(mapRequest.mock.calls.every(([op])=>op==='services')).toBe(true);
});
it('shows discovery exclusions only on demand while usable layers remain selectable',async()=>{
 const matrix={id:'coarse',scaleDenominator:(Math.PI*6378137/180)/.00028,topLeft:[0,4],tileWidth:2,tileHeight:2,matrixWidth:4,matrixHeight:4};
 const wmtsService={...service,layers:[{...layer,styles:['default'],wmts:{format:'image/png',defaultStyle:'default',timeIdentifier:'Time',links:[{matrixSet:'regional',limits:[]}]}}],wmts:{matrixSets:[{id:'regional',crs:'EPSG:4326',declaredCrs:'CRS:84',matrices:[matrix]}],excludedLayers:[{name:'Bad_Grid_Layer',reason:'Declared tile limits exceed every compatible matrix set'}]}};
 mapRequest.mockImplementation(async op=>op==='services'?[wmtsService]:{id:'wmts-image'});const user=userEvent.setup();render(<MapServiceDialog areaBounds={[0,0,4,2]} onClose={()=>{}} onSaved={()=>{}}/>);
 const notes=await screen.findByRole('button',{name:'Service compatibility notes · 1'});expect(notes.getAttribute('aria-expanded')).toBe('false');expect(screen.queryByText('Bad Grid Layer')).toBeNull();
 await user.click(notes);expect(await screen.findByText('Bad Grid Layer')).toBeTruthy();expect(screen.getByText('Declared tile limits exceed every compatible matrix set')).toBeTruthy();
 await user.click(screen.getByRole('combobox',{name:'Map layer'}));expect(screen.queryByRole('option',{name:'Bad Grid Layer'})).toBeNull();await user.click(await screen.findByRole('option',{name:'Land imagery'}));expect(screen.getByRole('button',{name:'Save map image'}).disabled).toBe(false);
});
