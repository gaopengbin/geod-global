import test from 'node:test';
import assert from 'node:assert/strict';
import actual from './stac-map-next-ghsl.fixture.json' with { type:'json' };
import profiles from '../../crates/geod-runtime/src/datasets/next-ghsl-profiles.json' with { type:'json' };
import nativePresets from './public-dataset-next-ten.fixture.json' with { type:'json' };
import { genericMapPlacement, genericCoordinateToPixel, reviewedGhslDisplay } from './stac-map-model.js';
import { validateStacInspection, validateStacSnapshot, validateStacPixel } from './stac-client.js';

// The POP response is from a completed real UI download. S/V cases below use
// independent full-file profiles and real native presets for scope tests; they
// are not claimed as rendered browser or successful UI-download receipts.
function profileCase(profile) {
  const dataset = nativePresets.catalog.find(item=>item.id===profile.datasetId);
  const proof = { schemaVersion:2, dataset, layer:dataset.layers[0],
    request:{datasetId:dataset.id,layer:profile.layer,year:2020,bounds:[116.3,39.8,116.5,40]},
    declarationKind:'reviewed-client-preset; publisher file bytes and embedded metadata verified separately',
    sourceId:profile.url.split('/').at(-1).replace(/\.tif$/,''),
    sourceTitle:`JRC ${profile.code} V1.0 · total · reference epoch 2020`, sourceBounds:profile.queryBounds };
  const source = structuredClone(actual.snapshot);
  source.properties.datasetPreset = proof;
  source.provenance.documentUrl = profile.url;
  source.assets[0].href = profile.url;
  source.assets[0].metadata.href = profile.url;
  source.assets[0].metadata.datasetPreset = structuredClone(proof);
  const data = { ...structuredClone(actual.inspection), sha256:profile.sha256,
    width:profile.width,height:profile.height,transform:profile.transform,bounds:profile.sourceBounds,
    bands:[{index:1,dataType:profile.dataType,nodata:null}] };
  return { data,source };
}

test('actual successful POP inspection preserves schema, Float64 raw samples and the two original edge columns', () => {
  const {inspection:data,snapshot:source,pixels} = actual;
  assert.equal(validateStacInspection(data,data.jobId),data);
  assert.equal(validateStacSnapshot(source),source);
  assert.equal(reviewedGhslDisplay(data,data.bounds,source),true);
  assert.equal(data.width,43202); assert.equal(data.bands[0].dataType,'Float64');
  assert.deepEqual(pixels.map(p=>({column:p.column,row:p.row,value:p.values[0],nodata:p.noData[0]})),[
    {column:35568,row:5903,value:4307.731181065241,nodata:false},
    {column:0,row:0,value:0,nodata:false},
    {column:43201,row:100,value:0,nodata:false},
  ]);
  for(const pixel of pixels) {
    validateStacPixel(pixel,{id:data.jobId,column:pixel.column,row:pixel.row});
    const coordinate=[data.transform[2]+(pixel.column+0.5)*data.transform[0],data.transform[5]+(pixel.row+0.5)*data.transform[4]];
    assert.deepEqual(genericCoordinateToPixel(coordinate,data),{column:pixel.column,row:pixel.row});
  }
});

test('all three immutable GHSL profiles clip and fit the display without replacing the PNG extent or original click grid', () => {
  for(const profile of profiles.profiles) {
    const {data,source}=profileCase(profile), before=JSON.stringify({data,source});
    const placement=genericMapPlacement(data,data.bounds,source);
    assert.deepEqual(placement.fitBounds,profile.queryBounds);
    assert.deepEqual(placement.clipBounds,profile.queryBounds);
    assert.deepEqual(placement.imageBounds,profile.sourceBounds);
    assert.notDeepEqual(placement.imageBounds,placement.fitBounds);
    assert.equal(JSON.stringify({data,source}),before);
    const column=35568,row=5903;
    const geographic=[data.transform[2]+(column+0.5)*data.transform[0],data.transform[5]+(row+0.5)*data.transform[4]];
    assert.deepEqual(genericCoordinateToPixel(geographic,data),{column,row});
  }
});

test('GHSL display exception rejects wrong file/grid/type/NoData and mismatched source or scientific proof', () => {
  const mutations=[
    (d,s)=>d.sha256='0'.repeat(64), (d,s)=>d.crs='EPSG:4269', (d,s)=>d.horizontalCrs='EPSG:3857',
    (d,s)=>d.verticalCrs='EPSG:3855', (d,s)=>d.width--, (d,s)=>d.pixelInterpretation='point',
    (d,s)=>d.storageLayout='strip', (d,s)=>d.bands[0].dataType='Float32', (d,s)=>d.bands[0].nodata=0,
    (d,s)=>d.transform[0]*=1.000001, (d,s)=>d.bounds[0]-=0.001,
    (d,s)=>s.provenance.documentUrl='https://example.org/copied.tif', (d,s)=>s.provenance.search={},
    (d,s)=>s.collectionId='ghsl', (d,s)=>s.datetime='2020-01-01T00:00:00Z',
    (d,s)=>s.assets[0].href+='?sig=not-allowed', (d,s)=>s.assets[0].eligible=false,
    (d,s)=>s.assets.push(structuredClone(s.assets[0])), (d,s)=>s.assets[0].transportAdapter='planetary-anonymous-sas',
    (d,s)=>s.assets[0].metadata.datasetPreset.request.year=2021,
    (d,s)=>s.properties.datasetPreset.sourceBounds[0]=-179,
    (d,s)=>s.properties.datasetPreset.dataset.id='ghsl-smod',
    (d,s)=>s.properties.datasetPreset.dataset.version+=' other release',
    (d,s)=>{s.properties.datasetPreset.request.year=2021;s.assets[0].metadata.datasetPreset=structuredClone(s.properties.datasetPreset);},
    (d,s)=>{s.properties.datasetPreset.request.layer='density';s.assets[0].metadata.datasetPreset=structuredClone(s.properties.datasetPreset);},
    (d,s)=>{s.properties.datasetPreset.layer.nodata=0;s.assets[0].metadata.datasetPreset=structuredClone(s.properties.datasetPreset);},
    (d,s)=>delete s.properties.datasetPreset,
  ];
  for(const mutate of mutations) {
    const data=structuredClone(actual.inspection),source=structuredClone(actual.snapshot); mutate(data,source);
    assert.equal(reviewedGhslDisplay(data,data.bounds,source),false);
    assert.throws(()=>genericMapPlacement(data,data.bounds,source));
  }
  assert.throws(()=>genericMapPlacement(actual.inspection,actual.inspection.bounds,null));
  assert.throws(()=>genericMapPlacement(actual.inspection,[-181,...actual.inspection.bounds.slice(1)],actual.snapshot));
});

test('ordinary legal rasters receive no clip while arbitrary overflowing geographic files remain rejected', () => {
  const data={crs:'EPSG:4326',pixelInterpretation:'area',width:100,height:100,
    transform:[0.01,0,116,0,-0.01,40],bounds:[116,39,117,40]};
  assert.deepEqual(genericMapPlacement(data,data.bounds),{fitBounds:data.bounds,imageBounds:data.bounds,clipBounds:null});
  const arbitrary={...data,width:43202,height:21384,transform:actual.inspection.transform,bounds:actual.inspection.bounds};
  assert.throws(()=>genericMapPlacement(arbitrary,arbitrary.bounds));
});
