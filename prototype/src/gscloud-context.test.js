import test from 'node:test';
import assert from 'node:assert/strict';
import {gscloudMapContext} from './startup.js';
const request={productId:'305',bounds:[116,39,117,40],start:'1980-01-01',end:'2026-10-15',cloudMax:null,presence:'none',page:1,pageSize:20};
test('GSCloud side-panel context retains its exact product filters and cannot leak to Home or another source/area',()=>{
  const input={page:'Explore',entry:{kind:'gscloud'},context:{request,catalog:null},bounds:request.bounds};
  const value=gscloudMapContext(input);assert.equal(value.provider,'gscloud');assert.deepEqual(value.gscloud,request);
  for(const changed of [{page:'Home'},{entry:{kind:'stac'}},{bounds:[115,39,116,40]},{context:null}])assert.equal(gscloudMapContext({...input,...changed}),null);
});
