import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultLiveSearch, SAMPLE_BBOX } from './catalog.js';
import { agentMapContext, selectedSearchBounds } from './startup.js';

test('startup has no AOI and sends no invented map context to Agent', () => {
  const input = defaultLiveSearch();
  assert.equal(selectedSearchBounds(input), null);
  for (const page of ['Home','Explore','Tasks']) assert.equal(agentMapContext({page,input}), null);
});
test('manual/project bounds are preserved, while Home uses native conversation defaults', () => {
  const input = {...defaultLiveSearch(),bbox:'13.1, 52.3, 13.8, 52.7',provider:'planetary-computer'};
  assert.deepEqual(selectedSearchBounds(input), [13.1,52.3,13.8,52.7]);
  assert.equal(agentMapContext({page:'Home',input}), null);
  const appliedSearch={...input,bbox:SAMPLE_BBOX};
  const context=agentMapContext({page:'Explore',input,appliedSearch,projectId:'saved-project'});
  assert.deepEqual(context.bounds, SAMPLE_BBOX);
  assert.equal(context.projectId,'saved-project');
  assert.equal(context.provider,'planetary-computer');
});
