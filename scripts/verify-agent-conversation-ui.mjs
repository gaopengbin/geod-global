// Owned hidden browser: actual accepted transcript replay plus controlled UI
// states. No live model calls, saved account changes, or user desktop input.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {createServer} from 'vite';
import {chromium} from 'playwright';
const root=process.cwd(),output=path.join(root,'.verification',`conversation-ui-${Date.now()}`);
await mkdir(output,{recursive:true});
const latest=JSON.parse(await readFile(path.join(root,'.verification/agent-conversation-latest.json'),'utf8'));
const accepted=JSON.parse(await readFile(path.join(latest.directory,'acceptance.json'),'utf8'));
assert.equal(accepted.status,'passed');
const complete=accepted.snapshot;
// Display metadata for this recorded test connection only; no saved registry
// or credential is created. The original native transcript is unchanged.
complete.model={label:'验收连接',model:accepted.modelRoute};delete complete.registry;
const empty={...complete,revision:0,busy:false,selected:null,sessions:[],plans:[],execution:{...complete.execution,mode:'confirm-each',defaultMode:'confirm-each'}};
const fsUrl=value=>'/@fs/'+value.replaceAll('\\','/'),entry=path.join(output,'entry.jsx'),html=path.join(output,'index.html');
await writeFile(entry,`import React from 'react';import {createRoot} from 'react-dom/client';import {I18nProvider} from '${fsUrl(path.join(root,'prototype/src/i18n.jsx'))}';import {AgentPanel} from '${fsUrl(path.join(root,'prototype/src/agent-panel.jsx'))}';import '${fsUrl(path.join(root,'prototype/src/ui/foundation.css'))}';createRoot(document.getElementById('root')).render(<I18nProvider><AgentPanel onClose={()=>{}} onOpenProject={()=>{}} onOpenTasks={()=>{}} onOpenResult={view=>{window.resultViews.push(view);location.hash='Workspace?file='+view.id;}}/></I18nProvider>);`);
await writeFile(html,`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><base href="/"><title>Owned conversation check</title><style>body{margin:0;font-family:Arial,"Microsoft YaHei",sans-serif;color:var(--ink);background:var(--canvas)}#root{width:min(460px,100vw);height:100vh;margin:auto;background:var(--surface);border-inline:1px solid var(--line)}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(entry)}"></script></body></html>`);
const server=await createServer({configFile:path.join(root,'prototype/vite.config.mjs'),root:path.join(root,'prototype'),server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}},logLevel:'error'});
let browser;const errors=[];
try{
  await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;
  browser=await chromium.launch({channel:'msedge',headless:true});const page=await browser.newPage({viewport:{width:1000,height:840}});
  page.on('pageerror',error=>errors.push(error.message));await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
  await page.addInitScript(({empty})=>{
    localStorage.setItem('geod-global-locale','zh-CN');window.state=empty;window.resultViews=[];window.uiCalls=[];
    window.__TAURI__={core:{invoke:async(method,args)=>{
      window.uiCalls.push({method,args});if(method==='set_desktop_locale')return;
      if(method==='agent_execution_mode'){window.state.execution.mode=args.mode;window.state.execution.defaultMode=args.mode;}
      if(method==='agent_acknowledge_view')window.state.selected.workspaceView.acknowledged=true;
      if(method==='agent_interrupt')window.state.selected.workflow.status='paused';
      if(method==='agent_select'&&args.id===null){window.state.selected=null;window.state.plans=[];}
      if(!['agent_snapshot','agent_execution_mode','agent_acknowledge_view','agent_interrupt','agent_select'].includes(method))throw Error('Unexpected UI operation');
      return structuredClone(window.state);
    }}};
  },{empty});
  await page.goto(origin+fsUrl(html));await page.getByRole('combobox',{name:'执行方式'}).waitFor();
  await page.getByRole('combobox',{name:'执行方式'}).click();await page.getByRole('option',{name:'自动执行',exact:true}).waitFor();await page.waitForTimeout(180);
  await page.screenshot({path:path.join(output,'mode-desktop.png')});await page.getByRole('option',{name:'自动执行',exact:true}).click();await page.getByText('可以直接交代任务',{exact:true}).waitFor();
  await page.setViewportSize({width:320,height:780});await page.waitForTimeout(120);
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.screenshot({path:path.join(output,'empty-compact.png')});
  await page.evaluate(complete=>{window.state=structuredClone(complete);},complete);
  await page.waitForFunction(()=>window.resultViews.length===1);
  assert.equal(await page.evaluate(()=>location.hash),'#Workspace?file='+complete.selected.workspaceView.id);
  await page.getByText('流程已完成',{exact:true}).waitFor();await page.screenshot({path:path.join(output,'completed-compact.png')});
  const waiting=structuredClone(complete);waiting.selected.workspaceView.acknowledged=true;waiting.selected.workflow.status='waiting';waiting.selected.workflow.waitingPlans=1;
  waiting.selected.entries.push({id:'owned-notice',type:'system',origin:'desktop',status:'completed',text:'Plans submitted to the native task system. Actual results will be checked when tasks settle.'});
  await page.evaluate(value=>{window.state=value;},waiting);await page.getByRole('button',{name:'暂停自动推进',exact:true}).waitFor();await page.getByRole('button',{name:'暂停自动推进',exact:true}).click();await page.getByText('自动推进已暂停',{exact:true}).waitFor();
  await page.screenshot({path:path.join(output,'paused-compact.png')});
  await page.evaluate(complete=>{window.state=structuredClone(complete);window.state.selected.workspaceView.acknowledged=true;},complete);
  await page.setViewportSize({width:1000,height:840});await page.getByText('流程已完成',{exact:true}).waitFor();await page.screenshot({path:path.join(output,'completed-desktop.png')});
  assert.equal(await page.evaluate(()=>window.resultViews.length),1);assert.deepEqual(errors,[]);
  const calls=await page.evaluate(()=>window.uiCalls);assert(!calls.some(call=>['agent_send','agent_save_model','agent_test_model'].includes(call.method)));
  const receipt={status:'passed',scope:'Actual completed native transcript replay and controlled mode/paused states; not a Windows WebView acceptance',viewports:[{width:1000,height:840,panelWidth:460},{width:320,height:780}],usedUserDesktop:false,modelCalls:0,nativeMapRendered:false,workspaceNavigation:true,errors,screenshots:['mode-desktop.png','empty-compact.png','completed-compact.png','paused-compact.png','completed-desktop.png']};
  await writeFile(path.join(output,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify({status:'passed',output}));
}finally{await browser?.close();await server.close();}
