import test from 'node:test';import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';
import {fetchCatalogPage,normalizeScene,searchURL} from './catalog.js';import {failureDiagnostics} from './usage-diagnostics.js';
const captured=JSON.parse(readFileSync(new URL('../qa/composite-year-end-items.json',import.meta.url)));
test('actual year-end PC intervals are retained verbatim while display and source IDs keep the product period',()=>{
 for(const [key,provider] of [['modis','planetary-modis'],['vegetation','planetary-vegetation']]){
  const raw=structuredClone(captured[key]),original=JSON.stringify(raw),scene=normalizeScene(raw,provider);
  assert.equal(scene.endDate,'2019-12-31T23:59:59Z');assert.equal(scene.properties.end_datetime,raw.properties.end_datetime);
  assert.equal(JSON.stringify(raw),original);
  const bad=structuredClone(raw);bad.properties.end_datetime='2020-01-04T23:59:59Z';assert.throws(()=>normalizeScene(bad,provider));
 }
});
test('one rejected record does not discard verified neighbors, while an all-invalid page remains a validation failure',async()=>{
 const valid=captured.modis,invalid={...valid,id:'unverified-item'};
 const url=searchURL({provider:'planetary-modis',bbox:[116,39,117,40],start:'2019-12-01',end:'2019-12-31',limit:20});
 const response=features=>async()=>new Response(JSON.stringify({type:'FeatureCollection',features,links:[]}));
 const page=await fetchCatalogPage(url,{fetcher:response([invalid,valid])});assert.equal(page.scenes.length,1);assert.equal(page.rejectedCount,1);assert.equal(page.scenes[0].id,valid.id);
 await assert.rejects(fetchCatalogPage(url,{fetcher:response([invalid])}),error=>failureDiagnostics(error).stage==='catalog_validation');
 await assert.rejects(fetchCatalogPage(url,{fetcher:async()=>new Response('null')}),error=>failureDiagnostics(error).stage==='catalog_validation');
 await assert.rejects(fetchCatalogPage(url,{fetcher:async()=>new Response(JSON.stringify({type:'FeatureCollection',features:[valid],links:[{rel:'next',href:'https://untrusted.example/search'}]}))}),error=>failureDiagnostics(error).stage==='catalog_validation');
});
test('catalog transport retries transient failures without retrying bad parameters or malformed JSON',async()=>{
 const url=searchURL({provider:'planetary-modis',bbox:[116,39,117,40],start:'2019-12-01',end:'2019-12-31',limit:20});let count=0;
 const page=await fetchCatalogPage(url,{fetcher:async()=>{count++;return count===1?new Response('unavailable',{status:503}):new Response(JSON.stringify({type:'FeatureCollection',features:[captured.modis],links:[]}));}});
 assert.equal(count,2);assert.equal(page.scenes.length,1);
 count=0;await assert.rejects(fetchCatalogPage(url,{fetcher:async()=>{count++;return new Response('bad request',{status:400});}}),error=>failureDiagnostics(error).httpStatus==='400');assert.equal(count,1);
 count=0;await assert.rejects(fetchCatalogPage(url,{fetcher:async()=>{count++;return new Response('{');}}),error=>failureDiagnostics(error).stage==='catalog_decode');assert.equal(count,1);
});
