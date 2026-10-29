import test from 'node:test';
import assert from 'node:assert/strict';
import {createUsageClient,USAGE_STORAGE_KEY,usageEndpoint} from './usage-client.js';
import {beginUsage,usageContext,usageSource} from './usage-events.js';

function fixture(options={}) {
  const values=new Map(),storage={getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)};
  let time=Date.now();const calls=[];
  const client=createUsageClient({storage,now:()=>time,privacy:()=>false,endpoint:'http://127.0.0.1:4392/v1/events',
    fetcher:async(url,request)=>{calls.push({url,request});return {ok:true,json:async()=>({acceptedIds:JSON.parse(request.body).events.map(event=>event.id)})};},...options});
  return {client,storage,values,calls,advance:ms=>{time+=ms;}};
}
test('fresh installations record and upload on startup without visiting settings',async()=>{
  const {client,values,calls}=fixture();
  assert.equal(client.snapshot().enabled,true);assert.equal(client.snapshot().sharing,true);assert.equal(values.size,0);
  const stop=client.start();
  try{
    await new Promise(resolve=>setImmediate(resolve));
    assert.equal(client.report().counts.app_first_run,1);assert.equal(client.report().counts.app_started,1);
    assert.equal(calls.length,1);assert.equal(client.snapshot().pending,0);
    assert.equal(calls[0].request.keepalive,true);assert.equal(values.size,1);
  }finally{stop();}
});

test('saved collection and sharing opt-outs survive reopening and upgrades',async()=>{
  const {client,storage,calls}=fixture();
  client.setPreferences({enabled:false});
  const reopened=createUsageClient({storage,privacy:()=>false,endpoint:'http://127.0.0.1:4392/v1/events',fetcher:async()=>{throw new Error('must not send');}});
  const stop=reopened.start();reopened.record('page_view',{page:'Home'});await reopened.flush();stop();
  assert.equal(reopened.snapshot().enabled,false);assert.equal(reopened.snapshot().recorded,0);assert.equal(calls.length,0);
  client.setPreferences({enabled:true,share:false});
  const local=createUsageClient({storage,privacy:()=>false,endpoint:'http://127.0.0.1:4392/v1/events'});
  assert.equal(local.snapshot().enabled,true);assert.equal(local.snapshot().sharing,false);
});

test('an automatic local-only default starts sharing when a receiver becomes available, but a chosen opt-out does not',async()=>{
  const {client,storage}=fixture({endpoint:null});client.record('page_view',{page:'Home'});
  await Promise.resolve();
  const connected=createUsageClient({storage,privacy:()=>false,endpoint:'https://geod-global.laogao.xyz/api/usage/v1/events'});
  assert.equal(connected.snapshot().sharing,true);
  connected.setPreferences({share:false});
  const reopened=createUsageClient({storage,privacy:()=>false,endpoint:'https://geod-global.laogao.xyz/api/usage/v1/events'});
  assert.equal(reopened.snapshot().sharing,false);
});
test('only allowlisted coarse fields survive; accounts, content, coordinates and URLs are excluded',()=>{
  const {client}=fixture();client.setPreferences({enabled:true});
  client.record('source_selected',{source:'planetary-naip',surface:'manual_explore',channel:'manual',
    message:'private dialogue',key:'secret',bounds:[1,2,3,4],path:'C:/private.tif',project:'private name',url:'https://source.test/?token=private'});
  const event=client.report().events.at(-1);
  assert.deepEqual(event.fields,{surface:'manual_explore',channel:'manual',source:'planetary-naip'});
  assert.equal(event.product,'geod-global');assert.ok(event.installationId);assert.equal(client.snapshot().sharing,true);
  assert.equal(client.record('arbitrary_user_event',{value:'private'}),null);
  assert.equal(usageSource('https://private.test'),'other');
});
test('queued retries keep event IDs and only acknowledge the exact accepted batch',async()=>{
  let failed=true;const sent=[];
  const {client,advance}=fixture({fetcher:async(_url,request)=>{const body=JSON.parse(request.body);sent.push(body);if(failed)throw new Error('offline');return {ok:true,json:async()=>({acceptedIds:body.events.map(event=>event.id)})};}});
  client.setPreferences({enabled:true,share:true});await client.flush();
  assert.equal(client.snapshot().status,'waiting');await client.flush();assert.equal(sent.length,1);
  failed=false;advance(16000);await client.flush();
  assert.deepEqual(sent[0],sent[1]);assert.equal(client.snapshot().pending,0);
});
test('local recording and anonymous sharing are separate preferences and transport omits credentials',async()=>{
  const {client,calls}=fixture();client.setPreferences({enabled:true,share:false});await client.flush();assert.equal(calls.length,0);
  client.setPreferences({share:true});await client.flush();
  assert.equal(calls[0].request.credentials,'omit');assert.equal(calls[0].request.referrerPolicy,'no-referrer');
  assert.equal(client.snapshot().pending,0);
});
test('revoking consent aborts pending uploads, removes local records and rejects late results',async()=>{
  let settle,request;
  const {client,storage}=fixture({fetcher:async(_url,value)=>{request=value;return new Promise(resolve=>{settle=resolve;});}});
  client.setPreferences({enabled:true,share:true});const pending=client.flush();
  client.setPreferences({enabled:false});assert.equal(request.signal.aborted,true);
  settle({ok:true,json:async()=>({acceptedIds:JSON.parse(request.body).events.map(event=>event.id)})});await pending;
  assert.equal(client.snapshot().recorded,0);assert.equal(client.snapshot().enabled,false);
  assert.equal(JSON.parse(storage.getItem(USAGE_STORAGE_KEY)).installationId,null);
});
test('GPC or DNT blocks collection and sharing even with a stored preference',async()=>{
  const {client,calls}=fixture({privacy:()=>true});client.setPreferences({enabled:true,share:true});client.record('app_started');await client.flush();
  assert.equal(client.snapshot().blocked,true);assert.equal(client.snapshot().recorded,0);assert.equal(calls.length,0);
});
test('local retention is bounded and clearing rotates the anonymous installation without a remote request',()=>{
  const {client,calls,advance}=fixture();client.setPreferences({enabled:true});
  const old=client.report().events[0].installationId;
  for(let n=0;n<2010;n++)client.record('page_view',{page:'Explore'});
  assert.equal(client.report().events.length,2000);advance(31*86400000);assert.equal(client.report().events.length,0);
  client.clear();client.record('page_view',{page:'Home'});
  assert.notEqual(client.report().events[0].installationId,old);assert.equal(calls.length,0);
});
test('job completion is observed after submission, persists through reopening, and is deduplicated',()=>{
  const {client,storage}=fixture();client.setPreferences({enabled:true});
  const job={id:crypto.randomUUID(),kind:'download',status:'queued'};
  client.watchJobs([job],{surface:'agent_explore',channel:'agent',action:'download_submit'});
  assert.equal(client.report().counts.download_completed,0);
  const reopened=createUsageClient({storage,privacy:()=>false});
  reopened.observeJobs([{...job,status:'succeeded',sha256:'a'.repeat(64),bytesDownloaded:1024}]);
  reopened.observeJobs([{...job,status:'succeeded',sha256:'a'.repeat(64)}]);
  assert.equal(reopened.report().counts.download_completed,1);assert.equal(reopened.report().counts.artifact_validated,1);
  assert.equal(reopened.report().events.at(-1).fields.surface,'agent_explore');
  assert.equal(JSON.stringify(reopened.report().events).includes(job.id),false);
  reopened.observeJobs([{id:crypto.randomUUID(),status:'succeeded',sha256:'b'.repeat(64)}]);
  assert.equal(reopened.report().counts.download_completed,1);
});
test('cancelled batches retain native queue versus transfer stage through reopening without exposing job IDs',()=>{
  const {client,storage}=fixture();client.setPreferences({enabled:true});
  const queued={id:crypto.randomUUID(),kind:'download',status:'queued',bytesDownloaded:0};
  const transferring={...queued,id:crypto.randomUUID()};
  client.watchJobs([queued,transferring],{source:'earth-search',channel:'manual',action:'download_submit'});
  client.observeJobs([{...transferring,status:'running',bytesDownloaded:1024}]);
  const reopened=createUsageClient({storage,privacy:()=>false});
  reopened.observeJobs([{...queued,status:'cancelled',networkRetries:0,error:'Cancelled while waiting for a download slot. Retry restarts the operation.'},{...transferring,status:'cancelled',bytesDownloaded:1024,networkRetries:1,error:'Cancelled during an active download. Retry restarts the operation.'}]);
  const events=reopened.report().events.filter(e=>e.name==='download_cancelled');
  assert.equal(events.length,2);
  assert.equal(events[0].fields.stage,'download_queue');assert.equal(events[0].fields.retries,'0');
  assert.equal(events[1].fields.stage,'transfer');assert.equal(events[1].fields.retries,'1');
  assert.equal(events[0].fields.diagnosticOrigin,'explicit');
  assert.equal(events[0].fields.reason,'cancelled');assert.equal(JSON.stringify(events).includes(queued.id),false);
  const legacy={...queued,id:crypto.randomUUID()};reopened.watchJobs([legacy],{action:'download_submit'});
  reopened.observeJobs([{...legacy,status:'cancelled',error:'Cancelled by user. Retry restarts the operation.'}]);
  assert.equal(reopened.report().events.at(-1).fields.stage,'operation');
  assert.equal(reopened.report().events.at(-1).fields.diagnosticOrigin,'classified');
});

test('operation outcomes are once-only and an old consent generation cannot reappear',()=>{
  const {client}=fixture();client.setPreferences({enabled:true});
  const request=beginUsage('search',{surface:'manual_explore',source:'earth-search'},client);request.finish({scenes:[]});request.finish({scenes:[{}]});
  assert.equal(client.report().counts.search_completed,1);assert.equal(client.report().events.at(-1).fields.result,'empty');
  const old=beginUsage('search',{},client);client.setPreferences({enabled:false});client.setPreferences({enabled:true});old.finish({scenes:[{}]});
  assert.equal(client.report().counts.search_completed,0);
  const current=beginUsage('search',{},client);client.setPreferences({share:true});current.finish({scenes:[{}]});
  assert.equal(client.report().counts.search_completed,1);
});
test('scene surfaces and navigation omit route parameters and distinguish each Agent entry',()=>{
  assert.deepEqual(usageContext({page:'Explore'}),{page:'Explore',surface:'manual_explore',channel:'manual'});
  assert.equal(usageContext({page:'Explore',agent:true}).surface,'agent_explore');
  assert.equal(usageContext({page:'Home',agent:true}).surface,'agent_home');
  assert.equal(usageContext({page:'Workspace',agent:true}).surface,'agent_workspace');
});
test('invalid receipts are retried and corrupt or blocked storage never breaks application work',async()=>{
  const {client}=fixture({fetcher:async()=>({ok:true,json:async()=>({acceptedIds:[crypto.randomUUID()]})})});
  client.setPreferences({enabled:true,share:true});await client.flush();assert.ok(client.snapshot().pending>0);
  const broken=createUsageClient({storage:{getItem(){throw new Error('denied');},setItem(){throw new Error('full');}},privacy:()=>false});
  assert.equal(broken.snapshot().enabled,false);assert.doesNotThrow(()=>broken.setPreferences({enabled:true}));
  assert.equal(usageEndpoint('http://external.test/v1/events'),null);assert.equal(usageEndpoint('https://a.test/v1/events?token=x'),null);
});

test('sharing consent is bound to the configured receiver and is not transferred to a changed endpoint',()=>{
  const {client,storage}=fixture();client.setPreferences({enabled:true,share:true});
  const same=createUsageClient({storage,privacy:()=>false,endpoint:'http://127.0.0.1:4392/v1/events'});
  const changed=createUsageClient({storage,privacy:()=>false,endpoint:'https://geod-global.laogao.xyz/api/usage/v1/events'});
  assert.equal(same.snapshot().sharing,true);assert.equal(changed.snapshot().sharing,false);
  assert.equal(changed.snapshot().enabled,true);assert.equal(changed.report().events.length,client.report().events.length);
});
