import test from 'node:test';
import assert from 'node:assert/strict';
import {checkToolArguments, ToolArgumentsError} from './tool-arguments.mjs';
import {safeToolFailure} from './codex-host.mjs';
const project={name:'geod_stac_project_plan',inputSchema:{type:'object',properties:{name:{},bounds:{},selections:{items:{required:['snapshotId','assetKey']}}},required:['bounds','selections'],additionalProperties:false}};
test('invalid product handoff reports the actual required native fields without echoing supplied identifiers or URLs',()=>{
  const active={};let error;
  try{checkToolArguments(active,project,{name:'Sample',connectionIds:['https://private.test/?token=secret']});}catch(cause){error=cause;}
  assert(error instanceof ToolArgumentsError);
  const text=safeToolFailure(error);
  assert.match(text,/Required fields: bounds, selections/);
  assert.match(text,/snapshotId, assetKey/);
  assert.match(text,/do not.*drop required bounds/i);
  assert(!text.includes('private.test'));assert(!text.includes('token=secret'));
  assert.doesNotThrow(()=>checkToolArguments(active,project,{name:'Sample',bounds:[1,2,3,4],selections:[{snapshotId:'a'.repeat(64),assetKey:'data'}]}));
});
test('two malformed attempts stop the same tool for the turn without blocking an unrelated tool or the next turn',()=>{
  const active={};for(let i=0;i<2;i++)assert.throws(()=>checkToolArguments(active,project,{}),ToolArgumentsError);
  assert.throws(()=>checkToolArguments(active,project,{bounds:[1,2,3,4],selections:[]}),error=>/Stop repeating/.test(safeToolFailure(error)));
  const assets={name:'geod_stac_assets',inputSchema:{properties:{id:{}},required:['id'],additionalProperties:false}};
  assert.doesNotThrow(()=>checkToolArguments(active,assets,{id:'a'.repeat(64)}));
  assert.doesNotThrow(()=>checkToolArguments({},project,{bounds:[1,2,3,4],selections:[]}));
});
test('generic schema errors never give coverage-only instructions to STAC project tools',()=>{
  for(const message of ['Invalid Agent tool arguments.','Invalid GeoD tool arguments','Invalid dataset connector arguments.']){
    const text=safeToolFailure(new Error(message));assert(!text.includes('not an extra bounds'));assert.match(text,/Keep required bounds/);
  }
  assert.match(safeToolFailure(new Error('Invalid coverage selection.')),/not an extra bounds/);
});
