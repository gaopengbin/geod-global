import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene } from './catalog.js';
import { vegetationPreview, vegetationBrowsePreview, vegetationTileBlob } from './vegetation-preview.js';
import { canDisplayImagery, providerById } from './providers.js';

const fixture = JSON.parse(readFileSync(new URL('../public/samples/modis-vegetation-response.json', import.meta.url)));
const scene = normalizeScene(fixture.features[0], 'planetary-vegetation');

test('online index preview pins the item and independent science band without enabling the RGB loader', () => {
  for (const index of ['ndvi', 'evi']) {
    const preview = vegetationPreview(scene, index);
    const url = new URL(preview.url.replace('{z}/{x}/{y}', '9/81/197'));
    assert.equal(url.origin, 'https://planetarycomputer.microsoft.com');
    assert.equal(url.searchParams.get('item'), scene.id);
    assert.equal(url.searchParams.get('collection'), 'modis-13Q1-061');
    assert.equal(url.searchParams.get('assets'), `250m_16_days_${index.toUpperCase()}`);
    assert.equal(url.searchParams.get('rescale'), '-2000,10000');
    assert.equal(url.searchParams.get('unscale'), 'false');
    assert.equal(url.searchParams.get('nodata'), '-3000');
    assert.equal(url.searchParams.get('return_mask'), 'true');
    assert.equal(url.searchParams.get('reproject'), 'nearest');
    assert(!url.searchParams.has('expression'));
    assert.notEqual(preview.bounds, scene.bbox);
  }
  assert.equal(canDisplayImagery(providerById('planetary-vegetation')), false);
  const browse = new URL(vegetationBrowsePreview(scene));
  assert.equal(browse.pathname, '/api/data/v1/item/preview.png');
  assert.equal(browse.searchParams.get('item'), scene.id);
  assert.equal(browse.searchParams.get('assets'), '250m_16_days_NDVI');
  assert.equal(browse.searchParams.get('colormap_name'), 'rdylgn');
  assert.equal(browse.searchParams.get('rescale'), '-2000,10000');
  assert.equal(browse.searchParams.get('max_size'), '128');
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
