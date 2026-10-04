// Run against an isolated geod-runtime --data-dir .verification/glo90-runtime.
// Catalogue files are captured public responses; all files/processing/pixels use the real runtime.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {normalizeScene} from '../prototype/src/catalog.js';
import {projectRequest} from '../prototype/src/projects-client.js';
import {verifiedMapMetadata,verifyPixelResult} from '../prototype/src/workspace-map-geometry.js';
const base=process.argv[2] || 'http://127.0.0.1:4368';
assert.match(base,/^http:\/\/127\.0\.0\.1:\d+$/);
const out=path.resolve('.verification/glo90-public');await mkdir(out,{recursive:true});
async function api(resource,body) {
  const response=await fetch(base+resource,{method:body?'POST':'GET',headers:{'x-geod-client':'geod-global',...(body?{'Content-Type':'application/json'}:{})},...(body?{body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(60000)});
  const data=await response.json();assert.ok(response.ok,JSON.stringify(data));return data;
}
async function complete(id) {
  const end=Date.now()+240000;
  while(Date.now()<end) {
    const job=await api(`/jobs/${id}`);
    if(['succeeded','failed','cancelled','interrupted'].includes(job.status)) {assert.equal(job.status,'succeeded',job.error);return job;}
    await new Promise(resolve=>setTimeout(resolve,700));
  }
  throw new Error('Actual GLO-90 job timed out');
}
const health=await api('/health');
assert.equal((await api('/jobs')).length,0,'Use a fresh isolated verification data directory');
const catalog=JSON.parse(await readFile('prototype/public/samples/cop-dem-90-response.json','utf8'));
assert.equal(catalog.features.length,2);
const scenes=catalog.features.map(item=>normalizeScene(item,'copernicus-dem-90'));
const project=await api('/projects',projectRequest({scenes,bounds:[-.1,51.5,.1,51.6],name:'GLO-90 · 两瓦片原始网格验收'}));
const queued=await api(`/projects/${project.id}/downloads`,{assetKey:'elevation'});
const originals=[];
for(const entry of queued.jobs) {
  const job=await complete(entry.id), data=await api(`/jobs/${job.id}/raster`);
  verifiedMapMetadata(job,data);assert.equal(data.elevation.product,'cop-dem-glo-90');
  assert.deepEqual([data.width,data.height],[800,1200]);
  const pixels=[];
  for(const [col,row] of [[0,0],[799,1199],[400,600],[73,51],[799,599]]) {
    const coordinate=[data.bounds[0]+(col+.5)*data.pixelSize[0],data.bounds[3]-(row+.5)*data.pixelSize[1]];
    const pixel=await api(`/jobs/${job.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`);
    verifyPixelResult(pixel,job,data,coordinate);pixels.push(pixel);
  }
  const thumbnail=await api(`/jobs/${job.id}/thumbnail`);
  originals.push({job,metadata:{...data,previewDataUrl:undefined},pixels,thumbnail});
  console.log(JSON.stringify({stage:'original',itemId:job.itemId,bytes:job.bytesDownloaded,dimensions:[data.width,data.height]}));
}
const lowCatalog=JSON.parse(await readFile('.verification/glo90-low-latitude-catalog.json','utf8'));
assert.equal(lowCatalog.id,'Copernicus_DSM_COG_30_N37_00_W123_00_DEM');
const low=normalizeScene(lowCatalog,'copernicus-dem-90');
const lowJob=await complete((await api('/jobs',{itemId:low.id,assetKey:'elevation',href:low.assets.elevation.href,mediaType:low.assets.elevation.type})).id);
const lowData=await api(`/jobs/${lowJob.id}/raster`);verifiedMapMetadata(lowJob,lowData);assert.deepEqual([lowData.width,lowData.height],[1200,1200]);
const lowPixels=[];
for(const [col,row] of [[0,0],[1199,1199],[600,600],[251,181],[901,805]]) {
  const coordinate=[lowData.bounds[0]+(col+.5)*lowData.pixelSize[0],lowData.bounds[3]-(row+.5)*lowData.pixelSize[1]];
  const pixel=await api(`/jobs/${lowJob.id}/pixel?x=${coordinate[0]}&y=${coordinate[1]}`);verifyPixelResult(pixel,lowJob,lowData,coordinate);lowPixels.push(pixel);
}
const lowThumbnail=await api(`/jobs/${lowJob.id}/thumbnail`);
originals.push({job:lowJob,metadata:{...lowData,previewDataUrl:undefined},pixels:lowPixels,thumbnail:lowThumbnail});
console.log(JSON.stringify({stage:'original',itemId:lowJob.itemId,bytes:lowJob.bytesDownloaded,dimensions:[lowData.width,lowData.height]}));
const results=[];
async function processProject(project,label) {
  const job=await complete((await api(`/projects/${project.id}/mosaics`,{assetKey:'elevation'})).id);
  const data=await api(`/jobs/${job.id}/raster`);verifiedMapMetadata(job,data);
  const thumbnail=await api(`/jobs/${job.id}/thumbnail`);
  results.push({label,project,job,metadata:{...data,previewDataUrl:undefined},thumbnail});
  console.log(JSON.stringify({stage:label,status:job.status,dimensions:[data.width,data.height],covered:job.mosaicOutput.coveredPixels,masked:job.mosaicOutput.maskedPixels}));
}
await processProject(project,'seam-mosaic');
const single=await api('/projects',projectRequest({scenes:[scenes[0]],bounds:[-.08,51.50,-.05,51.53],name:'GLO-90 · 单瓦片裁剪验收'}));
await processProject(single,'single-tile-clip');
const geometry={type:'Polygon',coordinates:[[[-.0837,51.5073],[.0771,51.5112],[.0427,51.5881],[-.0837,51.5073]]]};
const masked=await api('/projects',projectRequest({scenes,bounds:[-.1,51.5,.1,51.6],geometry,name:'GLO-90 · 多边形掩膜验收'}));
await processProject(masked,'polygon-mask');
assert.ok((await api('/jobs')).every(job=>!['queued','running'].includes(job.status)));
await writeFile(path.join(out,'runtime.json'),JSON.stringify({health,project,originals,results},null,2));
console.log(JSON.stringify({passed:true,originals:originals.length,processedOutputs:results.length,receipt:path.join(out,'runtime.json')}));
