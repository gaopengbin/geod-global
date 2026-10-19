import test from 'node:test';
import assert from 'node:assert/strict';
import itemRecords from '../../crates/geod-runtime/src/datasets/next-planetary-reviewed-items.json' with {type:'json'};
import collectionRecords from '../../crates/geod-runtime/src/datasets/next-planetary-reviewed-collections.json' with {type:'json'};
import ghslProfiles from '../../crates/geod-runtime/src/datasets/next-ghsl-profiles.json' with {type:'json'};
import hydroProfiles from '../../crates/geod-runtime/src/datasets/next-hydro-geoid-reviewed-index.json' with {type:'json'};
import nativeFixture from './public-dataset-next-ten.fixture.json' with {type:'json'};
import {NEXT_PC_PRODUCTS,validNextPlanetarySnapshot} from './next-public-products.js';
import {publicDatasetAssetLayer,publicDatasetPreviewRange,validatePublicDataset,validatePublicDatasetPlan,validatePublicDatasetItem} from './public-dataset-client.js';
import {validateStacSnapshot,validStacUrl} from './stac-client.js';
const clone=structuredClone, receipt='a'.repeat(64), uuid='11111111-1111-4111-8111-111111111111';

function pcCase(record,key) {
  const profile=NEXT_PC_PRODUCTS[record.datasetId],raw=record.document;
  const dataset=clone(nativeFixture.catalog.find(d=>d.id===record.datasetId));
  const request={datasetId:dataset.id,bounds:clone(raw.bbox),year:profile.year,layer:key,...(profile.month==null?{}:{month:profile.month})};
  const proof={schemaVersion:2,dataset,request,layer:dataset.layers.find(v=>v.id===key),sourceId:raw.id,sourceTitle:`${profile.title} · ${raw.id}`,sourceBounds:clone(raw.bbox),declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  const assets=Object.entries(raw.assets).map(([assetKey,metadata])=>({key:assetKey,title:metadata.title||assetKey,href:metadata.href,mediaType:metadata.type||null,roles:metadata.roles||[],metadata:clone(metadata),eligible:assetKey===key,reason:assetKey===key?null:'Unselected original',...(assetKey===key?{originalHref:metadata.href,transportAdapter:'planetary-anonymous-sas'}:{})}));
  const asset=assets.find(a=>a.key===key);
  const item={id:receipt,connectionId:uuid,itemId:raw.id,title:raw.id,collectionId:raw.collection,datetime:raw.properties.datetime??null,startDatetime:raw.properties.start_datetime??null,endDatetime:raw.properties.end_datetime??null,bbox:clone(raw.bbox),geometry:clone(raw.geometry),properties:clone(raw.properties),assets,retrievedAt:'2026-10-14T00:00:00Z',documentSha256:receipt,temporalStatus:raw.properties.datetime?'instant':'interval',warnings:[],provenance:{documentUrl:record.url,metadataDocuments:[{url:record.collectionUrl,sha256:receipt}],search:null,collection:clone(collectionRecords.find(c=>c.datasetId===dataset.id).document),reviewedDataset:proof,assetTransports:[{assetKey:key,originalHref:asset.href,href:asset.href,adapter:asset.transportAdapter}]}};
  return {dataset,request,item,asset};
}
function directCase(profile) {
  const dataset={id:profile.datasetId,title:profile.title,layers:[{id:profile.layer,unit:profile.unit,scale:1,offset:0,...(profile.nodata==null?{}:{nodata:profile.nodata})}]};
  const request={datasetId:dataset.id,layer:profile.layer,...(profile.year==null?{}:{year:profile.year})};
  const proof={schemaVersion:2,dataset,request,layer:dataset.layers[0],sourceId:profile.id||profile.url.split('/').at(-1).replace(/\.tif$/,''),sourceTitle:profile.title,sourceBounds:profile.queryBounds||profile.bounds.map((v,i)=>i===0?Math.max(-180,v):i===1?Math.max(-90,v):i===2?Math.min(180,v):Math.min(90,v)),declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  return {dataset,asset:{key:'raster',href:profile.url,metadata:{kind:'direct-raster',href:profile.url,datasetPreset:proof}}};
}

test('all ten actual native definitions and live resolved plans obey the existing frontend schema',()=>{
  assert.equal(nativeFixture.catalog.length,10);assert.equal(nativeFixture.cases.length,10);
  for(const dataset of nativeFixture.catalog) validatePublicDataset(dataset);
  for(const {request,plan} of nativeFixture.cases) validatePublicDatasetPlan(plan,request);
  const climate=nativeFixture.catalog.find(d=>d.id==='noaa-nclimgrid');
  assert.equal(new URL(climate.licenseUrl).hash,'');
  const changed=clone(climate);changed.licenseUrl+='#Constraints';
  assert.throws(()=>validatePublicDataset(changed));
  assert.equal(new URL(collectionRecords.find(c=>c.datasetId==='noaa-nclimgrid').document.links.find(l=>l.rel==='license').href).hash,'#Constraints');
});

test('four PC product scopes bind actual publisher metadata, selected variables and canonical unsigned originals',()=>{
  let count=0;
  for(const record of itemRecords) for(const key of NEXT_PC_PRODUCTS[record.datasetId].keys) {
    const c=pcCase(record,key);count++;const original=JSON.stringify(c);
    assert.equal(validNextPlanetarySnapshot(c.item,c.asset),true);
    validateStacSnapshot(c.item);
    const raw=record.document,proof=c.item.provenance.reviewedDataset;
    const source={id:raw.id,title:proof.sourceTitle,bounds:clone(raw.bbox),stac:{url:record.url,kind:'item'},
      startDatetime:raw.properties.datetime??raw.properties.start_datetime,endDatetime:raw.properties.datetime??raw.properties.end_datetime};
    validatePublicDatasetItem(c.item,{dataset:c.dataset,request:c.request,sources:[source]},[{id:uuid,url:record.url,snapshotIds:[receipt]}]);
    assert.equal(publicDatasetAssetLayer(c.dataset,c.request,key),c.dataset.layers.find(l=>l.id===key));
    assert.ok(publicDatasetPreviewRange(c.dataset,c.asset,c.item)?.length===2);
    assert.equal(JSON.stringify(c),original);
    for(const wrong of ['thumbnail','visual','raster',...NEXT_PC_PRODUCTS[c.dataset.id].keys].filter(k=>k!==key))
      assert.equal(publicDatasetAssetLayer(c.dataset,c.request,wrong),null);
    const mutations=[
      v=>v.item.provenance.reviewedDataset.request.year++,v=>v.item.provenance.reviewedDataset.request.layer='unselected',
      v=>v.item.provenance.reviewedDataset.sourceId+='-other',v=>v.item.provenance.collection.license='CC0',
      v=>v.item.properties.datetime='2030-01-01T00:00:00Z',v=>v.asset.metadata.roles=['overview'],
      v=>v.asset.href+='?sig=never-persist',v=>v.asset.originalHref+='?sig=never-persist',
      v=>v.asset.href=v.asset.href.replace('.blob.core.windows.net','-other.blob.core.windows.net'),
      v=>v.asset.transportAdapter='custom-sas',v=>v.asset.metadata['raster:bands']=[{data_type:'uint8',nodata:0}],
      v=>v.item.provenance.reviewedDataset.layer.unit='unreviewed unit',v=>v.item.provenance.reviewedDataset.dataset.version='future version',
    ];
    for(const mutate of mutations) {
      const bad=clone(c);mutate(bad);
      assert.equal(validNextPlanetarySnapshot(bad.item,bad.asset),false);
      assert.equal(publicDatasetPreviewRange(bad.dataset,bad.asset,bad.item),undefined);
      assert.throws(()=>validateStacSnapshot(bad.item));
    }
  }
  assert.equal(count,7);
  assert.equal(validStacUrl('https://hreadatasa.blob.core.windows.net/hrea/a.tif?sig=never-persist'),false);
});

test('HREA remains a point reference date while biodiversity and NOAA retain their actual intervals',()=>{
  for(const record of itemRecords) {
    const {item}=pcCase(record,NEXT_PC_PRODUCTS[record.datasetId].keys[0]);
    if(record.datasetId==='pc-hrea') {
      assert.equal(item.temporalStatus,'instant');assert.equal(item.datetime,'2019-12-31T00:00:00Z');
      assert.equal(item.startDatetime,null);assert.equal(item.endDatetime,null);
    } else {
      assert.equal(item.temporalStatus,'interval');assert.equal(item.datetime,null);
      assert.equal(item.startDatetime,record.document.properties.start_datetime);assert.equal(item.endDatetime,record.document.properties.end_datetime);
    }
  }
});

test('GHSL actual unsigned/fractional profiles preserve absent NoData and bind only exact 2020 originals',()=>{
  for(const profile of ghslProfiles.profiles) {
    const c=directCase({...profile,year:2020}),original=JSON.stringify(c);
    assert.ok(publicDatasetPreviewRange(c.dataset,c.asset)?.length===2);
    assert.equal(JSON.stringify(c),original);
    const bad=clone(c);bad.asset.metadata.datasetPreset.layer.nodata=profile.publisherNoData;
    assert.equal(publicDatasetPreviewRange(bad.dataset,bad.asset),undefined);
    const wrong=clone(c);wrong.asset.metadata.datasetPreset.request.year=2025;
    assert.equal(publicDatasetPreviewRange(wrong.dataset,wrong.asset),undefined);
    const other=clone(c);other.asset.href=other.asset.href.replace('/2020/','/2025/');
    assert.equal(publicDatasetPreviewRange(other.dataset,other.asset),undefined);
  }
});

test('header-reviewed hydro/geoid originals use separate scientific units, raw masks and display-only ranges',()=>{
  for(const id of ['hydrosheds-accumulation','etopo-geoid']) for(const key of id==='etopo-geoid'?['geoid']:['acc','aca']) {
    const p=hydroProfiles.originals.find(r=>r.datasetId===id&&r.layer===key);
    const c=directCase({...p,unit:key==='acc'?'cells':key==='aca'?'km²':'m'}),original=JSON.stringify(c);
    assert.ok(publicDatasetPreviewRange(c.dataset,c.asset)?.length===2);
    assert.equal(JSON.stringify(c),original);
    const changed=clone(c);changed.asset.metadata.datasetPreset.layer.nodata=0;
    assert.equal(publicDatasetPreviewRange(changed.dataset,changed.asset),undefined);
    const wrong=clone(c);wrong.asset.metadata.datasetPreset.layer.unit='metres of terrain height';
    assert.equal(publicDatasetPreviewRange(wrong.dataset,wrong.asset),undefined);
    const foreign=clone(c);foreign.asset.href=foreign.asset.href.replace('https://','https://other.');
    assert.equal(publicDatasetPreviewRange(foreign.dataset,foreign.asset),undefined);
  }
});

test('ETH contrast applies only to header-reviewed author originals and never supplies a missing-value mask',()=>{
  const url='https://libdrive.ethz.ch/index.php/s/cO8or7iOe5dT2Rt/download?path=%2F3deg_cogs&files=ETH_GlobalCanopyHeight_10m_2020_N51E003_Map.tif';
  const c=directCase({datasetId:'eth-canopy-height',title:'ETH global canopy height · 2020',layer:'height',unit:'m',nodata:255,year:2020,id:'N51E003',url,bounds:[3,51,6,54]});
  const original=JSON.stringify(c);assert.deepEqual(publicDatasetPreviewRange(c.dataset,c.asset),[0,50]);assert.equal(JSON.stringify(c),original);
  const other=clone(c);other.asset.metadata.datasetPreset.sourceId='N48E003';
  assert.equal(publicDatasetPreviewRange(other.dataset,other.asset),undefined);
  const changed=clone(c);changed.asset.metadata.datasetPreset.layer.nodata=0;
  assert.equal(publicDatasetPreviewRange(changed.dataset,changed.asset),undefined);
});
