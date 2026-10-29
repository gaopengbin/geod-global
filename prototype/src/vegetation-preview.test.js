import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene } from './catalog.js';
import { vegetationPreview, vegetationBrowsePreview, vegetationTileBlob } from './vegetation-preview.js';
import { canDisplayImagery, providerById } from './providers.js';

const fixture = JSON.parse(readFileSync(new URL('../public/samples/modis-vegetation-response.json', import.meta.url)));
const scene = normalizeScene(fixture.features[0], 'planetary-vegetation');

test('index preview pins the original science band and avoids the rejected managed tiler without enabling RGB', () => {
  for (const index of ['ndvi', 'evi']) {
    const preview = vegetationPreview(scene, index);
    assert.equal(preview.itemId,scene.id);assert.equal(preview.index,index);
    assert.deepEqual(preview.hrefs,[scene.assets[index].href]);
    assert.equal(preview.url,undefined);assert.equal(preview.browseURL,undefined);
    assert.notEqual(preview.bounds, scene.bbox);
  }
  assert.equal(canDisplayImagery(providerById('planetary-vegetation')), false);
  assert.equal(vegetationBrowsePreview(scene),null);
});

test('a substituted item, channel, host, calibration or geographic bound cannot become a preview', () => {
  for (const mutate of [
    value => value.provider = 'planetary',
    value => value.collection = 'modis-09A1-061',
    value => value.crs = 'EPSG:4326',
    value => value.id = value.id.replace('h08v05', 'h09v05'),
    value => value.assets.ndvi.href = value.assets.evi.href,
    value => value.assets.ndvi.href = value.assets.ndvi.href.replace('modiseuwest.blob.core.windows.net', 'example.com'),
    value => value.assets.ndvi.rasterBand.dataType = 'uint16',
    value => value.assets.ndvi.rasterBand.scale = 1,
    value => value.assets.ndvi.rasterBand.nodata = -32768,
    value => value.assets.ndvi.rasterBand.offset = 1,
    value => value.bbox = [-200, 30, -103, 40],
    value => value.bbox = [-130, 40, -103, 30],
  ]) {
    const invalid = structuredClone(scene); mutate(invalid);
    assert.throws(() => vegetationPreview(invalid, 'ndvi'), /no verified NDVI\/EVI/);
  }
  assert.throws(() => vegetationPreview(scene, 'vi_quality'), /no verified/);
  assert.throws(() => vegetationPreview(scene, 'NDVI'), /no verified/);
  assert.equal(vegetationBrowsePreview({ id: scene.id, assets: {} }), null);
});

test('only the documented out-of-bounds response becomes transparent; failures remain errors', async () => {
  const request = async body => new Response(JSON.stringify(body), { status: 404, headers: { 'content-type': 'application/json' } });
  assert.equal(await vegetationTileBlob('https://example.test', undefined, () => request({ detail: 'Tile(x=107, y=193, z=9) is outside bounds' })), null);
  for (const response of [
    await request({ detail: 'Item not found' }),
    new Response('busy', { status: 429 }),
    new Response('error', { status: 500 }),
    new Response('<html>login</html>', { headers: { 'content-type': 'text/html' } }),
  ]) await assert.rejects(vegetationTileBlob('https://example.test', undefined, async () => response), /index preview tiles/);
});

test('preview requests omit credentials and respect abort signals', async () => {
  const controller = new AbortController();
  let options;
  const blob = await vegetationTileBlob('https://example.test', controller.signal, async (_url, init) => {
    options = init;
    return new Response(new Uint8Array([137, 80, 78, 71]), { headers: { 'content-type': 'image/png' } });
  });
  assert.equal(blob.type, 'image/png');
  assert.equal(options.signal, controller.signal);
  assert.equal(options.credentials, 'omit');
  assert.equal(options.redirect, 'error');
  controller.abort();
  await assert.rejects(vegetationTileBlob('https://example.test', controller.signal, async (_url, init) => { init.signal.throwIfAborted(); }), { name: 'AbortError' });
});
