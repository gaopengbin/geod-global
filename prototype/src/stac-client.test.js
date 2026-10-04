import test from 'node:test';
import assert from 'node:assert/strict';
import { validateStacConnection, validateStacSnapshot, validateStacInspection, validateStacPixel, validStacUrl, stacRequest } from './stac-client.js';
import { genericMapMetadata, genericCoordinateToPixel, genericProjectionDefinition } from './stac-map-model.js';
import { jobsForProject } from './projects-client.js';
import { stacConnection, stacSnapshot, stacInspection, stacJob, stacProject, sourceHash, jobId, snapshotId } from './stac-fixtures.js';

test('custom sources retain unknown assets, interval times and original metadata without sensor interpretation', () => {
  assert.equal(validateStacConnection(stacConnection), stacConnection);
  assert.equal(validateStacSnapshot(stacSnapshot), stacSnapshot);
  assert.equal(validateStacSnapshot({ ...stacSnapshot, collectionId: null, datetime: null, startDatetime: null, endDatetime: null, bbox: null, properties: {}, temporalStatus: 'missing' }).datetime, null);
  assert.throws(() => validateStacSnapshot({ ...stacSnapshot, id: stacConnection.id }));
  assert.throws(() => validateStacConnection({ ...stacConnection, snapshotIds: [stacConnection.id] }));
  assert.throws(() => validateStacSnapshot({ ...stacSnapshot, assets: [stacSnapshot.assets[0], stacSnapshot.assets[0]] }));
  for (const value of ['http://example.com/file.tif', 'https://user:password@example.com/file.tif', 'https://127.0.0.1/file.tif', 'https://localhost/file.tif', 'https://example.com/file.tif?X-Amz-Signature=abc', 'https://example.com/file.tif?token=secret']) assert.equal(validStacUrl(value), false, value);
});
test('generic file metadata supports raw float and mixed bands while rejecting unsafe previews and mismatched files', () => {
  assert.equal(validateStacInspection(stacInspection, jobId), stacInspection);
  assert.equal(validateStacInspection({ ...stacInspection, bands: [{ index: 1, dataType: 'UInt64', nodata: '18446744073709551615' }] }, jobId).bands[0].nodata, '18446744073709551615');
  assert.equal(validateStacInspection({ ...stacInspection, crs: null, bounds: null, transform: null, previewDataUrl: null, previewWidth: null, previewHeight: null }, jobId).crs, null);
  assert.throws(() => validateStacInspection({ ...stacInspection, previewDataUrl: 'https://example.com/preview.png' }, jobId));
  assert.throws(() => validateStacInspection({ ...stacInspection, bands: [{ ...stacInspection.bands[0], index: 3 }] }, jobId));
  assert.throws(() => validateStacInspection(stacInspection, 'wrong-job'));
  const request = { id: jobId, column: 1, row: 0 }, result = { jobId, column: 1, row: 0, sha256: sourceHash, values: [-5, '9007199254740993', 'NaN'], noData: [false, false, true] };
  assert.equal(validateStacPixel(result, request), result);
  assert.throws(() => validateStacPixel({ ...result, column: 0 }, request));
  assert.throws(() => validateStacPixel({ ...result, values: ['<script>'] }, request));
  assert.throws(() => validateStacPixel({ ...result, noData: [false] }, request));
});
test('generic map uses file transform and half-open pixel extents without classification assumptions', () => {
  assert.equal(genericMapMetadata(stacInspection), stacInspection);
  assert.deepEqual(genericCoordinateToPixel([10.75, 20.25], stacInspection), { column: 1, row: 1 });
  assert.deepEqual(genericCoordinateToPixel([10, 21], stacInspection), { column: 0, row: 0 });
  assert.equal(genericCoordinateToPixel([11, 20.5], stacInspection), null);
  assert.equal(genericCoordinateToPixel([10.5, 20], stacInspection), null);
  assert.throws(() => genericMapMetadata({ ...stacInspection, transform: [1, 0, 10, 0, -1, 21] }));
  assert.throws(() => genericMapMetadata({ ...stacInspection, crs: 'EPSG:27700' }));
  assert.equal(genericProjectionDefinition('EPSG:3857'), null);
  assert.match(genericProjectionDefinition('EPSG:26910'), /NAD83/);
  assert.match(genericProjectionDefinition('EPSG:32760'), /south/);
});
test('project ownership matches immutable source pin plus href, not the generic asset name or repeated item ID', () => {
  const unrelated = { ...stacJob, id: 'unrelated', stacSource: { ...stacJob.stacSource, snapshotId: 'd'.repeat(64) } };
  const wrongHref = { ...stacJob, id: 'wrong-href', href: 'https://example.com/other.tif' };
  assert.deepEqual(jobsForProject(stacProject, [stacJob, unrelated, wrongHref]), [stacJob]);
});
test('custom runtime routes use snapshot hashes and explicit POST bodies, checking source identity on search', async () => {
  const original = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => stacSnapshot }; };
  try {
    await stacRequest('snapshot', { id: snapshotId });
    assert.equal(calls[0].url, `http://127.0.0.1:4318/stac/snapshots/${snapshotId}`);
    await assert.rejects(stacRequest('snapshot', { id: stacConnection.id }));
    globalThis.fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => ({ items: [{ ...stacSnapshot, collectionId: 'wrong' }], nextCursor: null, complete: true, limitReached: false }) }; };
    await assert.rejects(stacRequest('search', { connectionId: stacConnection.id, collectionId: 'temperature', bounds: [10,20,11,21] }), /different source or collection/);
    assert.equal(calls[1].options.headers['X-GeoD-Client'], 'geod-global');
    assert.equal(calls[1].options.method, 'POST');
  } finally { globalThis.fetch = original; }
});
test('original page request is bounded, retains POST bodies and rejects another source or write method', () => {
  const snapshot = { ...stacSnapshot, provenance: { ...stacSnapshot.provenance, documentRequest: { url: stacSnapshot.provenance.documentUrl, method: 'POST', body: { collections: ['temperature'], token: 'page-id' } } } };
  assert.equal(validateStacSnapshot(snapshot), snapshot);
  for (const request of [
    { ...snapshot.provenance.documentRequest, url: 'https://other.example/search' },
    { ...snapshot.provenance.documentRequest, method: 'DELETE' },
    { ...snapshot.provenance.documentRequest, method: 'GET' },
    { ...snapshot.provenance.documentRequest, body: [] },
    { ...snapshot.provenance.documentRequest, body: { token: 'a'.repeat(16384) } },
  ]) assert.throws(() => validateStacSnapshot({ ...snapshot, provenance: { ...snapshot.provenance, documentRequest: request } }));
  assert.throws(() => validateStacConnection({ ...stacConnection, searchMethod: 'DELETE' }));
});
test('native STAC operations preserve their command envelopes and reject incomplete project receipts', async () => {
  const original = globalThis.window, calls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => { calls.push({ command, args }); return command === 'stac_snapshot' ? stacSnapshot : command === 'save_stac_project' ? stacProject : { projectId: stacProject.id, assetKey: 'stac_asset', jobs: [stacJob] }; } } } };
  try {
    await stacRequest('snapshot', { id: snapshotId });
    assert.deepEqual(calls[0], { command: 'stac_snapshot', args: { id: snapshotId } });
    const payload = { projectId: stacProject.id, bounds: stacProject.bounds, selections: [stacJob.stacSource] };
    await stacRequest('project', payload);
    assert.deepEqual(calls[1], { command: 'save_stac_project', args: { request: payload } });
    await stacRequest('downloads', { projectId: stacProject.id, selections: [stacJob.stacSource] });
    globalThis.window.__TAURI__.core.invoke = async () => ({ ...stacProject, stacItems: [] });
    await assert.rejects(stacRequest('project', payload), /selected original assets/);
    globalThis.window.__TAURI__.core.invoke = async () => { throw 'The source is unavailable'; };
    await assert.rejects(stacRequest('snapshot', { id: snapshotId }), /The source is unavailable/);
  } finally { if (original === undefined) delete globalThis.window; else globalThis.window = original; }
});

test('refreshed selections reuse canonical project and download pins only after reading both local snapshots', async () => {
  const previous = globalThis.window, calls = [], refreshedId = 'e'.repeat(64);
  const refreshed = { ...stacSnapshot, id: refreshedId, retrievedAt: '2026-10-03T01:00:00Z', documentSha256: 'f'.repeat(64),
    properties: Object.fromEntries(Object.entries(stacSnapshot.properties).reverse()),
    provenance: { ...stacSnapshot.provenance, documentUrl: 'https://example.com/search?limit=20', documentRequest: { url: 'https://example.com/search?limit=20', method: 'POST', body: { limit: 20 } } } };
  const canonical = stacJob.stacSource, selection = { ...canonical, snapshotId: refreshedId };
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => {
    calls.push({ command, args });
    if (command === 'stac_snapshot') return args.id === refreshedId ? refreshed : stacSnapshot;
    return command === 'save_stac_project' ? stacProject : { projectId: stacProject.id, assetKey: 'stac_asset', jobs: [{ ...stacJob, mediaType: stacSnapshot.assets[0].mediaType }] };
  } } } };
  try {
    const project = await stacRequest('project', { projectId: stacProject.id, bounds: stacProject.bounds, selections: [selection] });
    assert.deepEqual(project.canonicalSelections, [canonical]);
    assert.deepEqual(project.stacItems, stacProject.stacItems);
    assert.deepEqual(calls.filter(call => call.command === 'stac_snapshot').map(call => call.args.id), [refreshedId, snapshotId]);
    const downloads = await stacRequest('downloads', { projectId: stacProject.id, selections: [selection] });
    assert.deepEqual(downloads.jobs[0].stacSource, canonical);
  } finally { if (previous === undefined) delete globalThis.window; else globalThis.window = previous; }
});

test('canonical reuse rejects changed sources, original declarations, incomplete queues and cancellation', async () => {
  const previous = globalThis.window, refreshedId = 'e'.repeat(64), selection = { snapshotId: refreshedId, assetKey: 'surface_temp' };
  let refreshed = { ...stacSnapshot, id: refreshedId }, jobs = [stacJob], abort;
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => {
    if (command === 'stac_snapshot') { abort?.abort(); return args.id === refreshedId ? refreshed : stacSnapshot; }
    return command === 'save_stac_project' ? stacProject : { projectId: stacProject.id, assetKey: 'stac_asset', jobs };
  } } } };
  try {
    for (const change of [
      { connectionId: 'f2f1b1c2-0ee2-44ff-89d2-7351c142391a' },
      { provenance: { ...stacSnapshot.provenance, documentUrl: 'https://other.example.com/search' } },
      { properties: { ...stacSnapshot.properties, units: 'C' } },
      { geometry: { type: 'Point', coordinates: [10, 20] } }, { bbox: [10, 20, 10.5, 21] },
      { provenance: { ...stacSnapshot.provenance, collection: { id: 'temperature', license: 'changed' } } },
      { assets: [{ ...stacSnapshot.assets[0], metadata: { 'raster:bands': [{ unit: 'C' }] } }] },
      { assets: [{ ...stacSnapshot.assets[0], href: 'https://example.com/changed.tif' }] },
    ]) {
      refreshed = { ...stacSnapshot, id: refreshedId, ...change };
      await assert.rejects(stacRequest('project', { projectId: stacProject.id, bounds: stacProject.bounds, selections: [selection] }), /selected original assets/);
    }
    for (const receipt of [[], [stacJob, { ...stacJob, id: 'c0d69d66-388b-4b16-a10f-f0fc52a95d3b', stacSource: { ...stacJob.stacSource, snapshotId: 'd'.repeat(64) } }]]) {
      jobs = receipt;
      await assert.rejects(stacRequest('downloads', { projectId: stacProject.id, selections: [stacJob.stacSource] }), /saved raster selection/);
    }
    refreshed = { ...stacSnapshot, id: refreshedId }; abort = new AbortController();
    await assert.rejects(stacRequest('project', { projectId: stacProject.id, bounds: stacProject.bounds, selections: [selection] }, abort.signal), { name: 'AbortError' });
  } finally { if (previous === undefined) delete globalThis.window; else globalThis.window = previous; }
});
