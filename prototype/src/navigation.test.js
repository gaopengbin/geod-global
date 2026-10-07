import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeNavigationHash } from './navigation.js';

test('navigation preserves direct project, file and library links through canonicalization', () => {
  const id = '933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b';
  for (const hash of [`#Workspace?vector=${id}`, '#My%20Data?view=vectors', `#Workspace?tiles=${id}`, '#My%20Data?view=tiles', `#Workspace?rgb=${id}`, `#Workspace?file=${id}`, `#Explore?project=${id}`, `#My%20Data?project=${id}`, '#My%20Data?view=files'])
    assert.equal(normalizeNavigationHash(hash), hash);
  assert.equal(normalizeNavigationHash(`#Workspace?file=${id}&project=${id}&extra=ignored`), `#Workspace?project=${id}&file=${id}`);
  assert.equal(normalizeNavigationHash(`#Workspace?file=${id}&project=invalid`), `#Workspace?file=${id}`);
  assert.equal(normalizeNavigationHash('#Workspace?file=missing'), '#Workspace');
  assert.equal(normalizeNavigationHash('#Workspace?rgb=invalid'), '#Workspace');
  assert.equal(normalizeNavigationHash(`#Workspace?rgb=${id}&file=${id}`), `#Workspace?rgb=${id}`);
  assert.equal(normalizeNavigationHash('#Recipes'), '#My%20Data');
  assert.equal(normalizeNavigationHash('#%invalid'), '#Home');
});
test('fresh and invalid startup links open the AI homepage, while explicit exploration remains available', () => {
  for (const hash of ['', '#', '#Home', '#unknown', '#Home?bounds=sample&token=private']) assert.equal(normalizeNavigationHash(hash), '#Home');
  assert.equal(normalizeNavigationHash('#Explore'), '#Explore');
});
test('retired 3D library links fall back to My Data without reopening a 3D screen', () => {
  assert.equal(normalizeNavigationHash('#My%20Data?view=3d'), '#My%20Data');
});
test('provider authorization links retain only reviewed account targets', () => {
  assert.equal(normalizeNavigationHash('#Settings?account=nasa-earthdata'), '#Settings?account=nasa-earthdata');
  assert.equal(normalizeNavigationHash('#Settings?account=copernicus&token=private'), '#Settings?account=copernicus');
  assert.equal(normalizeNavigationHash('#Settings?account=unreviewed'), '#Settings');
});
test('Agent task links keep a validated job while removing unrelated or unsafe query fields', () => {
  const id = '933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b';
  assert.equal(normalizeNavigationHash(`#Tasks?job=${id}&token=private`), `#Tasks?job=${id}`);
  assert.equal(normalizeNavigationHash(`#Tasks?job=${id.toUpperCase()}`), `#Tasks?job=${id}`);
  assert.equal(normalizeNavigationHash('#Tasks?job=../../private'), '#Tasks');
});
