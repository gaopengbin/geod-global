import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

export async function verifyBrand(root) {
  const files = [
    ['prototype/public/brand/geod-symbol.png', 'ad28ec7751fc8d6ae9abae81038ad0ad408a642439bc34d26528189b95958000'],
    ['src-tauri/icons/icon.png', 'e424e2fea2b024fb631552f2893dad64849b977d41d78f13239475e10d9f08cc'],
    ['src-tauri/icons/icon.ico', 'c7419b5b911bf88708ae87c5f38f591ae4c1140e6737adeb6c2b0a900f73b09b'],
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
  const config = JSON.parse(await readFile(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
  assert.deepEqual(config.bundle.icon, ['icons/icon.ico', 'icons/icon.png']);
  return { verifiedGeoDBrand: { assets: files.length, windowsIconSizes: [16, 24, 32, 48, 64, 256] } };
}
