import test from 'node:test';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
import {validScene,requestScene,sceneKind,validPlanScopes,sceneInput} from './scenes.mjs';
test('entry-point data has a bounded page and subject and never imports an ambient map into Home',()=>{
  assert.deepEqual(requestScene(null,null),{kind:'conversation',page:'Home'});
  assert.deepEqual(requestScene(null,{page:'Explore'}),{kind:'workspace',page:'Explore'});
  assert(validScene({kind:'workspace',page:'My Data',subject:{kind:'project',id:randomUUID()}}));
  for(const scene of [{kind:'conversation',page:'Explore'},{kind:'workspace',page:'Home'},{kind:'workspace',page:'Workspace',subject:{kind:'raster',id:'G:/private.tif'}},{kind:'conversation',page:'Home',permission:'full-access'}])assert(!validScene(scene));
  assert.throws(()=>requestScene({kind:'conversation',page:'Home'},{page:'Explore'}),/ambient map/);
  assert.equal(sceneKind({}),'conversation');
  assert(!validPlanScopes({private:{scene:{kind:'workspace',page:'Explore'},context:null}}));
});
test('each request exposes its current bounded list observation while Home discards ambient data',()=>{
  const context={provider:'gscloud',gscloud:{productId:'421',presence:'has',password:'never-send'},catalog:{state:'ready',totalCount:1,filteredCount:1,selectedCount:1,loadedCount:0,visibleLoadedCount:0,complete:true,selectedItemIds:['ASTGTM2_N40E116']}};
  const first=sceneInput({kind:'workspace',page:'Explore'},context);assert(first.includes('"productId":"421"'));assert(first.includes('"filteredCount":1'));assert(!first.includes('never-send'));
  context.catalog.filteredCount=0;context.catalog.textFilter='no-match';
  const next=sceneInput({kind:'workspace',page:'Explore'},context);assert(next.includes('"filteredCount":0'));assert(next.includes('"selectedCount":1'));assert(next.includes('map previews'));
  const home=sceneInput({kind:'conversation',page:'Home'},context);assert(home.includes('observation: null'));assert(!home.includes('"productId"'));assert(!home.includes('ASTGTM2_N40E116'));
  context.catalog.selectedItemIds=Array.from({length:32},()=> 'x'.repeat(180));
  assert(sceneInput({kind:'workspace',page:'Explore'},context).length<4500);
});
