// Real VH/HH/HV originals and native single, multi-scene and polygon outputs.
// All mutable state stays in a private verification store, with no user UI.
import {spawn} from 'node:child_process';
import {createReadStream} from 'node:fs';
import {mkdir,readFile,writeFile,open} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4605);
const checkRecovery=process.argv.includes('--check-recovery');
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('radar-polarizations-'));
await mkdir(root,{recursive:true});
const apiURL='https://planetarycomputer.microsoft.com/api/stac/v1/search';
const groups=[
  {name:'vh',keys:['vh'],searchBounds:[-122.55,37.65,-122.4,37.8],bounds:[-122.5,37.71,-122.48,37.73],
    starts:['S1C_IW_GRDH_1SDV_20250630T140654_','S1A_IW_GRDH_1SDV_20250624T140800_']},
  {name:'hh-hv',keys:['hh','hv'],searchBounds:[-49.6,68.95,-49.3,69.25],bounds:[-49.52,69.13,-49.5,69.15],
    starts:['S1A_IW_GRDH_1SDH_20250712T095223_','S1A_IW_GRDH_1SDH_20250630T095224_']},
];
const exe=path.join(workspace,'target/debug/geod-runtime.exe'),base=`http://127.0.0.1:${port}`;
const report={schema:'geod-radar-polarizations-native/v1',checkedAt:new Date().toISOString(),qaOnly:true,
  nativeBinarySha256:createHash('sha256').update(await readFile(exe)).digest('hex'),catalogs:[],originals:[],outputs:[],cases:[],downloadFailures:[],
  furtherCalibrationApplied:false,speckleFilterApplied:false,terrainCorrectionAccuracyAssessed:false};
async function save(){await writeFile(path.join(root,'native-polarizations-verification.json'),JSON.stringify(report,null,2));}
async function fileSha256(file){const hash=createHash('sha256');for await(const chunk of createReadStream(file))hash.update(chunk);return hash.digest('hex');}
// Verify a ready project without stopping the acquisition owner or replacing
// its evolving full-matrix receipt. Only independently accepted originals count.
async function verifyCompletedProjects(){
  const readyOriginals=process.argv.includes('--use-ready-originals');
  const sourceFile=readyOriginals?'early-originals-native-verification.json':'native-polarizations-verification.json';
  const raw=await readFile(path.join(root,sourceFile));
  const source=JSON.parse(raw),proof=JSON.parse(await readFile(path.join(root,readyOriginals?'early-originals-independent-verification.json':'independent-polarizations-verification.json'),'utf8'));
  assert.equal(source.nativeBinarySha256,report.nativeBinarySha256);assert.equal(proof.nativeBinarySha256,source.nativeBinarySha256);
  if(readyOriginals)assert.equal(proof.nativeReceiptSha256,createHash('sha256').update(raw).digest('hex'));
  const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));
  const accepted=new Map(proof.originals.map(entry=>[entry.jobId,entry.sha256]));
  const originals=source.originals.filter(entry=>accepted.get(entry.job.id)===entry.job.sha256);
  assert(originals.length>0,'No independently accepted originals are ready');
  const projects=await api('/projects'),jobs=await api('/jobs'),targets=[];
  for(const entry of source.cases)for(const key of entry.keys){
    const pins=entry.project.scenes.map(scene=>originals.find(original=>original.job.itemId===scene.itemId&&original.job.assetKey===key&&original.job.href===scene.assets[key]?.href)?.job);
    if(pins.some(pin=>!pin))continue;
    const current=projects.find(project=>project.id===entry.project.id);assert(current,'Missing private verification project');
    for(const field of ['scenes','bounds','geometry'])assert.deepEqual(current[field],entry.project[field]);
    targets.push({entry,key,pins});
  }
  assert(targets.length>0,'No complete project source set is ready');
  for(const original of originals){
    const current=await api('/jobs/'+original.job.id);
    assert(current.status==='succeeded'&&current.settled);assert.equal(current.sha256,original.job.sha256);
    assert.equal(await fileSha256(current.outputPath),current.sha256);
  }
  const subset={...source,status:'partial',checkedAt:new Date().toISOString(),
    scope:'Actual ready project subset in the active acquisition store; owner and full-matrix receipt remain untouched.',
    sourceNativeReceiptFile:sourceFile,sourceNativeReceiptSha256:createHash('sha256').update(raw).digest('hex'),
    originals,outputs:[],cases:source.cases.filter(entry=>targets.some(target=>target.entry===entry))};
  for(const {entry,key,pins}of targets){
    const matching=jobs.find(job=>job.kind==='raster_mosaic'&&job.assetKey===key&&job.mosaic?.projectId===entry.project.id
      &&['queued','running','succeeded'].includes(job.status)&&job.mosaic.sources.length===pins.length
      &&job.mosaic.sources.every(source=>pins.some(pin=>source.jobId===pin.id&&source.sha256===pin.sha256)));
    const initial=matching||await api(`/projects/${entry.project.id}/mosaics`,{assetKey:key});
    let job,lastPrint=0;
    while(true){
      job=await api('/jobs/'+initial.id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
      if(job.status==='succeeded'&&job.settled)break;
      if(Date.now()-lastPrint>30000){console.log(JSON.stringify({stage:'ready-project',id:job.id,key,status:job.status}));lastPrint=Date.now();}
      await new Promise(resolve=>setTimeout(resolve,1000));
    }
    assert.equal(await fileSha256(job.outputPath),job.sha256);
    assert.equal(job.mosaic.projectId,entry.project.id);assert.equal(job.mosaic.sources.length,pins.length);
    assert(job.mosaic.sources.every(source=>pins.some(pin=>source.jobId===pin.id&&source.sha256===pin.sha256)));
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`),pixels=[];
    for(const row of [0,Math.floor(metadata.height/2),metadata.height-1])for(const col of [0,Math.floor(metadata.width/2),metadata.width-1]){
      const point=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
      pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${point[0]}&y=${point[1]}`),job,metadata,point));
    }
    subset.outputs.push({group:entry.group,case:entry.kind,key,job,metadata,thumbnail,pixels});
    await writeFile(path.join(root,'early-projects-native-verification.json'),JSON.stringify(subset,null,2));
    console.log(JSON.stringify({stage:'ready-project-verified',case:entry.kind,key,jobId:job.id,sha256:job.sha256,width:metadata.width,height:metadata.height}));
  }
}
if(process.argv.includes('--completed-projects')){await verifyCompletedProjects();process.exit(0);}
for(const group of groups){
  const body={collections:['sentinel-1-rtc'],bbox:group.searchBounds,datetime:'2025-06-01T00:00:00Z/2025-07-15T23:59:59Z',limit:100};
  const response=await fetch(apiURL,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(60000)});
  assert(response.ok);const catalog=await response.json();assert(!catalog.links?.some(link=>link.rel==='next'));
  const items=group.starts.map(start=>{const matches=catalog.features.filter(item=>item.id.startsWith(start));assert.equal(matches.length,1);return matches[0];});
  assert(items.every(item=>item.properties['proj:epsg']===items[0].properties['proj:epsg']&&group.keys.every(key=>item.assets[key])));
  const file=group.name+'-catalog.json';await writeFile(path.join(root,file),JSON.stringify(catalog,null,2));
  group.scenes=items.map(item=>normalizeScene(item,'planetary-radar'));
  report.catalogs.push({url:apiURL,method:'POST',body,file,returnedItems:catalog.features.length,selectedItemIds:items.map(item=>item.id)});
}
let stderr='',runtime;
function startRuntime(){
  runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  runtime.stderr.on('data',data=>stderr+=data);runtime.stdout.on('data',()=>{});
}
startRuntime();
async function api(route,body){
  for(let attempt=0;attempt<240;attempt++){
    const response=await fetch(base+route,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(120000)});
    const value=await response.json();
    if(response.ok)return value;
    const busy=!body&&/The raster worker is busy|Preview worker is busy/.test(JSON.stringify(value));
    if(!busy||attempt===239)assert.fail(JSON.stringify(value));
    await new Promise(resolve=>setTimeout(resolve,500));
  }
}
async function wait(initial){
  let lastPrint=0;
  // Poll the existing native operation until its authoritative terminal state.
  // An observation deadline must not terminate healthy multi-gigabyte transfers.
  while(true){
    const job=await api('/jobs/'+initial.id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(job.status==='succeeded'&&job.settled)return job;
    if(Date.now()-lastPrint>30000){console.log(JSON.stringify({stage:'waiting',key:job.assetKey,itemId:job.itemId,bytes:job.bytesDownloaded,total:job.totalBytes,status:job.status}));lastPrint=Date.now();}
    assert.equal(runtime.exitCode,null,stderr);await new Promise(resolve=>setTimeout(resolve,1000));
  }
}
async function ready(){
  for(let i=0;i<100;i++){try{await api('/health');return;}catch(error){assert.equal(runtime.exitCode,null,stderr);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
}
async function verifyRecovery(pending){
  const target=pending[0],receiptFile=path.join(root,'assets',target.id+'.part.resume.json');
  // Wait for a checkpoint actually written by the native download, never
  // synthesize one or adopt a partial from the legacy client.
  let pin;
  while(true){
    const job=await api('/jobs/'+target.id);assert(['queued','running'].includes(job.status),JSON.stringify(job));
    try{pin=JSON.parse(await readFile(receiptFile,'utf8'));if(pin.bytes>=1024*1024)break;}catch{}
    await new Promise(resolve=>setTimeout(resolve,500));
  }
  const oldPid=runtime.pid,exited=new Promise(resolve=>runtime.once('exit',resolve));runtime.kill();await exited;
  pin=JSON.parse(await readFile(receiptFile,'utf8'));
  const part=path.join(root,'assets',target.id+'.part'),hash=createHash('sha256');
  for await(const chunk of createReadStream(part,{start:0,end:pin.bytes-1}))hash.update(chunk);
  assert.equal(hash.digest('hex'),pin.sha256);
  startRuntime();await ready();
  assert.equal((await api('/jobs/'+target.id)).status,'interrupted');
  for(const job of pending){const current=await api('/jobs/'+job.id);if(['failed','interrupted'].includes(current.status))assert.equal((await api('/jobs/'+job.id+'/retry',{})).id,job.id);}
  let resumed;
  while(true){
    resumed=await api('/jobs/'+target.id);assert(['queued','running','succeeded'].includes(resumed.status),JSON.stringify(resumed));
    if(resumed.transfer?.mode==='resumed'&&resumed.bytesDownloaded>pin.bytes+32768)break;
    await new Promise(resolve=>setTimeout(resolve,500));
  }
  assert.equal(resumed.transfer.resumedBytes,pin.bytes);assert.equal(resumed.totalBytes,pin.total);
  // Independently fetch the small byte interval crossing the recovery boundary.
  // Official SAS remains in memory and is never included in the receipt.
  const source=new URL(target.href),account=source.hostname.split('.')[0],container=source.pathname.split('/')[1];
  const tokenResponse=await fetch(`https://planetarycomputer.microsoft.com/api/sas/v1/token/${encodeURIComponent(account)}/${encodeURIComponent(container)}`,{signal:AbortSignal.timeout(60000)});assert(tokenResponse.ok);
  const token=await tokenResponse.json();assert.equal(typeof token.token,'string');source.search=token.token;
  const from=pin.bytes-32768,to=pin.bytes+32767;
  const remote=await fetch(source,{headers:{Range:`bytes=${from}-${to}`,'If-Range':pin.etag,'Accept-Encoding':'identity'},signal:AbortSignal.timeout(60000)});
  assert.equal(remote.status,206);assert.equal(remote.headers.get('etag'),pin.etag);assert.equal(remote.headers.get('content-range'),`bytes ${from}-${to}/${pin.total}`);
  const remoteBytes=Buffer.from(await remote.arrayBuffer());assert.equal(remoteBytes.length,65536);
  const file=await open(part,'r'),local=Buffer.alloc(65536);
  try{assert.equal((await file.read(local,0,local.length,from)).bytesRead,local.length);}finally{await file.close();}
  assert.deepEqual(local,remoteBytes);
  report.recovery={schema:'geod-live-range-recovery/v1',checkedAt:new Date().toISOString(),jobId:target.id,
    itemId:target.itemId,key:target.assetKey,originalBytes:pin.total,verifiedPrefixBytes:pin.bytes,
    prefixSha256:pin.sha256,etag:pin.etag,oldPid,newPid:runtime.pid,sameJobId:true,
    crashCheckpointRestored:true,rangeAccepted:true,independentBoundary:{from,to,bytes:local.length,sha256:createHash('sha256').update(local).digest('hex'),exactSourceBytes:true},
    originalFileAccepted:false,status:'recovery_verified_original_still_downloading'};
  await writeFile(path.join(root,'resume-live-verification.json'),JSON.stringify(report.recovery,null,2));await save();
  console.log(JSON.stringify({stage:'verified-recovery',jobId:target.id,bytesResumed:pin.bytes,boundaryBytes:local.length,totalBytes:pin.total}));
}
try{
  await ready();
  await api('/proxy',{mode:'system'});
  const existing=await api('/projects'),records=await api('/jobs'),pending=[];
  for(const group of groups){
    const [west,south,east,north]=group.bounds,dx=east-west,dy=north-south;
    const geometry={type:'Polygon',coordinates:[
      [[west,south],[east,south],[east,north],[west,north],[west,south]],
      [[west+dx*.4,south+dy*.4],[west+dx*.4,south+dy*.6],[west+dx*.6,south+dy*.6],[west+dx*.6,south+dy*.4],[west+dx*.4,south+dy*.4]],
    ]};
    for(const [kind,scenes,shape]of[['single',[group.scenes[0]],undefined],['mosaic',group.scenes,undefined],['polygon',group.scenes,geometry]]){
      const draft=projectRequest({name:`QA · RTC ${group.name.toUpperCase()} · ${kind}`,bounds:group.bounds,scenes,geometry:shape});
      const project=existing.find(project=>project.name===draft.name)||await api('/projects',draft);assert.deepEqual(project.scenes,draft.scenes);
      report.cases.push({group:group.name,kind,keys:group.keys,project});
      if(kind==='mosaic')for(const key of group.keys)for(const scene of project.scenes){
        const previous=records.filter(job=>job.kind==='download'&&job.itemId===scene.itemId&&job.assetKey===key&&job.href===scene.assets[key].href&&job.status!=='cancelled').sort((a,b)=>b.createdAt.localeCompare(a.createdAt))[0];
        if(previous){pending.push(['failed','interrupted'].includes(previous.status)?await api('/jobs/'+previous.id+'/retry',{}):previous);}
        else{const queued=await api(`/projects/${project.id}/downloads`,{assetKey:key,itemIds:[scene.itemId]});assert.equal(queued.jobs.length,1);pending.push(queued.jobs[0]);}
      }
    }
  }
  await save();
  if(checkRecovery)await verifyRecovery(pending);
  for(const initial of pending){
    let job;
    try{job=await wait(initial);}catch(error){
      const failed=await api('/jobs/'+initial.id);assert.equal(failed.status,'failed',error.message);report.downloadFailures.push(failed);await save();
      job=await wait(await api('/jobs/'+initial.id+'/retry',{}));
    }
    assert.equal(await fileSha256(job.outputPath),job.sha256);
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`);
    const pixels=[];
    for(const [col,row]of[[0,0],[Math.floor(metadata.width/2),Math.floor(metadata.height/2)],[Math.floor(metadata.width/3),Math.floor(metadata.height/2)],[metadata.width-1,metadata.height-1]]){
      const point=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
      pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${point[0]}&y=${point[1]}`),job,metadata,point));
    }
    report.originals.push({job,metadata,thumbnail,pixels});await save();
    console.log(JSON.stringify({stage:'original',key:job.assetKey,itemId:job.itemId,bytes:job.bytesDownloaded,sha256:job.sha256}));
  }
  for(const entry of report.cases)for(const key of entry.keys){
    const job=await wait(await api(`/projects/${entry.project.id}/mosaics`,{assetKey:key}));
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`),pixels=[];
    for(const row of[0,Math.floor(metadata.height/2),metadata.height-1])for(const col of[0,Math.floor(metadata.width/2),metadata.width-1]){
      const point=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
      pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${point[0]}&y=${point[1]}`),job,metadata,point));
    }
    report.outputs.push({group:entry.group,case:entry.kind,key,job,metadata,thumbnail,pixels});await save();
    console.log(JSON.stringify({stage:'output',case:entry.kind,key,width:metadata.width,height:metadata.height,covered:job.mosaicOutput.coveredPixels,masked:job.mosaicOutput.maskedPixels}));
  }
  await api('/proxy',{mode:'custom',url:'http://127.0.0.1:9'});report.status='passed';await save();
}finally{
  if(runtime.exitCode===null&&runtime.signalCode===null){
    const jobs=await api('/jobs').catch(()=>null);
    // A failed observer must not terminate other healthy native transfers.
    // Unknown service state also preserves the owner for later inspection.
    if(jobs&&!jobs.some(job=>['queued','running'].includes(job.status))){
      const ended=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await ended;
    }else{
      console.log(JSON.stringify({stage:'owner-retained',pid:runtime.pid,reason:jobs?'native work remains active':'service state could not be inspected'}));
    }
  }
  await writeFile(path.join(root,'runtime-polarizations.stderr.log'),stderr);
}
