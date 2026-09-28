import test from 'node:test';
import assert from 'node:assert/strict';
import { administrativePlace, searchAdministrativePlaces } from './admin-areas.js';
import { readFileSync } from 'node:fs';
import GeoJSON from 'ol/format/GeoJSON.js';
import { POLYGON_RECIPE_SCHEMA, validateRecipe } from './processing-client.js';

test('administrative place search accepts localized names and keeps country results ahead of provinces', () => {
  const feature = (properties, bounds) => ({ getProperties: () => properties, getGeometry: () => ({ getExtent: () => bounds }) });
  const china = administrativePlace(feature({ ADM0_A3: 'CHN', NAME_EN: 'China', NAME_ZH: '中国' }, [73, 18, 135, 54]), 'country');
  const province = administrativePlace(feature({ adm1_code: 'CHN-123', adm0_a3: 'CHN', name_en: 'Guangdong', name_zh: '广东' }, [109, 20, 118, 25]), 'province');
  assert.deepEqual(searchAdministrativePlaces([province, china], '中国'), [china]);
  assert.deepEqual(searchAdministrativePlaces([province, china], 'Guangdong'), [province]);
  assert.deepEqual(searchAdministrativePlaces([province, china], 'chn').map(item => item.kind), ['country', 'province']);
  assert.deepEqual(searchAdministrativePlaces([province, china], ''), []);
});

test('a bundled real administrative boundary remains a reusable v2 clip polygon', () => {
  const file = JSON.parse(readFileSync(new URL('../public/basemaps/natural-earth-50m-admin-1-states-provinces.geojson', import.meta.url)));
  const original = file.features.find(feature => feature.properties.name_en === 'California');
  const format = new GeoJSON();
  const feature = format.readFeature(original, { dataProjection: 'EPSG:4326', featureProjection: 'EPSG:4326' });
  const place = administrativePlace(feature, 'province');
  assert.equal(searchAdministrativePlaces([place], 'California')[0], place);
  const geometry = format.writeGeometryObject(feature.getGeometry(), { dataProjection: 'EPSG:4326', featureProjection: 'EPSG:4326' });
  const recipe = validateRecipe({ schemaVersion: POLYGON_RECIPE_SCHEMA, name: 'California clip', source: { jobId: '48bb6e18-3657-48ed-b62c-72472fb39d88', sha256: 'a'.repeat(64) }, operation: { type: 'clip', crs: 'EPSG:4326', bounds: place.bounds, geometry }, output: { format: 'GeoTIFF' } });
  assert.deepEqual(recipe.operation.geometry, original.geometry);
  assert.equal(recipe.operation.bounds[0], original.bbox[0]);
});
