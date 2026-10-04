import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const base='http://127.0.0.1:4368',out='.verification/glo90-public';
const before=JSON.parse(await readFile(`${out}/runtime.json`,'utf8'));
const get=async resource=>{const response=await fetch(base+resource,{signal:AbortSignal.timeout(60000)});assert.ok(response.ok);return response.json();};
const jobs=await get('/jobs');assert.equal(jobs.length,6);assert.ok(jobs.every(job=>job.status==='succeeded'));
const rows=[];
for(const entry of [...before.originals,...before.results]) {
  const job=await get(`/jobs/${entry.job.id}`);assert.equal(job.sha256,entry.job.sha256);assert.equal(job.bytesDownloaded,entry.job.bytesDownloaded);
  const data=await get(`/jobs/${job.id}/raster`);delete data.previewDataUrl;assert.deepEqual(data,entry.metadata);
  const thumb=await get(`/jobs/${job.id}/thumbnail`);assert.equal(thumb.dataUrl,entry.thumbnail.dataUrl);
  const fileHash=createHash('sha256').update(await readFile(job.outputPath)).digest('hex');assert.equal(fileHash,job.sha256);
  let checked=0;
  for(const pixel of entry.pixels || []) {
    const actual=await get(`/jobs/${job.id}/pixel?x=${pixel.coordinate[0]}&y=${pixel.coordinate[1]}`);assert.deepEqual(actual,pixel);checked++;
  }
  rows.push({jobId:job.id,itemId:job.itemId,sha256:job.sha256,bytes:job.bytesDownloaded,product:data.elevation.product,thumbnailIdentical:true,thumbnailSha256:createHash('sha256').update(Buffer.from(thumb.dataUrl.split(',')[1],'base64')).digest('hex'),metadataIdentical:true,recheckedOriginalPixels:checked});
}
const projects=await get('/projects');assert.ok(projects.find(project=>project.id===before.project.id));
await writeFile(`${out}/restart.json`,JSON.stringify({passed:true,checkedAt:new Date().toISOString(),jobs:rows,sourceAndOutputBytesUnchanged:true,projectRestored:true,noNewJobs:true},null,2));
console.log(JSON.stringify({passed:true,persistentThumbnails:rows.length,recheckedOriginalPixels:rows.reduce((sum,row)=>sum+row.recheckedOriginalPixels,0)}));
