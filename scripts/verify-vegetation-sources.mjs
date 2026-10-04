// Actual public originals through the native project/download adapter. No fixture transport.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile,copyFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {normalizeScene,searchURL} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {projectCatalogScenes} from '../prototype/src/project-explore.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const root=path.resolve(process.argv[2]),exe=path.resolve(process.argv[3]),port=Number(process.argv[4]||4633),base=`http://127.0.0.1:${port}`;
assert.equal(path.dirname(root),path.resolve('.verification'));assert(path.basename(root).startsWith('modis-vegetation-'));
await mkdir(root,{recursive:false});
const binarySha256=createHash('sha256').update(await readFile(exe)).digest('hex'),frozen=path.join(root,`runtime-${binarySha256.slice(0,16)}.exe`);await copyFile(exe,frozen);
const report={schema:'geod-modis-vegetation-sources/v1',checkedAt:new Date().toISOString(),nativeBinarySha256:binarySha256,cases:[],nativeWindowTested:false};
const runtime=spawn(frozen,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';runtime.stderr.on('data',d=>stderr+=d);runtime.stdout.on('data',()=>{});
async function api(url,body){const r=await fetch(base+url,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(60000)});const d=await r.json();assert(r.ok,JSON.stringify(d));return d;}
async function wait(job){const end=Date.now()+480000;while(Date.now()<end){const j=await api('/jobs/'+job.id);assert(!['failed','interrupted','cancelled'].includes(j.status),JSON.stringify(j));if(j.status==='succeeded'&&j.settled)return j;await new Promise(r=>setTimeout(r,1000));}throw Error('Actual original download did not settle.');}
try{
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert(runtime.exitCode===null,stderr);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
  const catalog=JSON.parse(await readFile('.verification/modis-vegetation-source-20261004/catalog.json','utf8'));
  const normalized=catalog.features.map(item=>normalizeScene(item,'planetary-vegetation'));assert.equal(normalized.length,6);
  const selected=[catalog.features.find(f=>f.id.startsWith('MOD13Q1.A2025177.')),catalog.features.find(f=>f.id.startsWith('MYD13Q1.A2025169.'))];assert(selected.every(Boolean));
  const url=searchURL({provider:'planetary-vegetation',bbox:[-116.2,37.6,-113,38.1],start:'2025-06-26',end:'2025-06-26',limit:20});
  const response=await fetch(url,{signal:AbortSignal.timeout(30000)});assert(response.ok);const neighbor=await response.json();await writeFile(path.join(root,'neighbor-catalog.json'),JSON.stringify(neighbor,null,2));
  const east=neighbor.features.find(f=>f.id.startsWith('MOD13Q1.A2025177.h09v05.'));assert(east,'Actual adjacent Terra tile missing');selected.push(east);
  await writeFile(path.join(root,'source-items.json'),JSON.stringify(selected,null,2));report.catalogue={originalSearchCount:normalized.length,neighborUrl:url};
  const scenes=selected.map(item=>normalizeScene(item,'planetary-vegetation'));
  const project=await api('/projects',projectRequest({name:'QA · MODIS Terra / Aqua NDVI + EVI originals',bounds:[-123.0,37.6,-113,38.1],scenes}));
  assert.deepEqual(projectCatalogScenes(project).map(s=>s.id),scenes.map(s=>s.id));report.project=project;
  for(const key of ['ndvi','evi']){
    const queued=await api(`/projects/${project.id}/downloads`,{assetKey:key});assert.equal(queued.jobs.length,3);
    for(const initial of queued.jobs){
      const job=await wait(initial);assert.equal(createHash('sha256').update(await readFile(job.outputPath)).digest('hex'),job.sha256);
      const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`));assert(metadata.vegetation&&!metadata.reflectance);
      const thumbnail=await api(`/jobs/${job.id}/thumbnail`),pixels=[];
      const stem=job.itemId.split('.')[0]+'-'+job.itemId.split('.')[2]+'-'+key;
      await writeFile(path.join(root,stem+'-preview.png'),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));
      await writeFile(path.join(root,stem+'-thumbnail.png'),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
      for(const [col,row]of[[0,0],[4799,4799],[0,4799],[4799,0],[2400,2400],[1234,4321],[2100,700],[3900,3100]]){
        const coordinate=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
        pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));
      }
      report.cases.push({job,metadata,thumbnail,pixels});await writeFile(path.join(root,'source-verification.json'),JSON.stringify(report,null,2));
      console.log(JSON.stringify({item:job.itemId,key,bytes:job.bytesDownloaded,sha256:job.sha256,dimensions:[metadata.width,metadata.height],previewValid:metadata.vegetation.validSampleCount,rawSamples:pixels.length}));
    }
  }
  report.status='passed';await writeFile(path.join(root,'source-verification.json'),JSON.stringify(report,null,2));
}finally{if(runtime.exitCode===null){runtime.kill();await new Promise(r=>runtime.once('exit',r));}await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
