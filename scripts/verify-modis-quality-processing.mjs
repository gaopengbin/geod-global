// Three real MODIS composites, all five original COG types and native project outputs.
// Usage: node scripts/verify-modis-quality-processing.mjs .verification/modis-quality-processing-YYYYMMDD PORT
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4603);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('modis-quality-processing-')&&port>1024&&port<65536);
await mkdir(root,{recursive:true});
const keys=['red','green','blue','modis_qc','modis_state'],bounds=[-110.1,34.96,-109.7,35.04];
const catalogURL='https://planetarycomputer.microsoft.com/api/stac/v1/search';
const body={collections:['modis-09A1-061'],bbox:bounds,datetime:'2025-06-18T00:00:00Z/2025-07-03T23:59:59Z',limit:100};
const response=await fetch(catalogURL,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(30000)});
assert(response.ok,`Catalogue status ${response.status}`);const catalog=await response.json();
assert(!catalog.links?.some(l=>l.rel==='next'));
const items=catalog.features.filter(f=>f.id.startsWith('MYD09A1.A2025177.h08v05.')||f.id.startsWith('MYD09A1.A2025177.h09v05.')||f.id.startsWith('MYD09A1.A2025169.h08v05.'));
assert.equal(items.length,3);
await writeFile(path.join(root,'source-catalog.json'),JSON.stringify(catalog,null,2));
const catalogRequest={url:catalogURL,method:'POST',body,retrievedAt:new Date().toISOString()};
await writeFile(path.join(root,'source-catalog-request.json'),JSON.stringify(catalogRequest,null,2));
await writeFile(path.join(root,'source-items.json'),JSON.stringify(items,null,2));
const scenes=items.map(item=>normalizeScene(item,'planetary-modis'));
const base=`http://127.0.0.1:${port}`,exe=path.join(workspace,'target/debug/geod-runtime.exe');
const report={schema:'geod-modis-quality-processing-native/v1',checkedAt:new Date().toISOString(),qaOnly:true,
  nativeBinarySha256:createHash('sha256').update(await readFile(exe)).digest('hex'),catalog:catalogRequest,originals:[],qualityOriginals:[],downloadFailures:[],cases:[],outputs:[],qualityMaskApplied:false};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let errors='';runtime.stderr.on('data',d=>errors+=d);runtime.stdout.on('data',()=>{});
async function api(url,body){
  const response=await fetch(base+url,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(60000)});
  const data=await response.json();assert(response.ok,JSON.stringify(data));return data;
}
async function persist(){await writeFile(path.join(root,'native-processing-verification.json'),JSON.stringify(report,null,2));}
async function wait(job){
  const deadline=Date.now()+480000;
  while(Date.now()<deadline){const next=await api('/jobs/'+job.id);assert(!['failed','interrupted','cancelled'].includes(next.status),JSON.stringify(next));
    if(next.status==='succeeded'&&next.settled)return next;
    assert.equal(runtime.exitCode,null,errors);await new Promise(r=>setTimeout(r,800));}
  throw new Error(`Job ${job.id} did not settle`);
}
try{
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert.equal(runtime.exitCode,null,errors);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
  await api('/proxy',{mode:'system'}); // Only this verifier's isolated storage.
  const requests=[
    {name:'QA · MODIS five layers · single clip',bounds,scenes:[scenes.find(s=>s.id.includes('.h08v05.')&&s.id.includes('A2025177'))]},
    {name:'QA · MODIS five layers · adjacent and overlapping composites',bounds,scenes},
    {name:'QA · MODIS five layers · polygon with hole',bounds,scenes,geometry:{type:'Polygon',coordinates:[
      [[-110.1,34.96],[-109.7,34.96],[-109.7,35.04],[-110.1,35.04],[-110.1,34.96]],
      [[-110.01,34.99],[-110.01,35.01],[-109.99,35.01],[-109.99,34.99],[-110.01,34.99]],
    ]}},
  ];
  const existing=await api('/projects');
  for(const [i,request]of requests.entries()){
    const draft=projectRequest(request);assert(draft.scenes.every(s=>Object.keys(s.assets).length===5));
    const project=existing.find(p=>p.name===draft.name)||await api('/projects',draft);
    assert.deepEqual(project.scenes,draft.scenes);report.cases.push({name:['single','mosaic','polygon'][i],project});
  }
  const project=report.cases[1].project;
  const pending=[];
  for(const key of keys){const queued=await api(`/projects/${project.id}/downloads`,{assetKey:key});assert.equal(queued.jobs.length,3);pending.push(...queued.jobs);}
  await persist();
  for(const initial of pending){
    let job;
    try{job=await wait(initial);}catch(error){
      const failed=await api('/jobs/'+initial.id);assert.equal(failed.status,'failed',error.message);report.downloadFailures.push(failed);await persist();
      const retry=await api(`/projects/${project.id}/downloads`,{assetKey:initial.assetKey,itemIds:[initial.itemId]});assert.equal(retry.jobs.length,1);job=await wait(retry.jobs[0]);
    }
    assert.equal(createHash('sha256').update(await readFile(job.outputPath)).digest('hex'),job.sha256);
    report.originals.push(job);
    if(['modis_qc','modis_state'].includes(job.assetKey)){
      const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`));
      const thumbnail=await api(`/jobs/${job.id}/thumbnail`);
      const previewFile=`original-${job.id}-preview.png`,thumbnailFile=`original-${job.id}-thumbnail.png`;
      await writeFile(path.join(root,previewFile),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));
      await writeFile(path.join(root,thumbnailFile),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
      report.qualityOriginals.push({jobId:job.id,metadata,thumbnail,previewFile,thumbnailFile});
    }
    await persist();
    console.log(JSON.stringify({stage:'original',key:job.assetKey,itemId:job.itemId,bytes:job.bytesDownloaded,sha256:job.sha256}));
  }
  for(const entry of report.cases)for(const key of keys){
    const job=await wait(await api(`/projects/${entry.project.id}/mosaics`,{assetKey:key}));
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`));
    const thumbnail=await api(`/jobs/${job.id}/thumbnail`);
    const previewFile=`${entry.name}-${key}-preview.png`,thumbnailFile=`${entry.name}-${key}-thumbnail.png`;
    await writeFile(path.join(root,previewFile),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));
    await writeFile(path.join(root,thumbnailFile),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
    const pixels=[];
    for(const row of [0,Math.floor(metadata.height/2),metadata.height-1])for(const col of [0,Math.floor(metadata.width/2),metadata.width-1]){
      const coordinate=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
      pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));
    }
    assert.equal(createHash('sha256').update(await readFile(job.outputPath)).digest('hex'),job.sha256);
    report.outputs.push({case:entry.name,key,job,metadata,thumbnail,previewFile,thumbnailFile,pixels});await persist();
    console.log(JSON.stringify({stage:'output',case:entry.name,key,width:metadata.width,height:metadata.height,covered:job.mosaicOutput.coveredPixels,masked:job.mosaicOutput.maskedPixels}));
  }
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});
  report.restartProxy='rejected loopback:9; only local original and derived files can be read';report.status='passed';await persist();
}finally{
  if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}
  await writeFile(path.join(root,'runtime-processing.stderr.log'),errors);
}
