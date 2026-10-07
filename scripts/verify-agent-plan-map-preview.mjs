import assert from 'node:assert/strict';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {createServer} from 'vite';
import {chromium} from 'playwright';
import {validateAgentSnapshot} from '../prototype/src/agent-client.js';
import {validatePlanMapPreview} from '../prototype/src/agent-map-preview.js';
import {emptyDistribution} from '../prototype/src/distribution-client.js';
const root=process.cwd(),output=path.join(root,'.verification',`agent-plan-map-preview-${Date.now()}`);
await mkdir(output,{recursive:true});const fsUrl=file=>'/@fs/'+file.replaceAll('\\','/');
const dataRoot=path.join(process.env.LOCALAPPDATA,'xyz.laogao.geod.global');
// Read only the selected native review and its owning conversation. No registry, secrets or writes to user storage.
const history=JSON.parse(await readFile(path.join(dataRoot,'agent/sessions.json'),'utf8'));
const session=history.sessions.find(s=>s.id===history.selectedId);assert(session);
const ref=session.entries.flatMap(e=>e.references||[]).filter(r=>r.kind==='plan').at(-1);assert(ref);
const stored=JSON.parse(await readFile(path.join(dataRoot,'runtime/agent-plans',ref.id+'.json'),'utf8'));
assert.equal(stored.sessionId,session.id);assert.equal(stored.action.kind,'download');
const {query,files}=stored.action;
const preview={planId:stored.id,planHash:stored.hash,provider:query.provider,bounds:query.bounds,geometry:null,
  selections:files.map(f=>({itemId:f.request.itemId,assets:{[f.request.assetKey]:f.request.href}}))};
validatePlanMapPreview(preview);
const plan={planId:stored.id,planHash:stored.hash,kind:'download',status:'pending',source:'Earth Search · Sentinel-2 L2A',bounds:query.bounds,start:query.start,end:query.end,
  expiresAt:stored.expiresAt,approvalRequired:true,expectedBytes:files.reduce((n,f)=>n+f.pin.bytes,0),files:files.map(f=>({itemId:f.request.itemId,assetKey:f.request.assetKey,date:f.date,bytes:f.pin.bytes})),
  notes:['Original complete files; search bounds do not crop source rasters.'],jobs:[]};
const snapshot={version:1,revision:1,runtimeAvailable:true,configured:true,busy:false,mode:'review-first',model:{label:'UI acceptance',model:'test-model',protocol:'openai-compatible',baseUrl:'http://127.0.0.1:1/v1'},
  sessions:[{id:session.id,title:'Selected native imagery review',status:'completed',compatible:true}],selected:{id:session.id,status:'completed',entries:[{id:'native-review',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:stored.id,label:'Native download review'}]}]},plans:[plan]};
validateAgentSnapshot(snapshot);
await writeFile(path.join(output,'entry.jsx'),`import '${fsUrl(path.join(root,'prototype/src/main.jsx'))}';`);
await writeFile(path.join(output,'index.html'),`<!doctype html><html><head><meta charset="utf-8"><base href="/"><style>body{margin:0}#root{height:100vh;width:100vw;background:var(--surface);color:var(--ink)}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(path.join(output,'entry.jsx'))}"></script></body></html>`);
const server=await createServer({configFile:path.join(root,'prototype/vite.config.mjs'),root:path.join(root,'prototype'),logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}}});let browser;
const errors=[],network=[],measurements=[];
try {
  await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,height,theme] of [[1280,900,'light'],[900,900,'dark']]) {
    const page=await browser.newPage({viewport:{width,height}});page.on('pageerror',e=>errors.push(e.message));
    page.on('response',r=>{if(new URL(r.url()).origin!==origin)network.push({url:r.url(),status:r.status()});});
    await page.route('**/*',route=>{const u=new URL(route.request().url());return u.origin===origin||['earth-search.aws.element84.com','sentinel-cogs.s3.us-west-2.amazonaws.com'].includes(u.hostname)?route.continue():route.abort();});
    await page.addInitScript(({snapshot,preview,theme,distribution})=>{
      localStorage.setItem('geod-global-locale','zh-CN');localStorage.setItem('geod-design-theme',JSON.stringify(theme));localStorage.setItem('geod-design-nav-collapsed','true');window.calls=[];
      window.__TAURI__={event:{listen:async()=>()=>{}},core:{invoke:async(method,args)=>{
        window.calls.push({method,args});if(['set_desktop_locale','set_desktop_appearance'].includes(method))return;
        if(method==='agent_snapshot')return structuredClone(snapshot);
        if(method==='agent_select')return structuredClone(snapshot);
        if(method==='activate_desktop_frame')return 'native';
        if(method==='health')return {};
        if(['list_jobs','list_projects'].includes(method))return [];
        if(method==='distribution_snapshot')return distribution;
        if(method==='agent_plan_map_preview') {if(args.planId!==preview.planId||args.planHash!==preview.planHash||args.sessionId!==snapshot.selected.id)throw Error('Review mismatch');return structuredClone(preview);}
        throw Error('Unexpected IPC: '+method);
      }}};document.addEventListener('DOMContentLoaded',()=>document.documentElement.dataset.theme=theme);
    },{snapshot,preview,theme,distribution:emptyDistribution()});
    await page.goto(origin+fsUrl(path.join(output,'index.html')));
    await page.getByRole('button',{name:'历史会话'}).click();await page.getByRole('button',{name:'Selected native imagery review'}).click();
    const composer=page.getByRole('textbox',{name:'向 GeoD 助手发送消息'});
    await composer.fill('想进一步对比这些影像');await page.getByRole('button',{name:'地图预览',exact:true}).click();
    const panel=page.getByRole('region',{name:'任务地图预览'});await panel.waitFor();assert.equal(await page.getByRole('dialog').count(),0);
    await page.waitForFunction(count=>document.querySelectorAll('.agent-map-preview-scenes [role=checkbox]').length===count,preview.selections.length,{timeout:40000});
    await page.waitForFunction(()=>!!document.querySelector('.agent-map-preview-canvas canvas'),{},{timeout:15000});
    await page.waitForTimeout(1800);
    const boxes=panel.getByRole('checkbox');assert.equal(await boxes.count(),preview.selections.length);
    assert.equal(await panel.locator('[role=checkbox][aria-checked=true]').count(),preview.selections.length);
    await page.waitForFunction(count=>Number(document.querySelector('.agent-map-preview-panel .explore-map-root')?.dataset.imageryLayers)===count,preview.selections.length);
    await panel.locator('.explore-map-progress').waitFor({state:'detached',timeout:45000});
    for(const selection of preview.selections)assert(network.some(r=>r.url===selection.assets.visual&&r.status===206));
    await boxes.nth(1).click();assert.equal(await boxes.nth(1).getAttribute('aria-checked'),'false');
    await page.waitForFunction(count=>Number(document.querySelector('.agent-map-preview-panel .explore-map-root')?.dataset.imageryLayers)===count,preview.selections.length-1);
    await panel.getByRole('button',{name:'全部隐藏'}).click();await page.waitForFunction(()=>document.querySelector('.agent-map-preview-panel .explore-map-root')?.dataset.imageryLayers==='0');
    await panel.getByRole('button',{name:'全部显示'}).click();await page.waitForFunction(count=>Number(document.querySelector('.agent-map-preview-panel .explore-map-root')?.dataset.imageryLayers)===count,preview.selections.length);
    await panel.getByRole('button',{name:'定位搜索区域'}).click();await page.waitForTimeout(500);
    const divider=page.getByRole('separator',{name:'调整地图预览面板宽度'}),before=(await panel.boundingBox()).width;
    await divider.focus();await divider.press('ArrowLeft');await page.waitForTimeout(300);
    assert.notEqual((await panel.boundingBox()).width,before);
    await panel.locator('.explore-map-progress').waitFor({state:'detached',timeout:30000});
    const layout=await page.evaluate(()=>{
      const panel=document.querySelector('.agent-map-preview-panel'),map=document.querySelector('.agent-map-preview-canvas'),chat=document.querySelector('#agent-pane');
      return {width:innerWidth,overflow:document.documentElement.scrollWidth>innerWidth,panelWidth:panel.getBoundingClientRect().width,mapHeight:map.getBoundingClientRect().height,
        mapIsRightOfChat:panel.getBoundingClientRect().left>=chat.getBoundingClientRect().right,conversationVisible:chat.getBoundingClientRect().width>=320,
        footerVisible:document.querySelector('.agent-map-preview-footer').getBoundingClientRect().bottom<=innerHeight,errors:[...document.querySelectorAll('.explore-map-message[role=alert],.agent-map-preview-warning')].map(x=>x.textContent)};
    });
    assert.equal(layout.width,width);assert.equal(layout.overflow,false);assert(layout.footerVisible);assert(layout.mapHeight>=240);assert(layout.mapIsRightOfChat);assert(layout.conversationVisible);assert.deepEqual(layout.errors,[]);measurements.push({theme,...layout});
    await page.screenshot({path:path.join(output,`preview-${width}-${theme}.png`)});
    assert.equal(await composer.inputValue(),'想进一步对比这些影像');assert(await composer.isEnabled());assert(await page.getByRole('button',{name:'确认下载'}).isEnabled());
    await panel.getByRole('button',{name:'关闭地图预览'}).click();await panel.waitFor({state:'detached'});assert.equal(await composer.inputValue(),'想进一步对比这些影像');assert.equal(await page.getByRole('button',{name:'确认下载'}).count(),1);
    const allowed=['agent_snapshot','agent_select','agent_plan_map_preview','set_desktop_locale','set_desktop_appearance','health','list_jobs','list_projects','activate_desktop_frame','distribution_snapshot'];
    assert.deepEqual([...new Set((await page.evaluate(()=>window.calls)).filter(c=>!allowed.includes(c.method)).map(c=>c.method))],[]);await page.close();
  }
  assert.deepEqual(errors,[]);assert(network.some(r=>r.url.includes('/items/')&&r.status===200));assert(network.some(r=>r.url.endsWith('.tif')&&r.status===206));
  await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',scope:'Real App/React/OpenLayers right panel with controlled readonly IPC. Every reviewed scene defaults visible; real COG range reads for all four. Independent hide/show, all-off/all-on, resize, composer/confirmation and closing verified. No native WebView or download execution.',planId:stored.id,scenes:preview.selections.map(s=>s.itemId),measurements,network:network.map(r=>({host:new URL(r.url).hostname,path:new URL(r.url).pathname,status:r.status})),errors},null,2));console.log(JSON.stringify({status:'passed',output}));
} finally {await browser?.close();await server.close();}
