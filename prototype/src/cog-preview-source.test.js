import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {loadPreviewRange,CogPreviewSource} from './cog-preview-source.js';
import {failureDiagnostics} from './usage-diagnostics.js';

test('a real range endpoint recovers from a 503 and preserves the exact bytes and Content-Range',async()=>{
  let attempts=0;
  const server=createServer((req,res)=>{
    attempts++;
    assert.equal(req.headers.range,'bytes=0-3');
    if(attempts===1){res.writeHead(503);res.end('unavailable');return;}
    res.writeHead(206,{'Content-Range':'bytes 0-3/10','Content-Length':'4'});res.end(Buffer.from([73,73,42,0]));
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  try {
    const response=await loadPreviewRange(`http://127.0.0.1:${server.address().port}/cog`,{Range:'bytes=0-3'});
    assert.equal(response.status,206);assert.equal(response.headers.get('content-range'),'bytes 0-3/10');
    assert.deepEqual([...new Uint8Array(await response.arrayBuffer())],[73,73,42,0]);assert.equal(attempts,2);
  }finally{server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
});

test('body-read failures also retry, but a permanent 403 does not',async()=>{
  let calls=0;
  const response=await loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async()=>{
    if(++calls===1)return {ok:true,status:206,headers:new Headers(),arrayBuffer:async()=>{throw new TypeError('network body interrupted');}};
    return new Response('abcd',{status:206,headers:{'Content-Range':'bytes 0-3/4'}});
  });
  assert.equal(await response.text(),'abcd');assert.equal(calls,2);
  calls=0;
  await assert.rejects(loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async()=>{calls++;return new Response('',{status:403});}),/HTTP 403/);
  assert.equal(calls,1);
});

test('a range-ignoring 200 is rejected without consuming the whole original',async()=>{
  let consumed=false,cancelled=false;
  await assert.rejects(loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async()=>({ok:true,status:200,
    body:{cancel:async()=>{cancelled=true;}},arrayBuffer:async()=>{consumed=true;}})),/HTTP 200/);
  assert.equal(consumed,false);assert.equal(cancelled,true);
});

test('a wrong byte interval is rejected before geotiff can cache it as a valid tile',async()=>{
  let calls=0;
  await assert.rejects(loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async()=>{
    calls++;return new Response('abcd',{status:206,headers:{'Content-Range':'bytes 4-7/10'}});
  }),/invalid Content-Range/);
  assert.equal(calls,1);
});

test('a stalled range gets a fresh bounded attempt without requiring a CORS-exposed Content-Range',async()=>{
  let calls=0;
  const response=await loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async(_url,{signal})=>{
    if(++calls===1)return new Promise((_resolve,reject)=>signal.addEventListener('abort',()=>reject(signal.reason),{once:true}));
    return new Response('abcd',{status:206});
  },undefined,{totalMs:800,attemptMs:25});
  assert.equal(await response.text(),'abcd');assert.equal(calls,2);
});

test('disposing a source cancels its range read and does not retry cancellation',async()=>{
  const lifetime=new AbortController();let requests=0;
  const pending=loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async(_url,{signal})=>{
    requests++;return new Promise((_resolve,reject)=>signal.addEventListener('abort',()=>reject(signal.reason),{once:true}));
  },lifetime.signal);
  lifetime.abort(new DOMException('source disposed','AbortError'));
  await assert.rejects(pending,{name:'AbortError'});assert.equal(requests,1);
});

test('real geotiff blocked reads retain the underlying 503 and exhausted retries',async()=>{
  let attempts=0;
  const server=createServer((_req,res)=>{attempts++;res.writeHead(503);res.end();});
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const source=new CogPreviewSource({sources:[{url:`http://127.0.0.1:${server.address().port}/cog?sig=private`}]});
  try {
    await assert.rejects(source.getView(),error=>{
      assert.deepEqual(failureDiagnostics(error),{stage:'preview_metadata',reason:'upstream_unavailable',httpStatus:'503',retries:'2',diagnosticOrigin:'explicit'});
      assert.ok(!error.message.includes('private'));return true;
    });
    assert.equal(attempts,3);
    const tile={getError:()=>new Error('Request failed')};
    source.dispatchEvent({type:'tileloaderror',tile});
    assert.equal(failureDiagnostics(tile.previewFailure).stage,'preview_tile');
    assert.equal(failureDiagnostics(tile.previewFailure).httpStatus,'503');
  }finally{source.dispose();server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
});
