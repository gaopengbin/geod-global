import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {publicDatasetAssetLayer,publicDatasetPreviewRange,publicDatasetPreviewPolicy} from './public-dataset-client.js';

const records=['jaxa-land-batch-reviewed-items.json','jaxa-ocean-batch-reviewed-items.json'].flatMap(file=>
  JSON.parse(readFileSync(new URL(`../../crates/geod-runtime/src/datasets/${file}`,import.meta.url),'utf8')));
const layers={'jaxa-sgli-et':['ET','et',4],'jaxa-sgli-et-index':['ETidx','et-index',4],
  'jaxa-sgli-aerosol':['AROT','aod',9],'jaxa-modis-aerosol':['T500','aod',9],
  'jaxa-amsr2-wind':['SSW','ssw',9],'jaxa-amsr2-sst':['SST','sst',10],'jaxa-gsmap-spi':['SPI','spi',9]};

test('ten-product JAXA mappings use real archived bands, separate calibration and exact reviewed dates',()=>{
  const seen=new Set();
  for(const record of records) {
    const [key,layer,month]=layers[record.datasetId];seen.add(record.datasetId);
    const dataset={id:record.datasetId,layers:[{id:layer}]},request={datasetId:dataset.id,year:2026,month,layer};
    const asset={key,metadata:structuredClone(record.document.assets[key])};
    const item={provenance:{reviewedDataset:{dataset,request}}};
    const original=JSON.stringify({asset,item});
    assert.equal(publicDatasetAssetLayer(dataset,request,key),dataset.layers[0]);
    for(const wrong of ['raster','thumbnail','ET','ETidx','AROT','T500','SSW','SST','SPI'].filter(v=>v!==key))
      assert.equal(publicDatasetAssetLayer(dataset,request,wrong),null);
    assert.equal(publicDatasetAssetLayer(dataset,{...request,layer:'reflectance'},key),null);
    const range=publicDatasetPreviewRange(dataset,asset,item);
    assert.ok(range?.length===2&&range[0]<range[1],`Missing display range for ${dataset.id}`);
    assert.equal(JSON.stringify({asset,item}),original);
    assert.equal(publicDatasetPreviewRange(dataset,asset,{provenance:{reviewedDataset:{dataset,request:{...request,year:2025}}}}),undefined);
    const changed=structuredClone(asset);changed.metadata['je:rasters'].dn2value.slope+=1;
    assert.equal(publicDatasetPreviewRange(dataset,changed,item),undefined);
  }
  assert.equal(seen.size,7);
});

test('bad SRTM15 overviews are excluded only for the exact reviewed file and surface recipe',()=>{
  const dataset={id:'srtm15plus-bathymetry'},href='https://opentopography.s3.sdsc.edu/raster/SRTM15Plus/SRTM15Plus_srtm/SRTM15_V2.5.5_02_10.tif';
  const proof={schemaVersion:2,dataset,request:{datasetId:dataset.id,layer:'surface'},layer:{id:'surface'},sourceId:'SRTM15_V2.5.5_02_10'};
  const asset={key:'raster',href,metadata:{href,datasetPreset:proof}};
  const original=JSON.stringify(asset);
  assert.equal(publicDatasetPreviewPolicy(dataset,asset),'srtm15plus-02-10-v255');
  assert.equal(JSON.stringify(asset),original);
  for(const wrong of [
    {...asset,key:'thumbnail'}, {...asset,href:href.replace('02_10','02_09')},
    {...asset,metadata:{...asset.metadata,datasetPreset:{...proof,sourceId:'SRTM15_V2.5.5_02_09'}}},
    {...asset,metadata:{...asset.metadata,datasetPreset:{...proof,request:{...proof.request,layer:'depth'}}}},
  ]) assert.equal(publicDatasetPreviewPolicy(dataset,wrong),undefined);
  assert.equal(publicDatasetPreviewPolicy({id:'etopo'},asset),undefined);
});
