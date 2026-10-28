import test from 'node:test';
import assert from 'node:assert/strict';
import {failureDiagnostics} from './usage-diagnostics.js';
import { readFileSync } from 'node:fs';
import { publicDatasetRequest, validatePublicDataset, validateDatasetRequest, validatePublicDatasetPlan, validatePublicDatasetConnection, validatePublicDatasetItem, publicDatasetAssetLayer, publicDatasetPreviewRange } from './public-dataset-client.js';
import { deafricaDatasets, deafricaCases } from './public-dataset-deafrica-fixtures.js';
import { worldcerealDataset, worldcerealRequest, worldcerealPlan, worldcerealConnection, worldcerealSnapshot } from './public-dataset-worldcereal-fixtures.js';
import { validateStacSearchPage, validateStacSnapshot, validateStacInspection, validateStacPixel } from './stac-client.js';
import { publicDataset, datasetPayload, datasetPlan, datasetConnected, datasetStacConnection, catalogDataset, catalogPayload, catalogPlan, catalogConnected, catalogPage, catalogSnapshot } from './public-dataset-fixtures.js';

test('real request boundaries distinguish transport, JSON decoding and product validation without exposing upstream text',async()=>{
  const previousFetch=globalThis.fetch,previousWindow=globalThis.window;
  try{
    delete globalThis.window;
    for(const [fetcher,stage,status,reason] of [
      [async()=>{throw new TypeError('Failed to fetch https://private.invalid/path');},'service_request','unknown','network'],
      [async()=>({status:503,ok:false,json:async()=>{throw new SyntaxError('private response');}}),'response_decode','503','upstream_unavailable'],
      [async()=>({status:403,ok:false,json:async()=>({error:'Access denied for a private resource'})}),'service_request','403','permission_denied'],
      [async()=>({status:200,ok:true,json:async()=>({...catalogConnected,searches:[]})}),'response_validation','unknown','validation'],
    ]){
      globalThis.fetch=fetcher;
      await assert.rejects(publicDatasetRequest('connect',catalogPayload),error=>{
        const details=failureDiagnostics(error);
        assert.equal(details.stage,stage);assert.equal(details.httpStatus,status);assert.equal(details.reason,reason);assert.equal(details.diagnosticOrigin,'explicit');
        assert.ok(!JSON.stringify(details).includes('private'));return true;
      });
    }
    globalThis.window={__TAURI__:{core:{invoke:async()=>{throw 'Native connector rejected this request';}}}};
    await assert.rejects(publicDatasetRequest('connect',catalogPayload),error=>failureDiagnostics(error).stage==='service_request');
  }finally{globalThis.fetch=previousFetch;if(previousWindow===undefined)delete globalThis.window;else globalThis.window=previousWindow;}
});

test('FNF display stretch binds the publisher category range without treating other UInt8 products as classes',()=>{
  const dataset={id:'jaxa-palsar-fnf'},asset={key:'FNF',metadata:{'je:rasters':{dn:{data_type:'uint8',min:1,max:4,nodata:0}}}};
  const original=structuredClone(asset);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[1,4]);
  assert.deepEqual(asset,original);
  assert.equal(publicDatasetPreviewRange({id:'jaxa-sgli-chla'},asset),undefined);
  for(const patch of [{data_type:'uint16'},{min:0},{max:255},{nodata:255}]) {
    const copy=structuredClone(asset);Object.assign(copy.metadata['je:rasters'].dn,patch);
    assert.equal(publicDatasetPreviewRange(dataset,copy),undefined);
  }
  assert.equal(publicDatasetPreviewRange(dataset,{...asset,key:'raster'}),undefined);
});

test('GSMaP grayscale range uses its exact publisher Float32 rate range without changing physical values',()=>{
  const dataset={id:'jaxa-gsmap-rainfall'},raster={dn:{data_type:'float32',min:0,max:13.173989295959473,nodata:-999.9},dn2value:{slope:1,offset:0},value:{unit:'mm/hr'}};
  const asset={key:'PRECIP',metadata:{'je:rasters':raster}},original=structuredClone(asset);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[0,raster.dn.max]);
  assert.deepEqual(asset,original);
  assert.equal(publicDatasetPreviewRange({id:'chirps'},asset),undefined);
  assert.equal(publicDatasetPreviewRange(dataset,{...asset,key:'raster'}),undefined);
  for(const patch of [{data_type:'uint16'},{min:-4},{max:Infinity},{max:0},{nodata:0}]) {
    const changed=structuredClone(asset);Object.assign(changed.metadata['je:rasters'].dn,patch);
    assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  }
  const changed=structuredClone(asset);changed.metadata['je:rasters'].dn2value.slope=720;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
});

test('HRLULC grayscale stretch keeps original Japan class codes and rejects a changed legend or missing-value declaration',()=>{
  const items=JSON.parse(readFileSync(new URL('../../crates/geod-runtime/src/datasets/jaxa-extra-reviewed-items.json',import.meta.url),'utf8'));
  const metadata=items.find(item=>item.datasetId==='jaxa-hrlulc-japan').document.assets.HRLULC;
  const dataset={id:'jaxa-hrlulc-japan'},asset={key:'HRLULC',metadata},original=structuredClone(asset);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[1,15]);
  assert.deepEqual(asset,original);
  assert.equal(publicDatasetPreviewRange({id:'jaxa-palsar-fnf'},asset),undefined);
  const changed=structuredClone(asset); changed.metadata['classification:classes'][15].value=14;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  changed.metadata=structuredClone(metadata); changed.metadata['je:rasters'].dn.nodata=255;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  changed.metadata=structuredClone(metadata); changed.metadata['classification:classes'][0].nodata=false;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
});

test('WPM multispectral selection never includes PAN, quicklook or another processing layer',()=>{
  const layer={id:'wpm-ms',unit:'raw DN'},dataset={id:'cbers-wpm',layers:[layer]};
  for(const key of ['BAND1','BAND2','BAND3','BAND4']) assert.equal(publicDatasetAssetLayer(dataset,{layer:'wpm-ms'},key),layer);
  for(const key of ['BAND0','thumbnail','rgb','BAND5','raster']) assert.equal(publicDatasetAssetLayer(dataset,{layer:'wpm-ms'},key),null);
  assert.equal(publicDatasetAssetLayer(dataset,{layer:'cbers4a-mux'},'BAND1'),null);
});

test('AMAZONIA WFI selects independent DN originals and rejects thumbnails, metadata and a different processing layer',()=>{
  const layer={id:'wfi-ms',unit:'raw DN'},dataset={id:'amazonia-wfi-dn',layers:[layer]};
  for(const key of ['BAND1','BAND2','BAND3','BAND4']) assert.equal(publicDatasetAssetLayer(dataset,{layer:'wfi-ms'},key),layer);
  for(const key of ['thumbnail','BAND1_xml','LEFT_BAND1_xml','rgb','raster','CMASK']) assert.equal(publicDatasetAssetLayer(dataset,{layer:'wfi-ms'},key),null);
  assert.equal(publicDatasetAssetLayer(dataset,{layer:'surface-reflectance'},'BAND1'),null);
});

test('WFI contrast stays bound to the exact original DN collection and does not apply conflicting reflectance metadata',()=>{
  const dataset={id:'amazonia-wfi-dn',layers:[{id:'wfi-ms'}]},collection='AMZ1-WFI-L4-DN-1';
  const item={itemId:'AMAZONIA_1_WFI_20261012_040_018_L4',collectionId:collection,provenance:{search:{collectionId:collection},collection:{id:collection}}};
  for(const [key,common_name] of Object.entries({BAND1:'blue',BAND2:'green',BAND3:'red',BAND4:'nir'})) {
    const href=`https://data.inpe.br/bdc/data/AMZ1-WFI-L4-DN/2026_10/original/${item.itemId}_${key}.tif`;
    const asset={key,href,roles:['data'],metadata:{href,'eo:bands':[{name:key,common_name,data_type:'int16',resolution_x:64,resolution_y:64,nodata:-9999,scale:0.0001,max:10000}]}};
    const original=JSON.stringify({asset,item});
    assert.deepEqual(publicDatasetPreviewRange(dataset,asset,item),[0,1023]);
    assert.equal(JSON.stringify({asset,item}),original);
    assert.equal(publicDatasetPreviewRange(dataset,asset),undefined);
    for(const changed of [
      {...item,collectionId:'AMZ1-WFI-L4-SR-1'},
      {...item,itemId:'AMAZONIA_1_WFI_20261012_040_018_L2'},
      {...item,provenance:{...item.provenance,search:{collectionId:'AMZ1-WFI-L4-SR-1'}}},
      {...item,provenance:{...item.provenance,collection:{id:'another'}}},
    ]) assert.equal(publicDatasetPreviewRange(dataset,asset,changed),undefined);
    for(const changed of [
      {...asset,key:'thumbnail'}, {...asset,roles:['thumbnail']},
      {...asset,href:href.replace('L4-DN','L4-SR')},
      {...asset,metadata:{...asset.metadata,href:'https://other.example/original.tif'}},
      {...asset,metadata:{...asset.metadata,'eo:bands':[{...asset.metadata['eo:bands'][0],resolution_x:8}]}},
      {...asset,metadata:{...asset.metadata,'eo:bands':[{...asset.metadata['eo:bands'][0],common_name:'pan'}]}},
    ]) assert.equal(publicDatasetPreviewRange(dataset,changed,item),undefined);
  }
});

test('SWR preview uses publisher DN range without calibrating samples or widening the half-month observation period',()=>{
  const items=JSON.parse(readFileSync(new URL('../../crates/geod-runtime/src/datasets/jaxa-extra-reviewed-items.json',import.meta.url),'utf8'));
  const item=items.find(item=>item.datasetId==='jaxa-modis-swr'),metadata=item.document.assets.swr;
  const asset={key:'swr',metadata},dataset={id:'jaxa-modis-swr'},original=structuredClone(item);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[0,metadata['je:rasters'].dn.max]);
  assert.equal(item.document.properties.end_datetime,'2026-09-15T23:59:59.999999Z');
  assert.deepEqual(item,original);
  for(const patch of [{data_type:'float32'},{max:65535},{nodata:0}]) {
    const changed=structuredClone(asset);Object.assign(changed.metadata['je:rasters'].dn,patch);
    assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  }
  const changed=structuredClone(asset);changed.metadata['je:rasters'].dn2value.slope=1;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
});

test('SST display keeps the published DN and thermal calibration separate, and never includes mission error codes in its stretch',()=>{
  const items=JSON.parse(readFileSync(new URL('../../crates/geod-runtime/src/datasets/jaxa-more-reviewed-items.json',import.meta.url),'utf8'));
  const item=items.find(entry=>entry.document.bbox[0]===90 && entry.document.bbox[1]===0);
  const dataset={id:'jaxa-sgli-sst',layers:[{id:'sst'}]},asset={key:'SST',metadata:item.document.assets.SST};
  const unchanged=structuredClone(item),dn=asset.metadata['je:rasters'].dn;
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[dn.min,dn.max]);
  assert.equal(publicDatasetAssetLayer(dataset,{layer:'sst'},'SST'),dataset.layers[0]);
  assert.deepEqual(item,unchanged);
  assert.equal(publicDatasetPreviewRange({id:'jaxa-sgli-lst'},asset),undefined);
  for(const patch of [{data_type:'float32'},{min:-1},{max:65532},{nodata:0}]) {
    const changed=structuredClone(asset);Object.assign(changed.metadata['je:rasters'].dn,patch);
    assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  }
  const changed=structuredClone(asset);changed.metadata['je:rasters'].dn2value.slope=1;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
});

test('annual nightlight contrast binds the reviewed raw-DN preset without applying its unresolved physical scale',()=>{
  const dataset={id:'openlandmap-nightlights'},preset={schemaVersion:2,dataset,request:{year:2024},layer:{id:'annual-dn',scale:10,offset:0,nodata:-32768}};
  const asset={key:'raster',metadata:{datasetPreset:preset}},unchanged=structuredClone(asset);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[0,200]);
  assert.deepEqual(asset,unchanged);
  const changed=structuredClone(asset);changed.metadata.datasetPreset.layer.scale=0.1;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  changed.metadata.datasetPreset=structuredClone(preset);changed.metadata.datasetPreset.request.year=2005;
  assert.equal(publicDatasetPreviewRange(dataset,changed),undefined);
  assert.equal(publicDatasetPreviewRange({id:'eog-nightlights'},asset),undefined);
});

test('actual standalone publisher items bind region, thematic season, source footprint and separate reviewed native provenance', () => {
  assert.equal(validatePublicDataset(worldcerealDataset), worldcerealDataset);
  assert.equal(validateDatasetRequest(worldcerealRequest), worldcerealRequest);
  assert.equal(validatePublicDatasetPlan(worldcerealPlan, worldcerealRequest), worldcerealPlan);
  assert.equal(validatePublicDatasetConnection(worldcerealConnection, worldcerealRequest), worldcerealConnection);
  assert.equal(validatePublicDatasetItem(worldcerealSnapshot, worldcerealPlan, worldcerealConnection.stacConnections), worldcerealSnapshot);
  for (const change of [{region:'other'}, {season:'tc-maize-main'}, {month:1}, {year:2022}, {bounds:[0,0,1,1]}]) {
    assert.throws(() => validatePublicDatasetPlan({...worldcerealPlan,request:{...worldcerealRequest,...change}}));
  }
  for (const change of [{region:'other'}, {season:'tc-maize-main'}, {startDatetime:'2022-04-03T00:00:00Z'}, {bounds:[0,0,1,1]}, {stac:{...worldcerealPlan.sources[0].stac,kind:'api'}}]) {
    const plan={...worldcerealPlan,sources:[{...worldcerealPlan.sources[0],...change}]};
    if (change.startDatetime) assert.throws(() => validatePublicDatasetItem(worldcerealSnapshot, plan, worldcerealConnection.stacConnections));
    else assert.throws(() => validatePublicDatasetPlan(plan,worldcerealRequest));
  }
  for (const change of [{reviewedDataset:undefined}, {reviewedDataset:{...worldcerealSnapshot.provenance.reviewedDataset,request:{...worldcerealRequest,region:'other'}}}]) {
    assert.throws(() => validatePublicDatasetItem({...worldcerealSnapshot,provenance:{...worldcerealSnapshot.provenance,...change}},worldcerealPlan,worldcerealConnection.stacConnections));
  }
  assert.equal(worldcerealSnapshot.properties['worldcereal:season'],undefined);
  assert.equal(worldcerealSnapshot.provenance.collection,null);
  assert.equal(worldcerealSnapshot.assets[0].metadata.href,worldcerealSnapshot.assets[0].originalHref);
});

test('regional and season option contracts reject unknown combinations and defaults while old catalogs omit those fields', () => {
  for (const change of [{regions:[{...worldcerealDataset.regions[0],bounds:[0,0,0,1]}]}, {defaultRegion:'unknown'}, {defaultSeason:'unknown'},
    {seasons:[...worldcerealDataset.seasons,...worldcerealDataset.seasons]}, {layers:[{...worldcerealDataset.layers[0],seasons:['unknown']}]}]) {
    assert.throws(() => validatePublicDataset({...worldcerealDataset,...change}));
  }
  assert.equal(validatePublicDataset(publicDataset).regions,undefined);
  assert.throws(() => validatePublicDatasetPlan({...datasetPlan,request:{...datasetPayload,region:'9026'}}));
  assert.throws(() => validatePublicDatasetPlan({...datasetPlan,request:{...datasetPayload,season:'tc-annual'}}));
});

test('original public S3 assets require an exact regional HTTPS receipt and never mutate the publisher asset declaration', () => {
  const original=worldcerealSnapshot.assets[0];
  assert.equal(validateStacSnapshot(worldcerealSnapshot),worldcerealSnapshot);
  for (const change of [{originalHref:original.originalHref.replace('deafrica-input-datasets','other-bucket')},
    {originalHref:original.originalHref+'?signature=secret'}, {href:original.href.replace('af-south-1','us-east-1')},
    {metadata:{...original.metadata,href:original.href}}, {transportAdapter:'generic-s3'}]) {
    assert.throws(()=>validateStacSnapshot({...worldcerealSnapshot,assets:[{...original,...change}]}),/transport/);
  }
  for (const change of [{assetTransports:[]}, {assetTransports:[...worldcerealSnapshot.provenance.assetTransports,...worldcerealSnapshot.provenance.assetTransports]},
    {reviewedDataset:undefined}, {reviewedDataset:{...worldcerealSnapshot.provenance.reviewedDataset,dataset:{id:'other'}}}]) {
    assert.throws(()=>validateStacSnapshot({...worldcerealSnapshot,provenance:{...worldcerealSnapshot.provenance,...change}}),/transport/);
  }
});

test('adding annual and historical item recipes does not loosen WorldCereal UUID, bbox, region or season binding', () => {
  const source=worldcerealPlan.sources[0];
  const shifted={...worldcerealPlan,sources:[{...source,bounds:source.bounds.map((coordinate,index)=>coordinate+(index===0?1e-10:0))}]};
  assert.throws(()=>validatePublicDatasetItem(worldcerealSnapshot,shifted,worldcerealConnection.stacConnections));
  assert.throws(()=>validatePublicDatasetItem({...worldcerealSnapshot,itemId:'client-recipe-key'},worldcerealPlan,worldcerealConnection.stacConnections));
  const dataset={...worldcerealDataset,regions:undefined,seasons:undefined,layers:worldcerealDataset.layers.map(layer=>({...layer,seasons:undefined}))};
  const request={...worldcerealRequest,region:undefined,season:undefined};
  assert.throws(()=>validatePublicDatasetPlan({...worldcerealPlan,dataset,request,sources:[{...source,region:undefined,season:undefined}]}));
  for(const id of ['deafrica-fractional-cover','deafrica-wofs-annual','cgls-landcover','wsf-settlements','gmw-mangroves']) {
    const proof={...worldcerealSnapshot.provenance.reviewedDataset,dataset:{...worldcerealDataset,id}};
    assert.throws(()=>validateStacSnapshot({...worldcerealSnapshot,provenance:{...worldcerealSnapshot.provenance,reviewedDataset:proof}}),/transport/);
  }
});

test('actual annual recipes retain publisher UUIDs, dtr intervals and relative assets separately from their projected-grid candidate key', () => {
  deafricaDatasets.forEach(validatePublicDataset);
  const c=deafricaCases.find(value=>value.datasetId==='deafrica-fractional-cover');
  validatePublicDatasetPlan(c.plan,c.request);validatePublicDatasetConnection(c.connection,c.request);
  assert.equal(validatePublicDatasetItem(c.snapshot,c.plan,c.connection.stacConnections),c.snapshot);
  assert.notEqual(c.snapshot.itemId,c.plan.sources[0].id);
  assert.equal(c.snapshot.startDatetime,c.snapshot.properties['dtr:start_datetime']);
  assert.equal(c.snapshot.endDatetime,c.snapshot.properties['dtr:end_datetime']);
  assert.equal(c.plan.dataset.regions,undefined);assert.equal(c.plan.dataset.seasons,undefined);
  assert.equal(c.snapshot.assets[0].originalHref,undefined);assert.equal(c.snapshot.provenance.assetTransports,undefined);
  const near={...c.snapshot,bbox:c.snapshot.bbox.map((value,index)=>value+(index===0?2e-9:0))};
  assert.equal(validatePublicDatasetItem(near,c.plan,c.connection.stacConnections),near);
  for(const change of [{bbox:c.snapshot.bbox.map((value,index)=>value+(index===0?2e-8:0))},
    {startDatetime:'2025-01-02T00:00:00Z'}, {properties:{...c.snapshot.properties,'dtr:start_datetime':'2025-01-02T00:00:00Z'}}]) {
    assert.throws(()=>validatePublicDatasetItem({...c.snapshot,...change},c.plan,c.connection.stacConnections));
  }
});

test('actual annual asset keys preserve percentage, count and quality semantics independently of the selected green-percentile layer', () => {
  const c=deafricaCases.find(value=>value.datasetId==='deafrica-fractional-cover');
  const measurements=c.snapshot.assets.map(asset=>publicDatasetAssetLayer(c.plan.dataset,c.request,asset.key));
  assert.equal(measurements.length,11);assert.ok(measurements.every(Boolean));
  for(const key of ['pv_pc_10','npv_pc_50','bs_pc_90']) assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,key).unit,'modelled percentage');
  assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'count_valid').unit,'observation count');
  assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'qa').unit,'quality code');
  assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'unknown'),null);
  for(const layer of ['pv_pc_50/override','https://example.com','COUNT_CLEAR','_pv_pc_50']) assert.throws(()=>validateDatasetRequest({...c.request,layer}));
});

for (const c of deafricaCases) test(`actual ${c.datasetId} ${c.request.year} ${c.layer} source binds its original item, period and reviewed selection`, () => {
  assert.equal(validatePublicDatasetPlan(c.plan,c.request),c.plan);
  assert.equal(validatePublicDatasetConnection(c.connection,c.request),c.connection);
  assert.equal(validatePublicDatasetItem(c.snapshot,c.plan,c.connection.stacConnections),c.snapshot);
  assert.throws(()=>validatePublicDatasetItem({...c.snapshot,provenance:{...c.snapshot.provenance,reviewedDataset:{...c.snapshot.provenance.reviewedDataset,request:{...c.request,year:2024}}}},c.plan,c.connection.stacConnections));
  if (!c.datasetId.startsWith('deafrica-')) {
    assert.equal(c.snapshot.itemId,c.plan.sources[0].id);
    assert.deepEqual(c.snapshot.bbox,c.plan.sources[0].bounds);
    assert.throws(()=>validatePublicDatasetItem({...c.snapshot,itemId:'client-key'},c.plan,c.connection.stacConnections));
    assert.throws(()=>validatePublicDatasetItem({...c.snapshot,bbox:c.snapshot.bbox.map((x,i)=>x+(i===0?1e-10:0))},c.plan,c.connection.stacConnections));
  }
});

test('historical S3 transports bind the exact product, year, version, original key and public item URL', () => {
  for (const c of deafricaCases.filter(c=>!c.datasetId.startsWith('deafrica-'))) {
    const asset=c.snapshot.assets.find(a=>a.eligible), proof=c.snapshot.provenance.reviewedDataset;
    assert.equal(validateStacSnapshot(c.snapshot),c.snapshot);
    const alteredAsset=change=>({...c.snapshot,assets:c.snapshot.assets.map(a=>a.key===asset.key?{...asset,...change}:a)});
    for(const change of [{transportAdapter:'generic-s3'},{originalHref:asset.originalHref.replace('deafrica-input-datasets','other-bucket')},
      {href:asset.href.replace('af-south-1','us-east-1')},{metadata:{...asset.metadata,href:asset.href}}]) {
      assert.throws(()=>validateStacSnapshot(alteredAsset(change)),/transport/);
    }
    for(const request of [{...proof.request,year:2021},{...proof.request,layer:'unknown'}]) {
      assert.throws(()=>validateStacSnapshot({...c.snapshot,provenance:{...c.snapshot.provenance,reviewedDataset:{...proof,request}}}),/transport/);
    }
    const url=c.snapshot.provenance.documentUrl.replace('.stac-item.json','-other.stac-item.json');
    assert.throws(()=>validateStacSnapshot({...c.snapshot,provenance:{...c.snapshot.provenance,documentUrl:url,documentRequest:{...c.snapshot.provenance.documentRequest,url}}}));
    const expectedKey=c.datasetId==='cgls-landcover'?'permanentwater_cover_fraction':c.datasetId==='wsf-settlements'?`wsf${c.request.year}`:'mangrove';
    assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,expectedKey).id,c.layer);
    assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'unknown'),null);
    if(c.datasetId==='cgls-landcover') assert.equal(c.snapshot.assets.filter(a=>a.eligible).length,1);
  }
});

test('actual file NoData and raw NaN remain separate from publisher preset missing-value declarations', () => {
  for(const layer of ['qa','count_valid']) {
    const c=deafricaCases.find(c=>c.layer===layer);
    assert.equal(c.plan.dataset.layers.find(l=>l.id===layer).nodata,255);
    assert.equal(validateStacInspection(c.inspection,c.job.id).bands[0].nodata,null);
    for(const p of c.pixels) assert.equal(validateStacPixel(p,{id:c.job.id,column:p.column,row:p.row}).noData[0],false);
  }
  const c=deafricaCases.find(c=>c.layer==='frequency');
  assert.equal(validateStacInspection(c.inspection,c.job.id).bands[0].nodata,'NaN');
  const missing=c.pixels.find(p=>p.values[0]==='NaN'),dry=c.pixels.find(p=>p.values[0]===0);
  assert.equal(validateStacPixel(missing,{id:c.job.id,column:missing.column,row:missing.row}).noData[0],true);
  assert.equal(validateStacPixel(dry,{id:c.job.id,column:dry.column,row:dry.row}).noData[0],false);
  assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'count_wet').unit,'observation count');
  assert.equal(publicDatasetAssetLayer(c.plan.dataset,c.request,'frequency').unit,'fraction (0–1)');
});

test('dataset contracts preserve scientific meaning and reject invalid layers, years and credential-bearing links', () => {
  assert.equal(validatePublicDataset(publicDataset), publicDataset);
  assert.equal(validatePublicDataset({ ...publicDataset, layers: [{ ...publicDataset.layers[0], scale: 0.01, offset: -1, nodata: 255 }] }).layers[0].scale, 0.01);
  for (const change of [{ layers: [] }, { defaultLayer: 'rgb' }, { defaultYear: 2026 }, { years: [2021, 2021] }, { licenseUrl: 'https://example.com/license?token=secret' }, { layers: [{ ...publicDataset.layers[0], scale: Infinity }] }]) assert.throws(() => validatePublicDataset({ ...publicDataset, ...change }));
  assert.throws(() => validateDatasetRequest({ ...datasetPayload, url: 'https://example.com/override.tif' }));
  assert.throws(() => validateDatasetRequest({ ...datasetPayload, country: 'CN' }));
});

test('resolved recipes are bound to requested area, year, layer and product, without claiming a download exists', () => {
  assert.equal(validatePublicDatasetPlan(datasetPlan, datasetPayload), datasetPlan);
  assert.equal(validatePublicDatasetPlan({ ...datasetPlan, sources: [] }, datasetPayload).sources.length, 0);
  for (const request of [{ ...datasetPayload, year: 2020 }, { ...datasetPayload, bounds: [10,20,12,21] }, { ...datasetPayload, datasetId: 'worldpop' }, { ...datasetPayload, layer: 'probability' }]) assert.throws(() => validatePublicDatasetPlan(datasetPlan, request));
  assert.throws(() => validatePublicDatasetPlan({ ...datasetPlan, sources: [...datasetPlan.sources, ...datasetPlan.sources] }));
  assert.throws(() => validatePublicDatasetPlan({ ...datasetPlan, sources: [{ ...datasetPlan.sources[0], wcs: { name: 'Other', url: 'https://example.com/wcs' } }] }));
});

test('monthly products bind the checked plan to an allowed month and retain compatibility with unmodified older catalogs', () => {
  const monthly = { ...publicDataset, id: 'chirps', years: [2024], defaultYear: 2024, months: Array.from({length:12}, (_, index) => index + 1), defaultMonth: 1 };
  const payload = { ...datasetPayload, datasetId: 'chirps', year: 2024, month: 1 };
  const plan = { ...datasetPlan, dataset: monthly, request: payload };
  assert.equal(validatePublicDataset(publicDataset), publicDataset);
  assert.equal(validatePublicDataset(monthly), monthly);
  assert.equal(validateDatasetRequest(payload), payload);
  assert.equal(validatePublicDatasetPlan(plan, payload), plan);
  for (const change of [{ months: [0] }, { months: [13] }, { months: [1, 1] }, { months: [1.5] }, { months: '1' }, { defaultMonth: 0 }, { months: [1], defaultMonth: 2 }, { months: undefined, defaultMonth: 1 }]) assert.throws(() => validatePublicDataset({ ...monthly, ...change }));
  for (const month of [0, 13, 1.5, '1']) assert.throws(() => validateDatasetRequest({ ...payload, month }));
  assert.throws(() => validatePublicDatasetPlan(plan, { ...payload, month: 2 }));
  assert.throws(() => validatePublicDatasetPlan({ ...plan, request: { ...payload, month: undefined } }, payload));
  assert.throws(() => validatePublicDatasetPlan({ ...datasetPlan, request: { ...datasetPayload, month: 1 } }));
  assert.throws(() => validatePublicDatasetConnection({ ...datasetConnected, plan }, { ...payload, month: 2 }));
  assert.equal(validatePublicDatasetConnection({ ...datasetConnected, plan }, payload).plan.request.month, 1);
});

test('monthly request parameters travel unchanged through native recipe commands', async () => {
  const previous = globalThis.window, calls = [];
  const dataset = { ...publicDataset, id: 'chirps', years: [2024], defaultYear: 2024, months: [1, 2], defaultMonth: 1 };
  const payload = { ...datasetPayload, datasetId: 'chirps', year: 2024, month: 2 };
  const plan = { ...datasetPlan, dataset, request: payload };
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => { calls.push({ command, args }); return command === 'resolve_public_dataset' ? plan : { ...datasetConnected, plan }; } } } };
  try {
    await publicDatasetRequest('resolve', payload); await publicDatasetRequest('connect', payload);
    assert.deepEqual(calls, [{ command: 'resolve_public_dataset', args: { request: payload } }, { command: 'connect_public_dataset', args: { request: payload } }]);
  } finally { if (previous === undefined) delete globalThis.window; else globalThis.window = previous; }
});

test('controlled catalog recipes bind API identity and the exact area and UTC reference window without altering direct-file contracts', () => {
  assert.equal(validatePublicDataset(catalogDataset), catalogDataset);
  assert.equal(validatePublicDatasetPlan(catalogPlan, catalogPayload), catalogPlan);
  const source = catalogPlan.sources[0], query = source.search;
  for (const change of [{ discovery:'catalog' }, { discovery:'api' }]) assert.throws(() => validatePublicDataset({...catalogDataset,...change}));
  for (const change of [{ bounds:catalogPayload.bounds }, { search:undefined }, { search:{...query,bounds:[12,20,13,21]} }, { search:{...query,limit:100} },
    { search:{...query,datetime:'2026-08-01T00:00:00Z/2026-08-31T23:59:59.999Z'} }, { stac:{...source.stac,kind:'raster'} }]) {
    assert.throws(() => validatePublicDatasetPlan({...catalogPlan,sources:[{...source,...change}]},catalogPayload));
  }
  assert.throws(() => validatePublicDatasetPlan({...datasetPlan,sources:[{...datasetPlan.sources[0],search:query}]}));
  const annual = {...catalogDataset,id:'io-lulc',months:undefined,defaultMonth:undefined,years:[2024],defaultYear:2024};
  const request = {...catalogPayload,datasetId:'io-lulc',year:2024,month:undefined};
  const plan = {...catalogPlan,dataset:annual,request,sources:[{...source,search:{...query,datetime:'2024-01-01T00:00:00Z/2024-12-31T23:59:59.999999Z'}}]};
  assert.equal(validatePublicDatasetPlan(plan,request),plan);
});

test('catalog metadata connection success is separate from page completeness and only current query snapshots are accepted', () => {
  assert.equal(validatePublicDatasetConnection(catalogConnected,catalogPayload),catalogConnected);
  assert.equal(catalogConnected.complete,true); assert.equal(catalogPage.complete,false);
  const search = catalogConnected.searches[0];
  const alterItem = item => ({...catalogConnected,searches:[{...search,page:{...catalogPage,items:[item]}}]});
  for (const change of [{ searches:undefined }, { searches:[{...search,sourceId:'other'}] }, { searches:[{...search,connectionId:'6cb470c6-92da-4e5b-a06c-63f25be71e12'}] },
    { stacConnections:[{...catalogConnected.stacConnections[0],snapshotIds:[catalogSnapshot.id,'f'.repeat(64)]}] }]) assert.throws(() => validatePublicDatasetConnection({...catalogConnected,...change},catalogPayload));
  for (const change of [{bounds:[12,20,13,21]}, {datetime:'2026-08-01T00:00:00Z/2026-08-31T23:59:59.999999Z'}, {limit:100}, {cursor:'previous-query'}]) {
    assert.throws(() => validatePublicDatasetConnection(alterItem({...catalogSnapshot,provenance:{...catalogSnapshot.provenance,search:{...catalogSnapshot.provenance.search,...change}}}),catalogPayload));
  }
  const empty = {...catalogConnected,stacConnections:[{...catalogConnected.stacConnections[0],snapshotIds:[]}],searches:[{...search,page:{items:[],nextCursor:null,complete:true,limitReached:false}}]};
  assert.equal(validatePublicDatasetConnection(empty,catalogPayload),empty);
  assert.throws(() => validatePublicDatasetConnection({...datasetConnected,searches:catalogConnected.searches},datasetPayload));
});

test('connected source receipts cannot silently omit or substitute reviewed files', () => {
  assert.equal(validatePublicDatasetConnection(datasetConnected, datasetPayload), datasetConnected);
  assert.throws(() => validatePublicDatasetConnection({ ...datasetConnected, stacConnections: [] }, datasetPayload));
  assert.throws(() => validatePublicDatasetConnection({ ...datasetConnected, stacConnections: [{ ...datasetStacConnection, url: 'https://example.com/other.tif' }] }, datasetPayload));
  assert.throws(() => validatePublicDatasetConnection({ ...datasetConnected, stacConnections: [{ ...datasetStacConnection, snapshotIds: [] }] }, datasetPayload));
  const failed = { ...datasetConnected, stacConnections: [], failures: [{ sourceId: datasetPlan.sources[0].id, error: 'Provider HTTP 404' }], complete: false };
  assert.equal(validatePublicDatasetConnection(failed, datasetPayload), failed);
  assert.throws(() => validatePublicDatasetConnection({ ...failed, complete: true }, datasetPayload));
  assert.throws(() => validatePublicDatasetConnection({ ...failed, failures: [{ sourceId: 'unknown', error: 'Provider HTTP 404' }] }, datasetPayload));
  assert.throws(() => validatePublicDatasetConnection({ ...failed, stacConnections: datasetConnected.stacConnections }, datasetPayload));
});

test('catalog pagination retains the canonical query while validating actual item area and instant or interval time', () => {
  const request = {...catalogSnapshot.provenance.search,cursor:'opaque-next-page'};
  assert.equal(validateStacSearchPage(catalogPage, request, true), catalogPage);
  const pageWith = change => ({...catalogPage,items:[{...catalogSnapshot,...change}]});
  const interval = {datetime:null,startDatetime:'2026-08-30T00:00:00Z',endDatetime:'2026-09-02T00:00:00Z',temporalStatus:'interval'};
  assert.equal(validateStacSearchPage(pageWith(interval),request,true).items[0].temporalStatus,'interval');
  for (const change of [
    {bbox:null}, {bbox:[12,20,13,21]}, {datetime:'2026-08-31T23:59:59Z'}, {datetime:'2026-10-01T00:00:00Z'},
    {datetime:null,startDatetime:null,endDatetime:null,temporalStatus:'missing'},
    {...interval,startDatetime:'2026-08-01T00:00:00Z',endDatetime:'2026-08-31T23:59:59Z'},
    {...interval,startDatetime:'2026-09-03T00:00:00Z',endDatetime:'2026-09-02T00:00:00Z'},
  ]) assert.throws(()=>validateStacSearchPage(pageWith(change),request,true), /outside the requested area or time/);
  assert.throws(()=>validateStacSearchPage(pageWith({provenance:{...catalogSnapshot.provenance,search:request}}),request,true),/different query/);
  assert.throws(()=>validateStacSearchPage(catalogPage,{...request,bounds:[12,20,13,21]},true),/different query/);
  assert.equal(validateStacSearchPage(pageWith({bbox:null}),request).items[0].bbox,null);
});

test('desktop preset operations use native controlled recipe commands and never generic download commands', async () => {
  const previous = globalThis.window, calls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => { calls.push({ command, args }); return ({ list_public_datasets: [publicDataset], resolve_public_dataset: datasetPlan, connect_public_dataset: datasetConnected })[command]; } } } };
  try {
    await publicDatasetRequest('list'); await publicDatasetRequest('resolve', datasetPayload); await publicDatasetRequest('connect', datasetPayload);
    assert.deepEqual(calls, [{ command: 'list_public_datasets', args: {} }, { command: 'resolve_public_dataset', args: { request: datasetPayload } }, { command: 'connect_public_dataset', args: { request: datasetPayload } }]);
    const abort = new AbortController(); abort.abort(); await assert.rejects(publicDatasetRequest('connect', datasetPayload, abort.signal), { name: 'AbortError' });
    assert.equal(calls.length, 3);
  } finally { if (previous === undefined) delete globalThis.window; else globalThis.window = previous; }
});

test('loopback requests preserve explicit parameters and protect JSON mutations while surfacing provider errors', async () => {
  const previous = globalThis.fetch, calls = [];
  globalThis.fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => url.endsWith('/resolve') ? datasetPlan : url.endsWith('/connect') ? datasetConnected : [publicDataset] }; };
  try {
    await publicDatasetRequest('list'); await publicDatasetRequest('resolve', datasetPayload); await publicDatasetRequest('connect', datasetPayload);
    assert.equal(calls[0].url, 'http://127.0.0.1:4318/datasets'); assert.equal(calls[0].options.method, 'GET');
    assert.equal(calls[1].url, 'http://127.0.0.1:4318/datasets/resolve'); assert.equal(calls[1].options.headers['X-GeoD-Client'], 'geod-global');
    assert.deepEqual(JSON.parse(calls[2].options.body), datasetPayload);
    globalThis.fetch = async () => ({ ok: false, json: async () => ({ error: 'The provider did not publish this country and year.' }) });
    await assert.rejects(publicDatasetRequest('connect', datasetPayload), /did not publish/);
  } finally { globalThis.fetch = previous; }
});
test('WPM display uses its declared ten-bit range without rewriting the contradictory dtype or admitting PAN', () => {
  const dataset={id:'cbers-wpm'},band={name:'BAND1',min:0,max:1024,nodata:0,scale:1,scale_add:null,resolution_x:8,resolution_y:8,data_type:'uint8'};
  const asset={key:'BAND1',metadata:{'eo:bands':[band]}};
  const before=JSON.stringify(asset);
  assert.deepEqual(publicDatasetPreviewRange(dataset,asset),[0,1024]);
  assert.equal(JSON.stringify(asset),before);
  assert.equal(publicDatasetPreviewRange(dataset,{...asset,key:'BAND0'}),undefined);
  assert.equal(publicDatasetPreviewRange({id:'cbers-sr'},asset),undefined);
  for(const change of [{min:1},{max:65535},{nodata:-9999},{scale:0.0001},{scale_add:1},{resolution_x:2}])
    assert.equal(publicDatasetPreviewRange(dataset,{...asset,metadata:{'eo:bands':[{...band,...change}]}}),undefined);
});
