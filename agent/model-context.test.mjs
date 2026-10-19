import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultContextWindow, modelContextPolicy, validContextWindow, MIN_CONTEXT_WINDOW, MAX_CONTEXT_WINDOW } from './model-context.mjs';
import { validateModelConfig, translateResponses } from './protocol.mjs';
import { codexContextConfig } from './codex-host.mjs';
const config = {label:'Context test',protocol:'openai-compatible',baseUrl:'https://api.deepseek.com',model:'deepseek-flash',apiKey:'synthetic-key'};

test('capacity presets match documented official routes, never aliases on a gateway', () => {
  assert.deepEqual(defaultContextWindow(config), {contextWindow:1_048_576,known:true});
  for (const changed of [{baseUrl:'https://gateway.test/v1'},{model:'other-model'},{baseUrl:'https://api.deepseek.com.fake.test'},
    {baseUrl:'https://api.deepseek.com/proxy'},{baseUrl:'http://api.deepseek.com'},{protocol:'anthropic-messages'}]) {
    assert.deepEqual(defaultContextWindow({...config,...changed}), {contextWindow:32_768,known:false});
  }
  assert.equal(modelContextPolicy({...config,baseUrl:'https://gateway.test/v1',contextWindow:131_072}).contextWindow,131_072);
});

test('custom capacity is strictly bounded and compaction leaves room for output', () => {
  for (const size of [null,MIN_CONTEXT_WINDOW,65_536,131_072,1_048_576,MAX_CONTEXT_WINDOW]) {
    assert(validContextWindow(size));
    const checked=validateModelConfig({...config,contextWindow:size});
    const policy=modelContextPolicy(checked);
    assert.equal(policy.contextWindow,size ?? 1_048_576);
    assert(policy.autoCompactTokenLimit + policy.outputTokens < policy.contextWindow);
    assert(policy.autoCompactTokenLimit <= Math.floor(policy.contextWindow * .85));
    const native=codexContextConfig(checked);
    assert(native.includes(`model_context_window = ${policy.contextWindow}\n`));
    assert(native.includes(`model_auto_compact_token_limit = ${policy.autoCompactTokenLimit}\n`));
    assert(native.includes('model_auto_compact_token_limit_scope = "total"'));
    assert(!native.includes(checked.apiKey));
  }
  for (const size of [0,MIN_CONTEXT_WINDOW-1,MAX_CONTEXT_WINDOW+1,1.5,'131072',NaN,Infinity,true,{}]) {
    assert(!validContextWindow(size));
    assert.throws(()=>validateModelConfig({...config,contextWindow:size}),/context window/);
  }
  assert.equal(validateModelConfig(config).contextWindow,null);
});

test('larger context is not still blocked by the previous two-megabyte text cap', () => {
  const body={input:[{role:'user',content:'x'.repeat(2_100_000)}]};
  assert.throws(()=>translateResponses(body,[]),/text request is too large/);
  assert.equal(translateResponses(body,[],null,{...config,contextWindow:1_048_576}).messages[0].content,body.input[0].content);
});
