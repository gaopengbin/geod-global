// Real original MODIS COGs through the native project/download/read pipeline.
// Usage: node scripts/verify-modis-quality.mjs .verification/modis-quality-YYYYMMDD PORT
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {projectCatalogScenes} from '../prototype/src/project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4601);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('modis-quality-')&&port>1024&&port<65536);
await mkdir(root,{recursive:true});
const base=`http://127.0.0.1:${port}`,exe=path.join(workspace,'target/debug/geod-runtime.exe');
const report={schema:'geod-modis-quality-native/v1',checkedAt:new Date().toISOString(),qaOnly:true,nativeBinarySha256:createHash('sha256').update(await readFile(exe)).digest('hex'),cases:[]};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let errors='';runtime.stderr.on('data',d=>errors+=d);runtime.stdout.on('data',()=>{});
async function api(url,body){const response=await fetch(base+url,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(60000)});const data=await response.json();assert(response.ok,JSON.stringify(data));return data;}
async function wait(job){const deadline=Date.now()+240000;while(Date.now()<deadline){const next=await api('/jobs/'+job.id);assert(!['failed','interrupted','cancelled'].includes(next.status),JSON.stringify(next));if(next.status==='succeeded'&&next.settled)return next;await new Promise(r=>setTimeout(r,800));}throw new Error('Original COG download did not settle');}
try{
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert(runtime.exitCode===null,errors);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
  const itemURL='https://planetarycomputer.microsoft.com/api/stac/v1/collections/modis-09A1-061/items/MYD09A1.A2025177.h08v05.061.2025189031924';
  const response=await fetch(itemURL,{signal:AbortSignal.timeout(30000)});assert(response.ok);const item=await response.json();
  await writeFile(path.join(root,'source-item.json'),JSON.stringify(item,null,2));
  const scene=normalizeScene(item,'planetary-modis'),request=projectRequest({name:'QA · MODIS original quality and state',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]});
  assert.equal(Object.keys(request.scenes[0].assets).length,5);
  const existing=await api('/projects');const project=existing.find(p=>p.name===request.name)||await api('/projects',request);
  const legacyRequest=structuredClone(request);legacyRequest.name='QA · legacy MODIS RGB with added quality';delete legacyRequest.scenes[0].assets.modis_qc;delete legacyRequest.scenes[0].assets.modis_state;
  const legacy=existing.find(p=>p.name===legacyRequest.name)||await api('/projects',legacyRequest);
  const upgraded=await api(`/projects/${legacy.id}/scenes`,{scenes:request.scenes});
  assert.equal(upgraded.scenes[0].assets.modis_qc.href,request.scenes[0].assets.modis_qc.href);
  for(const key of ['red','green','blue'])assert.deepEqual(upgraded.scenes[0].assets[key],legacy.scenes[0].assets[key]);
  assert.equal(projectCatalogScenes(upgraded)[0].assets.modis_state.href,scene.assets.modis_state.href);
  report.catalog={url:itemURL,itemId:scene.id,period:[scene.date,scene.endDate],assetKeys:Object.keys(request.scenes[0].assets)};report.project=project;report.legacyUpgrade={id:upgraded.id,unchangedRgb:true,assets:upgraded.scenes[0].assets};
  for(const key of ['modis_qc','modis_state']){
    const queued=await api(`/projects/${project.id}/downloads`,{assetKey:key});assert.equal(queued.jobs.length,1);
    const initial=queued.jobs[0];if(['failed','interrupted','cancelled'].includes(initial.status))await api(`/jobs/${initial.id}/retry`,{});
    const job=await wait(initial);
    assert.equal(createHash('sha256').update(await readFile(job.outputPath)).digest('hex'),job.sha256);
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`));
    await writeFile(path.join(root,key+'-preview.png'),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));
    const thumbnail=await api(`/jobs/${job.id}/thumbnail`);await writeFile(path.join(root,key+'-thumbnail.png'),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
    const pixels=[];
    for(let row=0;row<2400;row+=157)for(let col=0;col<2400;col+=173){const coordinate=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));}
    for(const [col,row]of[[2399,2399],[0,2399],[2399,0],[1200,1200]]){const coordinate=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));}
    report.cases.push({key,job,metadata,thumbnail,pixels});
    await writeFile(path.join(root,'native-verification.json'),JSON.stringify(report,null,2));
    console.log(JSON.stringify({key,id:job.id,bytes:job.bytesDownloaded,sha256:job.sha256,validPixels:metadata.quality.validSampleCount,classes:metadata.classes.map(c=>c.count),samples:pixels.length}));
  }
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});report.restartProxy='rejected loopback:9; only cached local rasters can be read';
  report.status='passed';await writeFile(path.join(root,'native-verification.json'),JSON.stringify(report,null,2));
}finally{
  if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}
  await writeFile(path.join(root,'runtime.stderr.log'),errors);
}
