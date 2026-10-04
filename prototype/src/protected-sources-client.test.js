import test from 'node:test';
import assert from 'node:assert/strict';
import { runtimeRequest } from './runtime-client.js';

test('public product resolution uses matching native and protected loopback request contracts', async () => {
  const previousWindow = globalThis.window, previousFetch = globalThis.fetch;
  const request = {itemIds:['S2C_MSIL2A_20250627T184941_N0511_R113_T10SEG_20250627T234511']};
  try {
    const calls=[];
    globalThis.window={__TAURI__:{core:{invoke:async(command,payload)=>{calls.push({command,payload});return [];}}}};
    assert.deepEqual(await runtimeRequest('resolveProducts',request),[]);
    assert.deepEqual(calls,[{command:'resolve_copernicus_products',payload:{request}}]);
    delete globalThis.window.__TAURI__;
    globalThis.fetch=async(url,options)=>{
      assert.equal(url,'http://127.0.0.1:4318/providers/copernicus/products');
      assert.equal(options.method,'POST');
      assert.equal(options.headers['X-GeoD-Client'],'geod-global');
      assert.deepEqual(JSON.parse(options.body),request);
      assert.equal(options.headers.Authorization,undefined);
      return {ok:true,json:async()=>[]};
    };
    assert.deepEqual(await runtimeRequest('resolveProducts',request),[]);
  } finally {globalThis.window=previousWindow;globalThis.fetch=previousFetch;}
});
