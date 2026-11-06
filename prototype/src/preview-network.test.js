import test from 'node:test';
import assert from 'node:assert/strict';
import {fetchSourceResponse,retryPreviewRequest} from './preview-network.js';
import {failureDiagnostics} from './usage-diagnostics.js';

function policyDocument(t){
  const previous=globalThis.document,target=new EventTarget();let listeners=0;
  target.addEventListener=(...args)=>{listeners++;EventTarget.prototype.addEventListener.call(target,...args);};
  target.removeEventListener=(...args)=>{listeners--;EventTarget.prototype.removeEventListener.call(target,...args);};
  globalThis.document=target;
  t.after(()=>{if(previous===undefined)delete globalThis.document;else globalThis.document=previous;assert.equal(listeners,0);});
  return (blockedURI,effectiveDirective='connect-src',disposition='enforce')=>{
    const event=new Event('securitypolicyviolation');Object.assign(event,{blockedURI,effectiveDirective,disposition});target.dispatchEvent(event);
  };
}

test('enforced source policy rejection is explicit and never retried or logged with its signed URL',async t=>{
  const violation=policyDocument(t);let calls=0;
  await assert.rejects(retryPreviewRequest(()=>fetchSourceResponse('https://source.test/file.tif?sig=secret',{},async()=>{
    calls++;setTimeout(()=>violation('https://source.test'),0);throw new TypeError('Failed to fetch');
  })),error=>{
    assert.equal(failureDiagnostics(error,'preview_metadata').reason,'permission_denied');
    assert.equal(failureDiagnostics(error).httpStatus,'unknown');
    assert.equal(error.retryable,false);assert(!error.message.includes('secret'));return true;
  });
  assert.equal(calls,1);
});

test('other origins, image policy events and report-only policies do not relabel an actual network failure',async t=>{
  const violation=policyDocument(t);
  for(const [url,directive,disposition]of[['https://other.test','connect-src','enforce'],['https://source.test','img-src','enforce'],['https://source.test','connect-src','report']]){
    const network=new TypeError('Network request failed');
    await assert.rejects(fetchSourceResponse('https://source.test/file',{},async()=>{violation(url,directive,disposition);throw network;}),error=>error===network);
  }
});

test('success and cancellation dispose the policy listener without changing response or cancellation',async t=>{
  policyDocument(t);
  const response=new Response('original');
  assert.equal(await fetchSourceResponse('https://source.test/file',{},async()=>response),response);
  const cancelled=new DOMException('Stopped','AbortError');
  await assert.rejects(fetchSourceResponse('https://source.test/file',{},async()=>{throw cancelled;}),error=>error===cancelled);
});
