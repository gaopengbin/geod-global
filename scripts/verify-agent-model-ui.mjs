// Owned, hidden renderer check. Synthetic IPC; no saved keys or model calls.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'vite';
import { chromium } from 'playwright';
import { MODEL_CAPABILITIES } from '../prototype/src/agent-model-registry.js';

const root=process.cwd(), output=path.join(root,'.verification',`agent-model-ui-${Date.now()}`);
await mkdir(output,{recursive:true});
const fsUrl=file=>'/@fs/'+file.replaceAll('\\','/');
const connections=[{id:'d1234567-1234-1234-1234-123456789abc',provider:'deepseek',label:'ds',model:'deepseek-flash',baseUrl:'https://api.deepseek.com'},
  {id:'e1234567-1234-1234-1234-123456789abc',provider:'custom',label:'备用连接',model:'another-model',baseUrl:'https://example.test/v1'}]
  .map(connection=>({...connection,protocol:'openai-compatible',capabilities:MODEL_CAPABILITIES,verification:'not-verified'}));
const snapshot={version:1,revision:1,runtimeAvailable:true,mode:'review-first',configured:true,busy:false,
  model:connections[0],registry:{version:1,selectedId:connections[0].id,connections},sessions:[],selected:null,plans:[]};
const entry=path.join(output,'entry.jsx'), html=path.join(output,'index.html');
await writeFile(entry,`import React from 'react';import {createRoot} from 'react-dom/client';
import {I18nProvider} from '${fsUrl(path.join(root,'prototype/src/i18n.jsx'))}';
import {AgentPanel} from '${fsUrl(path.join(root,'prototype/src/agent-panel.jsx'))}';
import '${fsUrl(path.join(root,'prototype/src/ui/foundation.css'))}';
createRoot(document.getElementById('root')).render(<I18nProvider><AgentPanel onClose={()=>{}} onOpenProject={()=>{}} onOpenTasks={()=>{}}/></I18nProvider>);`);
await writeFile(html,`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>Owned model UI check</title><style>body{margin:0;background:var(--canvas);font-family:Arial,"Microsoft YaHei",sans-serif;color:var(--ink)}#root{width:460px;height:840px;margin:auto;background:var(--surface);border-inline:1px solid var(--line)}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(entry)}"></script></body></html>`);
const server=await createServer({configFile:path.join(root,'prototype/vite.config.mjs'),root:path.join(root,'prototype'),server:{port:0,strictPort:false,host:'127.0.0.1',fs:{allow:[root]}},logLevel:'error'});
let browser;const errors=[];
try {
  await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;
  browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:1000,height:840}});
  page.on('pageerror',error=>errors.push(error.message));
  await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
  await page.addInitScript(({snapshot})=>{
    localStorage.setItem('geod-global-locale','zh-CN');window.modelUiCalls=[];
    window.__TAURI__={core:{invoke:async(method,args)=>{
      window.modelUiCalls.push({method,args});
      if(method==='set_desktop_locale')return;
      if(method==='agent_snapshot')return structuredClone(snapshot);
      if(method==='agent_test_model') {await new Promise(resolve=>setTimeout(resolve,350));return {version:1,status:'passed',text:true,functionCalls:true,latencyMs:521,checkedAt:'2026-10-06T12:00:00Z'};}
      throw Error('Unexpected owned UI operation: '+method);
    }}};
  },{snapshot});
  await page.goto(origin+fsUrl(html));
  await page.getByRole('combobox',{name:'Agent 模型选择'}).click();
  const item=page.getByRole('option',{name:'ds · deepseek-flash',exact:true});await item.waitFor();
  assert.equal(await item.getAttribute('data-description-layout'),null);
  assert.equal(await page.getByRole('option',{name:'模型与连接…',exact:true}).locator('.lucide-bot').count(),1);
  assert(await page.getByText('尚未验证此连接的可用性',{exact:true}).count()===0);
  await page.waitForTimeout(200);
  const menu=await page.locator('.agent-model-menu').boundingBox(),toolbar=await page.locator('.prompt-input-toolbar').boundingBox();
  await page.screenshot({path:path.join(output,'model-picker.png'),clip:{x:Math.floor(menu.x-8),y:Math.floor(menu.y-8),width:Math.ceil(Math.max(menu.width,toolbar.width)+16),height:Math.ceil(toolbar.y+toolbar.height-menu.y+28)}});
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Agent 模型连接',exact:true}).click();
  const test=page.getByRole('button',{name:'测试连接',exact:true});await test.click();
  await page.getByText('连接正常',{exact:true}).waitFor();
  const calls=await page.evaluate(()=>window.modelUiCalls);
  assert(calls.filter(call=>call.method==='agent_test_model').length===1);
  assert(!calls.some(call=>call.method==='agent_save_model'||call.method==='agent_send'));
  const save=await page.getByRole('button',{name:'保存连接',exact:true}).boundingBox();assert(save.y+save.height<840);
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.locator('.agent-model-dialog').screenshot({path:path.join(output,'model-dialog.png')});
  assert.deepEqual(errors,[]);
  await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',scope:'Chinese model menu and connection dialog; synthetic IPC only',usedUserDesktop:false,modelCalls:0,nativeExecution:false,savedConnectionsChanged:false,screenshots:['model-picker.png','model-dialog.png']},null,2));
  console.log(JSON.stringify({status:'passed',output}));
} finally {await browser?.close();await server.close();}
