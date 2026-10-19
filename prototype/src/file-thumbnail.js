import { runtimeRequest } from './runtime-client.js';
import {stacRequest,validateStacInspection} from './stac-client.js';
import {reviewedArchiveSelection,validateOriginalRasterInspection} from './original-backlog-client.js';

// Share bounded native PNGs between project details and the library.
const cache = new Map();
const pending = new Map();
let queue = Promise.resolve();
const BUSY_PREVIEW = 'Preview worker is busy. Try again shortly.';
const BUSY_WAIT_MS = 60000;
const CACHE_MAX_CHARS=8*1024*1024;

export function fileThumbnailKey(job) { return `${job.id}:${job.sha256}${job.stacSource?':'+job.stacSource.snapshotId+':'+job.stacSource.assetKey:''}`; }
export function cachedFileThumbnail(job) { return cache.get(fileThumbnailKey(job)); }

export function loadFileThumbnail(job, request = runtimeRequest, canLoad = () => true,sourceRequest=stacRequest) {
  const key = fileThumbnailKey(job);
  if (cache.has(key)) return Promise.resolve(cache.get(key));
  if (pending.has(key)) return pending.get(key);
  const task = queue.then(async () => {
    // A previous page/window can still own the single native preview worker.
    // Keep this file in the shared queue instead of failing every queued card.
    const deadline = Date.now() + BUSY_WAIT_MS;
    let data,expectedSha=job.sha256;
    if(!canLoad())throw new Error('The task service is offline.');
    if(job.stacSource&&job.mediaType==='application/zip') {
      const source=await sourceRequest('snapshot',{id:job.stacSource.snapshotId});
      const asset=source.assets.find(a=>a.key===job.stacSource.assetKey);
      if(reviewedArchiveSelection(asset)) {
        if(!canLoad())throw new Error('The task service is offline.');
        const original=await sourceRequest('inspect',{id:job.id});
        validateStacInspection(original,job.id);validateOriginalRasterInspection(original,job,source);
        if(!original.previewDataUrl)throw new Error('No display preview is available for this original TIFF member.');
        expectedSha=original.sha256;
        // Use the already bounded native member PNG. Image dimensions stay
        // exact; CSS makes the card small without changing original samples.
        data={jobId:job.id,sha256:original.sha256,width:original.previewWidth,height:original.previewHeight,dataUrl:original.previewDataUrl};
      }
    }
    for (;;) {
      if(data)break;
      if (!canLoad()) throw new Error('The task service is offline.');
      try { data = await request('thumbnail', { id: job.id }); break; }
      catch (error) {
        if (error?.message !== BUSY_PREVIEW || Date.now() >= deadline) throw error;
        await new Promise(resolve => setTimeout(resolve, Math.min(500, deadline - Date.now())));
      }
    }
    if (data.jobId !== job.id || data.sha256 !== expectedSha) throw new Error('The preview does not match this file.');
    cache.set(key, data);
    while(cache.size>64||[...cache.values()].reduce((total,item)=>total+(item.dataUrl?.length||0),0)>CACHE_MAX_CHARS)cache.delete(cache.keys().next().value);
    return data;
  });
  pending.set(key, task);
  queue = task.catch(() => {});
  task.finally(() => pending.delete(key)).catch(() => {});
  return task;
}
