import test from 'node:test';
import assert from 'node:assert/strict';
import { parseRecipeJSON, planMatches, processingRequest, RECIPE_SCHEMA, validatePlanResponse, validateRecipe } from './processing-client.js';

const recipe = () => ({ schemaVersion: RECIPE_SCHEMA, name: 'Bay classification clip', source: { jobId: '48bb6e18-3657-48ed-b62c-72472fb39d88', sha256: 'a'.repeat(64) }, operation: { type: 'clip', crs: 'source', bounds: [500000, 4100000, 500080, 4100080] }, output: { format: 'GeoTIFF' } });
const response = value => ({ recipe: structuredClone(value), plan: { width: 4, height: 4, crs: 'EPSG:32610', bounds: value.operation.bounds, pixelSize: [20, 20], window: [0, 0, 4, 4], requestedBounds: value.operation.bounds, requestedCrs: 'source', warnings: [], sourceSha256: value.source.sha256 } });

test('recipe import preserves a pinned local source and rejects paths or unsupported processing', () => {
  const input = recipe();
  assert.deepEqual(parseRecipeJSON(JSON.stringify(input)), input);
  for (const changed of [
    { ...input, path: 'C:/private/source.tif' },
    { ...input, source: { ...input.source, path: 'C:/private/source.tif' } },
    { ...input, schemaVersion: 'design-prototype/v1' },
    { ...input, operation: { ...input.operation, type: 'reproject' } },
    { ...input, output: { format: 'COG' } },
    { ...input, source: { ...input.source, sha256: 'unknown' } },
  ]) assert.throws(() => parseRecipeJSON(JSON.stringify(changed)));
  assert.throws(() => parseRecipeJSON('{ broken'), /not valid JSON/);
});

test('recipe validation handles Unicode names and rejects invalid or unsupported coordinate extents', () => {
  const input = recipe();
  assert.equal([...validateRecipe({ ...input, name: '🌍'.repeat(120) }).name].length, 120);
  assert.throws(() => validateRecipe({ ...input, name: '🌍'.repeat(121) }));
  assert.throws(() => validateRecipe({ ...input, name: 'name\ncommand' }));
  assert.equal(validateRecipe({ ...input, source: { jobId: input.source.jobId.toUpperCase(), sha256: 'A'.repeat(64) } }).source.jobId, input.source.jobId);
  for (const bounds of [[1, 0, 1, 2], [0, 0, 1, NaN], [0, 0, 1, null]]) {
    assert.throws(() => validateRecipe({ ...input, operation: { type: 'clip', crs: 'source', bounds } }));
  }
  for (const bounds of [[-122, -81, -121, 0], [-122, 0, -121, 85], [-170, 0, 170, 5], [170, 0, -170, 5]]) {
    assert.throws(() => validateRecipe({ ...input, operation: { type: 'clip', crs: 'EPSG:4326', bounds } }));
  }
});

test('editing any planned input invalidates the plan used to enable Save and Run', () => {
  const input = recipe();
  const review = validatePlanResponse(response(input), input);
  assert.equal(planMatches(input, review), true);
  const changes = [
    { ...input, name: 'Edited name' },
    { ...input, operation: { ...input.operation, bounds: [500000, 4100000, 500100, 4100080] } },
    { ...input, source: { ...input.source, sha256: 'b'.repeat(64) } },
  ];
  for (const change of changes) assert.equal(planMatches(change, review), false);
  assert.equal(planMatches(input, null), false);
  assert.throws(() => validatePlanResponse(response(changes[0]), input), /different recipe/);
});

test('plan response must describe a consistent real pixel window and pinned checksum', () => {
  const input = recipe();
  const valid = response(input);
  for (const changes of [{ width: 0 }, { window: [0, 0, 5, 4] }, { pixelSize: [0, 20] }, { sourceSha256: 'b'.repeat(64) }]) {
    assert.throws(() => validatePlanResponse({ ...valid, plan: { ...valid.plan, ...changes } }, input));
  }
});

test('HTTP recipe submission sends the validated recipe to the intended operation', async () => {
  const previousFetch = globalThis.fetch;
  const calls = [];
  const input = recipe();
  globalThis.fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => response(input) }; };
  try {
    const review = await processingRequest('plan', input);
    assert.equal(planMatches(input, review), true);
    assert.equal(calls[0].url, 'http://127.0.0.1:4318/recipes/plan');
    assert.equal(calls[0].options.method, 'POST');
    assert.equal(calls[0].options.headers['X-GeoD-Client'], 'geod-global');
    assert.deepEqual(JSON.parse(calls[0].options.body), input);
  } finally { globalThis.fetch = previousFetch; }
});

test('native recipe submission wraps only the validated recipe argument', async () => {
  const calls = [];
  const input = recipe();
  globalThis.window = { __TAURI__: { core: { invoke: async (command, payload) => { calls.push({ command, payload }); return { id: 'created-job' }; } } } };
  try {
    await processingRequest('run', input);
    assert.deepEqual(calls, [{ command: 'run_recipe', payload: { recipe: input } }]);
  } finally { delete globalThis.window; }
});
