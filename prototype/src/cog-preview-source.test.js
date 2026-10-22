import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {loadPreviewRange} from './cog-preview-source.js';

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

test('disposing a source cancels its range read and does not retry cancellation',async()=>{
  const lifetime=new AbortController();let requests=0;
  const pending=loadPreviewRange('https://source.test/cog',{Range:'bytes=0-3'},undefined,async(_url,{signal})=>{
    requests++;return new Promise((_resolve,reject)=>signal.addEventListener('abort',()=>reject(signal.reason),{once:true}));
  },lifetime.signal);
  lifetime.abort(new DOMException('source disposed','AbortError'));
  await assert.rejects(pending,{name:'AbortError'});assert.equal(requests,1);
});
