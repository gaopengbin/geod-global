import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { INITIAL_SEARCH, defaultLiveSearch, validateBounds, validateSearch, searchURL, normalizeScene, normalizeSample, compatibleScenes, nextPageURL, createSearchRunner } from "./catalog.js";
const fixture = JSON.parse(readFileSync(new URL("../public/samples/earth-search-response.json", import.meta.url)));
test("the live catalog defaults to a rolling UTC month", () => {
  assert.deepEqual(defaultLiveSearch(new Date("2026-03-01T01:00:00Z")), { ...INITIAL_SEARCH, bbox: '', start: "2026-01-31", end: "2026-03-01" });
  assert.deepEqual(defaultLiveSearch(new Date("2026-09-29T23:59:00Z")), { ...INITIAL_SEARCH, bbox: '', start: "2026-08-31", end: "2026-09-29" });
});
test("query validates geographic/date inputs and sends server-side cloud and UTC end date", () => {
  const url = new URL(searchURL(INITIAL_SEARCH));
  assert.deepEqual(JSON.parse(url.searchParams.get("query")), { "eo:cloud_cover": { gte: 0, lte: 60 } });
  assert.equal(url.searchParams.get("datetime"), "2025-06-01T00:00:00Z/2025-06-30T23:59:59.999Z");
  assert.equal(url.searchParams.get("sortby"), "-properties.datetime");
  for (const values of [{ bbox: "1,,2,3" }, { bbox: "180,2,-180,3" }, { bbox: "1,91,2,93" }, { bbox: "1,2,3" }, { start: "2025-02-30" }, { end: "2025-05-01" }, { limit: 101 }, { cloud: -1 }, { cloudMin: 61 }, { cloudMin: -1 }]) assert.throws(() => validateSearch({ ...INITIAL_SEARCH, ...values }));
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
  const fetcher = () => pending.length >= 2
    ? Promise.reject(new Error("current network error"))
    : new Promise((resolve, reject) => pending.push({ resolve, reject }));
  const first = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  const second = runner.run(searchURL(INITIAL_SEARCH), { fetcher });
  pending[0].reject(new Error("old network error"));
  assert.equal(await first, null);
  pending[1].reject(new Error("current network error"));
  await assert.rejects(second, /current network error/);
});

test("automatic pagination reports all unique scenes and stops after cancellation", async () => {
  const first = structuredClone(fixture);
  const next = `${searchURL(INITIAL_SEARCH)}&page=2`;
  first.links = [{ rel: "next", href: next }];
  const second = structuredClone(fixture);
  second.features = [structuredClone(fixture.features[0]), { ...structuredClone(fixture.features[0]), id: "second-scene" }];
  second.links = [];
  const pages = [];
  const runner = createSearchRunner();
  const result = await runner.runAll(searchURL(INITIAL_SEARCH), { fetcher: async url => ({ ok: true, json: async () => url === next ? second : first }), onPage: page => pages.push(page) });
  assert.equal(result.scenes.length, new Set(fixture.features.map(item => item.id)).size + 1);
  assert.equal(pages.length, 2);
  assert.equal(pages[0].complete, false);
  assert.equal(pages[1].complete, true);

  const pending = [];
  const interrupted = runner.runAll(searchURL(INITIAL_SEARCH), { fetcher: () => new Promise(resolve => pending.push(resolve)) });
  runner.cancel();
  pending[0]({ ok: true, json: async () => first });
  assert.equal(await interrupted, null);
});

test("automatic pagination keeps the first page visible when a later page fails", async () => {
  const first = structuredClone(fixture);
  first.links = [{ rel: "next", href: `${searchURL(INITIAL_SEARCH)}&page=2` }];
  const seen = [];
  const runner = createSearchRunner();
  await assert.rejects(runner.runAll(searchURL(INITIAL_SEARCH), {
    fetcher: async url => url.includes('page=2') ? { ok: false, status: 503 } : { ok: true, json: async () => first },
    onPage: page => seen.push(page),
  }), /HTTP 503/);
  assert.equal(seen.length, 1);
  assert.equal(seen[0].complete, false);
  assert.ok(seen[0].scenes.length);
});

// Controlled publisher-metadata playback, not live catalog or file-transfer evidence.
function catalogBatch(offset, size = 100, next = null) {
  return {...fixture, features:Array.from({length:size},(_,index)=>({...structuredClone(fixture.features[0]),id:`scene-${offset+index}`})),links:next?[{rel:'next',href:next}]:[]};
}
test('interactive searches stop at 100 and append one page only on demand, deduplicating overlapping records', async()=>{
  const runner=createSearchRunner(),url=searchURL({...INITIAL_SEARCH,limit:100}),next=`${url}&page=2`,last=`${url}&page=3`,calls=[];
  const fetcher=async url=>{calls.push(url);return {ok:true,json:async()=>url===next?catalogBatch(99,100,last):catalogBatch(0,100,next)};};
  const first=await runner.start(url,{fetcher});
  assert.equal(first.scenes.length,100);assert.equal(first.complete,false);assert.equal(first.next,next);
  await new Promise(resolve=>setImmediate(resolve));assert.equal(calls.length,1);
  const more=await runner.loadMore({fetcher});
  assert.equal(more.scenes.length,199);assert.equal(more.pages,2);assert.equal(more.next,last);assert.equal(calls.length,2);
  assert.equal(first.scenes.length,100);
});
test('a changed area invalidates pending pagination and repeated clicks do not duplicate its HTTP request',async()=>{
  const runner=createSearchRunner(),url=searchURL({...INITIAL_SEARCH,limit:100}),next=`${url}&page=2`;
  await runner.start(url,{fetcher:async()=>({ok:true,json:async()=>catalogBatch(0,100,next)})});
  let resolve,requests=0;
  const old=runner.loadMore({fetcher:()=>{requests++;return new Promise(done=>resolve=done);}});
  assert.equal(await runner.loadMore({fetcher:()=>{throw Error('Must not issue a duplicate');}}),null);assert.equal(requests,1);
  const region=await runner.start(searchURL({...INITIAL_SEARCH,bbox:'10,20,11,21'}),{fetcher:async()=>({ok:true,json:async()=>catalogBatch(1000,3)})});
  resolve({ok:true,json:async()=>catalogBatch(100,100)});
  assert.equal(await old,null);assert.deepEqual(region.scenes.map(s=>s.id),['scene-1000','scene-1001','scene-1002']);
  assert.equal(region.complete,true);assert.equal(await runner.loadMore(),null);
});
test('a failed or stopped next page can be retried without losing the first 100 results',async()=>{
  const runner=createSearchRunner(),url=searchURL({...INITIAL_SEARCH,limit:100}),next=`${url}&page=2`;
  await runner.start(url,{fetcher:async()=>({ok:true,json:async()=>catalogBatch(0,100,next)})});
  await assert.rejects(runner.loadMore({fetcher:async()=>({ok:false,status:503})}),/HTTP 503/);
  let resolve;const stopped=runner.loadMore({fetcher:()=>new Promise(done=>resolve=done)});runner.cancel();
  resolve({ok:true,json:async()=>catalogBatch(100,100)});assert.equal(await stopped,null);
  const retried=await runner.loadMore({fetcher:async url=>{assert.equal(url,next);return {ok:true,json:async()=>catalogBatch(100,2)};}});
  assert.equal(retried.scenes.length,102);assert.equal(retried.pages,2);assert.equal(retried.complete,true);
});
test('NAIP latest-year mode probes only one scene, fetches one year-limited page and resolves each area independently',async()=>{
  const naip=JSON.parse(readFileSync(new URL('../public/samples/naip-response.json',import.meta.url))),calls=[];
  const runner=createSearchRunner(),now=new Date('2026-10-19T12:00:00Z');
  const fetcher=async request=>{const url=new URL(request);calls.push(url);const response=structuredClone(naip);
    response.features=response.features.slice(0,1);response.features[0].properties.datetime=url.searchParams.get('bbox').startsWith('-79')?'2023-09-26T12:00:00Z':'2021-07-01T12:00:00Z';
    response.links=[{rel:'next',href:request+'&page=2'}];return {ok:true,json:async()=>response};};
  const first=await runner.start(searchURL({...INITIAL_SEARCH,provider:'planetary-naip',bbox:'-79,40,-72,45'}),{latestYear:true,now,fetcher});
  assert.equal(calls.length,2);assert.equal(calls[0].searchParams.get('limit'),'1');assert.equal(calls[0].searchParams.get('datetime'),'../2026-10-19T23:59:59.999Z');
  assert.equal(calls[1].searchParams.get('limit'),'100');assert.equal(calls[1].searchParams.get('datetime'),'2023-01-01T00:00:00Z/2023-12-31T23:59:59.999Z');
  assert.deepEqual(first.dateRange,{start:'2023-01-01',end:'2023-12-31',latestYear:'2023'});assert.equal(first.pages,1);assert.equal(first.complete,false);
  const second=await runner.start(searchURL({...INITIAL_SEARCH,provider:'planetary-naip',bbox:'-123,37,-122,38'}),{latestYear:true,now,fetcher});
  assert.equal(calls.length,4);assert.equal(second.dateRange.latestYear,'2021');assert.equal(calls[3].searchParams.get('datetime'),'2021-01-01T00:00:00Z/2021-12-31T23:59:59.999Z');
  calls.length=0;
  await runner.start(searchURL({...INITIAL_SEARCH,provider:'planetary-naip'}),{fetcher});
  assert.equal(calls.length,1);assert.equal(calls[0].searchParams.get('datetime'),'2025-06-01T00:00:00Z/2025-06-30T23:59:59.999Z');
});
test('NAIP without published acquisitions does not scan older pages or make a second request',async()=>{
  const runner=createSearchRunner();let requests=0;
  const result=await runner.start(searchURL({...INITIAL_SEARCH,provider:'planetary-naip'}),{latestYear:true,fetcher:async()=>{requests++;return {ok:true,json:async()=>({type:'FeatureCollection',features:[],links:[]})};}});
  assert.equal(requests,1);assert.equal(result.scenes.length,0);assert.equal(result.complete,true);assert.equal(await runner.loadMore(),null);
});
test('cancelled transports cannot unlock or overwrite an immediately retried next page',async()=>{
  const runner=createSearchRunner(),url=searchURL(INITIAL_SEARCH),next=`${url}&page=2`;
  await runner.start(url,{fetcher:async request=>{assert.equal(new URL(request).searchParams.get('limit'),'100');return {ok:true,json:async()=>catalogBatch(0,100,next)};}});
  let oldResolve,newResolve;
  const old=runner.loadMore({fetcher:()=>new Promise(done=>oldResolve=done)});runner.cancel();
  const retry=runner.loadMore({fetcher:()=>new Promise(done=>newResolve=done)});
  oldResolve({ok:true,json:async()=>catalogBatch(100,100)});assert.equal(await old,null);
  assert.equal(await runner.loadMore({fetcher:()=>{throw Error('Retry is already running');}}),null);
  newResolve({ok:true,json:async()=>catalogBatch(100,3)});
  assert.equal((await retry).scenes.length,103);
});
