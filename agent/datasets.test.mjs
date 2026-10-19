import test from 'node:test';
import assert from 'node:assert/strict';
import {beforePlaceTool,afterPlaceTool,CITY_SCOPE_ERROR} from './place-scope.mjs';
import {loadToolGroups,visibleTools,TOOL_LOADER} from './tool-catalog.mjs';
import {startToolRecord,finishToolRecord,validToolRecord} from './tool-record.mjs';

test('public dataset discovery preserves resolved human city scope across all search routes',()=>{
  const active={}, city=[116.2,39.7,116.6,40.1];
  beforePlaceTool(active,'geod_place_search',{query:'Beijing',kind:'city'});
  afterPlaceTool(active,'geod_place_search',{candidates:[{kind:'city',bounds:city}]});
  for(const tool of ['geod_dataset_resolve','geod_dataset_connect','geod_stac_search']) {
    assert.doesNotThrow(()=>beforePlaceTool(active,tool,{datasetId:'worldcover',bounds:city}));
    assert.throws(()=>beforePlaceTool(active,tool,{datasetId:'worldcover',bounds:[-74,40,-73,41]}),{message:CITY_SCOPE_ERROR});
  }
});
test('dataset tools load with STAC without resending all satellite or science tools',()=>{
  const names=['dataset_catalog','dataset_resolve','dataset_connect','stac_project_plan','scene_search'].map(n=>`geod_${n}`);
  const definitions=[TOOL_LOADER,...names.map(name=>({name}))];
  const loaded=loadToolGroups({groups:['stac']},definitions);
  assert.deepEqual(loaded.tools,names.slice(0,4));
  const input=[{type:'function_call',name:TOOL_LOADER.name,call_id:'load'},{type:'function_call_output',call_id:'load',output:JSON.stringify(loaded)}];
  assert(!visibleTools(input,new Set(definitions.map(d=>d.name))).has('geod_scene_search'));
});
test('public product records retain units and partial discovery without leaking URLs',()=>{
  const record=finishToolRecord(startToolRecord({datasetId:'worldcover',bounds:[1,2,3,4],year:2021}),{
    dataset:{id:'worldcover',title:'ESA WorldCover',unit:'class code',license:'CC-BY-4.0'},
    request:{datasetId:'worldcover',bounds:[1,2,3,4],year:2021,layer:'map'},count:1,complete:false,
    failures:[{sourceId:'missing-tile',error:'https://public.example/missing.tif'}],downloadsStarted:false,
  });
  assert(validToolRecord(record));
  assert.equal(record.result.dataset.unit,'class code');
  assert.equal(record.result.complete,false);
  assert.equal(record.result.downloadsStarted,false);
  assert(!JSON.stringify(record).includes('https://'));
});

test('monthly product receipts preserve the exact month without carrying transfer URLs',()=>{
  const request={datasetId:'chirps',bounds:[116,39,117,40],year:2024,month:2,layer:'monthly-precipitation'};
  const record=finishToolRecord(startToolRecord(request),{request,downloadsStarted:false,
    dataset:{id:'chirps',unit:'mm/month',license:'CC BY 4.0'},sources:[{id:'february',href:'https://example.com/file.tif'}]});
  assert(validToolRecord(record));
  assert.equal(record.input.month,2);
  assert.equal(record.result.request.month,2);
  assert(!JSON.stringify(record).includes('https://'));
});
