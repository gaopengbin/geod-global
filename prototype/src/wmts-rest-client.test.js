import {test} from 'node:test';import assert from 'node:assert/strict';
import {wmtsRestTemplate,wmtsRestTileUrl} from './wmts-rest-client.js';
import {wmtsPlan,validateWmtsService,validateWmtsImage} from './wmts-client.js';
const root='https://maps.example.com/wmts/1.0.0/capabilities.xml',template='https://maps.example.com/wmts/tiles/{Style}/{Time}/{TileMatrixSet}/{TileMatrix}/{TileRow}/{TileCol}.png';
const matrix={id:'level:a/b',scaleDenominator:(Math.PI*6378137/180)/.00028,topLeft:[-180,90],tileWidth:16,tileHeight:16,matrixWidth:32,matrixHeight:16};
function fixture(){
 const bounds=[-125,30,-110,43],p=wmtsPlan(bounds,'EPSG:4326',matrix);
 const source={serviceUrl:root,mapEndpoint:root,version:'1.0.0',layerName:'land',layerTitle:'Land',style:'space style',time:'2025-06-27',requestedAt:'2026-10-03T00:00:00Z',requestCrs:'EPSG:4326',selection:'pixel-window-rendered-tiles',capabilitiesSha256:'a'.repeat(64)};
 const w={resourceUrl:template,matrixSet:'regional',declaredCrs:'CRS:84',matrix,format:'image/png',timeIdentifier:'Time',requestedBounds:bounds,pixelWindow:p.window,archiveSha256:'b'.repeat(64),archiveBytes:1000};
 w.tiles=p.tiles.map(t=>({...t,requestUrl:wmtsRestTileUrl(source,w,t.row,t.col),bytes:10,sha256:'c'.repeat(64)}));source.wmts=w;source.requestUrl=w.tiles[0].requestUrl;
 const asset={id:'735f227b-5f95-473f-967b-077f3419bc68',name:'Land',bytes:100,sha256:'d'.repeat(64),width:p.width,height:p.height,bounds:p.bounds,imageExtent:p.extent,crs:'EPSG:4326',source};
 const service={id:asset.id,name:'REST',url:root,mapUrl:root,version:'1.0.0',capabilitiesSha256:'a'.repeat(64),connectedAt:source.requestedAt,maxWidth:2048,maxHeight:2048,
  wmts:{restOnly:true,capabilitiesDocument:true,matrixSets:[{id:'regional',crs:'EPSG:4326',declaredCrs:'CRS:84',matrices:[matrix]}]},
  layers:[{name:'land',title:'Land',crs:'EPSG:4326',styles:['space style'],time:{default:'2025-06-27',values:'2025-06-27'},wmts:{resourceUrl:template,format:'image/png',defaultStyle:'space style',timeIdentifier:'Time',links:[{matrixSet:'regional',limits:[]}]}}]};
 return{asset,service};
}
test('REST templates keep the advertised origin, exact dimensions and relative path semantics',()=>{
 assert.equal(wmtsRestTemplate('../tiles/{Style}/{Time}/{TileMatrixSet}/{TileMatrix}/{TileRow}/{TileCol}.png',root,'Time'),template);
 for(const raw of [template.replace('maps.example.com','other.example.com'),template+'?token=secret',template.replace('{TileCol}','{TileRow}'),template.replace('{Time}/',''),template.replace('{Time}','{Elevation}'),'https://{Style}.example.com/{Time}/{TileMatrix}/{TileRow}/{TileCol}.png'])assert.throws(()=>wmtsRestTemplate(raw,root,'Time'));
 assert.throws(()=>wmtsRestTemplate(template,root,null),/placeholder/);
});
test('REST request paths encode opaque identifiers without converting them into XYZ zooms',()=>{
 const{asset}=fixture(),url=wmtsRestTileUrl(asset.source,asset.source.wmts,2,3);
 assert.equal(url,'https://maps.example.com/wmts/tiles/space%20style/2025-06-27/regional/level%3Aa%2Fb/2/3.png');
 asset.source.style='..';assert.throws(()=>wmtsRestTileUrl(asset.source,asset.source.wmts,2,3),/identifier/);
});
test('REST-only discovery rejects missing templates and incompatible layer choices',()=>{
 const{service}=fixture();assert.equal(validateWmtsService(service),service);
 for(const mutate of [s=>delete s.layers[0].wmts.resourceUrl,s=>s.wmts.capabilitiesDocument='true',s=>s.mapUrl='https://maps.example.com/wmts/cgi',s=>{s.layers[0].styles.push('other');s.layers[0].wmts.resourceUrl=s.layers[0].wmts.resourceUrl.replace('{Style}','default');}]){const bad=structuredClone(service);mutate(bad);assert.throws(()=>validateWmtsService(bad));}
});
test('REST image receipts bind the exact template, grid, date and selected style',()=>{
 const{asset}=fixture();assert.equal(validateWmtsImage(asset),asset);
 for(const mutate of [a=>a.source.style='other',a=>a.source.time='2025-06-28',a=>delete a.source.wmts.resourceUrl,a=>a.source.wmts.resourceUrl=a.source.wmts.resourceUrl.replace('/tiles/','/different/'),a=>a.source.wmts.tiles[1].requestUrl+='?fake=1']){const bad=structuredClone(asset);mutate(bad);assert.throws(()=>validateWmtsImage(bad));}
});
