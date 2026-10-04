// Real USGS originals through the managed downloader; isolated project outputs.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4645);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('landsat-quality-processing-')&&port>1024&&port<65536);
await mkdir(root,{recursive:true});
const sha=buffer=>createHash('sha256').update(buffer).digest('hex');
const binary=JSON.parse(await readFile(path.join(root,'binary.json'),'utf8'));assert.equal(sha(await readFile(binary.path)),binary.sha256);
const keys=['qa_pixel','qa_radsat'],bounds=[-123.6,37.6,-122.2,38.1];
const catalogURL='https://planetarycomputer.microsoft.com/api/stac/v1/search';
const body={collections:['landsat-c2-l2'],bbox:bounds,datetime:'2025-06-01T00:00:00Z/2025-06-29T23:59:59Z',limit:100};
const r=await fetch(catalogURL,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(30000)});
assert(r.ok,`Catalogue ${r.status}`);const catalog=await r.json();assert(!catalog.links?.some(l=>l.rel==='next'));
const expected=['LC09_L2SP_044034_20250612_02_T1','LC08_L2SP_045034_20250627_02_T1','LC09_L2SP_044034_20250628_02_T1'];
const items=expected.map(id=>{const item=catalog.features.find(item=>item.id===id);assert(item,`Missing ${id}`);return item;});
await writeFile(path.join(root,'source-catalog.json'),JSON.stringify(catalog,null,2));
await writeFile(path.join(root,'source-items.json'),JSON.stringify(items,null,2));
const catalogRequest={url:catalogURL,method:'POST',body,retrievedAt:new Date().toISOString()};
await writeFile(path.join(root,'source-catalog-request.json'),JSON.stringify(catalogRequest,null,2));
const scenes=items.map(item=>normalizeScene(item,'planetary-landsat'));
const base=`http://127.0.0.1:${port}`,report={schema:'geod-landsat-quality-processing-native/v1',checkedAt:new Date().toISOString(),qaOnly:true,nativeBinary:binary.path,nativeBinarySha256:binary.sha256,catalog:catalogRequest,reusedOriginals:JSON.parse(await readFile(path.join(root,'original-reuse.json'),'utf8')),originals:[],downloadFailures:[],cases:[],outputs:[],rgbScreeningApplied:false};
const runtime=spawn(binary.path,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let errors='';runtime.stderr.on('data',d=>errors+=d);runtime.stdout.on('data',()=>{});
async function api(url,body){const response=await fetch(base+url,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(180000)});const data=await response.json();assert(response.ok,JSON.stringify(data));return data;}
async function persist(){await writeFile(path.join(root,'native-processing-verification.json'),JSON.stringify(report,null,2));}
async function wait(job){for(let start=Date.now();Date.now()-start<480000;){const next=await api('/jobs/'+job.id);assert(!['failed','interrupted','cancelled'].includes(next.status),JSON.stringify(next));if(next.status==='succeeded'&&next.settled)return next;assert.equal(runtime.exitCode,null,errors);await new Promise(r=>setTimeout(r,500));}throw new Error(`Unsettled ${job.id}`);}
try{
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert.equal(runtime.exitCode,null,errors);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
  await api('/proxy',{mode:'system'});
  const requests=[
    {name:'QA · Landsat flags · single clip',bounds,scenes:[scenes[2]]},
    {name:'QA · Landsat flags · adjacent and overlapping scenes',bounds,scenes},
    {name:'QA · Landsat flags · polygon with hole',bounds,scenes,geometry:{type:'Polygon',coordinates:[
      [[-123.6,37.6],[-122.2,37.6],[-122.2,38.1],[-123.6,38.1],[-123.6,37.6]],
      [[-123.1,37.76],[-123.1,37.88],[-122.93,37.88],[-122.93,37.76],[-123.1,37.76]],
    ]}},
    {name:'QA · Landsat flags · over eight million pixels',bounds:[-123.6,37.5,-122.2,38.15],scenes},
  ];
  const existing=await api('/projects');
  for(const [i,request]of requests.entries()){
    const draft=projectRequest(request);assert(draft.scenes.every(s=>Object.keys(s.assets).length===5));
    const p=existing.find(p=>p.name===draft.name)||await api('/projects',draft);assert.deepEqual(p.scenes,draft.scenes);
    report.cases.push({name:['single','mosaic','polygon','large'][i],project:p});
  }
  const p=report.cases[1].project;
  // The reused pair belongs to the newest item. Missing pixel quality must reject
  // saturation before queuing a task, while preserving every existing download.
  const before=await api('/jobs');
  if(!before.some(j=>j.assetKey==='qa_pixel'&&j.itemId===expected[0]&&j.status==='succeeded')){
    const bad=await fetch(`${base}/projects/${p.id}/mosaics`,{method:'POST',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:JSON.stringify({assetKey:'qa_radsat'})});assert(!bad.ok);report.missingSourcesRejected=await bad.json();assert.equal((await api('/jobs')).length,before.length);
  }
  const pending=[];
  for(const key of keys){const queued=await api(`/projects/${p.id}/downloads`,{assetKey:key});assert.equal(queued.jobs.length,3);pending.push(...queued.jobs);}
  await persist();
  for(const initial of pending){
    let job;try{job=await wait(initial);}catch(error){const failed=await api('/jobs/'+initial.id);assert.equal(failed.status,'failed',error.message);report.downloadFailures.push(failed);await persist();const retry=await api(`/projects/${p.id}/downloads`,{assetKey:initial.assetKey,itemIds:[initial.itemId]});assert.equal(retry.jobs.length,1);job=await wait(retry.jobs[0]);}
    assert.equal(sha(await readFile(job.outputPath)),job.sha256);report.originals.push(job);await persist();
    console.log(JSON.stringify({stage:'original',key:job.assetKey,itemId:job.itemId,bytes:job.bytesDownloaded,sha256:job.sha256}));
  }
  for(const entry of report.cases)for(const key of keys){
    const previous=(await api('/jobs')).find(job=>job.kind==='raster_mosaic'&&job.mosaic?.projectId===entry.project.id&&job.assetKey===key&&job.status==='succeeded');
    const job=await wait(previous||await api(`/projects/${entry.project.id}/mosaics`,{assetKey:key}));
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`);
    const previewFile=`${entry.name}-${key}-preview.png`,thumbnailFile=`${entry.name}-${key}-thumbnail.png`;
    await writeFile(path.join(root,previewFile),Buffer.from(metadata.previewDataUrl.split(',')[1],'base64'));await writeFile(path.join(root,thumbnailFile),Buffer.from(thumbnail.dataUrl.split(',')[1],'base64'));
    const pixels=[];
    for(const row of [0,Math.floor(metadata.height/2),metadata.height-1])for(const col of [0,Math.floor(metadata.width/2),metadata.width-1]){const coordinate=[metadata.bounds[0]+(col+.5)*30,metadata.bounds[3]-(row+.5)*30];pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`),job,metadata,coordinate));}
    assert.equal(sha(await readFile(job.outputPath)),job.sha256);
    report.outputs.push({case:entry.name,key,job,metadata,thumbnail,previewFile,thumbnailFile,pixels});await persist();
    console.log(JSON.stringify({stage:'output',case:entry.name,key,width:metadata.width,height:metadata.height,covered:job.mosaicOutput.coveredPixels,masked:job.mosaicOutput.maskedPixels}));
  }
  const coverageDraft=projectRequest({name:'QA · Landsat flags · missing coverage control',bounds,scenes:[{...scenes[0],assets:{qa_radsat:scenes[0].assets.qa_radsat}}]});
  const coverageProject=(await api('/projects')).find(p=>p.name===coverageDraft.name)||await api('/projects',coverageDraft);
  const countBefore=(await api('/jobs')).length;
  const coverageReply=await fetch(`${base}/projects/${coverageProject.id}/mosaics`,{method:'POST',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:JSON.stringify({assetKey:'qa_radsat'})});
  assert(!coverageReply.ok);const message=await coverageReply.json();assert(/pixel.quality|QA_PIXEL/i.test(JSON.stringify(message)));
  const countAfter=(await api('/jobs')).length;assert.equal(countBefore,countAfter);report.missingCoverageControl={projectId:coverageProject.id,status:coverageReply.status,message,countBefore,countAfter};
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});report.restartProxy='unreachable loopback:9; subsequent acceptance uses only local files';report.status='passed';await persist();
}catch(error){report.status='failed';report.failure=error.message;await persist();throw error;}
finally{if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}await writeFile(path.join(root,'runtime-processing.stderr.log'),errors);}
