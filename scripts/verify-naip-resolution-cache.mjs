// Offline restart of a completed real NAIP subset, without stopping acquisition.
// The snapshot is an explicit local copy of independently accepted files.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createReadStream} from 'node:fs';
import {appendFile,copyFile,mkdir,readFile,readdir,stat,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import net from 'node:net';
import path from 'node:path';

const workspace=process.cwd(),sourceRoot=path.resolve(process.argv[2]),root=path.resolve(process.argv[3]),port=Number(process.argv[4]||4610),group=process.argv[5]||'1m';
for(const directory of[sourceRoot,root]){assert.equal(path.dirname(directory),path.join(workspace,'.verification'));assert(path.basename(directory).startsWith('naip-resolutions-'));}
assert.notEqual(root,sourceRoot);assert.equal(group,'1m');
await new Promise((resolve,reject)=>{const check=net.createServer();check.once('error',reject);check.listen(port,'127.0.0.1',()=>check.close(resolve));});
const source=JSON.parse(await readFile(path.join(sourceRoot,'native-resolution-verification.json'),'utf8'));
const independent=JSON.parse(await readFile(path.join(sourceRoot,'independent-resolution-verification.json'),'utf8'));
assert.equal(source.nativeBinarySha256,independent.nativeBinarySha256);
const accepted=new Map([...independent.originals,...independent.outputs].map(entry=>[entry.jobId,entry.sha256]));
const entries=[...source.originals,...source.outputs].filter(entry=>entry.group===group&&accepted.get(entry.job.id)===entry.job.sha256);
assert.equal(entries.length,5);assert.equal(entries.filter(entry=>entry.job.kind==='download').length,2);
const projects=source.cases.filter(entry=>entry.group===group).map(entry=>entry.project);assert.equal(projects.length,3);
await mkdir(path.join(root,'assets'),{recursive:true});
async function hash(file){const digest=createHash('sha256');for await(const part of createReadStream(file))digest.update(part);return digest.digest('hex');}
const sourceExe=path.join(sourceRoot,`runtime-${source.nativeBinarySha256.slice(0,16)}.exe`),exe=path.join(root,path.basename(sourceExe));
assert.equal(await hash(sourceExe),source.nativeBinarySha256);await copyFile(sourceExe,exe);
const jobs={};
for(const entry of entries){
  assert.equal(entry.job.status,'succeeded');assert.equal(entry.job.settled,true);
  const file=path.join(root,'assets',`${entry.job.id}.tif`);
  assert.equal(await hash(entry.job.outputPath),entry.job.sha256);await copyFile(entry.job.outputPath,file);assert.equal(await hash(file),entry.job.sha256);
  const job={...entry.job,outputPath:path.toNamespacedPath(file)};delete job.settled;
  if(job.manifestPath){
    const metadata=path.join(root,'assets',`${job.id}.metadata.json`);await copyFile(job.manifestPath,metadata);
    job.manifestPath=path.toNamespacedPath(metadata);
  }
  jobs[job.id]=job;
}
await writeFile(path.join(root,'jobs.json'),JSON.stringify(jobs,null,2)+'\n');
await writeFile(path.join(root,'projects.json'),JSON.stringify(Object.fromEntries(projects.map(project=>[project.id,project])),null,2)+'\n');
await writeFile(path.join(root,'proxy-settings.json'),JSON.stringify({mode:'custom',url:'http://127.0.0.1:9'})+'\n');
const base=`http://127.0.0.1:${port}`;
let runtime,stderr='';
const alive=()=>runtime.exitCode===null&&runtime.signalCode===null;
function start(){runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});runtime.stdout.on('data',()=>{});runtime.stderr.on('data',part=>stderr+=part);}
async function stop(){if(alive()){const closed=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await closed;}}
async function api(route){const response=await fetch(base+route,{signal:AbortSignal.timeout(120000)});const result=await response.json();assert(response.ok,JSON.stringify(result));return result;}
async function ready(){
  for(let attempt=0;attempt<100;attempt++){
    try{const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));return;}
    catch(error){assert(alive(),stderr);if(attempt===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}
  }
}
async function restored(){
  assert.deepEqual(await api('/proxy'),{mode:'custom',url:'http://127.0.0.1:9'});
  const current=await api('/projects');assert.equal(current.length,projects.length);
  for(const project of projects)assert.deepEqual(current.find(entry=>entry.id===project.id),project);
  const currentJobs=await api('/jobs');assert.equal(currentJobs.length,entries.length);
  assert(currentJobs.every(job=>job.status==='succeeded'));
  for(const job of currentJobs)assert.equal((await api('/jobs/'+job.id)).settled,true);
}
async function cacheEntries(){
  const directory=path.join(root,'cache/thumbnails/v1'),result=new Map();
  for(const name of(await readdir(directory)).filter(name=>name.endsWith('.json'))){
    const file=path.join(directory,name),data=await readFile(file),entry=JSON.parse(data),identity=await stat(file,{bigint:true});
    if(accepted.has(entry.preview.jobId))result.set(entry.preview.jobId,{file,data,inode:identity.ino,created:identity.birthtimeNs});
  }
  assert.equal(result.size,entries.length);return result;
}
const report={schema:'geod-naip-resolution-subset-cache/v1',checkedAt:new Date().toISOString(),nativeBinarySha256:source.nativeBinarySha256,
  group,source:'Explicit local snapshot of two independently verified real originals and three actual native results. No new remote download is claimed.',
  nativeWindowTested:false,usedUserDesktop:false,networkMode:'unreachable upstream proxy',entries:[],status:'in_progress'};
start();
try{
  await ready();await restored();
  for(const entry of entries){
    assert.deepEqual(await api(`/jobs/${entry.job.id}/raster`),entry.metadata);
    assert.deepEqual(await api(`/jobs/${entry.job.id}/thumbnail`),entry.thumbnail);
  }
  const cache=await cacheEntries(),pinned=entries.find(entry=>entry.case==='polygon'),file=path.join(root,'assets',`${pinned.job.id}.tif`),original=await readFile(file);
  try{
    await appendFile(file,Buffer.from([0]));
    for(const route of['thumbnail','raster'])assert.equal((await fetch(base+`/jobs/${pinned.job.id}/${route}`)).ok,false);
    report.changedDerivedRejected=true;
  }finally{await writeFile(file,original);}
  assert.equal(await hash(file),pinned.job.sha256);assert.deepEqual(await api(`/jobs/${pinned.job.id}/thumbnail`),pinned.thumbnail);
  const damaged=cache.get(entries[0].job.id);await writeFile(damaged.file,'{invalid cache');
  assert.deepEqual(await api(`/jobs/${entries[0].job.id}/thumbnail`),entries[0].thumbnail);
  report.invalidCacheRebuiltFromUnchangedSource=true;
  const before=await cacheEntries();await stop();start();await ready();await restored();
  for(const entry of entries){
    const cached=before.get(entry.job.id),started=performance.now();
    assert.deepEqual(await api(`/jobs/${entry.job.id}/thumbnail`),entry.thumbnail);
    const milliseconds=performance.now()-started,identity=await stat(cached.file,{bigint:true});
    assert.deepEqual(await readFile(cached.file),cached.data);assert.equal(identity.ino,cached.inode);assert.equal(identity.birthtimeNs,cached.created);
    const pixels=[];
    for(const pixel of entry.pixels){const[x,y]=pixel.coordinate;assert.deepEqual(await api(`/jobs/${entry.job.id}/pixel?x=${x}&y=${y}`),pixel);pixels.push(pixel);}
    report.entries.push({jobId:entry.job.id,case:entry.case||'original',sha256:entry.job.sha256,thumbnailIdentical:true,
      cacheBytesFileIdentityCreationUnchanged:true,milliseconds,pixelsCompared:pixels.length});
  }
  report.status='passed';report.projectsRestored=projects.length;report.filesRestored=entries.length;report.completedAt=new Date().toISOString();await stop();
  await writeFile(path.join(sourceRoot,'subset-cache-verification.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({status:report.status,group,projects:projects.length,files:entries.length,cacheEntriesReused:report.entries.length,sourceDownloadsRestarted:false}));
}finally{await stop();await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
