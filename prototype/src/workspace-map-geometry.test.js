import test from 'node:test';
import assert from 'node:assert/strict';
import { coordinateToPixel, focusRasterExtent, intersectBounds, mapClipRecipe, previewPixelWindow, utmDefinition, verifiedMapMetadata, verifyPixelResult } from './workspace-map-geometry.js';

const job = { id: '933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b', status: 'succeeded', assetKey: 'scl', sha256: 'a'.repeat(64) };
const metadata = { crs: 'EPSG:32610', width: 5, height: 4, bounds: [100, 200, 200, 280], pixelSize: [20, 20], sha256: job.sha256, bandCount: 1, dataType: 'UInt8' };

test('map UTM definitions distinguish hemispheres and reject unsupported CRS', () => {
  assert.equal(utmDefinition('EPSG:32610'), '+proj=utm +zone=10 +datum=WGS84 +units=m +no_defs');
  assert.match(utmDefinition('EPSG:32760'), /zone=60 \+south/);
  for (const crs of ['EPSG:4326', 'EPSG:32600', 'EPSG:32661', 'EPSG:32810', null]) assert.throws(() => utmDefinition(crs));
});

test('map coordinate lookup respects the north-up pixel origin and exclusive outer edges', () => {
  assert.deepEqual(coordinateToPixel([100, 280], metadata), [0, 0]);
  assert.deepEqual(coordinateToPixel([129, 251], metadata), [1, 1]);
  assert.deepEqual(coordinateToPixel([199.99, 200.01], metadata), [4, 3]);
  for (const point of [[99, 280], [200, 240], [120, 200], [120, 281], [NaN, 250]]) assert.equal(coordinateToPixel(point, metadata), null);
});

test('selection preview intersects and rounds outward to source pixels', () => {
  assert.deepEqual(previewPixelWindow([119, 219, 161, 261], metadata), [0, 0, 4, 4]);
  assert.deepEqual(previewPixelWindow([120, 220, 160, 260], metadata), [1, 1, 2, 2]);
  assert.deepEqual(previewPixelWindow([0, 0, 1000, 1000], metadata), [0, 0, 5, 4]);
  assert.equal(previewPixelWindow([0, 0, 100, 200], metadata), null);
  assert.equal(intersectBounds([2, 2, 1, 1], metadata.bounds), null);
});

test('map focus stays on the selected raster when a search area spans multiple UTM zones', () => {
  const scene = [300000, 3290220, 409800, 3400020];
  const projectedSearch = [352737.94, 2014892.87, 7895012.82, 7723248.6];
  assert.deepEqual(focusRasterExtent(scene, projectedSearch), [352737.94, 3290220, 409800, 3400020]);
  assert.deepEqual(focusRasterExtent(scene, [0, 0, 100, 100]), scene);
  assert.deepEqual(focusRasterExtent(scene, [310000, 3300000, 320000, 3310000]), [310000, 3300000, 320000, 3310000]);
});

test('map loading rejects changed checksums, uncompleted jobs and inconsistent pixel geometry', () => {
  assert.equal(verifiedMapMetadata(job, metadata), metadata);
  assert.throws(() => verifiedMapMetadata({ ...job, status: 'running' }, metadata));
  assert.throws(() => verifiedMapMetadata(job, { ...metadata, sha256: 'b'.repeat(64) }));
  assert.throws(() => verifiedMapMetadata(job, { ...metadata, pixelSize: [19, 20] }));
  assert.throws(() => verifiedMapMetadata(job, { ...metadata, width: 0 }));
});

test('map clip recipe pins original bytes, preserves source coordinates and requires overlap', () => {
  const bounds = [111, 222, 155, 266];
  const recipe = mapClipRecipe(job, metadata, bounds, 'Selected area');
  assert.deepEqual(recipe.source, { jobId: job.id, sha256: job.sha256 });
  assert.deepEqual(recipe.operation, { type: 'clip', crs: 'source', bounds });
  assert.throws(() => mapClipRecipe(job, metadata, [1, 2, 3, 4], 'Outside'));
});

test('pixel results must identify the requested source, checksum, grid cell and coordinate', () => {
  const coordinate = [129, 251];
  const result = { jobId: job.id, sha256: job.sha256, crs: metadata.crs, coordinate, pixel: [1, 1], value: 6, label: 'Water', color: '#0000ff', isNoData: false };
  assert.equal(verifyPixelResult(result, job, metadata, coordinate), result);
  for (const changes of [{ jobId: 'other' }, { sha256: 'b'.repeat(64) }, { pixel: [1, 2] }, { coordinate: [130, 251] }, { value: 12 }])
    assert.throws(() => verifyPixelResult({ ...result, ...changes }, job, metadata, coordinate));
});
