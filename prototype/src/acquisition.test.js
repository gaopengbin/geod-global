import test from 'node:test';
import assert from 'node:assert/strict';
import {acquisitionLabel,initializeAcquisition} from './acquisition.js';
import {createUsageClient,USAGE_STORAGE_KEY} from './usage-client.js';

const label=source=>({schemaVersion:1,source,campaign:'landsat_batch_20261023'});
function fixture(initial){
  const map=new Map(initial?[[USAGE_STORAGE_KEY,JSON.stringify(initial)]]:[]);
  const storage={getItem:k=>map.get(k)||null,setItem:(k,v)=>map.set(k,v)};
  const client=createUsageClient({storage,privacy:()=>false});
  return {client,storage};
}

test('public campaign labels reject arbitrary values and extra payloads',()=>{
  assert.equal(acquisitionLabel({...label('zhihu'),url:'https://private'}),null);
  assert.equal(acquisitionLabel(label('private-account')),null);
  assert.equal(acquisitionLabel({...label('bilibili'),campaign:'private-name'}),null);
});
test('first installation and real task results retain the first distribution cohort after reopening',async()=>{
  const {client,storage}=fixture();
  await initializeAcquisition(client,async command=>{assert.equal(command,'acquisition_context');return label('zhihu');});
  client.record('search_completed',{source:'planetary-landsat',channel:'manual',acquisitionSource:'bilibili'});
  await Promise.resolve();
  const first=client.report().events;
  assert.equal(first[0].name,'app_first_run');
  for(const event of first){assert.equal(event.fields.acquisitionSource,'zhihu');assert.equal(event.fields.acquisitionKind,'new_installation');}
  const reopened=createUsageClient({storage,privacy:()=>false});
  assert.equal(reopened.setAcquisition(label('bilibili')),false);
  const job={id:crypto.randomUUID(),kind:'download',status:'queued'};
  reopened.watchJobs([job],{source:'planetary-landsat',channel:'manual'});
  reopened.observeJobs([{...job,status:'succeeded',sha256:'a'.repeat(64),bytesDownloaded:1024}]);
  const last=reopened.report().events.at(-1);
  assert.equal(last.name,'artifact_validated');assert.equal(last.fields.acquisitionSource,'zhihu');
  assert.equal(last.fields.channel,'manual');assert.equal(last.fields.source,'planetary-landsat');
});
test('existing anonymous installations are not miscounted as new campaign installs',async()=>{
  const {client}=fixture();client.record('app_started');
  client.setAcquisition(label('bilibili'));client.record('search_completed');
  assert.equal(client.report().events.at(-1).fields.acquisitionKind,'existing_installation');
});
test('saved opt-outs and privacy signals prevent campaign reads and persistence',async()=>{
  const {client,storage}=fixture();client.setPreferences({enabled:false});
  let calls=0;await initializeAcquisition(client,async()=>{calls++;return label('zhihu');});
  assert.equal(calls,0);assert.equal(client.setAcquisition(label('zhihu')),false);
  assert.equal(JSON.parse(storage.getItem(USAGE_STORAGE_KEY)).acquisition,undefined);
  const blocked=createUsageClient({storage:null,privacy:()=>true});
  await initializeAcquisition(blocked,async()=>{calls++;return label('zhihu');});assert.equal(calls,0);
});
test('missing or broken native command cannot prevent application startup',async()=>{
  const {client}=fixture();await initializeAcquisition(client,async()=>{throw new Error('unavailable');});
  client.record('app_started');assert.equal(client.report().events.at(-1).fields.acquisitionSource,undefined);
});
