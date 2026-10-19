import test from 'node:test';
import assert from 'node:assert/strict';
import { sceneSuggestions } from './agent-scenes.js';

test('product exploration asks about supported files even before its area form is complete',()=>{
  for(const id of ['chirps','nasadem','alos-aw3d30','srtm-gl1','hydrosheds']) {
    const prompts=sceneSuggestions({kind:'workspace',page:'Explore'},id);
    assert(prompts.some(text=>text.includes('form')));
    assert(prompts.every(text=>!text.includes('imagery scenes')));
  }
  assert(sceneSuggestions({page:'Explore'}).some(text=>text.includes('imagery scenes')));
  assert.deepEqual(sceneSuggestions({kind:'conversation',page:'Home'},'chirps'),sceneSuggestions({kind:'conversation',page:'Home'}));
});
