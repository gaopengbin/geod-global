import test from 'node:test';
import assert from 'node:assert/strict';
import { administrativePlace, searchAdministrativePlaces } from './admin-areas.js';

test('administrative place search accepts localized names and keeps country results ahead of provinces', () => {
  const feature = (properties, bounds) => ({ getProperties: () => properties, getGeometry: () => ({ getExtent: () => bounds }) });
  const china = administrativePlace(feature({ ADM0_A3: 'CHN', NAME_EN: 'China', NAME_ZH: '中国' }, [73, 18, 135, 54]), 'country');
  const province = administrativePlace(feature({ adm1_code: 'CHN-123', adm0_a3: 'CHN', name_en: 'Guangdong', name_zh: '广东' }, [109, 20, 118, 25]), 'province');
  assert.deepEqual(searchAdministrativePlaces([province, china], '中国'), [china]);
  assert.deepEqual(searchAdministrativePlaces([province, china], 'Guangdong'), [province]);
  assert.deepEqual(searchAdministrativePlaces([province, china], 'chn').map(item => item.kind), ['country', 'province']);
  assert.deepEqual(searchAdministrativePlaces([province, china], ''), []);
});
