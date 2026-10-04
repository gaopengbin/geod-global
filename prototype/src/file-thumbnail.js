import { runtimeRequest } from './runtime-client.js';

// Share previews between project details and the library. Only small PNGs are retained.
const cache = new Map();
const pending = new Map();
let queue = Promise.resolve();
const BUSY_PREVIEW = 'Preview worker is busy. Try again shortly.';
const BUSY_WAIT_MS = 60000;

export function fileThumbnailKey(job) { return `${job.id}:${job.sha256}`; }
export function cachedFileThumbnail(job) { return cache.get(fileThumbnailKey(job)); }

export function loadFileThumbnail(job, request = runtimeRequest, canLoad = () => true) {
  const key = fileThumbnailKey(job);
  if (cache.has(key)) return Promise.resolve(cache.get(key));
  if (pending.has(key)) return pending.get(key);
  const task = queue.then(async () => {
    // A previous page/window can still own the single native preview worker.
    // Keep this file in the shared queue instead of failing every queued card.
    const deadline = Date.now() + BUSY_WAIT_MS;
    let data;
    for (;;) {
      if (!canLoad()) throw new Error('The task service is offline.');
      try { data = await request('thumbnail', { id: job.id }); break; }
      catch (error) {
        if (error?.message !== BUSY_PREVIEW || Date.now() >= deadline) throw error;
        await new Promise(resolve => setTimeout(resolve, Math.min(500, deadline - Date.now())));
      }
    }
    if (data.jobId !== job.id || data.sha256 !== job.sha256) throw new Error('The preview does not match this file.');
    cache.set(key, data);
    if (cache.size > 64) cache.delete(cache.keys().next().value);
    return data;
  });
  pending.set(key, task);
  queue = task.catch(() => {});
  task.finally(() => pending.delete(key)).catch(() => {});
  return task;
}
