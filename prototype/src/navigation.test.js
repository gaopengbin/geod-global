import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeNavigationHash } from './navigation.js';

test('navigation preserves direct project, file and library links through canonicalization', () => {
  const id = '933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b';
  for (const hash of [`#Workspace?file=${id}`, `#Explore?project=${id}`, `#My%20Data?project=${id}`, '#My%20Data?view=files'])
    assert.equal(normalizeNavigationHash(hash), hash);
  assert.equal(normalizeNavigationHash(`#Workspace?file=${id}&project=${id}&extra=ignored`), `#Workspace?project=${id}&file=${id}`);
  assert.equal(normalizeNavigationHash(`#Workspace?file=${id}&project=invalid`), `#Workspace?file=${id}`);
  assert.equal(normalizeNavigationHash('#Workspace?file=missing'), '#Workspace');
  assert.equal(normalizeNavigationHash('#Recipes'), '#My%20Data');
  assert.equal(normalizeNavigationHash('#%invalid'), '#Explore');
});
