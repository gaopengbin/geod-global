import test from 'node:test';
import assert from 'node:assert/strict';
import {deliveryState,taskHref} from './acquisition-flow.js';
import {catalogProducts,productForSource,readProductPreferences,writeProductPreferences} from './product-catalog.js';

test('a completed unrelated task cannot make the current receipt ready',()=>{
  const value=deliveryState([{id:'unrelated',status:'succeeded'},{id:'one',status:'succeeded'}],['one','two']);
  assert.equal(value.status,'pending');assert.equal(value.done,1);assert.equal(value.total,2);assert.equal(value.missing,1);
});
test('mixed success, failure and cancellation remain partial',()=>{
  const value=deliveryState([{id:'one',status:'succeeded'},{id:'two',status:'failed'},{id:'three',status:'cancelled'}],['one','two','three']);
  assert.equal(value.status,'partial');assert.equal(value.attention,2);
});
test('an Agent transfer still finalizing cannot become a ready delivery',()=>{
  assert.equal(deliveryState([{id:'one',status:'succeeded',settled:false}],['one']).status,'pending');
  assert.equal(deliveryState([{id:'one',status:'succeeded',settled:true}],['one']).status,'ready');
});
test('an active receipt cannot claim its completed files are the final result',()=>{
  assert.equal(deliveryState([{id:'one',status:'succeeded'},{id:'two',status:'queued'}],['one','two']).status,'active');
  assert.equal(deliveryState([{id:'one',status:'succeeded'}],['one']).status,'ready');
  assert.equal(deliveryState([{id:'one',status:'succeeded'}],[]).status,'saved');
});
test('deduplicates task observations and keeps links scoped to a project or task',()=>{
  assert.equal(deliveryState([{id:'one',status:'running'},{id:'one',status:'succeeded'}],['one','one']).done,1);
  assert.equal(taskHref({projectId:'project A'}),'#Tasks?project=project%20A');
  assert.equal(taskHref({jobId:'one'}),'#Tasks?job=one');
});
test('groups declared equivalent Sentinel COG channels but keeps SAFE and land cover separate',()=>{
  const product=productForSource('earth-search');
  assert.deepEqual(product.channels.map(source=>source.id),['earth-search','planetary-computer']);
  assert.notEqual(product.id,productForSource('copernicus').id);
  const channels=catalogProducts().flatMap(product=>product.channels.map(source=>source.id));
  assert.equal(channels.length,new Set(channels).size);
});
test('ignores stale favorites and cross-product channel preferences',()=>{
  const storage={getItem:()=>JSON.stringify({favorites:['removed','sentinel-2-l2a-cog','sentinel-2-l2a-cog'],recent:['removed','earth-search'],preferred:{'sentinel-2-l2a-cog':'copernicus'}})};
  assert.deepEqual(readProductPreferences(storage),{favorites:['sentinel-2-l2a-cog'],recent:[],preferred:{}});
  assert.doesNotThrow(()=>writeProductPreferences({}, {setItem:()=>{throw Error('blocked');}}));
});
