import test from 'node:test';
import assert from 'node:assert/strict';
import items from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-items.json' with {type:'json'};
import collections from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-collections.json' with {type:'json'};
import previousNativeCatalog from './public-dataset-next-ten.fixture.json' with {type:'json'};
import {NEXT_PC_PRODUCTS,ADDITIONAL_PC_ORIGINALS,validNextPlanetarySnapshot} from './next-public-products.js';
import {validatePublicDataset,validateDatasetRequest,validatePublicDatasetPlan,validatePublicDatasetItem,publicDatasetAssetLayer,publicDatasetPreviewRange} from './public-dataset-client.js';
import {validateStacSnapshot} from './stac-client.js';
import {SOURCE_DIRECTORY} from './source-directory.js';
import chinese from './locales/public-dataset.zh-CN.js';
import clcdReview from '../../crates/geod-runtime/src/datasets/clcd-reviewed-index.json' with {type:'json'};

const receipt='a'.repeat(64),uuid='11111111-1111-4111-8111-111111111111';
// Contract test scaffolding; this is not a native download/UI acceptance receipt.
function contractCase(record,key) {
  const p=NEXT_PC_PRODUCTS[record.datasetId], raw=record.document;
  const c=collections.find(c=>c.datasetId===record.datasetId).document;
  const dataset={...structuredClone(previousNativeCatalog.catalog.find(d=>d.id==='noaa-nclimgrid')),
    id:record.datasetId,title:p.title,version:p.version,category:record.datasetId==='planetary-more'?'ocean':'land-cover',
    years:[2020],defaultYear:2020,months:[9],defaultMonth:9,license:c.license,
    licenseUrl:c.links.find(l=>l.rel==='license').href,layers:p.keys.map(k=>({id:k,title:k,unit:p.units[k],scale:p.scales[k],offset:0,...(p.nodata[k]==null?{}:{nodata:p.nodata[k]}),seasons:[],valueInterpretation:'Original publisher values; display-only contrast.'})),defaultLayer:p.keys[0]};
  const request={datasetId:dataset.id,bounds:[114.12,34.92,114.14,34.94],year:2020,month:9,layer:key};
  const source={id:raw.id,title:`${p.title} · ${raw.id}`,bounds:structuredClone(raw.bbox),stac:{name:p.title,url:record.url,kind:'item'},startDatetime:raw.properties.start_datetime,endDatetime:raw.properties.end_datetime};
  const proof={schemaVersion:2,dataset,request,layer:dataset.layers.find(l=>l.id===key),sourceId:raw.id,sourceTitle:source.title,sourceBounds:structuredClone(raw.bbox),declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'};
  const assets=Object.entries(raw.assets).map(([k,metadata])=>({key:k,title:metadata.title||k,href:metadata.href,mediaType:metadata.type||null,roles:metadata.roles||[],metadata:structuredClone(metadata),eligible:k===key,reason:k===key?null:'Unselected original',...(k===key?{originalHref:metadata.href,transportAdapter:'planetary-anonymous-sas'}:{})}));
  const asset=assets.find(a=>a.key===key);
  const item={id:receipt,connectionId:uuid,itemId:raw.id,title:raw.id,collectionId:raw.collection,datetime:null,startDatetime:raw.properties.start_datetime,endDatetime:raw.properties.end_datetime,bbox:structuredClone(raw.bbox),geometry:structuredClone(raw.geometry),properties:structuredClone(raw.properties),assets,retrievedAt:'2026-10-14T00:00:00Z',documentSha256:receipt,temporalStatus:'interval',warnings:[],provenance:{documentUrl:record.url,metadataDocuments:[{url:record.collectionUrl,sha256:receipt}],search:null,collection:structuredClone(c),reviewedDataset:proof,assetTransports:[{assetKey:key,originalHref:asset.href,href:asset.href,adapter:asset.transportAdapter}]}};
  const plan={dataset,request,sources:[source],warnings:[]};
  return {dataset,request,source,plan,item,asset};
}

test('original directory IDs remain Planned while their finite Explore actions are usable',()=>{
  assert.equal(SOURCE_DIRECTORY.length,133);
  for(const id of ['planetary-more','modis-more']) {
    const source=SOURCE_DIRECTORY.find(s=>s.id===id);
    assert.equal(source.status,'planned');assert.equal(source.partialAction,true);
    assert.deepEqual(source.action,{kind:'dataset',datasetId:id});
    assert.equal(SOURCE_DIRECTORY.filter(s=>s.id===id).length,1);
  }
  const clcd=SOURCE_DIRECTORY.find(s=>s.id==='clcd');
  if(!clcdReview.wholeFileVerified||!clcdReview.publisherMd5Verified||clcdReview.originalSha256==null) {
    assert.equal(clcd.status,'planned');assert.equal(Boolean(clcd.partialAction),false);assert.equal(clcd.action,undefined);
  }
});

test('exact author Item, selected-layer transport and raw display scopes reject scientific substitutions',()=>{
  for(const record of items) for(const key of NEXT_PC_PRODUCTS[record.datasetId].keys) {
    const c=contractCase(record,key), original=JSON.stringify(c);
    validatePublicDataset(c.dataset);validateDatasetRequest(c.request);validatePublicDatasetPlan(c.plan,c.request);
    assert.equal(validNextPlanetarySnapshot(c.item,c.asset),true);
    validateStacSnapshot(c.item);
    validatePublicDatasetItem(c.item,c.plan,[{id:uuid,url:record.url,snapshotIds:[receipt]}]);
    assert.equal(publicDatasetAssetLayer(c.dataset,c.request,key),c.dataset.layers.find(l=>l.id===key));
    assert.deepEqual(publicDatasetPreviewRange(c.dataset,c.asset,c.item),record.datasetId==='planetary-more'?[-200,3500]:key==='Burn_Date'?[-2,366]:[0,255]);
    assert.equal(JSON.stringify(c),original);
    assert.equal(ADDITIONAL_PC_ORIGINALS[record.datasetId][key].sha256.length,64);
    for(const wrong of ['visual','raster','hdf','anomaly','QA','Burn_Date'].filter(v=>v!==key)) assert.equal(publicDatasetAssetLayer(c.dataset,c.request,wrong),null);
    const mutations=[
      v=>v.item.properties.start_datetime='2020-09-02T00:00:00Z',
      v=>v.item.provenance.reviewedDataset.request.year=2021,
      v=>v.item.provenance.reviewedDataset.layer.nodata=0,
      v=>v.item.provenance.reviewedDataset.layer.scale=0.01,
      v=>v.item.provenance.collection.license='CC0',
      v=>v.asset.metadata.roles=['overview'],
      v=>v.asset.href+='?sig=not-persisted',
      v=>v.item.provenance.assetTransports[0].href+='?sig=not-persisted',
      v=>v.item.provenance.assetTransports.push({assetKey:'unselected',originalHref:v.asset.href,href:v.asset.href,adapter:'planetary-anonymous-sas'}),
    ];
    for(const mutate of mutations) {
      const changed=structuredClone(c);mutate(changed);
      assert.equal(validNextPlanetarySnapshot(changed.item,changed.asset),false);
      assert.equal(publicDatasetPreviewRange(changed.dataset,changed.asset,changed.item),undefined);
      assert.throws(()=>validateStacSnapshot(changed.item));
    }
  }
});

test('OISST Celsius calibration and burned-area codes never become an automatic value/mask conversion',()=>{
  const s=contractCase(items.find(r=>r.datasetId==='planetary-more'),'sst');
  assert.equal(s.dataset.layers[0].scale,0.009999999776482582);
  assert.equal(s.dataset.layers[0].nodata,-999);
  const record=items.find(r=>r.datasetId==='modis-more');
  const burn=contractCase(record,'Burn_Date'),qa=contractCase(record,'QA');
  assert.equal(burn.dataset.layers.find(l=>l.id==='Burn_Date').nodata,-1);
  assert.equal(qa.dataset.layers.find(l=>l.id==='QA').nodata,undefined);
  assert.equal(qa.asset.metadata['raster:bands'][0].nodata,undefined);
  assert.equal(qa.asset.metadata['raster:bands'][0].data_type,'uint8');
  assert.equal(record.document.properties['proj:epsg'],null);
  assert.throws(()=>validateDatasetRequest({...burn.request,layer:'OTHER_CAPITAL_LAYER'}));
  assert.equal(chinese['Original ordinal day of burn'],'原始年内火烧日');
  assert.equal(chinese['Original burned-area quality flags'],'原始火烧迹地质量标记');
});
