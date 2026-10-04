// Complete public NAIP originals and native four-channel area outputs.
// Private verification storage only; never opens or controls the user's desktop.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {copyFile,mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {projectCatalogScenes} from '../prototype/src/project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4608);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('naip-resolutions-'));
assert(Number.isInteger(port)&&port>1024&&port<65536);
await mkdir(root,{recursive:true});
await new Promise((resolve,reject)=>{
  const check=net.createServer();check.once('error',reject);
  check.listen(port,'127.0.0.1',()=>check.close(resolve));
});
async function fileSha256(file){const digest=createHash('sha256');for await(const part of createReadStream(file))digest.update(part);return digest.digest('hex');}
const buildExe=path.join(workspace,'.verification/naip-native-target/debug/geod-runtime.exe');
const nativeBinarySha256=await fileSha256(buildExe),exe=path.join(root,`runtime-${nativeBinarySha256.slice(0,16)}.exe`);
// Keep the Cargo output unlocked for further tests while this exact build runs.
await copyFile(buildExe,exe);assert.equal(await fileSha256(exe),nativeBinarySha256);
const base=`http://127.0.0.1:${port}`;
const captured=JSON.parse(await readFile(path.join(workspace,'prototype/qa/naip-resolution-catalog.json'),'utf8'));
const neighbors=JSON.parse(await readFile(path.join(root,'neighbor-catalog-1.json'),'utf8'));
function item(id,features){const matches=features.filter(feature=>feature.id===id);assert.equal(matches.length,1);return matches[0];}
const groups=[
  {name:'1m',spacing:1,bounds:[-80.535,27.994,-80.525,28.006],scenes:[
    item('fl_m_2808060_se_17_1_20171211_20180201',captured.features),
    item('fl_m_2708004_ne_17_1_20171211_20180201',neighbors.features),
  ].map(feature=>normalizeScene(feature,'planetary-naip'))},
  {name:'0.3m',spacing:0.3,bounds:[-69.162,45.027,-69.158,45.03],scenes:[
    item('me_m_4506963_se_19_030_20231115_20240103',captured.features),
  ].map(feature=>normalizeScene(feature,'planetary-naip'))},
];
const report={schema:'geod-naip-resolution-native/v1',checkedAt:new Date().toISOString(),qaOnly:true,
  status:'in_progress',nativeBinarySha256,catalogs:[{file:'prototype/qa/naip-resolution-catalog.json',sha256:await fileSha256('prototype/qa/naip-resolution-catalog.json')},{file:'neighbor-catalog-1.json',sha256:await fileSha256(path.join(root,'neighbor-catalog-1.json'))}],
  cases:[],originals:[],outputs:[],downloadFailures:[],originalFileIndependentAcceptance:false};
async function save(){await writeFile(path.join(root,'native-resolution-verification.json'),JSON.stringify(report,null,2)+'\n');}
let runtime,stderr='';
const alive=()=>runtime.exitCode===null&&runtime.signalCode===null;
function start(){
  runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  runtime.stdout.on('data',()=>{});runtime.stderr.on('data',part=>{stderr+=part;});
}
async function stop(){
  if(alive()){const ended=new Promise(resolve=>runtime.once('exit',resolve));runtime.kill();await ended;}
}
async function api(route,body){
  const response=await fetch(base+route,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(120000)});
  const result=await response.json();assert(response.ok,JSON.stringify(result));return result;
}
async function ready(){
  for(let attempt=0;attempt<100;attempt++){
    try{const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));assert.equal(health.maxAerialAssetBytes,4294967296);return;}
    catch(error){assert(alive(),stderr);if(attempt===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}
  }
}
async function wait(initial){
  let lastPrint=0,retries=0;
  while(true){
    const job=await api('/jobs/'+initial.id);
    if(job.status==='succeeded'&&job.settled)return job;
    if(job.status==='failed'&&job.settled&&retries<3&&/request|HTTP (429|500|502|503|504)|timeout|timed|connect|response|stream|transport/i.test(job.error||'')){
      report.downloadFailures.push(job);await save();retries++;
      await new Promise(resolve=>setTimeout(resolve,3000));assert.equal((await api('/jobs/'+job.id+'/retry',{})).id,job.id);continue;
    }
    assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(Date.now()-lastPrint>=30000){console.log(JSON.stringify({stage:'waiting',group:job.itemId,itemId:job.itemId,jobId:job.id,bytes:job.bytesDownloaded,total:job.totalBytes,status:job.status}));lastPrint=Date.now();}
    assert(alive(),stderr);await new Promise(resolve=>setTimeout(resolve,1000));
  }
}
async function inspect(initial,spacing){
  const job=await wait(initial);assert.equal(await fileSha256(job.outputPath),job.sha256);
  const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`);
  assert.equal(metadata.bandCount,4);assert.deepEqual(metadata.pixelSize,[spacing,spacing]);
  const pixels=[];
  for(const [column,row]of[[0,0],[511,511],[512,512],[Math.floor(metadata.width/2),Math.floor(metadata.height/2)],[metadata.width-1,metadata.height-1]]){
    if(column>=metadata.width||row>=metadata.height)continue;
    const coordinate=[metadata.bounds[0]+(column+.5)*spacing,metadata.bounds[3]-(row+.5)*spacing];
    pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));
  }
  return{job,metadata,thumbnail,pixels};
}
async function cacheSnapshot(){
  const directory=path.join(root,'cache/thumbnails/v1'),result={};
  for(const file of(await readdir(directory)).filter(file=>file.endsWith('.json'))){
    const content=await readFile(path.join(directory,file));result[file]=createHash('sha256').update(content).digest('hex');
  }
  return result;
}
start();
try{
  await ready();await api('/proxy',{mode:'system'});
  await writeFile(path.join(root,'native-run.json'),JSON.stringify({pid:runtime.pid,port,dataDir:root,exe,binarySha256:nativeBinarySha256,startedAt:new Date().toISOString()},null,2));
  const existing=await api('/projects'),records=await api('/jobs');
  for(const group of groups){
    const[west,south,east,north]=group.bounds,dx=east-west,dy=north-south;
    const geometry={type:'Polygon',coordinates:[
      [[west,south],[east,south],[east,north],[west,north],[west,south]],
      [[west+dx*.4,south+dy*.4],[west+dx*.4,south+dy*.6],[west+dx*.6,south+dy*.6],[west+dx*.6,south+dy*.4],[west+dx*.4,south+dy*.4]],
    ]};
    group.cases=[];
    const variants=[['single',[group.scenes[0]],undefined],...(group.scenes.length>1?[['mosaic',group.scenes,undefined]]:[]),['polygon',group.scenes,geometry]];
    for(const[kind,scenes,shape]of variants){
      const draft=projectRequest({name:`QA · NAIP ${group.name} · ${kind}`,bounds:group.bounds,geometry:shape,scenes});
      const project=existing.find(candidate=>candidate.name===draft.name)||await api('/projects',draft);
      assert.deepEqual(project.scenes,draft.scenes);assert.deepEqual(project.geometry??null,draft.geometry);
      const restored=projectCatalogScenes(project);assert(restored.every(scene=>scene.gsd===group.spacing));
      assert.deepEqual(restored.map(scene=>scene.id),draft.scenes.map(scene=>scene.itemId));
      group.cases.push({group:group.name,kind,project});report.cases.push(group.cases.at(-1));
    }
    group.pending=[];
    const project=group.cases.find(entry=>entry.kind==='mosaic')?.project||group.cases[0].project;
    for(const scene of project.scenes){
      const previous=records.filter(job=>job.kind==='download'&&job.assetKey==='aerial'&&job.itemId===scene.itemId&&job.href===scene.assets.aerial.href&&job.status!=='cancelled').sort((a,b)=>b.createdAt.localeCompare(a.createdAt))[0];
      if(previous)group.pending.push(['failed','interrupted'].includes(previous.status)?await api('/jobs/'+previous.id+'/retry',{}):previous);
      else{const queued=await api(`/projects/${project.id}/downloads`,{assetKey:'aerial',itemIds:[scene.itemId]});assert.equal(queued.jobs.length,1);group.pending.push(queued.jobs[0]);}
    }
  }
  await save();
  for(const group of groups){
    for(const initial of group.pending){
      const result=await inspect(initial,group.spacing);report.originals.push({group:group.name,...result});await save();
      console.log(JSON.stringify({stage:'original',group:group.name,itemId:result.job.itemId,bytes:result.job.bytesDownloaded,sha256:result.job.sha256,sourceExtraSample:result.metadata.aerial.sourceExtraSample??0}));
    }
    for(const entry of group.cases){
      const related=await api('/jobs');
      const previous=related.find(job=>job.kind==='raster_mosaic'&&job.mosaic?.projectId===entry.project.id&&job.assetKey==='aerial'&&job.status==='succeeded');
      const result=await inspect(previous||await api(`/projects/${entry.project.id}/mosaics`,{assetKey:'aerial'}),group.spacing);
      assert.equal(result.metadata.aerial.coverageMask,'internal-1bit');assert.equal(result.metadata.aerial.sourceExtraSample,undefined);
      report.outputs.push({group:group.name,case:entry.kind,...result});await save();
      console.log(JSON.stringify({stage:'output',group:group.name,case:entry.kind,jobId:result.job.id,width:result.metadata.width,height:result.metadata.height,covered:result.job.mosaicOutput.coveredPixels,masked:result.job.mosaicOutput.maskedPixels}));
    }
  }
  const entries=[...report.originals,...report.outputs],before=await cacheSnapshot();
  assert.equal(Object.keys(before).length,entries.length);
  assert(!(await api('/jobs')).some(job=>['running','queued'].includes(job.status)));
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});await stop();start();await ready();
  const restoredProjects=await api('/projects'),restoredJobs=await api('/jobs');
  for(const entry of report.cases)assert.deepEqual(restoredProjects.find(project=>project.id===entry.project.id),entry.project);
  const hits=[];
  for(const entry of entries){
    assert.equal(restoredJobs.find(job=>job.id===entry.job.id)?.sha256,entry.job.sha256);
    const started=performance.now(),thumbnail=await api(`/jobs/${entry.job.id}/thumbnail`);
    assert.deepEqual(thumbnail,entry.thumbnail);hits.push({jobId:entry.job.id,milliseconds:performance.now()-started});
  }
  assert.deepEqual(await cacheSnapshot(),before);
  report.restart={projectsRestored:restoredProjects.length,completedFilesRestored:entries.length,networkMode:'unreachable proxy; local reads only',thumbnailEntriesUnchanged:true,hits};
  report.status='passed';report.completedAt=new Date().toISOString();await save();await stop();
  console.log(JSON.stringify({stage:'native-complete',originals:report.originals.length,outputs:report.outputs.length,independentAcceptance:false}));
}catch(error){
  report.status='attention_required';report.failure=error.message;await save();
  // Preserve an owned runtime with ongoing transfers rather than cancelling
  // them because an observer or a different validation step failed.
  const jobs=await api('/jobs').catch(()=>[]);
  if(!jobs.some(job=>['queued','running'].includes(job.status)))await stop();
  console.error(JSON.stringify({stage:'attention-required',message:error.message,transfersPreserved:jobs.some(job=>['queued','running'].includes(job.status))&&alive()}));process.exitCode=1;
}finally{await writeFile(path.join(root,'runtime-resolution.stderr.log'),stderr);}
