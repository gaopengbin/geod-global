import test from 'node:test';
import assert from 'node:assert/strict';
import {beforePlaceTool,afterPlaceTool,failedPlaceTool,toolFailureCode,CITY_SCOPE_ERROR,STORAGE_SCOPE_ERROR} from './place-scope.mjs';
test('expiry records a safe recoverable category and does not poison geographic lookups',()=>{
  const active={};beforePlaceTool(active,'geod_place_search',{kind:'city',query:'Requested city'});
  const city=[-74.2,40.4,-73.7,41.0];afterPlaceTool(active,'geod_place_search',{candidates:[{kind:'city',bounds:city}]});
  const expired=Error('Scene search is stale or belongs to another conversation. Search again.');
  failedPlaceTool(active,expired);assert.equal(toolFailureCode(expired),'stale-search');
  assert.doesNotThrow(()=>beforePlaceTool(active,'geod_scene_search',{bounds:city}));
  assert.equal(toolFailureCode(Error('Agent plan expired. Create a new plan before confirming.')),'expired-plan');
  assert.equal(toolFailureCode(Error(expired.message+' private details')),'tool-failed');
});
test('city lookup failure blocks same-named states and unrelated current-map searches without blocking administrative reads',()=>{
  const active={}; beforePlaceTool(active,'geod_place_search',{kind:'city',query:'Requested city'});
  beforePlaceTool(active,'geod_region_search',{query:'Requested city'});
  afterPlaceTool(active,'geod_region_search',{candidates:[{adminLevel:1,bounds:[-80,40,-70,45]}]});
  for(const extent of [[-80,40,-70,45],[-122.55,37.68,-122.32,37.84]])
    assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:extent}),{message:CITY_SCOPE_ERROR});
  const actual=[-74.2,40.4,-73.7,41.0];
  afterPlaceTool(active,'geod_place_search',{candidates:[{kind:'city',bounds:actual}]});
  assert.doesNotThrow(()=>beforePlaceTool(active,'geod_scene_search',{bounds:actual.map(n=>n+0.0000001)}));
  assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:[-122.55,37.68,-122.32,37.84]}));
});
test('storage failures stop repeated geographic operations for this turn while an independent turn can retry',()=>{
  const active={}; beforePlaceTool(active,'geod_place_search',{kind:'city'});
  failedPlaceTool(active,Error('Agent record directory was redirected.'));
  for(const tool of ['geod_place_search','geod_region_levels','geod_region_search','geod_scene_search'])
    assert.throws(()=>beforePlaceTool(active,tool,{}),{message:STORAGE_SCOPE_ERROR});
  assert.doesNotThrow(()=>beforePlaceTool(active,'geod_health',{}));
  assert.doesNotThrow(()=>beforePlaceTool({},'geod_place_search',{kind:'city'}));
  assert.equal(toolFailureCode(Error('Agent record directory was redirected.')),'record-storage');
  assert.equal(toolFailureCode(Error('https://private.test/?token=secret')),'tool-failed');
});
test('verified borough polygon inside the resolved city can be searched without accepting arbitrary subareas or a state',()=>{
  const active={};const city=[-74.26,40.47,-73.70,40.92],borough=[-74.05,40.68,-73.90,40.89];
  beforePlaceTool(active,'geod_place_search',{kind:'city'});
  afterPlaceTool(active,'geod_place_search',{candidates:[{kind:'city',bounds:city}]});
  beforePlaceTool(active,'geod_place_search',{kind:'place'});
  afterPlaceTool(active,'geod_place_search',{candidates:[{kind:'district',bounds:borough}]});
  assert.doesNotThrow(()=>beforePlaceTool(active,'geod_scene_search',{bounds:city}));
  assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:borough}));
  const boundary={id:'native-boundary',sha256:'a'.repeat(64)};
  afterPlaceTool(active,'geod_boundary_read',{boundary,geometryType:'MultiPolygon',bounds:borough});
  assert.doesNotThrow(()=>beforePlaceTool(active,'geod_scene_search',{bounds:borough}));
  const state=[-80,40,-70,45];
  afterPlaceTool(active,'geod_boundary_read',{boundary,geometryType:'Polygon',bounds:state});
  assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:state}));
  assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:[-74,40.7,-73.99,40.8]}));
});
test('a verified administrative polygon cannot unlock a failed city lookup on its own',()=>{
  const active={};const area=[-74.05,40.68,-73.90,40.89];
  beforePlaceTool(active,'geod_place_search',{kind:'city'});
  afterPlaceTool(active,'geod_place_search',{candidates:[]});
  afterPlaceTool(active,'geod_boundary_read',{boundary:{id:'native',sha256:'a'.repeat(64)},geometryType:'Polygon',bounds:area});
  assert.throws(()=>beforePlaceTool(active,'geod_scene_search',{bounds:area}));
});
