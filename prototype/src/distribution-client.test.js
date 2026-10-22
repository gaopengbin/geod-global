import test from 'node:test';
import assert from 'node:assert/strict';
import {distributionRequest} from './distribution-client.js';
import {failureDiagnostics} from './usage-diagnostics.js';

test('native updater signature failures retain a fixed stage and reason without exposing native diagnostics',async()=>{
  const previous=globalThis.window;
  globalThis.window={__TAURI__:{core:{invoke:async()=>{throw '[update_verify|signature_invalid|unknown] Update signature is invalid.';}}}};
  try{
    await assert.rejects(distributionRequest('update_download',{version:'0.2.1'}),error=>{
      assert.equal(error.message,'Update signature is invalid.');
      assert.deepEqual(failureDiagnostics(error,'update_download'),{stage:'update_verify',reason:'signature_invalid',httpStatus:'unknown',retries:'unknown',diagnosticOrigin:'explicit'});
      return true;
    });
  }finally{if(previous===undefined)delete globalThis.window;else globalThis.window=previous;}
});
