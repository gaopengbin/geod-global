import test from 'node:test';
import assert from 'node:assert/strict';
import { downloadableAssets, formatBytes, runtimeRequest } from './runtime-client.js';

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

test('native command string errors become visible Error messages', async () => {
  globalThis.window = { __TAURI__: { core: { invoke: async () => { throw 'The completed file is missing'; } } } };
  try {
    await assert.rejects(runtimeRequest('reveal', { id: 'example' }), { name: 'Error', message: 'The completed file is missing' });
  } finally { delete globalThis.window; }
});
