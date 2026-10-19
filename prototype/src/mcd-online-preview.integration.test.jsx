import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {readFileSync} from 'node:fs';
import path from 'node:path';
import additionalItems from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-items.json' with {type:'json'};
import additionalCollections from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-collections.json' with {type:'json'};
import catalog from './public-dataset-next-ten.fixture.json' with {type:'json'};
import {NEXT_PC_PRODUCTS,nextOverviewPolicy,isMcdOverviewPolicy} from './next-public-products.js';
import {createProductRasterLoader,createProductRasterPreviewSource,validateProductRasterImages} from './public-dataset-preview-source.js';

const transport=vi.hoisted(()=>vi.fn());
vi.mock('geotiff',async original=>({...await original(),fromCustomClient:transport}));
const pin='a'.repeat(64);
beforeEach(()=>{transport.mockReset();delete window.__TAURI__;});
afterEach(()=>{delete window.__TAURI__;vi.restoreAllMocks();});

// Contract/image metadata scaffolding mirrors the observed author files. These
// synthetic objects do not assert a complete-file digest or visible map pixels.
function selection(key) {
  const record=additionalItems.find(r=>r.datasetId==='modis-more'), raw=record.document;
  const collection=additionalCollections.find(r=>r.datasetId==='modis-more').document;
  const spec=NEXT_PC_PRODUCTS['modis-more'];
  const dataset={...structuredClone(catalog.catalog.find(d=>d.id==='noaa-nclimgrid')),
    id:'modis-more',title:spec.title,version:spec.version,years:[2020],defaultYear:2020,
    months:[9],defaultMonth:9,discovery:'stac-items',license:collection.license,
    layers:spec.keys.map(id=>({id,title:id,unit:spec.units[id],scale:1,offset:0,
      ...(spec.nodata[id]===null?{}:{nodata:spec.nodata[id]}),seasons:[],valueInterpretation:'Original values'})),defaultLayer:'Burn_Date'};
  const proof={schemaVersion:2,dataset,request:{datasetId:'modis-more',year:2020,month:9,layer:key,bounds:[114.12,34.92,114.14,34.94]},
    layer:dataset.layers.find(l=>l.id===key),sourceId:raw.id,sourceTitle:`${spec.title} · ${raw.id}`,
    sourceBounds:structuredClone(raw.bbox),declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  const assets=Object.entries(raw.assets).map(([id,metadata])=>({key:id,title:metadata.title||id,href:metadata.href,
    mediaType:metadata.type,roles:structuredClone(metadata.roles),metadata:structuredClone(metadata),eligible:id===key,
    reason:id===key?null:'Unselected original',...(id===key?{originalHref:metadata.href,transportAdapter:'planetary-anonymous-sas'}:{})}));
  const item={id:pin,itemId:raw.id,collectionId:raw.collection,datetime:null,startDatetime:raw.properties.start_datetime,
    endDatetime:raw.properties.end_datetime,bbox:structuredClone(raw.bbox),geometry:structuredClone(raw.geometry),
    properties:structuredClone(raw.properties),assets,provenance:{documentUrl:record.url,collection:structuredClone(collection),
      reviewedDataset:proof,assetTransports:[{assetKey:key,originalHref:raw.assets[key].href,href:raw.assets[key].href,adapter:'planetary-anonymous-sas'}]}};
  return {dataset,item,asset:assets.find(a=>a.key===key)};
}
const geoKeys=[1,1,0,18,1024,0,1,1,1025,0,1,1,1026,34737,8,0,2048,0,1,32767,2049,34737,153,8,
  2050,0,1,32767,2054,0,1,9102,2056,0,1,32767,2057,34736,1,3,2058,34736,1,4,2061,34736,1,5,
  3072,0,1,32767,3074,0,1,32767,3075,0,1,24,3076,0,1,9001,3082,34736,1,1,3083,34736,1,2,3088,34736,1,0];
const ascii='unnamed|GCS Name = Unknown datum based upon the custom spheroid|Datum = Not specified (based on custom spheroid)|Ellipsoid = Custom spheroid|Primem = Greenwich||\0';
const scale=[463.3127165283333,463.3127165279167,0], tiepoint=[0,0,0,10007554.677014,4447802.078664,0];
function imagesFor(key) {
  return [2400,1200,600,300].map((edge,index)=>{
    const values={BitsPerSample:[key==='Burn_Date'?16:8],SampleFormat:[key==='Burn_Date'?2:1],Compression:8,
      PhotometricInterpretation:1,SamplesPerPixel:1,PlanarConfiguration:1,Predictor:1,NewSubfileType:index===0?0:1,
      ...(key==='Burn_Date'?{GDAL_NODATA:'-1\0'}:{}),...(index===0?{GeoKeyDirectory:[...geoKeys],
        GeoDoubleParams:[0,0,0,6371007.181,6371007.181,0],GeoAsciiParams:ascii,ModelPixelScale:[...scale],ModelTiepoint:[...tiepoint]}:{})};
    const keys=index===0?{GTModelTypeGeoKey:1,GTRasterTypeGeoKey:1,ProjectedCSTypeGeoKey:32767,GeographicTypeGeoKey:32767,
      ProjCoordTransGeoKey:24,ProjLinearUnitsGeoKey:9001,GeogSemiMajorAxisGeoKey:6371007.181,GeogSemiMinorAxisGeoKey:6371007.181,
      ProjCenterLongGeoKey:0,ProjFalseEastingGeoKey:0,ProjFalseNorthingGeoKey:0}:null;
    return {values,keys,getWidth:()=>edge,getHeight:()=>edge,getSamplesPerPixel:()=>1,getTileWidth:()=>512,getTileHeight:()=>512,
      getBytesPerPixel:()=>key==='Burn_Date'?2:1,getGDALNoData:()=>key==='Burn_Date'?-1:null,getGDALMetadata:()=>null,
      getGeoKeys:()=>keys,fileDirectory:{getValue:name=>values[name]},getOrigin:()=>tiepoint.slice(3),
      getResolution:reference=>[scale[0]*2400/edge,-scale[1]*2400/edge,0],
      getBoundingBox:()=>[tiepoint[3],tiepoint[4]-2400*scale[1],tiepoint[3]+2400*scale[0],tiepoint[4]]};
  });
}
function policyFor(key) {const {dataset,item,asset}=selection(key);return nextOverviewPolicy(dataset,asset,item);}
function serve(images) {transport.mockResolvedValue({getImageCount:async()=>images.length,getImage:async index=>images[index]});}

it.each(['Burn_Date','QA'])('creates the scoped %s view on its actual metre grid without a fabricated EPSG or whole hash',async key=>{
  const policy=policyFor(key);expect(isMcdOverviewPolicy(policy,{snapshotId:pin,assetKey:key})).toBe(true);
  serve(imagesFor(key));
  const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:key,overviewPolicy:policy,displayRange:key==='Burn_Date'?[-2,366]:[0,255]});
  try {
    const view=await preview.source.getView();
    expect(view.projection.getCode()).toBe('MODIS:Sinusoidal');
    expect(view.extent).toEqual([tiepoint[3],tiepoint[4]-2400*scale[1],tiepoint[3]+2400*scale[0],tiepoint[4]]);
    expect(preview.getMetadata()).toMatchObject({crs:'MODIS:Sinusoidal',width:2400,height:2400,bands:1,
      nodata:key==='Burn_Date'?-1:null,hasMask:false,pixelInterpretation:'area',wholeFileVerified:false});
    expect(preview.source.getTileGrid().getResolutions()).toHaveLength(4);
    expect(preview.source.bandCount).toBe(key==='Burn_Date'?2:1);
    expect(preview.getMetadata()).not.toHaveProperty('sha256');
  } finally {preview.dispose();}
});

it('issues no custom-grid token for an unselected, changed or unpinned publisher source',()=>{
  const original=selection('QA');expect(policyFor('QA')).toBeDefined();
  const changes=[v=>v.item.id='../untrusted',v=>v.asset.eligible=false,v=>v.asset.reason='unsupported',
    v=>v.item.provenance.reviewedDataset.request.month=10,v=>v.asset.href+='?sig=untrusted',
    v=>v.item.properties['modis:horizontal-tile']=28,v=>v.item.provenance.reviewedDataset.sourceId+='other',
    v=>v.item.assets.push(structuredClone(v.asset))];
  for (const change of changes) {const value=structuredClone(original);change(value);expect(nextOverviewPolicy(value.dataset,value.asset,value.item)).toBeUndefined();}
  const policy=policyFor('QA');
  expect(()=>createProductRasterPreviewSource({snapshotId:'b'.repeat(64),assetKey:'QA',overviewPolicy:policy})).toThrow('supported north-up');
  expect(()=>createProductRasterPreviewSource({snapshotId:pin,assetKey:'Burn_Date',overviewPolicy:policy})).toThrow('supported north-up');
  expect(()=>createProductRasterPreviewSource({snapshotId:pin,assetKey:'QA',overviewPolicy:{...policy}})).toThrow('supported north-up');
});

it.each(['Burn_Date','QA'])('rejects altered %s sphere, keys, grid, type, NoData, codec and mask declarations',key=>{
  const policy=policyFor(key);
  expect(()=>validateProductRasterImages([imagesFor(key)])).toThrow('supported north-up');
  const changes=[v=>v[0].values.GeoDoubleParams[3]=6378137,v=>v[0].values.GeoKeyDirectory[55]=9102,
    v=>v[0].values.GeoAsciiParams='guessed MODIS',v=>v[0].keys.ProjCoordTransGeoKey=11,
    v=>v[0].values.ModelPixelScale[0]+=0.001,v=>v[0].values.ModelTiepoint[3]+=30,
    v=>v[1].values.SampleFormat=[3],v=>v[2].values.BitsPerSample=[32],v=>v[0].values.GDAL_NODATA='0\0',
    v=>v[1].values.PhotometricInterpretation=3,v=>v[2].values.Compression=5,
    v=>v[3].values.NewSubfileType=4,v=>v[1].values.ModelPixelScale=[1,1,0],v=>v.pop()];
  for (const change of changes) {const images=imagesFor(key);change(images);expect(()=>validateProductRasterImages([images],policy)).toThrow();}
  expect(()=>validateProductRasterImages([imagesFor(key==='QA'?'Burn_Date':'QA')],policy)).toThrow();
});

it('keeps the scoped byte total coherent across logical range requests without hashing partial responses',async()=>{
  const controller=new AbortController();let total=811891;
  const fetcher=vi.fn(async()=>new Response(new Uint8Array([1,2]),{status:206,headers:{'Content-Range':`bytes 0-1/${total}`,'Content-Length':'2'}}));
  const loader=createProductRasterLoader('http://local/asset',controller.signal,fetcher,undefined,811891);
  expect((await loader('http://local/asset',{Range:'bytes=0-1'})).status).toBe(206);
  total++;
  await expect(loader('http://local/asset',{Range:'bytes=0-1'})).rejects.toThrow('requested original byte range');
});

const originalDir=process.env.GEOD_MCD_PREVIEW_ORIGINAL_DIR;
it.skipIf(!originalDir)('configures actual independently obtained MCD TIFF metadata and OL views without HTTP or full-preview hash claims',async()=>{
  const {fromArrayBuffer}=await import('geotiff');
  for (const key of ['Burn_Date','QA']) {
    const bytes=readFileSync(path.join(originalDir,`modis-more-${key}.tif`));
    const tif=await fromArrayBuffer(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength));
    const images=await Promise.all(Array.from({length:await tif.getImageCount()},(_,index)=>tif.getImage(index)));
    serve(images);const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:key,overviewPolicy:policyFor(key)});
    try {const view=await preview.source.getView();expect(view.projection.getCode()).toBe('MODIS:Sinusoidal');expect(preview.getMetadata().wholeFileVerified).toBe(false);}
    finally {preview.dispose();}
  }
});
