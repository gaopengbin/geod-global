import {test} from 'node:test';import assert from 'node:assert/strict';
import {wmtsPlan,wmtsCrs,validateWmtsImage,validateWmtsService} from './wmts-client.js';
const matrix={id:'opaque-level',scaleDenominator:(Math.PI*6378137/180)/0.00028,topLeft:[-180,90],tileWidth:16,tileHeight:16,matrixWidth:32,matrixHeight:16};
test('WMTS services retain bounded discovery exclusions without blocking usable layers or older saved connections',()=>{
 const service={id:'735f227b-5f95-473f-967b-077f3419bc68',name:'Test WMTS',url:'https://maps.example.com/wmts',mapUrl:'https://maps.example.com/wmts',version:'1.0.0',capabilitiesSha256:'a'.repeat(64),connectedAt:'2026-10-02T00:00:00Z',maxWidth:2048,maxHeight:2048,
  wmts:{matrixSets:[{id:'regional',crs:'EPSG:4326',declaredCrs:'CRS:84',matrices:[matrix]}]},
  layers:[{name:'land',title:'Land imagery',crs:'EPSG:4326',styles:['default'],wmts:{format:'image/png',defaultStyle:'default',links:[{matrixSet:'regional',limits:[]}]}}]};
 assert.equal(validateWmtsService(service),service);
 service.wmts.excludedLayers=[{name:'broken-grid',reason:'Declared tile limits exceed every compatible matrix set'}];assert.equal(validateWmtsService(service),service);
 for(const invalid of [{},[{name:'x',reason:''}],Array.from({length:4097},()=>({name:'x',reason:'invalid limits'})),[{name:'x',reason:'bad\nmetadata'}]])assert.throws(()=>validateWmtsService({...service,wmts:{...service.wmts,excludedLayers:invalid}}),/discovery exclusions/);
});
test('WMTS retains opaque identifiers and explicit rectangular tile geometry',()=>{
 const p=wmtsPlan([-125,30,-110,43],'EPSG:4326',matrix);
 assert.deepEqual(p.window,[55,47,15,13]);assert.deepEqual(p.bounds,[-125,30,-110,43]);assert.deepEqual(p.tiles,[{row:2,col:3},{row:2,col:4},{row:3,col:3},{row:3,col:4}]);
 assert.throws(()=>wmtsPlan([-125,30,-110,43],'EPSG:4326',matrix,[{minRow:2,maxRow:2,minCol:3,maxCol:4}]),/limits/);
 assert.equal(wmtsCrs('urn:ogc:def:crs:EPSG:6.18:3:3857'),'EPSG:3857');assert.throws(()=>wmtsCrs('EPSG:3413'),/projection/);
});
test('WMTS Web Mercator has native meter pixels and bounded tile requests',()=>{
 const m={...matrix,scaleDenominator:559082264.0287178/64,topLeft:[-20037508.34278925,20037508.34278925],tileWidth:256,tileHeight:256,matrixWidth:64,matrixHeight:64};
 const p=wmtsPlan([-125,30,-110,43],'EPSG:3857',m);assert.ok(p.extent[0]<-1e7);assert.ok(p.width>680&&p.height>730);assert.ok(p.tiles.length<=16);assert.ok(p.bounds[0]<=-125&&p.bounds[2]>=-110);
 assert.throws(()=>wmtsPlan([-10,-90,10,90],'EPSG:3857',m),/outside Web Mercator/);
 assert.throws(()=>wmtsPlan([-125,30,-110,43],'EPSG:4326',{...matrix,scaleDenominator:matrix.scaleDenominator/100}),/outside|coarser/);
});
test('WMTS pixel windows cover large declared matrix grids beyond u32 without losing integer precision',()=>{
 const m={...matrix,id:'high-resolution',scaleDenominator:matrix.scaleDenominator/2**26,tileWidth:1024,tileHeight:1024,matrixWidth:2**24,matrixHeight:2**24};
 const p=wmtsPlan([-90,0,-90+1e-6,1e-6],'EPSG:4326',m);assert.ok(p.window[0]>2**32);assert.ok(p.window[1]>2**32);assert.ok(p.width<100&&p.height<100);assert.ok(p.window.every(Number.isSafeInteger));
});
test('WMTS geographic bounds tolerate floating point grid edges while native extent is exact',()=>{
 const m={...matrix,id:'1',scaleDenominator:111816452.8057436,tileWidth:512,tileHeight:512,matrixWidth:3,matrixHeight:2};
 const p=wmtsPlan([179.9,0,180,1],'EPSG:4326',m);assert.equal(p.bounds[2],180);assert.ok(p.extent[2]>180&&p.extent[2]<180+1e-9);assert.equal(p.extent[2],m.topLeft[0]+(p.window[0]+p.width)*p.resolution);
 assert.throws(()=>wmtsPlan([179.9,0,180,1],'EPSG:4326',{...m,topLeft:[-179.999,90]}),/outside WGS84/);
});
test('WMTS source receipts bind all tile coordinates and exported extent',()=>{
 const p=wmtsPlan([-125,30,-110,43],'EPSG:4326',matrix),endpoint='https://maps.example.com/wmts/wmts.cgi';
 const source={serviceUrl:endpoint,mapEndpoint:endpoint,version:'1.0.0',layerName:'land',layerTitle:'Land imagery',style:'default',time:'2025-06-27',requestedAt:'2026-10-02T00:00:00Z',requestCrs:'EPSG:4326',selection:'pixel-window-rendered-tiles',capabilitiesSha256:'a'.repeat(64)};
 const wmts={matrixSet:'custom',declaredCrs:'CRS:84',matrix,format:'image/png',timeIdentifier:'Time',requestedBounds:[-125,30,-110,43],pixelWindow:p.window,archiveSha256:'b'.repeat(64),archiveBytes:1000,tiles:p.tiles.map(tile=>{
  const u=new URL(endpoint);for(const[k,v]of Object.entries({SERVICE:'WMTS',REQUEST:'GetTile',VERSION:'1.0.0',LAYER:'land',STYLE:'default',FORMAT:'image/png',TILEMATRIXSET:'custom',TILEMATRIX:'opaque-level',TILEROW:String(tile.row),TILECOL:String(tile.col),Time:'2025-06-27'}))u.searchParams.set(k,v);return{...tile,requestUrl:u.href,bytes:10,sha256:'c'.repeat(64)};
 })};source.wmts=wmts;source.requestUrl=wmts.tiles[0].requestUrl;
 const a={id:'735f227b-5f95-473f-967b-077f3419bc68',name:'Land imagery',bytes:100,sha256:'d'.repeat(64),width:p.width,height:p.height,bounds:p.bounds,imageExtent:p.extent,crs:'EPSG:4326',source};validateWmtsImage(a);
 assert.throws(()=>validateWmtsImage({...a,width:a.width+1}),/grid/);assert.throws(()=>validateWmtsImage({...a,imageExtent:[...a.imageExtent.slice(0,3),44]}),/grid/);
 const changed=structuredClone(a);changed.source.wmts.tiles[1].col++;assert.throws(()=>validateWmtsImage(changed),/receipts/);
 const duplicate=structuredClone(a);duplicate.source.wmts.tiles[0].requestUrl+='&Time=2025-06-27';duplicate.source.requestUrl=duplicate.source.wmts.tiles[0].requestUrl;assert.throws(()=>validateWmtsImage(duplicate),/receipts/);
});
