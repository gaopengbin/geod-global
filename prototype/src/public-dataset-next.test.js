import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { nextPublicDatasets, nextPublicCases, nextPublicReceipt } from './public-dataset-next-fixtures.js';
import { validatePublicDataset, validatePublicDatasetPlan, validatePublicDatasetConnection, validatePublicDatasetItem, publicDatasetAssetLayer } from './public-dataset-client.js';
import { validateStacSnapshot, validateStacInspection, validateStacPixel } from './stac-client.js';

const clone = value => structuredClone(value);
const byId = id => nextPublicCases.find(value => value.datasetId === id);
const ids = ['usgs-3dep', 'chelsa-climate', 'jaxa-amsr2-soil-moisture', 'jaxa-sgli-lst'];

test('four frozen responses originate from completed fresh original acceptance and retain actual preview bytes', () => {
  assert.equal(nextPublicReceipt.status, 'passed');
  assert.deepEqual(nextPublicCases.map(value => value.datasetId).sort(), [...ids].sort());
  for (const value of nextPublicCases) {
    assert.equal(value.discoveryStartedNoDownloads, true);
    assert.equal(value.independentReferenceSha256Matched, true);
    assert.equal(value.originalSha256Verified, true);
    assert.equal(value.job.transfer.mode, 'fresh');
    const preview = Buffer.from(value.inspection.previewDataUrl.split(',')[1], 'base64');
    assert.equal(createHash('sha256').update(preview).digest('hex'), value.previewSha256);
    assert.equal(validateStacInspection(value.inspection, value.job.id), value.inspection);
    for (const pixel of value.pixels) {
      assert.equal(validateStacPixel(pixel, {id:value.job.id,column:pixel.column,row:pixel.row}), pixel);
      assert.equal(pixel.sha256, value.job.sha256);
    }
  }
});

for (const value of nextPublicCases) {
  test(`${value.datasetId} accepts actual resolve, connection and bound original snapshot contracts`, () => {
    const dataset = nextPublicDatasets.find(dataset => dataset.id === value.datasetId);
    assert.equal(validatePublicDataset(dataset), dataset);
    assert.equal(validatePublicDatasetPlan(value.resolvedPlan, value.request), value.resolvedPlan);
    assert.equal(validatePublicDatasetPlan(value.plan, value.request), value.plan);
    assert.equal(validatePublicDatasetConnection(value.connection, value.request), value.connection);
    assert.equal(validateStacSnapshot(value.snapshot), value.snapshot);
    assert.equal(validatePublicDatasetItem(value.snapshot, value.plan, value.connection.stacConnections), value.snapshot);
    assert.equal(publicDatasetAssetLayer(dataset, value.request, value.assetKey).id, value.request.layer);
  });
}

test('USGS candidate query becomes a real catalog record without inventing a terrain observation timestamp', () => {
  const value = byId('usgs-3dep');
  assert.equal(value.resolvedPlan.sources[0].id, 'usgs-3dep-catalog');
  assert.equal(value.resolvedPlan.sources[0].catalogRaster, undefined);
  const source = value.plan.sources[0], record = source.catalogRaster.record;
  assert.equal(source.id, record.sourceId);
  assert.equal(source.stac.url, record.downloadURL);
  assert.equal(record.publicationDate, '2026-09-17');
  assert.equal(value.snapshot.datetime, null);
  assert.equal(value.snapshot.startDatetime, null);
  assert.equal(value.snapshot.endDatetime, null);
  assert.equal(value.snapshot.provenance.collection, null);
  assert.match(value.snapshot.properties.datasetPreset.declarationKind, /client-catalog-raster/);
  for (const change of [{year:2026}, {month:9}, {bounds:[0,0,1,1]}]) {
    const plan=clone(value.plan); Object.assign(plan.request, change);
    assert.throws(() => validatePublicDatasetPlan(plan, value.request));
  }
  for (const mutation of [
    snapshot => { snapshot.properties.datasetPreset.catalogRaster.record.publicationDate='2020-01-01'; },
    snapshot => { snapshot.assets[0].metadata['file:size']++; },
    snapshot => { snapshot.provenance.metadataDocuments=[]; },
    snapshot => { snapshot.startDatetime='2026-09-17T00:00:00Z'; snapshot.endDatetime='2026-09-18T00:00:00Z'; snapshot.temporalStatus='interval'; },
  ]) {
    const snapshot=clone(value.snapshot); mutation(snapshot);
    assert.throws(() => validatePublicDatasetItem(snapshot,value.plan,value.connection.stacConnections));
  }
});

test('USGS area query cannot gain pagination, extra filters or a mismatched footprint and its original cannot become a foreign raster', () => {
  const value=byId('usgs-3dep');
  for(const change of [
    url=>url.searchParams.set('bbox','0,0,1,1'),
    url=>url.searchParams.set('bbox','-73.994,40.71,-73.99'),
    url=>url.searchParams.set('max','20'),
    url=>url.searchParams.set('offset','100'),
    url=>url.searchParams.append('bbox',url.searchParams.get('bbox')),
    url=>url.searchParams.set('year','2026'),
    url=>{url.hostname='tnmaccess.nationalmap.gov.evil.example';},
  ]) {
    const plan=clone(value.resolvedPlan),url=new URL(plan.sources[0].stac.url);
    change(url);plan.sources[0].stac.url=url.href;
    assert.throws(()=>validatePublicDatasetPlan(plan,value.request));
    const connected=clone(value.plan);connected.sources[0].catalogRaster.catalogUrl=url.href;
    assert.throws(()=>validatePublicDatasetPlan(connected,value.request));
  }
  for(const mutation of [
    source=>{source.catalogRaster.record.sizeInBytes=512*1024*1024+1;},
    source=>{source.catalogRaster.record.boundingBox.minX=0;},
    source=>{source.catalogRaster.record.sourceId='different-publisher-id';},
    source=>{source.catalogRaster.record.urls.TIFF='https://example.com/foreign.tif';},
    source=>{const url=source.stac.url.replace('prd-tnm.s3.amazonaws.com','example.com');source.stac.url=url;source.catalogRaster.record.downloadURL=url;source.catalogRaster.record.urls.TIFF=url;},
  ]) {
    const plan=clone(value.plan);mutation(plan.sources[0]);
    assert.throws(()=>validatePublicDatasetPlan(plan,value.request));
  }
});

test('CHELSA normal keeps a signed negative sample and effective typed mask separately from historical client period', () => {
  const value=byId('chelsa-climate'),source=value.plan.sources[0],dataset=value.plan.dataset;
  assert.deepEqual(dataset.years, []); assert.equal(value.request.year, undefined); assert.equal(value.request.month, undefined);
  assert.equal(source.startDatetime,'1981-01-01T00:00:00Z'); assert.equal(source.endDatetime,'2010-12-31T23:59:59Z');
  assert.equal(value.snapshot.provenance.collection,null);
  assert.equal(value.inspection.bands[0].dataType,'Int16'); assert.equal(value.inspection.bands[0].nodata,32767);
  const negative=value.pixels.find(pixel=>pixel.column===35568&&pixel.row===5291);
  assert.deepEqual(negative.values,[-323]); assert.deepEqual(negative.noData,[false]);
  assert.equal(dataset.layers[0].scale,0.1); assert.equal(dataset.layers[0].offset,0);
  assert.ok(value.inspection.bounds[0] < -180); assert.equal(source.bounds[0],-180);
  for(const field of ['year','month']) {
    const plan=clone(value.plan);plan.request[field]=field==='year'?2010:12;
    assert.throws(()=>validatePublicDatasetPlan(plan,value.request));
  }
});

test('a direct CHELSA product cannot accept a foreign connection or another product snapshot', () => {
  const value=byId('chelsa-climate'),foreign=byId('usgs-3dep');
  assert.throws(()=>validatePublicDatasetItem(foreign.snapshot,value.plan,value.connection.stacConnections));
  const snapshot=clone(value.snapshot);snapshot.connectionId=foreign.snapshot.connectionId;
  assert.throws(()=>validatePublicDatasetItem(snapshot,value.plan,value.connection.stacConnections));
  const changed=clone(value.snapshot);changed.properties.datasetPreset.request.bounds=[0,0,1,1];
  assert.throws(()=>validatePublicDatasetItem(changed,value.plan,value.connection.stacConnections));
});

test('JAXA monthly soil errors are retained separately from NoData and temperature remains UInt16 DN', () => {
  const soil=byId('jaxa-amsr2-soil-moisture'),lst=byId('jaxa-sgli-lst');
  assert.equal(soil.plan.dataset.license,'proprietary');assert.equal(lst.plan.dataset.license,'proprietary');
  assert.deepEqual(soil.plan.dataset.years,[2025]);assert.deepEqual(soil.plan.dataset.months,[12]);
  assert.equal(soil.inspection.bands[0].nodata,-32768);
  assert.deepEqual(soil.pixels.find(pixel=>pixel.column===0&&pixel.row===0).values,[-32767]);
  assert.deepEqual(soil.pixels.find(pixel=>pixel.column===0&&pixel.row===0).noData,[false]);
  assert.deepEqual(soil.pixels.find(pixel=>pixel.column===600&&pixel.row===600).noData,[true]);
  assert.equal(lst.inspection.bands[0].dataType,'UInt16');assert.equal(lst.inspection.bands[0].nodata,65535);
  assert.deepEqual(lst.pixels.find(pixel=>pixel.column===1040&&pixel.row===2000).values,[13785]);
  assert.equal(lst.snapshot.startDatetime,'2025-11-30T23:55:26.400000Z');
  assert.equal(lst.snapshot.endDatetime,'2026-01-01T00:03:54Z');
  for(const value of [soil,lst]) {
    for(const change of [{year:2026},{month:11},{layer:value.request.layer==='smc'?'lst':'smc'}]) {
      const plan=clone(value.plan);Object.assign(plan.request,change);
      assert.throws(()=>validatePublicDatasetPlan(plan,value.request));
    }
    assert.throws(()=>validatePublicDatasetItem(value===soil?lst.snapshot:soil.snapshot,value.plan,value.connection.stacConnections));
    const changed=clone(value.snapshot);changed.provenance.reviewedDataset.request.month=11;
    assert.throws(()=>validatePublicDatasetItem(changed,value.plan,value.connection.stacConnections));
  }
});
