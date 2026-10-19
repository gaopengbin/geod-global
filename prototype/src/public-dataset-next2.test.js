import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { next2PublicCases, next2PublicDatasets, next2PublicReceipts, next2PublicFixtureScope } from './public-dataset-next2-fixtures.js';
import { validatePublicDataset, validatePublicDatasetPlan, validatePublicDatasetConnection, validatePublicDatasetItem, publicDatasetAssetLayer } from './public-dataset-client.js';
import { validateStacSnapshot, validateStacInspection, validateStacPixel } from './stac-client.js';
import { genericMapDisplayBounds, genericCoordinateToPixel, genericMapMetadata } from './stac-map-model.js';

const clone = value => structuredClone(value);
const byId = id => next2PublicCases.find(value => value.datasetId === id);
const mutateDirectProof = (snapshot, mutation) => {
  mutation(snapshot.properties.datasetPreset);
  snapshot.assets[0].metadata.datasetPreset = clone(snapshot.properties.datasetPreset);
};

test('selected native-tested fixtures preserve complete original preview bytes and never imply whole-catalog or live browser acceptance', () => {
  assert.equal(next2PublicFixtureScope.playbackIsLiveBrowserAcceptance, false);
  assert.match(next2PublicFixtureScope.fixtureScope, /Selected cases only/);
  assert.equal(next2PublicCases.length, 4);
  assert.deepEqual(new Set(next2PublicCases.map(value => value.datasetId)), new Set(['ghsl-smod', 'glad-landcover-2020', 'maxar-open-legacy']));
  assert.ok(next2PublicReceipts.every(receipt => receipt.status === 'passed' && receipt.nativeWindowTested === false));
  const reports = new Map();
  for (const receipt of next2PublicReceipts) {
    assert.match(receipt.source, /^docs\/next2-(ghsl|glad|maxar)-native-acceptance-20261012\.json$/);
    const bytes = readFileSync(new URL(`../../${receipt.source}`, import.meta.url));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), receipt.sha256, 'Frozen selected fixture must retain the actual acceptance report version');
    reports.set(receipt.source, JSON.parse(bytes));
  }
  for (const value of next2PublicCases) {
    assert.match(value.label, /Selected native-tested case/);
    assert.match(value.label, /not whole-catalog acceptance/);
    assert.equal(value.discoveryStartedNoDownloads, true);
    assert.equal(value.independentReferenceSha256Matched, true);
    assert.equal(value.originalSha256Verified, true);
    assert.equal(value.reopenedOriginalPreviewVerified, true);
    assert.equal(value.reopenedProjectAndSourceVerified, true);
    const original = reports.get(value.sourceReceipt).cases.find(row => row.job.id === value.job.id);
    assert.equal(value.settledTransferReusedForFinalInspection, true);
    assert.equal(value.settledTransferReusedForFinalInspection, original.settledTransferReusedForFinalInspection);
    assert.equal(value.downloadSeconds, original.downloadSeconds);
    assert.match(value.downloadTimingScope, /Historical settled-transfer duration/);
    const preview = Buffer.from(value.inspection.previewDataUrl.split(',')[1], 'base64');
    assert.equal(createHash('sha256').update(preview).digest('hex'), value.previewSha256);
    assert.deepEqual([preview.readUInt32BE(16), preview.readUInt32BE(20)], [value.inspection.previewWidth, value.inspection.previewHeight]);
    assert.equal(validateStacInspection(value.inspection, value.job.id), value.inspection);
    for (const pixel of value.pixels) {
      assert.equal(validateStacPixel(pixel, { id:value.job.id, column:pixel.column, row:pixel.row }), pixel);
      assert.equal(pixel.sha256, value.job.sha256);
    }
  }
});

for (const value of next2PublicCases) {
  test(`${value.label}: actual plan, connection, snapshot and original raw-pixel contracts validate`, () => {
    const dataset = next2PublicDatasets.find(dataset => dataset.id === value.datasetId);
    assert.equal(validatePublicDataset(dataset), dataset);
    assert.equal(validatePublicDatasetPlan(value.resolvedPlan, value.request), value.resolvedPlan);
    assert.equal(validatePublicDatasetPlan(value.plan, value.request), value.plan);
    assert.equal(validatePublicDatasetConnection(value.connection, value.request), value.connection);
    assert.equal(validateStacSnapshot(value.snapshot), value.snapshot);
    assert.equal(validatePublicDatasetItem(value.snapshot, value.plan, value.connection.stacConnections), value.snapshot);
    assert.equal(publicDatasetAssetLayer(dataset, value.request, value.assetKey).id, value.request.layer);
  });

  test(`${value.datasetId}/${value.assetKey}: no cross-source, epoch or original scientific-proof substitution`, () => {
    for (const change of [{ bounds:[0,0,1,1] }, { year:2030 }, { layer:'foreign-layer' }]) {
      const plan = clone(value.plan); Object.assign(plan.request, change);
      assert.throws(() => validatePublicDatasetPlan(plan, value.request));
    }
    const wrongConnection = clone(value.connection);
    wrongConnection.stacConnections[0].url = 'https://example.com/foreign.tif';
    assert.throws(() => validatePublicDatasetConnection(wrongConnection, value.request));
    const wrongSnapshot = clone(value.snapshot);
    wrongSnapshot.connectionId = '00000000-0000-4000-8000-000000000000';
    assert.throws(() => validatePublicDatasetItem(wrongSnapshot, value.plan, value.connection.stacConnections));
    if (value.plan.dataset.discovery == null) {
      for (const mutation of [
        proof => { proof.request.year = 2030; },
        proof => { proof.sourceId = 'foreign-original'; },
        proof => { proof.layer.nodata = 0; },
        proof => { proof.layer.unit = 'physical radiance'; },
        proof => { proof.dataset.valueInterpretation = 'All zero values are NoData.'; },
      ]) {
        const snapshot = clone(value.snapshot); mutateDirectProof(snapshot, mutation);
        assert.throws(() => validatePublicDatasetItem(snapshot, value.plan, value.connection.stacConnections));
      }
    }
    for (const foreign of next2PublicCases.filter(other => other.datasetId !== value.datasetId || other.assetKey !== value.assetKey)) {
      assert.throws(() => validatePublicDatasetItem(foreign.snapshot, value.plan, value.connection.stacConnections));
    }
  });
}

test('GHSL physical map fit uses its exact original SHA and source proof without cropping the original grid or changing raw zero/mask', () => {
  const value = byId('ghsl-smod'), inspection = value.inspection;
  const before = clone(inspection), sourceBefore = clone(value.snapshot);
  assert.equal(genericMapMetadata(inspection), inspection);
  const fit = genericMapDisplayBounds(inspection, inspection.bounds, value.snapshot);
  assert.deepEqual(fit, [-180, inspection.bounds[1], 180, inspection.bounds[3]]);
  assert.ok(inspection.bounds[0] < -180 && inspection.bounds[2] > 180);
  assert.deepEqual(inspection, before);
  assert.deepEqual(value.snapshot, sourceBefore);
  assert.deepEqual(genericCoordinateToPixel([-180, inspection.transform[5] - 502.5 * -inspection.transform[4]], inspection), { column:0, row:502 });
  const zero = value.pixels.find(pixel => pixel.column === 1 && pixel.row === 1329);
  const missing = value.pixels.find(pixel => pixel.column === 0 && pixel.row === 0);
  assert.deepEqual(zero.values, [0]); assert.deepEqual(zero.noData, [false]);
  assert.deepEqual(missing.values, [-200]); assert.deepEqual(missing.noData, [true]);
  assert.match(value.plan.dataset.valueInterpretation, /undocumented raw code/);
});

test('GHSL buffered antimeridian extent cannot gain a map exception from wrong SHA, URL, source, proof, epoch or grid', () => {
  const value = byId('ghsl-smod');
  const check = (inspection = value.inspection, snapshot = value.snapshot) => genericMapDisplayBounds(inspection, inspection.bounds, snapshot);
  assert.throws(() => genericMapDisplayBounds(value.inspection, value.inspection.bounds));
  for (const mutation of [
    inspection => { inspection.sha256 = '0'.repeat(64); },
    inspection => { inspection.width--; },
    inspection => { inspection.transform[0] *= 2; },
    inspection => { inspection.bounds[0] = -180.01; },
  ]) {
    const inspection = clone(value.inspection); mutation(inspection);
    assert.throws(() => check(inspection));
  }
  for (const [name, mutation] of [
    ['different publisher epoch path', snapshot => { snapshot.provenance.documentUrl = snapshot.provenance.documentUrl.replace('/2020/', '/2025/'); }],
    ['foreign asset URL', snapshot => { snapshot.assets[0].href = 'https://example.com/foreign.tif'; }],
    ['missing scientific proof', snapshot => { delete snapshot.properties.datasetPreset; }],
    ['wrong reference epoch', snapshot => { mutateDirectProof(snapshot, proof => { proof.request.year = 2025; }); }],
    ['foreign source identity', snapshot => { mutateDirectProof(snapshot, proof => { proof.sourceId = 'foreign-original'; }); }],
    ['invented zero mask', snapshot => { mutateDirectProof(snapshot, proof => { proof.layer.nodata = 0; }); }],
  ]) {
    const snapshot = clone(value.snapshot); mutation(snapshot);
    assert.throws(() => check(value.inspection, snapshot), name);
  }
});

test('GLAD original palette indices preserve valid desert zero separately from missing legend code and physical TIFF mask', () => {
  const value = byId('glad-landcover-2020');
  if (!value) return;
  assert.equal(value.inspection.bands[0].dataType, 'UInt8');
  assert.equal(value.inspection.bands[0].nodata, null);
  const zero = value.pixels.find(pixel => pixel.values[0] === 0);
  assert.ok(zero); assert.deepEqual(zero.noData, [false]);
  assert.match(value.plan.dataset.valueInterpretation, /no scalar NoData tag/);
  assert.equal(value.snapshot.datetime, null);
  assert.equal(value.snapshot.temporalStatus, 'missing');
});

for (const value of next2PublicCases.filter(value => value.datasetId === 'maxar-open-legacy')) {
  test(`Maxar ${value.assetKey} retains the legacy original properties.datetime point time and limits selection to its reviewed analytic layer`, () => {
    assert.equal(value.snapshot.datetime, value.snapshot.properties.datetime);
    assert.equal(value.snapshot.temporalStatus, 'instant');
    assert.equal(value.snapshot.startDatetime, null);
    assert.equal(value.snapshot.endDatetime, null);
    assert.equal(value.snapshot.provenance.reviewedDataset.request.layer, value.request.layer);
    assert.equal(publicDatasetAssetLayer(value.plan.dataset, value.request, value.request.layer).id, value.request.layer);
    for (const asset of value.snapshot.assets.filter(asset => asset.key !== value.request.layer)) {
      assert.equal(publicDatasetAssetLayer(value.plan.dataset, value.request, asset.key), null);
    }
    assert.equal(publicDatasetAssetLayer(value.plan.dataset, value.request, 'visual'), null);
    for (const mutation of [
      snapshot => { snapshot.datetime = '2020-01-01T00:00:00Z'; },
      snapshot => { snapshot.provenance.reviewedDataset.request.layer = 'visual'; },
      snapshot => { snapshot.startDatetime = snapshot.datetime; snapshot.endDatetime = snapshot.datetime; snapshot.datetime = null; snapshot.temporalStatus = 'interval'; },
    ]) {
      const snapshot = clone(value.snapshot); mutation(snapshot);
      assert.throws(() => validatePublicDatasetItem(snapshot, value.plan, value.connection.stacConnections));
    }
    assert.match(value.plan.dataset.valueInterpretation + value.plan.dataset.warnings.join(' '), /mask/i);
  });
}

for (const value of next2PublicCases.filter(value => value.datasetId === 'openlandmap-nightlights')) {
  test('2024 nighttime-light raw DN retains the embedded declaration conflict, genuine NoData and unconverted zero', () => {
    assert.equal(value.job.sha256, 'e6edde68952052f8c53050e2ce7082bab08ddd0e15a696f999215e2d57194c53');
    assert.equal(value.inspection.bands[0].dataType, 'Int16');
    assert.equal(value.inspection.bands[0].nodata, -32768);
    assert.deepEqual(value.plan.dataset.years, [2024]);
    assert.equal(value.plan.dataset.layers[0].scale, 10);
    assert.match(value.plan.dataset.unit, /calibration unresolved/);
    assert.match(value.plan.dataset.valueInterpretation, /0–2000/);
    assert.match(value.plan.dataset.warnings.join(' '), /2000–2011/);
    assert.match(value.plan.dataset.warnings.join(' '), /3,299/);
    const beijing = value.pixels.find(pixel => pixel.column === 71135 && pixel.row === 11392);
    assert.deepEqual(beijing.values, [150]); assert.deepEqual(beijing.noData, [false]);
    assert.ok(value.pixels.some(pixel => pixel.values[0] === 0 && pixel.noData[0] === false));
    assert.ok(value.pixels.some(pixel => pixel.values[0] === -32768 && pixel.noData[0] === true));
  });
}
