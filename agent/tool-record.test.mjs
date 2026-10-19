import test from 'node:test';
import assert from 'node:assert/strict';
import {startToolRecord,finishToolRecord,validToolRecord} from './tool-record.mjs';

test('GSCloud receipts keep product identity, presence, pagination and selected item metadata without session secrets',()=>{
  const input={productId:'421',bounds:[116.2,40.1,116.4,40.3],presence:'has',page:1,pageSize:2};
  const result={request:input,product:{id:'421',title:'GDEMV2 30M',seriesId:'DEM'},items:[{id:'ASTGTM2_N40E116',productId:'421',presence:'available',downloadStatus:'account-required'}],total:1,totalPages:1,hasMore:false,downloadsStarted:false,sid:'private',uid:'private'};
  const record=finishToolRecord(startToolRecord(input,0),result,500);
  assert(validToolRecord(record));assert.equal(record.input.productId,'421');assert.equal(record.input.presence,'has');
  assert.equal(record.result.items[0].id,'ASTGTM2_N40E116');assert.equal(record.result.items[0].productId,'421');assert.equal(record.result.totalPages,1);
  assert.equal(record.result.downloadsStarted,false);assert(!JSON.stringify(record).includes('private'));
});
test('native input and result projections retain actual scope and omit credentials, paths, URLs and geometry payloads',()=>{
  const record=finishToolRecord(startToolRecord({query:'Manhattan',countryCode:'USA',bounds:[-74,40,-73,41],password:'private',name:'https://example.test/?token=private'},0),
    {candidates:[{name:'New York County',adminLevel:2,countryCode:'USA',bounds:[-74,40,-73,41],geometry:{coordinates:[1,2]},url:'https://private.test'}],matchCount:1,apiKey:'private',outputPath:'C:/private/a.tif',provenance:{provider:'geoBoundaries',source:'https://example.test',dataYear:'2018'}},2350);
  assert.equal(validToolRecord(record),true);
  assert.equal(record.input.query,'Manhattan');assert.deepEqual(record.input.bounds,[-74,40,-73,41]);
  assert.equal(record.result.candidates[0].name,'New York County');assert.equal(record.result.provenance.dataYear,'2018');
  assert.equal(record.durationMs,2350);assert.equal(record.limited,true);
  assert(!JSON.stringify(record).includes('private'));assert(!JSON.stringify(record).includes('coordinates'));
});
test('large or deeply nested results are bounded and marked as excerpts without inventing complete coverage',()=>{
  const record=finishToolRecord(startToolRecord({},0),{coverage:{status:'partial',coveredFraction:0.4,basis:'catalog-footprints'},scenes:Array.from({length:200},(_,index)=>({itemId:`scene-${index}`,name:'"'.repeat(200),date:'2026-10-08',bounds:[1,2,3,4]}))},100);
  assert.equal(validToolRecord(record),true);assert.equal(record.limited,true);assert.equal(record.result.coverage.status,'partial');
  assert(record.result.scenes.length<=8);assert(JSON.stringify(record).length<2400);
});
test('receiver rejects forged display fields, secrets, unsafe strings and unbounded content',()=>{
  const record=finishToolRecord(startToolRecord({query:'Berlin'},0),{count:2},100);
  for(const changed of [{...record,approved:true},{...record,input:{password:'test'}},{...record,result:{name:'open C:/private/result.tif'}},{...record,result:{name:'https://private.test'}},{...record,result:{count:Infinity}},{...record,result:{scenes:Array(9).fill({})}},{...record,durationMs:-1}])assert.equal(validToolRecord(changed),false);
});
