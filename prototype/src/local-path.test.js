import test from 'node:test';
import assert from 'node:assert/strict';
import { displayLocalPath } from './local-path.js';

test('display hides Windows internal path prefixes while preserving drive and UNC paths', () => {
  assert.equal(displayLocalPath(String.raw`\\?\G:\data\scene.tif`), String.raw`G:\data\scene.tif`);
  assert.equal(displayLocalPath(String.raw`\\?\UNC\server\share\scene.tif`), String.raw`\\server\share\scene.tif`);
  for (const path of [String.raw`G:\data\scene.tif`, String.raw`\\server\share\scene.tif`, '/data/scene.tif']) assert.equal(displayLocalPath(path), path);
  assert.equal(displayLocalPath(null), '');
});
