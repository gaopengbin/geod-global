import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
import path from 'node:path';
import {createUsageClient} from '../prototype/src/usage-client.js';
import {beginUsage} from '../prototype/src/usage-events.js';

// Uses synthetic observations to test the real client -> HTTP -> SQLite chain.
// This is neither an imagery download nor a production collection claim.
const folder=path.resolve('.verification/usage-20261019');await mkdir(folder,{recursive:true});
const token=randomUUID(),db=path.join(folder,`acceptance-${randomUUID()}.sqlite`);
const child=spawn('python',['-X','utf8','services/usage/server.py','--db',db,'--port','0'],{
  cwd:process.cwd(),env:{...process.env,GEOD_USAGE_ADMIN_TOKEN:token},windowsHide:true,stdio:['ignore','pipe','pipe']});
let stderr='';child.stderr.on('data',data=>{stderr+=data;});
try{
  const port=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>reject(new Error('Usage receiver startup timed out.')),10000);let output='';
    child.stdout.on('data',data=>{output+=data;const match=/ready on 127\.0\.0\.1:(\d+)/.exec(output);if(match){clearTimeout(timer);resolve(Number(match[1]));}});
    child.once('error',error=>{clearTimeout(timer);reject(error);});
    child.once('exit',code=>{clearTimeout(timer);reject(new Error(`Usage receiver exited: ${code}. ${stderr}`));});
  });
  const endpoint=`http://127.0.0.1:${port}/v1/events`,batches=[];
  const storageMap=new Map(),storage={getItem:key=>storageMap.get(key)||null,setItem:(key,value)=>storageMap.set(key,value)};
  const client=createUsageClient({storage,endpoint,privacy:()=>false,environment:'development',platform:'desktop',
    fetcher:async(url,request)=>{batches.push(JSON.parse(request.body));return fetch(url,request);}});
  const manual={page:'Explore',surface:'manual_explore',channel:'manual',source:'planetary-naip'};
  const agent={page:'Home',surface:'agent_home',channel:'agent',source:'gscloud'};
  // No settings or opt-in call: fresh installations collect and share by default.
  assert.equal(client.snapshot().enabled,true);assert.equal(client.snapshot().sharing,true);
  const stop=client.start();stop();
  beginUsage('search',manual,client).finish({scenes:[{}]});
  client.record('download_review_opened',manual);client.record('download_confirmed',manual);
  const job={id:randomUUID(),kind:'download',status:'queued'};
  beginUsage('download_submit',manual,client).finish({jobs:[job]});
  assert.equal(client.report().counts.download_completed,0);
  client.observeJobs([{...job,status:'succeeded',bytesDownloaded:1024,sha256:createHash('sha256').update('synthetic verification fixture').digest('hex')}]);
  beginUsage('search',agent,client).finish({scenes:[]});
  const failed={id:randomUUID(),kind:'download',status:'running'};client.watchJobs([failed],agent);
  client.observeJobs([{...failed,status:'failed',error:'Network offline: private-token-sentinel https://private.test/115,39'}]);
  assert.equal(batches.length,0);
  await client.flush();assert.equal(client.snapshot().pending,0);
  const expected=client.report().events.length;
  const repeated=await fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(batches[0])});assert.equal(repeated.status,200);
  const report=await (await fetch(`http://127.0.0.1:${port}/v1/report`,{headers:{Authorization:`Bearer ${token}`}})).json();
  assert.equal(report.receivedEvents,expected);assert.equal(report.events.download_completed,1);assert.equal(report.events.artifact_validated,1);assert.equal(report.events.download_failed,1);
  assert.ok(report.bySurface.some(row=>row.surface==='manual_explore'));assert.ok(report.bySurface.some(row=>row.surface==='agent_home'));
  const payload=JSON.stringify(batches);for(const excluded of [job.id,failed.id,'private-token-sentinel','private.test','115,39'])assert.equal(payload.includes(excluded),false);
  const invalid=structuredClone(batches[0]);invalid.events[0].fields.message='private-token-sentinel';
  assert.equal((await fetch(endpoint,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(invalid)})).status,400);
  client.setPreferences({enabled:false});client.record('page_view',manual);await client.flush();assert.equal(client.snapshot().recorded,0);
  const receipt={type:'local-client-http-sqlite-integration',syntheticObservations:true,productionDeploymentVerified:false,
    receivedEvents:report.receivedEvents,defaultOnWithoutSettings:true,retryDeduplicated:true,sensitiveFieldsRejected:true,optOutStopsCollection:true,
    sourceAndSurfaceReport:report.bySurface,events:report.events,database:path.relative(process.cwd(),db)};
  await writeFile(path.join(folder,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify(receipt,null,2));
}finally{
  if(child.exitCode===null){child.kill();await new Promise(resolve=>child.once('exit',resolve));}
}
