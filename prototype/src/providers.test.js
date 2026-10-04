import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { assetReadURL, isSupportedAsset, prepareAssetAccess, providerForSearch, providerById, canDisplayImagery } from './providers.js';
import { INITIAL_SEARCH, fetchCatalogPage, normalizeScene, nextPageURL, searchURL, createSearchRunner } from './catalog.js';
import { downloadableAssets } from './runtime-client.js';
import { projectExploreSearch, projectCatalogScenes } from './project-explore.js';
import { projectRequest } from './projects-client.js';
import { prepareOriginalScenes } from './protected-sources.js';

const pc = JSON.parse(readFileSync(new URL('../public/samples/planetary-computer-response.json', import.meta.url)));
const cdse = JSON.parse(readFileSync(new URL('../public/samples/copernicus-response.json', import.meta.url)));
const landsat = JSON.parse(readFileSync(new URL('../public/samples/landsat-response.json', import.meta.url)));
const nasa = JSON.parse(readFileSync(new URL('../public/samples/nasa-response.json', import.meta.url)));
test('source-specific catalogue queries preserve area dates and cloud bounds and pin pagination to that source', async () => {
  for (const [provider, fixture] of [['planetary-computer', pc], ['copernicus', cdse], ['planetary-landsat', landsat], ['nasa-earthdata', nasa]]) {
    const url = searchURL({ ...INITIAL_SEARCH, provider });
    assert.equal(providerForSearch(url).id, provider);
    assert.equal(new URL(url).searchParams.get('bbox'), '-122.55,37.68,-122.32,37.84');
    const response = await fetchCatalogPage(url, { fetcher: async () => ({ ok: true, json: async () => fixture }) });
    assert.equal(response.scenes[0].provider, provider);
    assert.equal(providerForSearch(response.next).id, provider);
    assert.throws(() => nextPageURL({ links: [{ rel: 'next', href: searchURL(INITIAL_SEARCH) }] }, provider));
    assert.throws(() => nextPageURL({ links: [{ rel: 'next', href: `${url}#fragment` }] }, provider));
  }
  assert.ok(new URL(searchURL({ ...INITIAL_SEARCH, provider: 'copernicus' })).searchParams.get('filter'));
});
test('Landsat original bands preserve UInt16 reflectance conversion and restore the Landsat collection', () => {
  const scene = normalizeScene(landsat.features[0], 'planetary-landsat');
  assert.deepEqual(downloadableAssets(scene).map(asset => asset.key), ['red', 'green', 'blue']);
  assert.equal(scene.gsd, 30);
  assert.equal(scene.crs, 'EPSG:32610');
  assert.equal(canDisplayImagery(providerById(scene.provider)), true);
  const project = projectRequest({ scenes: [scene], bounds: [-122.55,37.68,-122.32,37.84], name: 'Landsat' });
  assert.deepEqual(project.scenes[0].assets.red.rasterBand, { dataType: 'uint16', scale: 0.0000275, offset: -0.2, nodata: 0, spatialResolution: 30 });
  assert.equal(projectExploreSearch(project, INITIAL_SEARCH).provider, 'planetary-landsat');
  const restored = projectCatalogScenes(project)[0];
  assert.equal(restored.collection, 'landsat-c2-l2');
  assert.equal(restored.gsd, 30);
  assert.deepEqual(projectRequest({ scenes: [restored], bounds: project.bounds, name: project.name }).scenes, project.scenes);
  assert.equal(isSupportedAsset(scene.assets.red.href, 'blue'), false);
  assert.equal(isSupportedAsset(scene.assets.red.href, 'visual'), false);
  assert.equal(isSupportedAsset(`${scene.assets.red.href}?sig=secret`, 'red'), false);
  assert.throws(() => nextPageURL({ links: [{ rel: 'next', href: searchURL({ ...INITIAL_SEARCH, provider: 'planetary-computer' }) }] }, 'planetary-landsat'));
});
test('NASA HLS originals preserve signed reflectance calibration and stable source identity across project restoration', () => {
  const scene = normalizeScene(nasa.features[0], 'nasa-earthdata');
  assert.match(scene.thumbnail, /lp-prod-public\/HLSL30\.020\//);
  assert.equal(scene.collection, 'HLSL30_2.0');
  assert.equal(scene.gsd, 30);
  assert.equal(scene.crs, null);
  assert.deepEqual(downloadableAssets(scene).map(asset => asset.key), ['red', 'green', 'blue']);
  assert.equal(providerById(scene.provider).download, true);
  assert.equal(providerById(scene.provider).account, 'nasa-earthdata');
  assert.equal(canDisplayImagery(providerById(scene.provider)), false);
  const project = projectRequest({ scenes: [scene], bounds: scene.bbox, name: 'NASA originals' });
  assert.deepEqual(project.scenes[0].assets.red.rasterBand, { dataType: 'int16', scale: 0.0001, offset: 0, nodata: -9999, spatialResolution: 30 });
  assert.equal(projectExploreSearch(project, INITIAL_SEARCH).provider, 'nasa-earthdata');
  assert.deepEqual(projectRequest({ scenes: projectCatalogScenes(project), bounds: project.bounds, name: project.name }).scenes, project.scenes);
  for (const [href, key] of [[scene.assets.red.href, 'blue'], [scene.assets.red.href, 'visual'], [`${scene.assets.red.href}?token=private`, 'red'], [scene.assets.red.href.replace('HLSL30.020', 'HLSS30.020'), 'red']]) assert.equal(isSupportedAsset(href, key), false);
});
test('NASA cloud filtering follows an empty page and preserves the selected range across provider cursors', async () => {
  const first = structuredClone(nasa);
  first.features[0].properties['eo:cloud_cover'] = 90;
  const second = structuredClone(nasa);
  second.features[0].id = 'HLS-next';
  second.features[0].properties['eo:cloud_cover'] = 29;
  second.links = [];
  const calls = [];
  const result = await createSearchRunner().runAll(searchURL({ ...INITIAL_SEARCH, provider: 'nasa-earthdata', cloudMin: 10, cloud: 40 }), { fetcher: async url => {
    calls.push(url); return { ok: true, json: async () => calls.length === 1 ? first : second };
  } });
  assert.equal(calls.length, 2);
  assert.deepEqual(JSON.parse(new URL(calls[1]).searchParams.get('query'))['eo:cloud_cover'], { gte: 10, lte: 40 });
  assert.deepEqual(result.scenes.map(scene => scene.cloud), [29]);
});
test('Planetary Computer exposes true color and SCL with original grids and stable source URLs in project snapshots', () => {
  const scene = normalizeScene(pc.features[0], 'planetary-computer');
  assert.deepEqual(downloadableAssets(scene).map(asset => asset.key), ['scl', 'visual']);
  assert.equal(scene.gsd, 10);
  assert.equal(scene.crs, 'EPSG:32610');
  assert.equal(scene.grid.transform[0], 10);
  assert.match(scene.thumbnail, /preview\.png/);
  const project = projectRequest({ scenes: [scene], bounds: [-122.55, 37.68, -122.32, 37.84], name: 'PC project' });
  assert.equal(projectExploreSearch(project, INITIAL_SEARCH).provider, 'planetary-computer');
  assert.ok(!JSON.stringify(project).includes('sig='));
});
test('Copernicus discovery keeps public thumbnails and does not advertise anonymous original COG downloads', () => {
  const scene = normalizeScene(cdse.features[0], 'copernicus');
  assert.match(scene.thumbnail, /^https:\/\/datahub\.creodias\.eu/);
  assert.deepEqual(downloadableAssets(scene), []);
  assert.throws(() => projectRequest({ scenes: [scene], bounds: [1, 2, 3, 4], name: 'Unsupported source' }), /no supported/);
});
test('Copernicus public resolution binds exact selected identities without storing access tokens or claiming a COG', async () => {
  const scene = normalizeScene(cdse.features[0], 'copernicus');
  const href = 'https://download.dataspace.copernicus.eu/odata/v1/Products(0d695b42-4b24-4954-ba09-f2a44303fdd8)/$value';
  const resolved = [{ itemId: scene.id, href, mediaType: 'application/zip', bytes: 1128466167 }];
  const prepared = await prepareOriginalScenes([scene], async (operation, payload) => {
    assert.equal(operation, 'resolveProducts'); assert.deepEqual(payload, {itemIds:[scene.id]}); return resolved;
  });
  const project = projectRequest({ scenes: prepared, bounds: scene.bbox, name: 'SAFE originals' });
  assert.deepEqual(project.scenes[0].assets.product, { href, mediaType: 'application/zip' });
  assert.equal(projectExploreSearch(project, INITIAL_SEARCH).provider, 'copernicus');
  assert.deepEqual(projectRequest({ scenes: projectCatalogScenes(project), bounds: scene.bbox, name:project.name }).scenes, project.scenes);
  assert.equal(canDisplayImagery(providerById('copernicus')), false);
  assert.equal(isSupportedAsset(href,'visual'), false);
  assert.equal(isSupportedAsset(`${href}?token=PRIVATE`,'product'), false);
  assert.equal(isSupportedAsset(href.replace('/$value','/$zip'),'product'), false);
  for (const invalid of [[], [{...resolved[0],itemId:'unselected'}], [{...resolved[0],href:href+'?token=PRIVATE'}], [{...resolved[0],token:'PRIVATE'}], [{...resolved[0],bytes:5 * 1024**3}]]) {
    await assert.rejects(prepareOriginalScenes([scene],async()=>invalid), /invalid/);
  }
  const abort = new AbortController(); abort.abort();
  await assert.rejects(prepareOriginalScenes([scene],async()=>resolved,abort.signal), /abort/i);
  assert.deepEqual(scene.assets.product, undefined);
});
test('temporary read signatures are coalesced, refreshed after expiry, and never mutate the persisted asset', async () => {
  const href = pc.features[0].assets.visual.href;
  let calls = 0;
  const now = Date.now();
  const fetcher = async () => { calls += 1; return { ok: true, json: async () => ({ 'msft:expiry': new Date(now + calls * 3_600_000).toISOString(), token: `sp=r&sig=memory-only-${calls}` }) }; };
  await Promise.all([prepareAssetAccess([href], { fetcher, now }), prepareAssetAccess([href], { fetcher, now })]);
  assert.equal(calls, 1);
  assert.match(assetReadURL(href, now), /sig=memory-only-1/);
  assert.ok(!pc.features[0].assets.visual.href.includes('?'));
  await prepareAssetAccess([href], { fetcher, now: now + 3_600_000 });
  assert.equal(calls, 2);
  assert.match(assetReadURL(href, now + 3_600_000), /sig=memory-only-2/);
  assert.throws(() => assetReadURL(href, now + 10_800_000), /expired/);
  assert.equal(isSupportedAsset(`${href}?sig=persisted`), false);
  assert.equal(isSupportedAsset(href.replace('sentinel2l2a01.blob.core.windows.net', 'unreviewed.blob.core.windows.net')), false);
  await assert.rejects(prepareAssetAccess([href], { force: true, fetcher: async () => ({ ok: true, json: async () => ({ token: 'sig=invalid&sp=w', 'msft:expiry': new Date(now + 3_600_000).toISOString() }) }) }), /invalid/);
});

test('Landsat map shares one read-only container signature across bands and never persists it', async () => {
  const scene = normalizeScene(landsat.features[0], 'planetary-landsat');
  const hrefs = ['red', 'green', 'blue'].map(key => scene.assets[key].href);
  const snapshot = JSON.stringify(scene), now = Date.now();
  let calls = 0;
  const fetcher = async (url, options) => {
    assert.equal(options.credentials, 'omit');
    assert.equal(url, 'https://planetarycomputer.microsoft.com/api/sas/v1/token/landsateuwest/landsat-c2');
    calls += 1;
    return { ok: true, json: async () => ({ token: `sp=r&sr=c&sig=read-only-${calls}`, 'msft:expiry': new Date(now + 3_600_000).toISOString() }) };
  };
  await Promise.all([prepareAssetAccess(hrefs, { fetcher, now }), prepareAssetAccess([...hrefs, hrefs[0]], { fetcher, now })]);
  assert.equal(calls, 1);
  for (const href of hrefs) {
    const signed = new URL(assetReadURL(href, now));
    assert.equal(signed.origin + signed.pathname, href); assert.equal(signed.searchParams.get('sig'), 'read-only-1');
  }
  assert.equal(JSON.stringify(scene), snapshot);
  assert.throws(() => assetReadURL(hrefs[0], now + 3_600_000), /expired/);
  const good = assetReadURL(hrefs[0], now);
  for (const data of [null, { token: null },
    { token: 'sp=w&sr=c&sig=private' }, { token: 'sp=r&sr=c&sig=private&sig=duplicate' },
    { token: 'sp=r&sr=b&sig=private' }, { token: 'sp=r&sig=private' },
    { token: 'sp=r&sr=c&sig=private', 'msft:expiry': new Date(now).toISOString() }]) {
    await assert.rejects(prepareAssetAccess([hrefs[0]], { now, force: true, fetcher: async () => ({ ok: true,
      json: async () => data && { 'msft:expiry': new Date(now + 3_600_000).toISOString(), ...data } }) }), /invalid/);
    assert.equal(assetReadURL(hrefs[0], now), good);
  }
  await assert.rejects(prepareAssetAccess(hrefs, { now, force: true, fetcher: async () => ({ ok: false, status: 429 }) }), /rate limiting/);
  await prepareAssetAccess(hrefs, { fetcher, force: true, now }); assert.equal(calls, 2);
  const abort = new AbortController(); abort.abort();
  await assert.rejects(prepareAssetAccess(hrefs, { fetcher, signal: abort.signal, now }), /abort/i);
  assert.equal(calls, 2); assert.equal(JSON.stringify(scene), snapshot);
});
