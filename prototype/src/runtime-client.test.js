import test from 'node:test';
import assert from 'node:assert/strict';
import { downloadableAssets, formatBytes, formatClassShare, runtimeRequest, syncDesktopLocale, validateRasterInspection } from './runtime-client.js';

test('tray language synchronization is desktop-only and accepts supported languages', async () => {
  const previousFetch = globalThis.fetch;
  globalThis.fetch = () => { throw new Error('Browser must not contact a service for tray settings'); };
  try {
    await syncDesktopLocale('en');
    const calls = [];
    globalThis.window = { __TAURI__: { core: { invoke: async (...args) => calls.push(args) } } };
    await syncDesktopLocale('zh-CN');
    await syncDesktopLocale('en');
    await assert.rejects(syncDesktopLocale('../invalid'), /Unsupported/);
    assert.deepEqual(calls, [['set_desktop_locale', { locale: 'zh-CN' }], ['set_desktop_locale', { locale: 'en' }]]);
  } finally { delete globalThis.window; globalThis.fetch = previousFetch; }
});

test('only supported original Sentinel assets enter download review', () => {
  const allowed = 'https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/test/SCL.tif';
  assert.deepEqual(downloadableAssets({ assets: { scl: { href: allowed, type: 'image/tiff' }, visual: { href: 'http://127.0.0.1/private' }, thumbnail: { href: 'https://sentinel-cogs.s3.us-west-2.amazonaws.com.evil.test/preview.jpg' }, red: { href: allowed } } }).map(asset => asset.key), ['scl']);
  assert.deepEqual(downloadableAssets(null), []);
});

test('unknown transfer size never becomes a fabricated percentage or zero', () => {
  assert.equal(formatBytes(null), 'Unknown size');
  assert.equal(formatBytes(undefined), 'Unknown size');
  assert.equal(formatBytes(0), '0 B');
  assert.equal(formatBytes(1048576), '1.0 MiB');
});

test('small nonzero scene classes never round down to zero percent', () => {
  assert.equal(formatClassShare(146, 30140100), '<0.01%');
  assert.equal(formatClassShare(256, 30140100, 'zh-CN'), '<0.01%');
  assert.equal(formatClassShare(0, 30140100), '0%');
  assert.equal(formatClassShare(10, 100), '10%');
});

test('native command string errors become visible Error messages', async () => {
  globalThis.window = { __TAURI__: { core: { invoke: async () => { throw 'The completed file is missing'; } } } };
  try {
    await assert.rejects(runtimeRequest('reveal', { id: 'example' }), { name: 'Error', message: 'The completed file is missing' });
  } finally { delete globalThis.window; }
});

test('proxy settings use the same payload contract in desktop and browser runtimes', async () => {
  const settings = { mode: 'custom', url: 'http://127.0.0.1:10808' };
  const nativeCalls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, payload) => {
    nativeCalls.push({ command, payload });
    return settings;
  } } } };
  try {
    await runtimeRequest('proxy');
    await runtimeRequest('saveProxy', settings);
    await runtimeRequest('testProxy', settings);
    assert.deepEqual(nativeCalls, [
      { command: 'get_proxy_settings', payload: {} },
      { command: 'save_proxy_settings', payload: { settings } },
      { command: 'test_proxy_settings', payload: { settings } },
    ]);
  } finally { delete globalThis.window; }

  const previousFetch = globalThis.fetch;
  const browserCalls = [];
  globalThis.fetch = async (url, options) => {
    browserCalls.push({ url, options });
    return { ok: true, json: async () => settings };
  };
  try {
    await runtimeRequest('proxy');
    await runtimeRequest('saveProxy', settings);
    await runtimeRequest('testProxy', settings);
    assert.deepEqual(browserCalls.map(call => [call.url, call.options.method, call.options.body]), [
      ['http://127.0.0.1:4318/proxy', 'GET', undefined],
      ['http://127.0.0.1:4318/proxy', 'POST', JSON.stringify(settings)],
      ['http://127.0.0.1:4318/proxy/test', 'POST', JSON.stringify(settings)],
    ]);
    assert.equal(browserCalls[1].options.headers['X-GeoD-Client'], 'geod-global');
  } finally { globalThis.fetch = previousFetch; }
});

const rasterFixture = () => ({
  width: 4, height: 4, bandCount: 1, dataType: 'UInt8', crs: 'EPSG:32610',
  bounds: [500000, 4100000, 500080, 4100080], pixelSize: [20, 20], nodata: 0,
  previewDataUrl: 'data:image/png;base64,iVBORw0KGgo=', previewWidth: 4, previewHeight: 4,
  classes: [{ value: 0, label: 'No data', color: '#000000', count: 4 }, { value: 4, label: 'Vegetation', color: '#008000', count: 12 }],
  sha256: 'a'.repeat(64),
});

test('project scene additions and selected downloads use matching native and HTTP contracts', async () => {
  const scenes = [{ itemId: 'new' }];
  const calls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, payload) => calls.push([command, payload]) } } };
  try {
    await runtimeRequest('addProjectScenes', { id: 'project-id', scenes });
    await runtimeRequest('downloadProject', { id: 'project-id', assetKey: 'scl', itemIds: ['new'] });
    assert.deepEqual(calls, [['add_project_scenes', { id: 'project-id', request: { scenes } }], ['download_project', { id: 'project-id', assetKey: 'scl', itemIds: ['new'] }]]);
  } finally { delete globalThis.window; }
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => { calls.push([url, options]); return { ok: true, json: async () => ({}) }; };
  try {
    await runtimeRequest('addProjectScenes', { id: 'project/id', scenes });
    await runtimeRequest('downloadProject', { id: 'project/id', assetKey: 'scl', itemIds: ['new'] });
    assert.equal(calls[2][0], 'http://127.0.0.1:4318/projects/project%2Fid/scenes');
    assert.equal(calls[2][1].body, JSON.stringify({ scenes }));
    assert.equal(calls[2][1].headers['X-GeoD-Client'], 'geod-global');
    assert.equal(calls[3][1].body, JSON.stringify({ assetKey: 'scl', itemIds: ['new'] }));
  } finally { globalThis.fetch = previousFetch; }
});

test('project rename uses matching native and protected HTTP contracts', async () => {
  const calls = [];
  globalThis.window = { __TAURI__: { core: { invoke: async (command, payload) => calls.push([command, payload]) } } };
  try {
    await runtimeRequest('renameProject', { id: 'project-id', name: 'New name' });
    assert.deepEqual(calls, [['rename_project', { id: 'project-id', name: 'New name' }]]);
  } finally { delete globalThis.window; }
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => { calls.push([url, options]); return { ok: true, json: async () => ({}) }; };
  try {
    await runtimeRequest('renameProject', { id: 'project/id', name: 'New name' });
    assert.equal(calls[1][0], 'http://127.0.0.1:4318/projects/project%2Fid/rename');
    assert.equal(calls[1][1].method, 'POST');
    assert.equal(calls[1][1].body, JSON.stringify({ name: 'New name' }));
    assert.equal(calls[1][1].headers['X-GeoD-Client'], 'geod-global');
  } finally { globalThis.fetch = previousFetch; }
});

test('raster inspection accepts only complete pixel metadata and local PNG previews', () => {
  const fixture = rasterFixture();
  assert.equal(validateRasterInspection(fixture), fixture);
  for (const changes of [
    { previewDataUrl: 'https://example.com/provider-thumbnail.jpg' },
    { previewDataUrl: 'data:image/svg+xml;base64,AAAA' },
    { previewWidth: 769 },
    { bounds: [0, 1, 2] },
    { pixelSize: [0, 20] },
    { classes: [{ value: 4, label: 'Vegetation', color: '#008000', count: 12 }] },
    { sha256: 'not-a-checksum' },
  ]) assert.throws(() => validateRasterInspection({ ...fixture, ...changes }), /invalid inspection data/);
});

test('RGB inspection accepts three channels without fabricating SCL classes', () => {
  const rgb = { ...rasterFixture(), bandCount: 3, classes: [] };
  assert.equal(validateRasterInspection(rgb), rgb);
  assert.throws(() => validateRasterInspection({ ...rgb, bandCount: 4 }), /invalid inspection data/);
  assert.throws(() => validateRasterInspection({ ...rgb, classes: rasterFixture().classes }), /invalid inspection data/);
  assert.throws(() => validateRasterInspection({ ...rgb, bandCount: 1 }), /invalid inspection data/);
});

test('native raster inspection sends only the job identifier and can stop waiting', async () => {
  const calls = [];
  let finish;
  globalThis.window = { __TAURI__: { core: { invoke: (command, payload) => {
    calls.push({ command, payload });
    return new Promise(resolve => { finish = resolve; });
  } } } };
  try {
    const controller = new AbortController();
    const pending = runtimeRequest('raster', { id: 'job-123' }, controller.signal);
    controller.abort();
    await assert.rejects(pending, { name: 'AbortError' });
    finish(rasterFixture());
    assert.deepEqual(calls, [{ command: 'inspect_raster', payload: { id: 'job-123' } }]);
  } finally { delete globalThis.window; }
});

test('browser raster inspection uses the job resource route and validates its response', async () => {
  const previousFetch = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return { ok: true, json: async () => rasterFixture() };
  };
  try {
    const result = await runtimeRequest('raster', { id: 'job/123' });
    assert.equal(result.width, 4);
    assert.equal(calls[0].url, 'http://127.0.0.1:4318/jobs/job%2F123/raster');
    assert.equal(calls[0].options.method, 'GET');
    assert.equal(calls[0].options.body, undefined);
    assert.ok(calls[0].options.signal instanceof AbortSignal);
  } finally { globalThis.fetch = previousFetch; }
});
