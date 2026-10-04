import test from 'node:test';
import assert from 'node:assert/strict';
import { validateWcsConnection, validateWcsDescription, validateWcsPlan, wcsRequest } from './wcs-client.js';
import { jobsForProject } from './projects-client.js';
import { wcsConnection, wcsDescription, wcsPlan, wcsProject, wcsJob, wcsInspection } from './wcs-fixtures.js';
test('WCS discovery and range metadata retain opaque identities, declared units and nil reasons', () => {
  assert.equal(validateWcsConnection(wcsConnection), wcsConnection);
  assert.equal(validateWcsDescription(wcsDescription), wcsDescription);
  assert.throws(() => validateWcsConnection({ ...wcsConnection, version: '1.0.0' }));
  assert.throws(() => validateWcsConnection({ ...wcsConnection, coverages: [...wcsConnection.coverages, ...wcsConnection.coverages] }));
  assert.throws(() => validateWcsConnection({ ...wcsConnection, coverageUrl: 'https://example.com/wcs?token=secret' }));
  assert.throws(() => validateWcsDescription({ ...wcsDescription, crs: 'EPSG:27700' }));
  assert.throws(() => validateWcsDescription({ ...wcsDescription, width: 101 }));
  assert.equal(validateWcsDescription({ ...wcsDescription, fields: [{ ...wcsDescription.fields[0], unit: null, nilValues: [] }] }).fields[0].unit, null);
  assert.deepEqual(validateWcsDescription({ ...wcsDescription, metadataLinks: ['../metadata.xml', 'urn:example:coverage', 'http://example.com/metadata'] }).metadataLinks, ['../metadata.xml', 'urn:example:coverage', 'http://example.com/metadata']);
  assert.throws(() => validateWcsDescription({ ...wcsDescription, metadataLinks: ['x'.repeat(8193)] }));
});
test('WCS plans distinguish actual native grid from requested area and bound predicted pixels and samples', () => {
  assert.equal(validateWcsPlan(wcsPlan), wcsPlan);
  assert.equal(validateWcsPlan({ ...wcsPlan, requestedBounds: [10.201,20.601,10.399,20.699] }).width, 20);
  for (const change of [{ id: wcsConnection.id }, { selection: 'rendered-map' }, { format: 'image/png' }, { format: null }, { transform: [0.02,0,10.2,0,-0.01,20.7] }]) assert.throws(() => validateWcsPlan({ ...wcsPlan, ...change }));
  const large = { ...wcsPlan, width: 4096, height: 4096, transform: [0.01,0,0,0,-0.01,42], nativeBounds: [0,1.04,40.96,42] };
  assert.equal(validateWcsPlan(large).width, 4096);
  const wide = { ...wcsPlan, width: 65536, height: 8192, transform: [0.001,0,0,0,-0.001,42], nativeBounds: [0,33.808,65.536,42] };
  assert.equal(validateWcsPlan(wide).width, 65536);
  assert.throws(() => validateWcsPlan({ ...wide, description: { ...wcsDescription, fields: [...wcsDescription.fields, { ...wcsDescription.fields[0], name: 'second' }] } }));
  assert.throws(() => validateWcsPlan({ ...wide, width: 65537, nativeBounds: [0,33.808,65.537,42] }));
});
test('WCS job membership uses the pinned plan and exact GetCoverage URL, not the coverage ID alone', () => {
  const different = { ...wcsJob, id: 'other', wcsSource: { planId: 'f'.repeat(64) } };
  const wrongUrl = { ...wcsJob, id: 'wrong-url', href: 'https://example.com/other' };
  assert.deepEqual(jobsForProject(wcsProject, [wcsJob, different, wrongUrl]), [wcsJob]);
});
test('WCS native request envelopes bind descriptions, plans and generic file inspection to their own source', async () => {
  const previous = globalThis.window, calls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => { calls.push({ command, args }); return ({ describe_wcs: wcsDescription, plan_wcs: wcsPlan, wcs_plan: wcsPlan, inspect_wcs_asset: wcsInspection, save_wcs_project: wcsProject, download_wcs_project: { projectId: wcsProject.id, assetKey: 'wcs_coverage', jobs: [wcsJob] } })[command]; } } } };
  try {
    const describe = { connectionId: wcsConnection.id, coverageId: wcsDescription.coverageId };
    await wcsRequest('describe', describe); assert.deepEqual(calls[0], { command: 'describe_wcs', args: { request: describe } });
    await wcsRequest('plan', { descriptionId: wcsDescription.id, bounds: wcsPlan.requestedBounds });
    await wcsRequest('savedPlan', { id: wcsPlan.id }); assert.deepEqual(calls[2], { command: 'wcs_plan', args: { id: wcsPlan.id } });
    await wcsRequest('inspect', { id: wcsJob.id });
    await assert.rejects(wcsRequest('plan', { descriptionId: wcsDescription.id, bounds: [10,20,11,21] }), /does not match/);
    await assert.rejects(wcsRequest('describe', { ...describe, coverageId: 'other' }), /different coverage/);
    await wcsRequest('project', { name: wcsProject.name, bounds: wcsProject.bounds, selections: [{ planId: wcsPlan.id }] });
    await wcsRequest('downloads', { projectId: wcsProject.id, selections: [{ planId: wcsPlan.id }] });
  } finally { if (previous === undefined) delete globalThis.window; else globalThis.window = previous; }
});
test('WCS loopback routes keep source acquisition explicit and saved plan reads local', async () => {
  const previous = globalThis.fetch, calls = [];
  globalThis.fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => wcsPlan }; };
  try { await wcsRequest('savedPlan', { id: wcsPlan.id }); assert.equal(calls[0].url, `http://127.0.0.1:4318/wcs/plans/${wcsPlan.id}`); assert.equal(calls[0].options.method, 'GET');
    await wcsRequest('plan', { descriptionId: wcsDescription.id, bounds: wcsPlan.requestedBounds }); assert.equal(calls[1].url, 'http://127.0.0.1:4318/wcs/plan'); assert.equal(calls[1].options.headers['X-GeoD-Client'], 'geod-global');
  } finally { globalThis.fetch = previous; }
});
