import test from 'node:test';import assert from 'node:assert/strict';import {createTracker,channelFrom,CAMPAIGN,wireDownloads} from './tutorial-tracker.mjs';
import {JSDOM} from 'jsdom';
function fixture(options={}){
 const map=new Map(),requests=[],storage={getItem:k=>map.get(k)||null,setItem:(k,v)=>map.set(k,v)};
 const tracker=createTracker({source:'zhihu',storage,fetcher:async(url,init)=>{requests.push({url,init});return {ok:true,json:async()=>({acceptedIds:JSON.parse(init.body).events.map(e=>e.id)})};},...options});
 return {tracker,requests,storage};
}
test('only fixed source and campaign values activate collection',()=>{
 assert.equal(channelFrom('?utm_source=zhihu&utm_campaign='+CAMPAIGN),'zhihu');
 assert.equal(channelFrom('?utm_source=https://private&utm_campaign='+CAMPAIGN),null);
 assert.equal(channelFrom('?utm_source=bilibili&utm_campaign=private'),null);
});
test('view and download click have separate events and anonymous receipts deduplicate retries',async()=>{
 const {tracker,requests}=fixture();tracker.record('tutorial_viewed');tracker.record('tutorial_download_clicked');await tracker.flush();await tracker.flush();
 assert.equal(requests.length,1);const b=JSON.parse(requests[0].init.body);assert.deepEqual(b.events.map(e=>e.name),['tutorial_viewed','tutorial_download_clicked']);
 assert.equal(b.events[0].installationId,b.events[1].installationId);assert.notEqual(b.events[0].id,b.events[1].id);
 assert.equal(requests[0].init.credentials,'omit');assert.equal(requests[0].init.referrerPolicy,'no-referrer');
 assert.equal(Object.keys(b.events[0].fields).length,3);
});
test('opt out survives reopening; GPC and DNT never collect; no source never collects',async()=>{
 const {tracker,storage}=fixture();tracker.setEnabled(false);
 const reopened=createTracker({source:'bilibili',storage});assert.equal(reopened.enabled(),false);
 for(const options of [{privacy:()=>true},{source:null}]){
  let sent=0;const {tracker:t}=fixture({...options,fetcher:async()=>{sent++;}});t.record('tutorial_viewed');await t.flush();assert.equal(sent,0);
 }
});
test('failed collection does not throw or break downloads, late receipts cannot undo opt out',async()=>{
 const {tracker}=fixture({fetcher:async()=>{throw new Error('offline');}});tracker.record('tutorial_download_clicked');await tracker.flush();
 const {tracker:t,storage}=fixture({fetcher:async(_url,init)=>{t.setEnabled(false);return {ok:true,json:async()=>({acceptedIds:JSON.parse(init.body).events.map(e=>e.id)})};}});
 t.record('tutorial_viewed');await t.flush();assert.equal(t.enabled(),false);assert.equal([...JSON.parse(storage.getItem('geod-global-tutorial-v1')).queue].length,0);
});
test('the sticky header and main download button keep the same channel and identify the ZIP honestly',()=>{
 const make=()=>new JSDOM('<a data-release-link="setup" href="https://example.test/setup.exe">下载 Windows 版</a><a data-tutorial-download data-package-zhihu="/zhihu.zip" data-package-bilibili="/bilibili.zip" href="https://example.test/portable.zip">下载 Windows 便携版</a>',{url:'https://example.test/case'});
 const dom=make(),events=[],tracker={record:n=>events.push(n),flush:async()=>{}};
 wireDownloads(dom.window.document,'zhihu',tracker);
 const links=[...dom.window.document.querySelectorAll('a')];
 assert.ok(links.every(a=>a.href==='https://example.test/zhihu.zip'));
 assert.equal(links[0].textContent,'下载 Windows 便携版');assert.equal(links[0].hasAttribute('data-release-link'),false);
 for(const a of links){a.addEventListener('click',e=>e.preventDefault());a.click();}
 assert.deepEqual(events,['tutorial_download_clicked','tutorial_download_clicked']);
 const direct=make();wireDownloads(direct.window.document,null,tracker);
 assert.equal(direct.window.document.querySelector('a').href,'https://example.test/setup.exe');
 dom.window.close();direct.window.close();
});
