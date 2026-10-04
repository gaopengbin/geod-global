// Inspect settled originals while other polarization downloads keep running.
// Retain a separate snapshot; never stop the owner or edit its matrix receipt.
import assert from 'node:assert/strict';
import {createReadStream} from 'node:fs';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4605);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('radar-polarizations-'));
const sourceRaw=await readFile(path.join(root,'native-polarizations-verification.json'));
const source=JSON.parse(sourceRaw),exe=path.join(workspace,'target/debug/geod-runtime.exe');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
assert.equal(sha(await readFile(exe)),source.nativeBinarySha256);
async function api(route){
  for(let attempt=0;attempt<120;attempt++){
    const response=await fetch(`http://127.0.0.1:${port}`+route,{headers:{'X-GeoD-Client':'geod-global'},signal:AbortSignal.timeout(120000)});
    const result=await response.json();
    if(response.ok)return result;
    if(!JSON.stringify(result).includes('The raster worker is busy')||attempt===119)assert.fail(JSON.stringify(result));
    await new Promise(resolve=>setTimeout(resolve,500));
  }
}
const health=await api('/health');
assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));
const records=await api('/jobs'),ready=new Map();
for(const entry of source.cases)for(const scene of entry.project.scenes)for(const key of entry.keys){
  const candidates=records.filter(job=>job.kind==='download'&&job.itemId===scene.itemId&&job.assetKey===key
    &&job.href===scene.assets[key]?.href&&job.status==='succeeded');
  candidates.sort((a,b)=>b.createdAt.localeCompare(a.createdAt));
  if(candidates[0]&&!ready.has(candidates[0].id)){
    const current=await api('/jobs/'+candidates[0].id);
    assert.equal(current.status,'succeeded');
    if(current.settled)ready.set(current.id,current);
  }
}
assert(ready.size>0,'No original downloads have settled');
const receipt={...source,status:'partial',checkedAt:new Date().toISOString(),originals:[],outputs:[],
  scope:'Settled actual originals in the active acquisition store; remaining downloads and full-matrix receipt are untouched.',
  sourceNativeReceiptFile:'native-polarizations-verification.json',sourceNativeReceiptSha256:sha(sourceRaw),usedUserDesktop:false};
for(const job of ready.values()){
  const hash=createHash('sha256');for await(const chunk of createReadStream(job.outputPath))hash.update(chunk);
  assert.equal(hash.digest('hex'),job.sha256);
  const existing=source.originals.find(entry=>entry.job.id===job.id&&entry.job.sha256===job.sha256);
  if(existing)receipt.originals.push(existing);
  else{
    const metadata=verifiedMapMetadata(job,await api(`/jobs/${job.id}/raster`)),thumbnail=await api(`/jobs/${job.id}/thumbnail`),pixels=[];
    for(const [col,row]of [[0,0],[Math.floor(metadata.width/2),Math.floor(metadata.height/2)],
      [Math.floor(metadata.width/3),Math.floor(metadata.height/2)],[metadata.width-1,metadata.height-1]]){
      const point=[metadata.bounds[0]+(col+.5)*metadata.pixelSize[0],metadata.bounds[3]-(row+.5)*metadata.pixelSize[1]];
      pixels.push(verifyPixelResult(await api(`/jobs/${job.id}/pixel?x=${point[0]}&y=${point[1]}`),job,metadata,point));
    }
    receipt.originals.push({job,metadata,thumbnail,pixels});
  }
  await writeFile(path.join(root,'early-originals-native-verification.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({stage:'settled-original',key:job.assetKey,itemId:job.itemId,bytes:job.bytesDownloaded,sha256:job.sha256}));
}
