import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'vite';
import { chromium } from 'playwright';
import { validateAgentSnapshot } from '../prototype/src/agent-client.js';
const root=process.cwd(),output=path.join(root,'.verification',`agent-goals-ui-${Date.now()}`);
await mkdir(output,{recursive:true});const fsUrl=file=>'/@fs/'+file.replaceAll('\\','/');
const id='a1234567-1234-1234-1234-123456789abc';
const goal={version:1,id:'b1234567-1234-1234-1234-123456789abc',objective:'下载柏林最新影像，按行政区裁剪，并交付原始影像与裁剪成果。',requestText:'下载柏林最新影像，按行政区裁剪。',status:'waiting_confirmation',engine:null,planIds:[],continuations:0,checkedAt:'2026-10-07T08:00:00Z',updatedAt:'2026-10-07T08:00:00Z',reason:null,
  outputs:[{id:'original',label:'原始卫星影像与来源信息',kind:'raster',planId:'d1234567-1234-1234-1234-123456789abc',entryId:null,state:'verified',verifiedIds:['c1234567-1234-1234-1234-123456789abc']},{id:'crop',label:'按柏林行政区边界裁剪的影像',kind:'raster',planId:'e1234567-1234-1234-1234-123456789abc',entryId:null,state:'waiting',verifiedIds:[]}]};
const snapshot={version:1,revision:1,runtimeAvailable:true,configured:true,busy:false,mode:'review-first',model:{label:'Controlled UI',model:'test-model',protocol:'openai-compatible',baseUrl:'http://127.0.0.1:1/v1'},
  sessions:[{id,title:'柏林影像交付',status:'completed',compatible:true}],selected:{id,status:'completed',goal,entries:[{id:'request',type:'user',text:goal.requestText,status:'completed'},{id:'reply',type:'assistant',text:'原始影像已经检查。裁剪方案正在等待确认，确认后继续处理并检查最终成果。',status:'completed'}]}};
validateAgentSnapshot(snapshot);
await writeFile(path.join(output,'entry.jsx'),`import React from 'react';import{createRoot}from'react-dom/client';import{I18nProvider}from'${fsUrl(path.join(root,'prototype/src/i18n.jsx'))}';import'${fsUrl(path.join(root,'prototype/src/ui/foundation.css'))}';import'${fsUrl(path.join(root,'prototype/src/styles.css'))}';import{AgentPanel}from'${fsUrl(path.join(root,'prototype/src/agent-panel.jsx'))}';createRoot(document.getElementById('root')).render(<I18nProvider><AgentPanel onClose={()=>{}} onOpenProject={()=>{}} onOpenTasks={()=>{}}/></I18nProvider>);`);
await writeFile(path.join(output,'index.html'),`<!doctype html><html><head><meta charset="utf-8"><base href="/"><style>body{margin:0}#root{height:100vh;width:100vw;background:var(--surface);color:var(--ink)}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(path.join(output,'entry.jsx'))}"></script></body></html>`);
const server=await createServer({configFile:path.join(root,'prototype/vite.config.mjs'),root:path.join(root,'prototype'),logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}}});let browser;const measurements=[],errors=[];
try{
  await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,theme,locale] of [[560,'light','zh-CN'],[360,'dark','zh-CN'],[360,'light','en']]){
    const page=await browser.newPage({viewport:{width,height:900}});page.on('pageerror',error=>errors.push(error.message));
    await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
    await page.addInitScript(({snapshot,theme,locale})=>{
      localStorage.setItem('geod-global-locale',locale);window.controlled=snapshot;window.calls=[];window.__TAURI__={event:{listen:async()=>()=>{}},core:{invoke:async(method,args)=>{
        window.calls.push({method,args});if(method==='set_desktop_locale')return;
        if(method==='agent_goal_control'){
          if(args.sessionId!==window.controlled.selected.id)throw Error('Goal scope mismatch');
          if(args.action==='clear')delete window.controlled.selected.goal;else window.controlled.selected.goal.status=args.action==='pause'?'paused':'active';
          window.controlled.revision++;
        }else if(method!=='agent_snapshot')throw Error('Unexpected IPC: '+method);
        return structuredClone(window.controlled);
      }}};document.addEventListener('DOMContentLoaded',()=>{document.documentElement.dataset.theme=theme;});
    },{snapshot,theme,locale});
    await page.goto(origin+fsUrl(path.join(output,'index.html')));const card=page.locator('.agent-goal-card');await card.waitFor();
    await card.getByRole('button',{name:locale==='en'?'Required outputs · 1/2 verified':'所需成果 · 已验证 1/2'}).click();
    await page.waitForTimeout(600);
    await page.evaluate(()=>document.fonts.ready);assert.equal(await card.locator('li').count(),2);
    await page.screenshot({path:path.join(output,`goal-${width}-${theme}-${locale}.png`)});
    const geometry=await page.evaluate(()=>({width:innerWidth,overflow:document.documentElement.scrollWidth>innerWidth,cardWidth:document.querySelector('.agent-goal-card').getBoundingClientRect().width,composerVisible:document.querySelector('.bui-prompt-input').getBoundingClientRect().bottom<=innerHeight}));
    assert.equal(geometry.width,width);assert.equal(geometry.overflow,false);assert(geometry.composerVisible);measurements.push({theme,locale,...geometry});
    await card.getByRole('button',{name:locale==='en'?'Pause goal':'暂停目标'}).click();
    await card.getByRole('button',{name:locale==='en'?'Continue goal':'继续目标'}).click();
    await card.getByRole('button',{name:locale==='en'?'Clear goal':'清除目标'}).click();await card.waitFor({state:'detached'});
    assert((await page.evaluate(()=>window.calls)).every(call=>['agent_snapshot','agent_goal_control','set_desktop_locale'].includes(call.method)));await page.close();
  }
  assert.deepEqual(errors,[]);await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',scope:'Controlled React/IPC layout and controls; no native WebView, external model or download.',measurements,errors},null,2));console.log(JSON.stringify({status:'passed',output}));
}finally{await browser?.close();await server.close();}
