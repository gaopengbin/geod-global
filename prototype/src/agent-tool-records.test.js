import test from 'node:test';
import assert from 'node:assert/strict';
import {toolObservations} from './agent-tool-records.js';
const i18n={t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]),number:(n,options)=>new Intl.NumberFormat('en',options).format(n)};

test('current catalog observations show filtered count and pagination state without claiming coverage/downloads',()=>{
  const catalog={totalCount:162,filteredCount:15,selectedCount:0,loadedCount:0,complete:true,state:'ready'};
  const entry={name:'geod_workspace_context',record:{result:{context:{catalog}}}};
  const ready=toolObservations(entry,[],i18n);
  assert.equal(ready.outcome,'Current imagery list · 15 scenes');
  assert(ready.observations.some(value=>value.label==='Catalog results'&&value.value==='162'));
  assert(ready.observations.some(value=>value.value==='Catalog search complete'));
  const partial=toolObservations({...entry,record:{result:{context:{catalog:{...catalog,complete:false,state:'loading'}}}}},[],i18n);
  assert(partial.observations.some(value=>value.value==='Catalog results are partial'));
  assert(!partial.observations.some(value=>/coverage|download/i.test(value.label)));
});
test('native place and coverage observations distinguish candidate extents from complete imagery coverage',()=>{
  const place=toolObservations({summary:{kind:'places',count:1,total:1},record:{result:{candidates:[{name:'Berlin',adminLevel:1,countryCode:'DEU',bounds:[1,2,3,4]}]}}},[],i18n);
  assert.match(place.outcome,/1 place matches · Berlin/);assert(!place.observations.some(v=>v.label.includes('coverage')));
  const partial=toolObservations({record:{result:{coverage:{status:'partial',coveredFraction:0.8,sceneCount:3}}}},[],i18n);
  assert.equal(partial.outcome,'Catalog coverage incomplete');assert(partial.observations.some(v=>v.value==='80%'));
  const unknown=toolObservations({record:{result:{coverage:{status:'unknown',coveredFraction:1}}}},[],i18n);
  assert.equal(unknown.outcome,'Catalog coverage unknown');assert(!unknown.observations.some(v=>v.value==='100%'));
});
test('old read records stay unknown and pending native reviews never claim a delivered file',()=>{
  assert.equal(toolObservations({summary:{kind:'read'}},[],i18n).outcome,'');
  const plan=toolObservations({references:[{kind:'plan',id:'owned'}]},[{planId:'owned',status:'pending',files:[{}],source:'Native catalog'}],i18n);
  assert.equal(plan.outcome,'Review awaiting confirmation');
  assert.equal(toolObservations({summary:{kind:'job',status:'succeeded',settled:false}},[],i18n).outcome,'Task is not complete');
});
