import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {validatePlanMapPreview,loadPlanPreviewScenes} from './agent-map-preview.js';
const catalog=JSON.parse(readFileSync(new URL('../public/samples/earth-search-response.json',import.meta.url),'utf8'));
const item=catalog.features[0];
const preview=()=>({planId:'a1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),provider:'earth-search',bounds:[-123,37,-122,38],geometry:null,selections:[{itemId:item.id,assets:{visual:item.assets.visual.href}}]});
test('preview accepts only bounded native files, source identities and valid geometry',()=>{
  assert.equal(validatePlanMapPreview(preview()).selections[0].itemId,item.id);
  for(const mutate of [v=>v.planHash='x',v=>v.provider='untrusted',v=>v.selections.push(v.selections[0]),v=>v.selections[0].assets.visual='https://evil.test/a.tif',v=>v.selections[0].itemId='../secret',v=>v.geometry={type:'Polygon',coordinates:[[[0,0],[1,1],[2,2]]]}]){
    const value=preview();mutate(value);assert.throws(()=>validatePlanMapPreview(value));
  }
});
test('preview reads exact reviewed catalog items, checks pinned originals and never searches or downloads',async()=>{
  const calls=[];
  const result=await loadPlanPreviewScenes(preview(),{fetcher:async(url,options)=>{calls.push({url,options});return new Response(JSON.stringify(item));}});
  assert.equal(result.scenes[0].id,item.id);assert.equal(result.failed,0);assert.equal(calls.length,1);
  assert(calls[0].url.endsWith('/items/'+item.id));assert(!calls[0].url.includes('/search'));assert.equal(calls[0].options.credentials,'omit');
  const changed=structuredClone(item);changed.assets.visual.href=changed.assets.visual.href.replace('/TCI.tif','/wrong.tif');
  const refused=await loadPlanPreviewScenes(preview(),{fetcher:async()=>new Response(JSON.stringify(changed))});
  assert.equal(refused.failed,1);assert.deepEqual(refused.scenes,[]);
});
test('partial preview failures do not replace missing files or remove them from the review',async()=>{
  const value=preview();value.selections.push({itemId:catalog.features[1].id,assets:{visual:catalog.features[1].assets.visual.href}});
  const unchanged=structuredClone(value);
  const result=await loadPlanPreviewScenes(value,{fetcher:async url=>url.endsWith(item.id)?new Response(JSON.stringify(item)):new Response('',{status:503})});
  assert.equal(result.scenes.length,1);assert.equal(result.failed,1);assert.deepEqual(value,unchanged);
});
