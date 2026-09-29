import test from 'node:test';
import assert from 'node:assert/strict';
import { MAX_PROJECT_SCENES, projectRequest } from './projects-client.js';

const source = 'https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2C_TEST/';
const scene = (id = 'S2C_TEST') => ({
  id, date: '2025-06-27T00:00:00Z', cloud: 5, crs: 'EPSG:32610',
  bbox: [-123, 37, -122, 38], properties: { 's2:mgrs_tile': '10SEG' },
  assets: {
    scl: { href: `${source}SCL.tif`, type: 'image/tiff; application=geotiff' },
    visual: { href: `${source}TCI.tif`, type: 'image/tiff; application=geotiff' },
    thumbnail: { href: `${source}preview.jpg`, type: 'image/jpeg' },
  },
});

test('project request preserves chosen scene provenance, both source types and polygon', () => {
  const geometry = { type: 'Polygon', coordinates: [[[-123, 37], [-122, 37], [-122, 38], [-123, 37]]] };
  const result = projectRequest({ scenes: [scene()], bounds: [-123, 37, -122, 38], geometry, name: ' Bay study ' });
  assert.equal(result.name, 'Bay study');
  assert.deepEqual(result.geometry, geometry);
  assert.deepEqual(Object.keys(result.scenes[0].assets), ['scl', 'visual']);
  assert.equal(result.scenes[0].gridCode, '10SEG');
});

test('project request refuses incomplete assets and excessive scene selections', () => {
  const incomplete = scene();
  delete incomplete.assets.visual;
  assert.throws(() => projectRequest({ scenes: [incomplete], bounds: [-123, 37, -122, 38], name: 'x' }), /both SCL and true-color/);
  assert.throws(() => projectRequest({ scenes: Array.from({ length: MAX_PROJECT_SCENES + 1 }, (_, index) => scene(`S2_${index}`)), bounds: [-123, 37, -122, 38], name: 'x' }), /Select 1 to/);
});
