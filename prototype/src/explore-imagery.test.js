import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene, compatibleScenes } from './catalog.js';
import { imageryHrefs, supportedImageryGrid, validateLandsatImages, landsatColorStyle } from './explore-imagery.js';

const fixture = JSON.parse(readFileSync(new URL('../public/samples/landsat-response.json', import.meta.url)));
const scene = normalizeScene(fixture.features[0], 'planetary-landsat');
const copy = () => structuredClone(scene);

test('Landsat map requires original B4/B3/B2 from the selected product and its supported grid', () => {
  assert.equal(supportedImageryGrid(scene), true);
  assert.equal(imageryHrefs(scene).length, 3);
  for (const change of [s => delete s.assets.green,
    s => { s.assets.blue.href = s.assets.red.href; },
    s => { s.assets.red.href = s.assets.red.href.replaceAll('20250628', '20250620'); },
    s => { s.assets.red['raster:bands'][0].scale = 0.0001; },
    s => { s.grid.transform[0] = 10; },
    s => { s.grid.transform[1] = 1; },
    s => { s.grid.shape[0] = 30000; },
    s => { s.crs = 'EPSG:4326'; },
    s => { s.assets.blue.href += '?sig=do-not-persist'; }]) {
    const invalid = copy(); change(invalid); assert.deepEqual(imageryHrefs(invalid), []);
  }
  const other = copy(); other.id = 'LC09_L2SP_044034_20250620_02_T1';
  for (const key of ['red', 'green', 'blue']) other.assets[key].href = other.assets[key].href.replaceAll('20250628', '20250620');
  assert.equal(compatibleScenes(scene, other), true);
  assert.equal(compatibleScenes(scene, { ...other, assets: {} }), false);
});

function image(type = 2, overrides = {}) {
  return {
    getWidth: () => scene.grid.shape[1], getHeight: () => scene.grid.shape[0], getSamplesPerPixel: () => 1,
    getGeoKeys: () => ({ GTRasterTypeGeoKey: type, ProjectedCSTypeGeoKey: 32610 }),
    fileDirectory: { getValue: key => ({ BitsPerSample: [16], SampleFormat: [1] })[key] },
    getGDALNoData: () => 0, getResolution: () => [30, -30, 0],
    getOrigin: () => [scene.grid.transform[2] + (type === 2 ? 15 : 0), scene.grid.transform[5] - (type === 2 ? 15 : 0), 0],
    ...overrides,
  };
}
test('COG geometry validation distinguishes Point tiepoints from Area edges and rejects mixed grids', () => {
  assert.deepEqual(validateLandsatImages([[image()], [image()], [image()]], scene), [1, 0, 0, 1, 15, -15]);
  assert.equal(validateLandsatImages([[image(1)], [image(1)], [image(1)]], scene), undefined);
  for (const bad of [image(1), image(2, { getWidth: () => 1 }), image(2, { getGDALNoData: () => -9999 }),
    image(2, { getOrigin: () => [scene.grid.transform[2], scene.grid.transform[5], 0] }),
    image(2, { getResolution: () => [10, -10, 0] }),
    image(2, { getGeoKeys: () => ({ GTRasterTypeGeoKey: 2, ProjectedCSTypeGeoKey: 32710 }) }),
    image(2, { fileDirectory: { getValue: key => ({ BitsPerSample: [16], SampleFormat: [2] })[key] } }),
    image(2, { fileDirectory: { getValue: key => ({ BitsPerSample: [8] })[key] } }),
    image(2, { fileDirectory: { getValue: key => ({ BitsPerSample: [16], ModelTransformation: [1] })[key] } })]) {
    assert.throws(() => validateLandsatImages([[image()], [bad], [image()]], scene), /metadata|geometry/);
  }
  assert.throws(() => validateLandsatImages([[image()]], scene), /three matching/);
});

function evaluate(expression, bands) {
  if (!Array.isArray(expression)) return expression;
  const [op, ...args] = expression, values = args.map(arg => evaluate(arg, bands));
  return ({ band: () => bands[values[0] - 1], '*': () => values[0] * values[1], '+': () => values[0] + values[1],
    '/': () => values[0] / values[1], '^': () => values[0] ** values[1], clamp: () => Math.max(values[1], Math.min(values[2], values[0])),
    '!=': () => values[0] !== values[1], all: () => values.every(Boolean), case: () => values[0] ? values[1] : values[2],
    color: () => values })[op]();
}
test('Landsat display calibrates all three raw channels without altering DN and masks any NoData band', () => {
  const bands = [10000, 14000, 18000], original = [...bands];
  const color = evaluate(landsatColorStyle().color, bands);
  bands.forEach((dn, index) => assert.ok(Math.abs(color[index] - 255 * (((dn * 0.0000275 - 0.2) / 0.3) ** (1 / 2.2))) < 1e-9));
  assert.equal(color[3], 1); assert.deepEqual(bands, original);
  assert.deepEqual(evaluate(landsatColorStyle().color, [1, 65535, 0]), [0, 255, 0, 0]);
});
