import test from 'node:test';
import assert from 'node:assert/strict';
import { PROVIDERS } from './providers.js';
import { originalsReleased, RELEASE_VERSION } from './release-policy.js';

test('candidate includes public-source downloads and defers every protected source', () => {
  assert.equal(RELEASE_VERSION, '0.1.0-rc.3');
  for (const source of PROVIDERS) {
    assert.equal(originalsReleased(source), Boolean(source.download && !source.account), source.id);
    assert.equal(originalsReleased(source.id), originalsReleased(source));
  }
  for (const id of ['nasa-earthdata','copernicus','nasa-srtm','nasa-viirs-npp','nasa-viirs-noaa20','nasa-viirs-noaa21']) {
    assert.equal(originalsReleased(id), false);
  }
  assert.throws(() => originalsReleased('unreviewed-source'));
  assert.equal(originalsReleased(null), false);
});
