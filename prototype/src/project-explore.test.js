import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeProjectCatalog, projectCatalogScenes, projectExploreSearch } from './project-explore.js';

const project = { bounds: [115, 39, 117, 41], scenes: [{ itemId: 'saved', date: '2024-02-28T00:00:00Z', bbox: [115, 39, 116, 40], cloud: 90, crs: 'EPSG:32650', assets: { visual: { href: 'https://example.com/pinned.tif', mediaType: 'image/tiff' } } }] };
test('project exploration restores its area and full acquisition months, including leap years', () => {
  assert.deepEqual(projectExploreSearch(project, { bbox: '-123,37,-122,38', start: '2026-09-01', end: '2026-09-30', cloudMin: 0, cloud: 60, limit: 100 }), { bbox: '115, 39, 117, 41', start: '2024-02-01', end: '2024-02-29', cloudMin: 0, cloud: 100, limit: 100 });
});
test('restored project scenes remain available outside the catalog and retain pinned links in refreshed results', () => {
  const saved = projectCatalogScenes(project);
  assert.equal(saved[0].id, 'saved');
  assert.equal(saved[0].crs, 'EPSG:32650');
  assert.equal(saved[0].assets.visual.type, 'image/tiff');
  const merged = mergeProjectCatalog({ pages: 2, complete: true, scenes: [{ id: 'saved', geometry: { type: 'Polygon' }, assets: { visual: { href: 'https://example.com/new.tif' }, thumbnail: { href: 'preview.jpg' } } }, { id: 'new' }] }, saved);
  assert.equal(merged.scenes.length, 2);
  assert.equal(merged.scenes[0].assets.visual.href, project.scenes[0].assets.visual.href);
  assert.equal(merged.scenes[0].assets.thumbnail.href, 'preview.jpg');
  assert.equal(merged.scenes[0].geometry.type, 'Polygon');
  assert.equal(merged.complete, true);
  assert.equal(mergeProjectCatalog({ scenes: [{ id: 'new' }] }, saved).scenes[1].id, 'saved');
  const grid = { shape: [10980, 10980], transform: [10, 0, 500000, 0, -10, 4100000] };
  const refreshed = mergeProjectCatalog({ scenes: [{ id: 'saved', grid, assets: { visual: { href: project.scenes[0].assets.visual.href, 'proj:shape': grid.shape } } }] }, saved).scenes[0];
  assert.deepEqual(refreshed.grid, grid);
  assert.deepEqual(refreshed.assets.visual['proj:shape'], grid.shape);
  assert.equal(merged.scenes[0].grid.shape, undefined);
});
