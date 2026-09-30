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
