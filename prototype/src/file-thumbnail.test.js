import test from 'node:test';
import assert from 'node:assert/strict';
import { loadFileThumbnail } from './file-thumbnail.js';
import { validateFileThumbnail } from './runtime-client.js';

test('previews validate file identity, dimensions and PNG payload', () => {
  const valid = { jobId: 'test-file', sha256: 'a'.repeat(64), width: 160, height: 120, dataUrl: 'data:image/png;base64,iVBORw0KGgo=' };
  assert.equal(validateFileThumbnail(valid, 'test-file'), valid);
  for (const change of [{ jobId: 'another-file' }, { width: 161 }, { height: 0 }, { dataUrl: 'https://example.com/scene.jpg' }]) {
    assert.throws(() => validateFileThumbnail({ ...valid, ...change }, 'test-file'), /invalid data/);
  }
});

test('duplicate file previews share work and run serially; changed checksums invalidate cache', async () => {
  let calls = 0, active = 0, max = 0;
  const request = async (_, { id }) => {
    calls++; active++; max = Math.max(max, active);
    await new Promise(resolve => setTimeout(resolve, 5));
    active--;
    return { jobId: id, sha256: 'a'.repeat(64), dataUrl: 'data:image/png;base64,AAAA', width: 1, height: 1 };
  };
  const one = { id: 'cache-one', sha256: 'a'.repeat(64) };
  const two = { id: 'cache-two', sha256: 'a'.repeat(64) };
  const result = await Promise.all([loadFileThumbnail(one, request), loadFileThumbnail(one, request), loadFileThumbnail(two, request)]);
  assert.equal(result[0], result[1]);
  assert.equal(calls, 2);
  assert.equal(max, 1);
  await loadFileThumbnail(one, request);
  assert.equal(calls, 2);
  await assert.rejects(loadFileThumbnail({ ...one, sha256: 'b'.repeat(64) }, request), /does not match/);
});

test('failed preview does not poison the queue and can be retried', async () => {
  const job = { id: 'retry-file', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => { throw new Error('offline'); }), /offline/);
  const data = await loadFileThumbnail(job, async () => ({ jobId: job.id, sha256: job.sha256 }));
  assert.equal(data.jobId, job.id);
});

test('another page owning the preview worker waits without failing all queued cards', async () => {
  const first = { id: 'busy-first', sha256: 'a'.repeat(64) };
  const second = { id: 'busy-second', sha256: 'a'.repeat(64) };
  const order = [];
  let attempts = 0;
  const request = async (_, { id }) => {
    order.push(id);
    if (id === first.id && ++attempts <= 2) throw new Error('Preview worker is busy. Try again shortly.');
    return { jobId: id, sha256: 'a'.repeat(64) };
  };
  const results = await Promise.all([loadFileThumbnail(first, request), loadFileThumbnail(first, request), loadFileThumbnail(second, request)]);
  assert.equal(results[0], results[1]);
  assert.deepEqual(order, [first.id, first.id, first.id, second.id]);
});

test('persistent busy replies have a deadline and do not poison the next file', async context => {
  let reads = 0, calls = 0;
  context.mock.method(Date, 'now', () => ++reads === 1 ? 0 : 60001);
  const job = { id: 'busy-expired', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => { calls++; throw new Error('Preview worker is busy. Try again shortly.'); }), /Preview worker is busy/);
  assert.equal(calls, 1);
  const next = { id: 'after-busy-expired', sha256: 'a'.repeat(64) };
  assert.equal((await loadFileThumbnail(next, async (_, { id }) => ({ jobId: id, sha256: next.sha256 }))).jobId, next.id);
});

test('disconnecting stops busy preview retries before another request', async () => {
  let connected = true, calls = 0;
  const job = { id: 'busy-disconnected', sha256: 'a'.repeat(64) };
  await assert.rejects(loadFileThumbnail(job, async () => {
    calls++; connected = false; throw new Error('Preview worker is busy. Try again shortly.');
  }, () => connected), /task service is offline/);
  assert.equal(calls, 1);
});
