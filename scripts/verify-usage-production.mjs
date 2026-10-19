import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {createUsageClient,DEFAULT_USAGE_ENDPOINT} from '../prototype/src/usage-client.js';

// Synthetic development-only canary; not a packaged-application or user-usage claim.
const origin='http://127.0.0.1:4317',endpoint=DEFAULT_USAGE_ENDPOINT,batches=[];
const preflight=await fetch(endpoint,{method:'OPTIONS',headers:{Origin:origin,'Access-Control-Request-Method':'POST','Access-Control-Request-Headers':'content-type'}});
assert.equal(preflight.status,200);assert.equal(preflight.headers.get('Access-Control-Allow-Origin'),origin);
const client=createUsageClient({storage:null,endpoint,privacy:()=>false,environment:'development',platform:'desktop',
  fetcher:async(url,request)=>{
    batches.push(JSON.parse(request.body));
    const response=await fetch(url,{...request,headers:{...request.headers,Origin:origin}});
    assert.equal(response.headers.get('Access-Control-Allow-Origin'),origin);return response;
  }});
assert.equal(client.snapshot().sharing,true);const stop=client.start();stop();
client.record('page_view',{page:'Settings',surface:'settings',channel:'manual'});
await client.flush();assert.equal(client.snapshot().pending,0);assert.equal(batches.length,1);
const retry=await fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/json',Origin:origin},body:JSON.stringify(batches[0])});
assert.equal(retry.status,200);assert.deepEqual((await retry.json()).acceptedIds,batches[0].events.map(event=>event.id));
const invalid=structuredClone(batches[0]);invalid.events[0].fields.message='must-be-rejected';
assert.equal((await fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/json',Origin:origin},body:JSON.stringify(invalid)})).status,400);
assert.equal((await fetch('https://geod-global.laogao.xyz/api/usage/v1/report')).status,404);
client.setPreferences({enabled:false});client.record('page_view');await client.flush();assert.equal(batches.length,1);
const receipt={type:'production-receiver-https-canary',at:new Date().toISOString(),syntheticObservations:true,environment:'development',
  packagedWebViewVerified:false,defaultOnWithoutSettings:true,corsVerified:true,receiptVerified:true,sensitiveFieldsRejected:true,
  reportNotPublic:true,optOutStopsSending:true,installationId:batches[0].events[0].installationId,eventIds:batches[0].events.map(event=>event.id)};
await mkdir('.verification/usage-20261019',{recursive:true});
await writeFile('.verification/usage-20261019/production-receiver.json',JSON.stringify(receipt,null,2)+'\n');
console.log(JSON.stringify(receipt,null,2));
