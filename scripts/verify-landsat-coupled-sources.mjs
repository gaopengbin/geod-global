// Real public download cohort. Reuses the previously verified originals and
// requests missing RGB bands through the application's native project adapter.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,copyFile,stat} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';

const root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4657);
assert.equal(path.dirname(root),path.resolve('.verification'));
assert(path.basename(root).startsWith('landsat-coupled-sources-'));
await mkdir(root);await mkdir(path.join(root,'assets'));
const digest=async file=>{const h=createHash('sha256');for await(const b of createReadStream(file))h.update(b);return h.digest('hex');};
const binarySource=path.resolve('.verification/landsat-rgb-mask-20261004/runtime-51a09564e71c921f.exe');
const binaryHash=await digest(binarySource);assert.equal(binaryHash,'51a09564e71c921f6f5ddb11e3b3cad8b19a2ed8ece139eda74e7d8642756774');
const exe=path.join(root,'runtime-'+binaryHash.slice(0,16)+'.exe');await copyFile(binarySource,exe);
const ids=['LC09_L2SP_044034_20250612_02_T1','LC08_L2SP_045034_20250627_02_T1','LC09_L2SP_044034_20250628_02_T1'];
const keys=['red','green','blue','qa_pixel','qa_radsat'];
const jobs={},reused=[];
for(const dir of ['.verification/landsat-quality-processing-20261004','.verification/local-rgb-native-20261002']){
  const stored=JSON.parse(await readFile(path.join(dir,'jobs.json'),'utf8'));
  for(const job of Object.values(stored).filter(j=>j.kind==='download'&&j.status==='succeeded'&&ids.includes(j.itemId)&&keys.includes(j.assetKey))){
    if(Object.values(jobs).some(j=>j.itemId===job.itemId&&j.assetKey===job.assetKey))continue;
    assert.equal(await digest(job.outputPath),job.sha256);
    const destination=path.join(root,'assets',job.id+'.tif');await copyFile(job.outputPath,destination);assert.equal(await digest(destination),job.sha256);
    reused.push({id:job.id,itemId:job.itemId,key:job.assetKey,sha256:job.sha256,sourcePath:job.outputPath,sourceMtimeMs:(await stat(job.outputPath)).mtimeMs});
    jobs[job.id]={...job,outputPath:destination};
  }
}
assert.equal(reused.length,9);await writeFile(path.join(root,'jobs.json'),JSON.stringify(jobs,null,2));
const url='https://planetarycomputer.microsoft.com/api/stac/v1/search';
const body={collections:['landsat-c2-l2'],bbox:[-123.6,37.6,-122.2,38.1],datetime:'2025-06-01T00:00:00Z/2025-06-29T23:59:59Z',limit:100};
const response=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(60000)});assert(response.ok);
const catalog=await response.json();assert(!catalog.links?.some(l=>l.rel==='next'));
const items=ids.map(id=>{const item=catalog.features.find(i=>i.id===id);assert(item);return item;});
await writeFile(path.join(root,'source-catalog.json'),JSON.stringify(catalog,null,2));
await writeFile(path.join(root,'source-items.json'),JSON.stringify(items,null,2));
const request={url,method:'POST',body,retrievedAt:new Date().toISOString()};
await writeFile(path.join(root,'source-catalog-request.json'),JSON.stringify(request,null,2));
const report={schema:'geod-landsat-coupled-sources/v1',checkedAt:new Date().toISOString(),status:'running',nativeBinary:exe,nativeBinarySha256:binaryHash,catalog:request,reusedOriginals:reused,originals:[],downloadFailures:[]};
const persist=()=>writeFile(path.join(root,'source-verification.json'),JSON.stringify(report,null,2));
await persist();
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','ignore','pipe']});let errors='';runtime.stderr.on('data',d=>errors+=d);
const base=`http://127.0.0.1:${port}`;
async function api(route,body){const r=await fetch(base+route,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(180000)});const data=await r.json();assert(r.ok,JSON.stringify(data));return data;}
async function wait(job){for(let started=Date.now();Date.now()-started<600000;){const j=await api('/jobs/'+job.id);if(j.status==='succeeded'&&j.settled)return j;assert(!['failed','cancelled','interrupted'].includes(j.status),JSON.stringify(j));assert.equal(runtime.exitCode,null,errors);await new Promise(resolve=>setTimeout(resolve,500));}throw new Error('Download did not settle');}
try{
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert.equal(runtime.exitCode,null,errors);if(i===99)throw e;await new Promise(resolve=>setTimeout(resolve,100));}}
  await api('/proxy',{mode:'system'});
  const draft=projectRequest({name:'QA · real Landsat 8/9 five-layer originals',bounds:body.bbox,scenes:items.map(item=>normalizeScene(item,'planetary-landsat'))});
  const project=await api('/projects',draft);assert.deepEqual(project.scenes,draft.scenes);report.project=project;
  for(const key of keys.slice(0,3)){
    const pending=await api(`/projects/${project.id}/downloads`,{assetKey:key});assert.equal(pending.jobs.length,3);
    for(const initial of pending.jobs){let job;try{job=await wait(initial);}catch(e){const failed=await api('/jobs/'+initial.id);assert.equal(failed.status,'failed',e.message);report.downloadFailures.push(failed);await persist();job=await wait((await api(`/projects/${project.id}/downloads`,{assetKey:key,itemIds:[initial.itemId]})).jobs[0]);}
      assert.equal(await digest(job.outputPath),job.sha256);report.originals.push(job);await persist();console.log(JSON.stringify({itemId:job.itemId,key,bytes:job.bytesDownloaded,sha256:job.sha256}));}
  }
  const all=await api('/jobs');report.originals=ids.flatMap(itemId=>keys.map(key=>{const found=all.find(j=>j.kind==='download'&&j.status==='succeeded'&&j.itemId===itemId&&j.assetKey===key);assert(found);return found;}));
  for(const j of report.originals)assert.equal(await digest(j.outputPath),j.sha256);
  for(const j of reused){assert.equal(await digest(j.sourcePath),j.sha256);assert.equal((await stat(j.sourcePath)).mtimeMs,j.sourceMtimeMs);}
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});report.status='passed';await persist();
}catch(error){report.status='failed';report.failure=error.message;await persist();throw error;}
finally{if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}await writeFile(path.join(root,'runtime.stderr.log'),errors);}
