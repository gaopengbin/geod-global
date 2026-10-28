import test from 'node:test';
import assert from 'node:assert/strict';
import { PROVIDERS } from './providers.js';
import { originalsReleased, RELEASE_VERSION, protectedOriginalsDeferred } from './release-policy.js';

test('accepted NASA original workflows retain account requirements while unverified Copernicus stays deferred', () => {
  assert.equal(RELEASE_VERSION, '0.2.4');
  for (const source of PROVIDERS) {
    assert.equal(originalsReleased(source), Boolean(source.download && source.id!=='copernicus'), source.id);
    assert.equal(originalsReleased(source.id), originalsReleased(source));
  }
  for (const id of ['nasa-earthdata','nasa-srtm','nasa-viirs-npp','nasa-viirs-noaa20','nasa-viirs-noaa21']) {
    assert.equal(originalsReleased(id), true);assert.equal(protectedOriginalsDeferred(id),false);
    assert.equal(PROVIDERS.find(p=>p.id===id).account,'nasa-earthdata');
  }
  assert.equal(originalsReleased('copernicus'),false);assert.equal(protectedOriginalsDeferred('copernicus'),true);
  assert.throws(() => originalsReleased('unreviewed-source'));
  assert.equal(originalsReleased(null), false);
});
