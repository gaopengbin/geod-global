import test from 'node:test';
import assert from 'node:assert/strict';
import { createProjectAndQueue, jobsForProject, MAX_PROJECT_SCENES, projectRequest } from './projects-client.js';

const source = 'https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2C_TEST/';
const scene = (id = 'S2C_TEST') => ({
  id, date: '2025-06-27T00:00:00Z', cloud: 5, crs: 'EPSG:32610',
  bbox: [-123, 37, -122, 38], properties: { 's2:mgrs_tile': '10SEG' },
  assets: {
    scl: { href: `${source}SCL.tif`, type: 'image/tiff; application=geotiff' },
    visual: { href: `${source}TCI.tif`, type: 'image/tiff; application=geotiff' },
    thumbnail: { href: `${source}preview.jpg`, type: 'image/jpeg' },
  },
});

test('project request preserves chosen scene provenance, both source types and polygon', () => {
  const geometry = { type: 'Polygon', coordinates: [[[-123, 37], [-122, 37], [-122, 38], [-123, 37]]] };
  const result = projectRequest({ scenes: [scene()], bounds: [-123, 37, -122, 38], geometry, name: ' Bay study ' });
  assert.equal(result.name, 'Bay study');
  assert.deepEqual(result.geometry, geometry);
  assert.deepEqual(Object.keys(result.scenes[0].assets), ['scl', 'visual']);
  assert.equal(result.scenes[0].gridCode, '10SEG');
});

test('projects support one source type but reject preview-only and excessive selections', () => {
  const incomplete = scene();
  delete incomplete.assets.visual;
  assert.deepEqual(Object.keys(projectRequest({ scenes: [incomplete], bounds: [-123, 37, -122, 38], name: 'x' }).scenes[0].assets), ['scl']);
  delete incomplete.assets.scl;
  assert.throws(() => projectRequest({ scenes: [incomplete], bounds: [-123, 37, -122, 38], name: 'x' }), /no supported source raster/);
  assert.throws(() => projectRequest({ scenes: Array.from({ length: MAX_PROJECT_SCENES + 1 }, (_, index) => scene(`S2_${index}`)), bounds: [-123, 37, -122, 38], name: 'x' }), /Select 1 to/);
});

test('download saves the named project before queuing both source types', async () => {
  const request = projectRequest({ scenes: [scene()], bounds: [-123, 37, -122, 38], name: 'My project' });
  const calls = [];
  const invoke = async (operation, payload) => {
    calls.push([operation, payload]);
    return operation === 'createProject' ? { id: 'p1', ...payload } : { jobs: [{ id: payload.assetKey }] };
  };
  const result = await createProjectAndQueue(request, ['visual', 'scl'], invoke);
  assert.equal(result.project.name, 'My project');
  assert.equal(result.downloads.length, 2);
  assert.deepEqual(calls, [['createProject', request], ['downloadProject', { id: 'p1', assetKey: 'visual' }], ['downloadProject', { id: 'p1', assetKey: 'scl' }]]);
});

test('partial download failure preserves the saved project and already queued jobs', async () => {
  const invoke = async (operation, payload) => {
    if (operation === 'createProject') return { id: 'saved' };
    if (payload.assetKey === 'scl') throw new Error('Queue unavailable');
    return { jobs: [{ id: 'visual-job' }] };
  };
  await assert.rejects(createProjectAndQueue({}, ['visual', 'scl'], invoke), error => {
    assert.equal(error.project.id, 'saved');
    assert.equal(error.downloads[0].jobs[0].id, 'visual-job');
    return error.message === 'Queue unavailable';
  });
  await assert.rejects(createProjectAndQueue({}, ['thumbnail'], () => { throw new Error('must not create'); }), /Choose SCL/);
});

test('project files include exact sources, mosaics and recursive clips but exclude unrelated files', () => {
  const project = { id: 'p', scenes: [{ itemId: 'scene', assets: { scl: { href: 'source' } } }] };
  const jobs = [
    { id: 'clip2', parentId: 'clip1' },
    { id: 'other', kind: 'download', itemId: 'scene', assetKey: 'scl', href: 'different' },
    { id: 'source', kind: 'download', itemId: 'scene', assetKey: 'scl', href: 'source' },
    { id: 'clip1', recipe: { source: { jobId: 'mosaic' } } },
    { id: 'mosaic', mosaic: { projectId: 'p' } },
    { id: 'other-mosaic', mosaic: { projectId: 'q' } },
    { id: 'outside-project-clip', parentId: 'source' },
  ];
  assert.deepEqual(jobsForProject(project, jobs).map(job => job.id), ['clip2', 'source', 'clip1', 'mosaic']);
});
