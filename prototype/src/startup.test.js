import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultLiveSearch, SAMPLE_BBOX } from './catalog.js';
import { agentMapContext, agentScene, selectedSearchBounds } from './startup.js';

test('startup has no AOI and sends no invented map context to Agent', () => {
  const input = defaultLiveSearch();
  assert.equal(selectedSearchBounds(input), null);
  for (const page of ['Home','Explore','Tasks']) assert.equal(agentMapContext({page,input}), null);
});

test('Explore shares the actual catalog counts and current list filters, not historical project/job totals', () => {
  const input={...defaultLiveSearch(),bbox:'-14,-12,51,37',cloudMin:0,cloud:0};
  const scenes=Array.from({length:162},(_,index)=>({id:String(index),date:index<147?'2026-10-09T12:00:00Z':'2026-10-10T12:00:00Z',assets:{visual:{href:'https://example.com/private'}}}));
  const catalog={scenes,pages:2,complete:true,retrievedAt:'2026-10-10T12:00:00Z'};
  const base={page:'Explore',input,catalog,liveState:'ready',selectedIds:['0','0','missing'],loadedIds:['0','1'],visibleLoadedIds:['1']};
  const context=agentMapContext(base);
  assert.equal(context.catalog.totalCount,162);
  assert.equal(context.catalog.filteredCount,162);
  assert.deepEqual(context.catalog.dateCounts,[{date:'2026-10-10',count:15},{date:'2026-10-09',count:147}]);
  assert.equal(context.catalog.selectedCount,1);assert.equal(context.catalog.loadedCount,2);assert.equal(context.catalog.visibleLoadedCount,1);
  assert.deepEqual(context.catalog.selectedItemIds,['0']);assert.deepEqual(context.catalog.visibleItemIds,['1']);
  assert.equal(context.cloudMin,0);assert.equal(context.cloudMax,0);
  const filtered=agentMapContext({...base,activeDay:'2026-10-10',filteredScenes:scenes.slice(147),query:'2026-10-10'});
  assert.equal(filtered.catalog.filteredCount,15);assert.equal(filtered.catalog.totalCount,162);
  assert.equal(filtered.catalog.activeDate,'2026-10-10');
  assert(!JSON.stringify(context).includes('https://'));
  assert.equal(agentMapContext({...base,page:'Home'}),null);
  assert.equal(agentMapContext({...base,page:'Tasks'}),null);
});

test('workspace pages resolve their own native subject rather than inheriting a stale Explore area',()=>{
  const id='a1234567-1234-1234-1234-123456789abc',input={...defaultLiveSearch(),bbox:'-14,-12,51,37'};
  for(const page of ['Home','Tasks','Workspace','My Data','Settings','Imagery monitoring'])assert.equal(agentMapContext({page,input,projectId:id}),null);
  assert.deepEqual(agentScene({page:'Home',projectId:id,hash:`#Home?file=${id}`}),{kind:'conversation',page:'Home'});
  assert.deepEqual(agentScene({page:'My Data',projectId:id}),{kind:'workspace',page:'My Data',subject:{kind:'project',id}});
  assert.deepEqual(agentScene({page:'Workspace',hash:`#Workspace?file=${id}`}).subject,{kind:'raster',id});
  assert.deepEqual(agentScene({page:'Tasks',hash:`#Tasks?job=${id}`}).subject,{kind:'job',id});
});

test('no catalog is unknown, an empty completed catalog is zero, and interrupted pagination stays partial', () => {
  const base={page:'Explore',input:{...defaultLiveSearch(),bbox:'-14,-12,51,37'}};
  assert.equal(agentMapContext(base).catalog,null);
  assert.equal(agentMapContext({...base,catalog:{scenes:[],complete:true},liveState:'ready'}).catalog.totalCount,0);
  const partial=agentMapContext({...base,catalog:{scenes:[{id:'one',date:'2026-10-09T12:00:00Z'}],pages:1,complete:false},liveState:'error'});
  assert.equal(partial.catalog.totalCount,1);assert.equal(partial.catalog.complete,false);assert.equal(partial.catalog.state,'error');
});
test('manual/project bounds are preserved, while Home uses native conversation defaults', () => {
  const input = {...defaultLiveSearch(),bbox:'13.1, 52.3, 13.8, 52.7',provider:'planetary-computer'};
  assert.deepEqual(selectedSearchBounds(input), [13.1,52.3,13.8,52.7]);
  assert.equal(agentMapContext({page:'Home',input}), null);
  const appliedSearch={...input,bbox:SAMPLE_BBOX};
  const context=agentMapContext({page:'Explore',input,appliedSearch,projectId:'saved-project'});
  assert.deepEqual(context.bounds, SAMPLE_BBOX);
  assert.equal(context.projectId,'saved-project');
  assert.equal(context.provider,'planetary-computer');
});

test('public products use the current form area and never inherit old satellite dates, counts or selections', () => {
  const bounds=[110,40,110.1,40.1];
  const input={...defaultLiveSearch(),bbox:bounds.join(',')};
  const datasetRequest={datasetId:'jrc-water',bounds,layer:'occurrence'};
  const base={page:'Explore',input,appliedSearch:{...input,bbox:[10,20,11,21]},datasetId:'jrc-water',datasetRequest,
    catalog:{scenes:[{id:'old-scene',date:'2025-01-01'}],complete:true},selectedIds:['old-scene']};
  const context=agentMapContext(base);
  assert.deepEqual(context.bounds,bounds);assert.equal(context.provider,'dataset-jrc-water');
  assert.deepEqual(context.dataset,datasetRequest);assert.equal(context.catalog,null);
  assert.equal(context.start,'');assert.equal(context.end,'');assert(!JSON.stringify(context).includes('old-scene'));
  assert(!agentMapContext({...base,datasetRequest:{...datasetRequest,bounds:[10,20,11,21]}}).dataset);
  assert.equal(agentMapContext({...base,page:'Home'}),null);
  const monthly={datasetId:'chirps',bounds,year:2024,month:2,layer:'monthly-precipitation'};
  assert.deepEqual(agentMapContext({...base,datasetId:'chirps',datasetRequest:monthly}).dataset,monthly);
});
