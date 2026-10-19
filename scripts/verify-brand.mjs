import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

export async function verifyBrand(root) {
  const files = [
    ['prototype/public/brand/geod-symbol.png', '0207e0d9e9ca020de08b98aed0d0ce7013255927f8ed7c88aca404e2d4bd1960'],
    ['src-tauri/icons/icon.png', '0207e0d9e9ca020de08b98aed0d0ce7013255927f8ed7c88aca404e2d4bd1960'],
    ['src-tauri/icons/icon.ico', '72dcbe0b23c6b9fcda90221f027752a460bf6329ee50fa71a06a6de9b58354b7'],
    ['prototype/public/brand/global-space-v1.webp', '44bf22dac960380d1e75cfb10b90c3f464467fb52b9470bec2171d94fd02e8f6'],
    ['prototype/public/brand/LICENSE', 'f608de0cfd6e3b735bd87eed58df98edabac405242770b02330f87a2c6c77191'],
  ];
  for (const [file, hash] of files) {
    const bytes = await readFile(path.join(root, file));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), hash, `GeoD brand asset changed: ${file}`);
    if (file.endsWith('.png')) {
      assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
      assert.equal(bytes.readUInt32BE(16), bytes.readUInt32BE(20), 'Application mark must be square');
      assert.equal(bytes[25], 6, 'GeoD PNG must preserve RGBA transparency');
    }
    if (file.endsWith('.ico')) {
      assert.equal(bytes.readUInt16LE(2), 1, 'Windows icon resource type');
      const count = bytes.readUInt16LE(4);
      const sizes = new Set(Array.from({ length: count }, (_, i) => bytes[6 + i * 16] || 256));
      for (const size of [16, 24, 32, 48, 64, 256]) assert(sizes.has(size), `Missing Windows icon size ${size}`);
    }
  }
  const index = await readFile(path.join(root, 'prototype/index.html'), 'utf8');
  assert(index.includes('brand/geod-symbol.png'), 'Favicon must use the approved GeoD mark');
  const identityCss = await readFile(path.join(root, 'prototype/src/identity.css'), 'utf8');
  assert(identityCss.includes('/brand/global-space-v1.webp'), 'Sign-in must use the Global kit background');
  const config = JSON.parse(await readFile(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
  assert.deepEqual(config.bundle.icon, ['icons/icon.ico', 'icons/icon.png']);
  return { verifiedGeoDBrand: { assets: files.length, windowsIconSizes: [16, 24, 32, 48, 64, 256] } };
}
