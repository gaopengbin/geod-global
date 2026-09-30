import { runtimeRequest } from './runtime-client.js';

// Share previews between project details and the library. Only small PNGs are retained.
const cache = new Map();
const pending = new Map();
let queue = Promise.resolve();

export function fileThumbnailKey(job) { return `${job.id}:${job.sha256}`; }
export function cachedFileThumbnail(job) { return cache.get(fileThumbnailKey(job)); }

export function loadFileThumbnail(job, request = runtimeRequest) {
  const key = fileThumbnailKey(job);
  if (cache.has(key)) return Promise.resolve(cache.get(key));
  if (pending.has(key)) return pending.get(key);
  const task = queue.then(async () => {
    const data = await request('thumbnail', { id: job.id });
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
