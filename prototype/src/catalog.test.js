import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { INITIAL_SEARCH, defaultLiveSearch, validateBounds, validateSearch, searchURL, normalizeScene, normalizeSample, compatibleScenes, nextPageURL, createSearchRunner } from "./catalog.js";
const fixture = JSON.parse(readFileSync(new URL("../public/samples/earth-search-response.json", import.meta.url)));
test("the live catalog defaults to a rolling UTC month", () => {
  assert.deepEqual(defaultLiveSearch(new Date("2026-03-01T01:00:00Z")), { ...INITIAL_SEARCH, start: "2026-01-31", end: "2026-03-01" });
  assert.deepEqual(defaultLiveSearch(new Date("2026-09-29T23:59:00Z")), { ...INITIAL_SEARCH, start: "2026-08-31", end: "2026-09-29" });
});
test("query validates geographic/date inputs and sends server-side cloud and UTC end date", () => {
  const url = new URL(searchURL(INITIAL_SEARCH));
  assert.deepEqual(JSON.parse(url.searchParams.get("query")), { "eo:cloud_cover": { lte: 60 } });
  assert.equal(url.searchParams.get("datetime"), "2025-06-01T00:00:00Z/2025-06-30T23:59:59.999Z");
  assert.equal(url.searchParams.get("sortby"), "-properties.datetime");
  for (const values of [{ bbox: "1,,2,3" }, { bbox: "180,2,-180,3" }, { bbox: "1,91,2,93" }, { bbox: "1,2,3" }, { start: "2025-02-30" }, { end: "2025-05-01" }, { limit: 101 }, { cloud: -1 }]) assert.throws(() => validateSearch({ ...INITIAL_SEARCH, ...values }));
});
test("map-drawn WGS 84 bounds use the same validation as catalog searches", () => {
  const bounds = validateBounds([12.34567, -3.5, 12.7, -3]);
  assert.deepEqual(validateSearch({ ...INITIAL_SEARCH, bbox: bounds }).bbox, bounds);
  for (const value of [[12, 0, 12, 2], [179, -1, -179, 1], [-181, 0, -170, 1], [10, -91, 11, 0]]) {
    assert.throws(() => validateBounds(value));
  }
});
test("STAC normalization preserves source assets and actual projection", () => {
  const item = structuredClone(fixture.features[0]);
  item.properties["proj:epsg"] = 32755;
  const scene = normalizeScene(item);
  assert.equal(scene.crs, "EPSG:32755");
  assert.equal(scene.assets.scl.href, item.assets.scl.href);
  assert.equal(scene.assets.visual.href, item.assets.visual.href);
  assert.equal(scene.thumbnail, item.assets.thumbnail.href);
  assert.deepEqual(scene.geometry, item.geometry);
  item.assets.thumbnail.href = "javascript:alert(1)";
  item.properties["eo:cloud_cover"] = null;
  assert.equal(normalizeScene(item).thumbnail, null);
  assert.equal(normalizeScene(item).cloud, null);
});
test("comparison requires two true-color COGs on the same CRS, transform and shape", () => {
  const a = normalizeScene(fixture.features[0]), b = { ...structuredClone(a), id: "other" };
  assert.equal(compatibleScenes(a, b), true);
  assert.equal(compatibleScenes(a, { ...b, thumbnail: null }), true);
  assert.equal(compatibleScenes(a, { ...b, assets: { ...b.assets, visual: undefined } }), false);
  b.grid.transform[2] += 10;
  assert.equal(compatibleScenes(a, b), false);
  assert.equal(compatibleScenes(a, { ...a, id: "third", crs: "EPSG:4326" }), false);
  assert.equal(compatibleScenes({ ...a, crs: "EPSG:4326" }, { ...b, crs: "EPSG:4326", grid: a.grid }), false);
  assert.equal(compatibleScenes(a, { ...a, id: "fourth", grid: {} }), false);
});
test("sample thumbnail stays local while real download hrefs remain intact", () => {
  const sample = JSON.parse(readFileSync(new URL("../public/samples/manifest.json", import.meta.url)));
  const scene = normalizeSample(sample).scenes[0];
  assert.match(scene.thumbnail, /^\.\/samples\//);
  assert.match(scene.assets.scl.href, /^https:\/\//);
  assert.ok(scene.grid.transform);
});
test("pagination follows the server link and refuses host, credential, path and method changes", () => {
  const href = `${searchURL(INITIAL_SEARCH)}&next=opaque-token`;
  assert.equal(nextPageURL({ links: [{ rel: "next", href }] }), href);
  for (const link of [{ href: "https://evil.example/v1/search" }, { href: "http://earth-search.aws.element84.com/v1/search" }, { href: "https://user@earth-search.aws.element84.com/v1/search" }, { href: "https://earth-search.aws.element84.com/v1/other" }, { href, method: "POST" }]) assert.throws(() => nextPageURL({ links: [{ rel: "next", ...link }] }));
});
test("late results and cancelled queries cannot replace current results", async () => {
  const runner = createSearchRunner(), pending = [];
  const fetcher = (_url, options) => new Promise((resolve) => pending.push({ resolve, signal: options.signal }));
  const first = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  const second = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  assert.equal(pending[0].signal.aborted, true);
  pending[1].resolve({ ok: true, json: async () => fixture });
  assert.ok((await second).scenes.length);
  pending[0].resolve({ ok: true, json: async () => fixture });
  assert.equal(await first, null);
  const third = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  runner.cancel();
  pending[2].resolve({ ok: true, json: async () => fixture });
  assert.equal(await third, null);
});
test("an old request rejection is ignored while current transport errors remain visible", async () => {
  const runner = createSearchRunner(), pending = [];
  const fetcher = () => new Promise((resolve, reject) => pending.push({ resolve, reject }));
  const first = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  const second = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  pending[0].reject(new Error("old network error"));
  assert.equal(await first, null);
  pending[1].reject(new Error("current network error"));
  await assert.rejects(second, /current network error/);
});
