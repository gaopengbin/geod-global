// Real QA originals through the catalogue -> native project -> download -> local read path.
// Writes only a fresh private QA directory. SAS remains inside the native process.
import {spawn,spawnSync} from 'node:child_process';
import {mkdir,readFile,writeFile,copyFile,stat,readdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {projectCatalogScenes} from '../prototype/src/project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4631);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('landsat-quality-'));
assert(port>1024&&port<65536);const resume=process.argv.includes('--resume');let diagnostic;
if(resume){const prior=await readFile(path.join(root,'native-verification.json'));const parsed=JSON.parse(prior);assert.equal(parsed.status,'failed');diagnostic=createHash('sha256').update(prior).digest('hex');await writeFile(path.join(root,`failed-native-${diagnostic.slice(0,16)}.json`),prior);}else await mkdir(root);const sourceExe=path.resolve((process.argv[4]?.startsWith('--')?undefined:process.argv[4])||'.verification/naip-native-target/debug/geod-runtime.exe');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');const binarySha=sha(await readFile(sourceExe));
const exe=path.join(root,`runtime-${binarySha.slice(0,16)}.exe`);if(resume)assert.equal(sha(await readFile(exe)),binarySha);else await copyFile(sourceExe,exe);
const base=`http://127.0.0.1:${port}`,receipt=path.join(root,'native-verification.json');
const report={schema:'geod-landsat-quality-native/v1',qaOnly:true,nativeBinary:exe,nativeBinarySha256:binarySha,checkedAt:new Date().toISOString(),status:'running',...(resume?{resumedFailedReceiptSha256:diagnostic,originalAcquisition:'earlier native downloads in this private QA directory; persisted originals reused'}:{}),cases:[]};
let runtime,stderr='';
async function api(url,body){const r=await fetch(base+url,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(60000)});const data=await r.json();assert(r.ok,JSON.stringify(data));return data;}
async function start(){runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});runtime.stderr.on('data',d=>stderr+=d);runtime.stdout.on('data',()=>{});for(let i=0;i<100;i++){try{await api('/health');return;}catch(e){assert.equal(runtime.exitCode,null,stderr);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}}
async function stop(){if(runtime?.exitCode===null){runtime.kill();await new Promise(r=>runtime.once('exit',r));}}
async function wait(job){for(let i=0;i<600;i++){const j=await api('/jobs/'+job.id);assert(!['failed','interrupted','cancelled'].includes(j.status),JSON.stringify(j));if(j.status==='succeeded'&&j.settled)return j;await new Promise(r=>setTimeout(r,500));}throw Error('Native QA download did not settle.');}
const save=()=>writeFile(receipt,JSON.stringify(report,null,2));
function oracle(mode){const result=spawnSync('python',['-X','utf8','scripts/verify-landsat-quality.py',root,...(mode?['--plan-samples']:[])],{cwd:workspace,windowsHide:true,encoding:'utf8',timeout:120000});assert.equal(result.status,0,result.stderr||result.stdout);if(result.stdout.trim())console.log(result.stdout.trim());}
try {
  await start();await api('/proxy',{mode:'system'});
  const itemURL='https://planetarycomputer.microsoft.com/api/stac/v1/collections/landsat-c2-l2/items/LC09_L2SP_044034_20250628_02_T1';
  const response=await fetch(itemURL,{signal:AbortSignal.timeout(30000)});assert(response.ok);const item=await response.json();await writeFile(path.join(root,'source-item.json'),JSON.stringify(item,null,2));
  const scene=normalizeScene(item,'planetary-landsat');const request=projectRequest({name:'QA · Landsat original quality flags',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]});
  assert.equal(Object.keys(request.scenes[0].assets).length,5);const project=await api('/projects',request);report.project=project;
  const legacy=structuredClone(request);legacy.name='QA · existing Landsat RGB project';delete legacy.scenes[0].assets.qa_pixel;delete legacy.scenes[0].assets.qa_radsat;
  const old=await api('/projects',legacy);const upgraded=await api(`/projects/${old.id}/scenes`,{scenes:request.scenes});
  for(const key of ['red','green','blue'])assert.deepEqual(upgraded.scenes[0].assets[key],old.scenes[0].assets[key]);
  assert.equal(projectCatalogScenes(upgraded)[0].assets.qa_pixel.href,scene.assets.qa_pixel.href);report.legacyUpgrade={id:old.id,unchangedRgb:true,assets:upgraded.scenes[0].assets};
  report.catalog={url:itemURL,itemId:item.id,keys:Object.keys(request.scenes[0].assets)};
  for(const key of ['qa_pixel','qa_radsat']){
    const queued=await api(`/projects/${project.id}/downloads`,{assetKey:key});assert.equal(queued.jobs.length,1);const job=await wait(queued.jobs[0]);
    assert.equal(sha(await readFile(job.outputPath)),job.sha256);assert(!job.href.includes('?'));
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`));
    const thumbnail=await api(`/jobs/${job.id}/thumbnail`);
    await writeFile(path.join(root,key+'-preview.png'),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));
    await writeFile(path.join(root,key+'-thumbnail.png'),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
    report.cases.push({key,job,metadata,thumbnail,pixels:[]});await save();
    console.log(JSON.stringify({key,bytes:job.bytesDownloaded,grid:[metadata.width,metadata.height],samples:metadata.quality.sampleCount,valid:metadata.quality.validSampleCount}));
  }
  oracle(true);const plans=JSON.parse(await readFile(path.join(root,'sample-plan.json'),'utf8'));
  for(const entry of report.cases){for(const target of plans[entry.key]){const p=verifyPixelResult(await api(`/jobs/${entry.job.id}/pixel?x=${target.coordinate[0]}&y=${target.coordinate[1]}`),entry.job,entry.metadata,target.coordinate);assert.equal(p.value,target.raw);entry.pixels.push(p);}}
  const cacheDir=path.join(root,'cache','thumbnails','v1');
  const cacheState=async()=>{const list=[];for(const file of await readdir(cacheDir)){const p=path.join(cacheDir,file),s=await stat(p,{bigint:true});list.push({name:file,sha256:sha(await readFile(p)),created:String(s.birthtimeNs),inode:String(s.ino)});}return list.sort((a,b)=>a.name.localeCompare(b.name));};
  const cached=await cacheState();assert(cached.length>=2);
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});await stop();await start();
  for(const entry of report.cases){assert.deepEqual(verifiedMapMetadata(entry.job,await api(`/jobs/${entry.job.id}/raster`)),entry.metadata);assert.deepEqual(await api(`/jobs/${entry.job.id}/thumbnail`),entry.thumbnail);const p=entry.pixels[0];assert.deepEqual(verifyPixelResult(await api(`/jobs/${entry.job.id}/pixel?x=${p.coordinate[0]}&y=${p.coordinate[1]}`),entry.job,entry.metadata,p.coordinate),p);}
  assert.deepEqual(await cacheState(),cached);report.restart={unreachableProxy:'127.0.0.1:9',originalsLocal:true,metadataAndPixelsUnchanged:true,cacheFilesUnchanged:cached.length};
  const first=report.cases[0],original=await readFile(first.job.outputPath),changed=Buffer.from(original);changed[changed.length-1]^=1;await writeFile(first.job.outputPath,changed);
  try{const r=await fetch(`${base}/jobs/${first.job.id}/thumbnail`);assert(!r.ok);assert((await r.json()).error.includes('SHA-256'));report.changedSourceRejected=true;}finally{await writeFile(first.job.outputPath,original);}
  for(const entry of report.cases)assert.equal(sha(await readFile(entry.job.outputPath)),entry.job.sha256);
  const firstCache=path.join(cacheDir,sha(Buffer.from(['thumbnail/1',first.job.id,first.job.sha256,first.key,'160'].join('\0')))+'.json');
  await writeFile(firstCache,'corrupt QA cache');assert.deepEqual(await api(`/jobs/${first.job.id}/thumbnail`),first.thumbnail);report.corruptCacheRebuilt=true;
  report.status='passed';await save();oracle(false);
}catch(error){report.status='failed';report.error=String(error.message);await save();throw error;}
finally{await stop();await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
