import test from 'node:test';
import assert from 'node:assert/strict';
import {usage} from './usage-client.js';
import {usageRequest} from './usage-events.js';
import {runtimeRequest} from './runtime-client.js';

test('real runtime request boundaries retain native results and count terminal outcomes without uploading task data',async()=>{
  const previousWindow=globalThis.window,previousLocation=globalThis.location;
  const job={id:crypto.randomUUID(),kind:'download',status:'queued',href:'https://naipeuwest.blob.core.windows.net/naip/private-scene.tif?sig=private-secret',title:'private project',outputPath:'C:/private/file.tif'};
  const calls=[];let snapshot=[job];
  globalThis.location={hash:'#Explore?project=private-project'};
  globalThis.window={__TAURI__:{core:{invoke:async(command,args)=>{calls.push([command,args]);return command==='list_jobs'?snapshot:{jobs:[job]};}}}};
  usage.setPreferences({enabled:true});
  try{
    const request={id:crypto.randomUUID(),assetKey:'aerial'};
    const receipt=await runtimeRequest('downloadProject',request);
    assert.deepEqual(receipt,{jobs:[job]});assert.deepEqual(calls[0],['download_project',request]);
    assert.equal(usage.report().counts.download_submit_completed,1);assert.equal(usage.report().counts.download_completed,0);
    snapshot=[{...job,status:'succeeded',sha256:'a'.repeat(64),bytesDownloaded:1000}];
    assert.deepEqual(await runtimeRequest('list'),snapshot);await runtimeRequest('list');
    assert.equal(usage.report().counts.download_completed,1);assert.equal(usage.report().counts.artifact_validated,1);
    const complete=usage.report().events.find(event=>event.name==='download_completed');
    assert.equal(complete.fields.source,'planetary-naip');assert.equal(complete.fields.surface,'manual_explore');
    const serialized=JSON.stringify(usage.report());
    for(const secret of [job.id,'private-secret','private project','private-scene','private/file','private-project'])assert.equal(serialized.includes(secret),false);
  }finally{usage.setPreferences({enabled:false});globalThis.window=previousWindow;globalThis.location=previousLocation;}
});

test('an Agent task keeps its Explore origin when polling from Home and separates acceptance from completion',async()=>{
  const previousLocation=globalThis.location;globalThis.location={hash:'#Explore'};usage.setPreferences({enabled:true});
  try{
    const running={selected:{id:crypto.randomUUID(),status:'running'},plans:[]};
    const accepted=await usageRequest('agent','send',{text:'private question',scene:{kind:'workspace',page:'Explore'}},async()=>running);
    assert.equal(accepted,running);assert.equal(usage.report().counts.agent_turn_completed,0);
    globalThis.location={hash:'#Home'};
    const job={id:crypto.randomUUID(),kind:'download',status:'running'};
    const finished={selected:{...running.selected,status:'completed'},plans:[{kind:'download',status:'submitted',jobs:[job]}]};
    await usageRequest('agent','snapshot',{},async()=>finished);
    assert.equal(usage.report().counts.agent_turn_completed,1);
    usage.observeJobs([{...job,status:'succeeded',sha256:'b'.repeat(64)}]);
    const records=usage.report().events.filter(event=>['agent_turn_completed','download_completed'].includes(event.name));
    assert.ok(records.every(event=>event.fields.surface==='agent_explore'&&event.fields.channel==='agent'));
    assert.equal(JSON.stringify(usage.report()).includes('private question'),false);
  }finally{usage.setPreferences({enabled:false});globalThis.location=previousLocation;}
});
