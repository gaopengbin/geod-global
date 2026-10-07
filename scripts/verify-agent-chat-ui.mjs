// Owned headless renderer, controlled native signals. No model requests,
// saved connection edits, user desktop interaction or download simulation.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'vite';
import { chromium } from 'playwright';
import { MODEL_CAPABILITIES } from '../prototype/src/agent-model-registry.js';

const root = process.cwd(), output = path.join(root, '.verification', `agent-chat-ui-${Date.now()}`);
await mkdir(output, { recursive:true });
const fsUrl = file => '/@fs/' + file.replaceAll('\\', '/');
const sessionId = 'a1234567-1234-1234-1234-123456789abc';
const longModel = 'long-model-' + 'reasoning-and-geographic-analysis-'.repeat(3);
const connections = [
  {id:'d1234567-1234-1234-1234-123456789abc', provider:'deepseek', label:'ds', model:'deepseek-flash', baseUrl:'https://api.deepseek.com'},
  {id:'e1234567-1234-1234-1234-123456789abc', provider:'deepseek', label:'备用连接', model:longModel, baseUrl:'https://api.deepseek.com'},
].map(value => ({...value, protocol:'openai-compatible', capabilities:MODEL_CAPABILITIES, verification:'not-verified'}));
const entries = Array.from({length:12}, (_, index) => [
  {id:`user-${index}`, type:'user', status:'completed', text:`历史问题 ${index + 1}：当前有哪些可用的影像？`},
  {id:`answer-${index}`, type:'assistant', status:'completed', text:`历史回复 ${index + 1}\n可以按区域和日期搜索影像，再选择原文件进行下载。\n已有工程和任务都保留在本地工作空间。`},
]).flat();
const snapshot = {version:1, revision:1, mode:'review-first', runtimeAvailable:true, configured:true, busy:false,
  model:connections[0], registry:{version:1, selectedId:connections[0].id, connections}, plans:[],
  execution:{mode:'confirm-each', defaultMode:'confirm-each', scope:'managed-projects-and-files', modelCanChangePermission:false},
  selected:{id:sessionId, status:'completed', entries},
  sessions:[{id:sessionId, title:'控件交互验收', status:'completed', compatible:true}],
};
const entry = path.join(output, 'entry.jsx'), html = path.join(output, 'index.html');
await writeFile(entry, `import React from 'react';import {createRoot} from 'react-dom/client';
import {I18nProvider} from '${fsUrl(path.join(root,'prototype/src/i18n.jsx'))}';
import '${fsUrl(path.join(root,'prototype/src/ui/foundation.css'))}';
import '${fsUrl(path.join(root,'prototype/src/styles.css'))}';
import '${fsUrl(path.join(root,'prototype/src/catalog.css'))}';
import '${fsUrl(path.join(root,'prototype/src/runtime.css'))}';
import {AgentPanel} from '${fsUrl(path.join(root,'prototype/src/agent-panel.jsx'))}';
const renderer=createRoot(document.getElementById('root'));window.unmount=()=>renderer.unmount();
renderer.render(<I18nProvider><AgentPanel onClose={()=>{}} onOpenProject={()=>{}} onOpenTasks={()=>{}}/></I18nProvider>);`);
await writeFile(html, `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><base href="/"><title>Owned chat UI check</title><style>body{margin:0;background:var(--canvas);color:var(--ink)}#root{width:min(460px,100vw);height:100vh;margin:auto;background:var(--surface);border-inline:1px solid var(--line);box-sizing:border-box}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(entry)}"></script></body></html>`);
const server = await createServer({configFile:path.join(root,'prototype/vite.config.mjs'), root:path.join(root,'prototype'), logLevel:'error', server:{host:'127.0.0.1', port:0, strictPort:false, fs:{allow:[root]}}});
let browser;
const errors = [], measurements = {};
try {
  await server.listen(); const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
  browser = await chromium.launch({channel:'msedge', headless:true});
  const page = await browser.newPage({viewport:{width:260, height:840}, permissions:['clipboard-read','clipboard-write']});
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
  await page.addInitScript(({snapshot}) => {
    localStorage.setItem('geod-global-locale','zh-CN');
    window.state = snapshot; window.calls = []; const listeners = new Set();
    window.listenerCount = () => listeners.size;
    window.signal = () => { for (const listener of listeners) listener({payload:window.state.revision}); };
    window.delta = text => {
      if (!window.state.busy) return;
      window.state.selected.entries.at(-1).text += text; window.state.revision++; window.signal();
    };
    window.__TAURI__ = {event:{listen:async(name, callback) => {
      if (name !== 'geod-agent-changed') throw Error('Unexpected event');
      listeners.add(callback); return () => listeners.delete(callback);
    }}, core:{invoke:async(method, args) => {
      const state = window.state; window.calls.push({method,args});
      if (method === 'set_desktop_locale') return;
      if (method === 'agent_send') {
        state.busy=true;state.selected.status='running';state.sessions[0].status='running';
        state.selected.entries.push({id:'new-user',type:'user',status:'completed',text:args.text}, {id:'stream-answer',type:'assistant',status:'running',text:''});state.revision++;
      } else if (method === 'agent_interrupt') {
        state.busy=false;state.selected.status='interrupted';state.sessions[0].status='interrupted';state.selected.entries.at(-1).status='interrupted';state.revision++;
      } else if (method === 'agent_save_model') {
        state.model=state.registry.connections.find(connection=>connection.id===args.request.id);state.registry.selectedId=state.model.id;state.revision++;
      } else if (method === 'agent_execution_mode') {
        state.execution.mode=args.mode;state.revision++;
      } else if (method === 'agent_select' && args.id === null) {state.selected=null;state.revision++;}
      else if (method !== 'agent_snapshot') throw Error('Unexpected UI operation: '+method);
      return structuredClone(state);
    }}};
  }, {snapshot});
  await page.goto(origin + fsUrl(html));
  const picker = page.getByRole('combobox',{name:'Agent 模型选择'}), viewport=page.locator('.agent-conversation');
  await picker.waitFor(); assert.equal(await picker.innerText(),'deepseek-flash');
  const input = page.getByRole('textbox',{name:'向 GeoD 助手发送消息'});
  const composer = page.locator('.bui-prompt-input');
  measurements.composerSizes=[];
  for (const width of [260,320,460,493]) {
    await page.setViewportSize({width:1000,height:840});
    await page.locator('#root').evaluate((element,width)=>element.style.width=`${width}px`,width);
    await page.waitForTimeout(100);
    const geometry=await composer.evaluate(element=>{
      const rect=node=>{const r=node.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};};
      return {form:rect(element),text:rect(element.querySelector('textarea')),controls:[...element.querySelectorAll('[data-prompt-control]')].map(node=>({name:node.dataset.promptControl,...rect(node)}))};
    });
    measurements.composerSizes.push({panelWidth:width,...geometry});
    assert.equal(geometry.text.height,48,'An empty input must retain two rows after a cold narrow mount or resize');
    assert.equal(geometry.form.height,116);
    assert.deepEqual(geometry.controls.map(control=>control.name),['attachment','permission','context','model','submit']);
    const first=geometry.controls[0];
    for (let index=0;index<geometry.controls.length;index++) {
      const control=geometry.controls[index];
      assert(Math.abs(control.y+control.height/2-first.y-first.height/2)<=2,'Toolbar controls must share a center line');
      assert(control.x>=geometry.form.x && control.x+control.width<=geometry.form.x+geometry.form.width,'All controls must remain inside the input');
      if(index) assert(control.x>=geometry.controls[index-1].x+geometry.controls[index-1].width-1,`Toolbar controls must not overlap: ${JSON.stringify({width,geometry})}`);
    }
    await input.fill('较长的地理数据处理需求。'.repeat(90));
    await page.waitForTimeout(60);assert.equal(await input.evaluate(element=>element.getBoundingClientRect().height),144);
    await input.fill('');await page.waitForTimeout(60);assert.equal(await input.evaluate(element=>element.getBoundingClientRect().height),48);
  }
  await page.locator('#root').evaluate(element=>element.style.width='min(460px,100vw)');
  await page.locator('.agent-composer-area').screenshot({path:path.join(output,'composer-light.png')});
  await page.getByRole('button',{name:'添加附件',exact:true}).click();
  const actions=page.locator('.prompt-input-actions');await actions.waitFor();
  assert.equal(await actions.locator('.prompt-input-action').count(),5);
  assert.deepEqual(await actions.locator('.prompt-input-action-icon > svg').evaluateAll(nodes=>nodes.map(node=>[...node.classList].find(name=>name.startsWith('lucide-')))),['lucide-plus','lucide-file','lucide-audio-lines','lucide-settings','lucide-map']);
  assert.equal(await actions.getByRole('button',{name:/添加音频/}).isDisabled(),true);
  await page.waitForTimeout(200);await page.screenshot({path:path.join(output,'attachment-menu.png')});
  const chooser=page.waitForEvent('filechooser');await actions.getByRole('button',{name:/添加图片/}).click();await chooser;
  assert.equal(await page.locator('input[type=file]').getAttribute('accept'),'image/png,image/jpeg,image/webp');
  const permission=page.getByRole('button',{name:'执行方式',exact:true});
  await permission.click();await page.locator('.composer-permission-menu').waitFor();
  await page.getByRole('button',{name:/完全访问.*自动执行/}).click();await page.waitForFunction(()=>window.state.execution.mode==='full-access');
  assert.equal(await permission.innerText(),'完全访问');await permission.click();
  await page.getByRole('button',{name:/每次确认.*方案/}).click();assert.equal(await permission.innerText(),'每次确认');
  await page.getByRole('button',{name:'上下文用量',exact:true}).click();
  await page.locator('.context-usage-panel').waitFor();assert.equal(await page.getByRole('meter').count(),0);
  await page.getByText('等待首次模型请求',{exact:true}).waitFor();
  await page.getByRole('button',{name:'发送哪些内容？',exact:true}).click();
  await page.keyboard.press('Escape');
  await page.evaluate(()=>{window.state.selected.contextState={status:'ready',count:1,lastCompletedAt:'2026-10-06T12:00:00Z',usedTokens:16000,windowTokens:32000};window.state.revision++;window.signal();});
  await page.getByRole('button',{name:'上下文用量',exact:true}).click();
  await page.getByText('50.0%',{exact:true}).waitFor();assert.equal(await page.getByRole('meter').getAttribute('aria-valuenow'),'16000');
  await page.keyboard.press('Escape');
  await picker.click();await page.getByRole('option',{name:'模型与连接…',exact:true}).click();
  await page.getByRole('dialog').waitFor();assert.equal(await page.getByLabel('模型 ID',{exact:true}).inputValue(),'deepseek-flash');
  await page.getByRole('button',{name:'取消',exact:true}).click();
  await page.evaluate(()=>document.documentElement.dataset.theme='dark');
  await page.evaluate(()=>document.activeElement?.blur());await page.waitForTimeout(300);
  await page.locator('.agent-composer-area').screenshot({path:path.join(output,'composer-dark.png')});
  await page.evaluate(()=>document.documentElement.dataset.theme='light');
  await page.waitForTimeout(300);
  const bounds = await picker.boundingBox();
  await picker.click(); await page.getByRole('option',{name:'ds · deepseek-flash',exact:true}).waitFor();
  await page.waitForTimeout(200);
  const menu = await page.locator('.agent-model-menu').boundingBox();
  measurements.normalPickerWidth = bounds.width; measurements.menuWidth = menu.width;
  assert(bounds.width <= 180 && menu.width <= 210 && menu.width > bounds.width);
  assert(menu.x >= 12 && menu.x + menu.width <= 988);
  await page.waitForTimeout(160); await page.screenshot({path:path.join(output,'model-light.png')});
  await page.keyboard.press('Escape'); await page.waitForFunction(()=>document.activeElement?.classList.contains('agent-model-select'));

  // The real input handles Chinese composition without an accidental send.
  await input.fill('查看当前工程和任务');
  await input.evaluate(element => {element.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));element.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,isComposing:true}));element.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));});
  assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.method==='agent_send').length),0);
  await input.press('Enter'); await page.getByRole('button',{name:'停止回答',exact:true}).waitFor();
  await page.evaluate(()=>window.delta('我会先检查当前工程。\n'));
  await page.getByText('我会先检查当前工程。',{exact:true}).waitFor();
  assert.equal(await page.locator('.bui-message-text[data-streaming=true]').count(),1);
  await page.evaluate(()=>window.delta('**当前步骤**\n- 读取工程信息\n- 检查本地任务\n'));
  await page.getByText('读取工程信息',{exact:true}).waitFor();
  await page.waitForTimeout(280); await page.locator('.agent-panel').screenshot({path:path.join(output,'stream-light.png')});
  await viewport.hover(); await page.mouse.wheel(0,-480);
  await page.getByRole('button',{name:'回到最新消息',exact:true}).waitFor();
  await page.waitForTimeout(180); const readingTop = await viewport.evaluate(element=>element.scrollTop);
  await page.evaluate(()=>window.delta('\n新的输出继续到达，但不会打断你正在查看的历史记录。\n'.repeat(6)));
  await page.getByText('新的输出继续到达，但不会打断你正在查看的历史记录。',{exact:true}).first().waitFor();
  assert(Math.abs(await viewport.evaluate(element=>element.scrollTop)-readingTop) <= 2);
  await page.getByRole('button',{name:'回到最新消息',exact:true}).click(); await page.waitForTimeout(350);
  assert(await viewport.evaluate(element=>element.scrollHeight-element.clientHeight-element.scrollTop < 3));
  await page.getByRole('button',{name:'停止回答',exact:true}).click();
  await page.waitForFunction(()=>!document.querySelector('.bui-message-text[data-streaming=true]'));
  const stoppedText=await page.evaluate(()=>window.state.selected.entries.at(-1).text);
  await page.evaluate(()=>window.delta('这个增量不应出现'));
  assert.equal(await page.evaluate(()=>window.state.selected.entries.at(-1).text),stoppedText);
  await page.getByRole('button',{name:'复制回复',exact:true}).last().click();
  await page.getByRole('button',{name:'回复已复制',exact:true}).waitFor();
  assert.equal((await page.evaluate(()=>navigator.clipboard.readText())).replaceAll('\r\n','\n'),stoppedText);

  // Completed execution records collapse, while the answer stays readable.
  await page.evaluate(()=>{
    const state=window.state;state.selected.entries=[
      {id:'check-user',type:'user',status:'completed',text:'检查我的工程和任务'},
      {id:'projects',type:'tool',name:'geod_projects_list',status:'completed',references:[],summary:{kind:'projects',count:2}},
      {id:'jobs',type:'tool',name:'geod_jobs_list',status:'completed',references:[],summary:{kind:'jobs',count:3}},
      {id:'final-answer',type:'assistant',status:'completed',text:'检查完成。\n- 两个工程已保存在本地\n- 当前没有正在下载的任务\n可以继续搜索影像，或打开已有工程进行处理。'},
    ];state.selected.status='completed';state.sessions[0].status='completed';state.revision++;window.signal();
  });
  const records=page.locator('.agent-work-records > .bui-disclosure > .bui-disclosure-trigger');
  await records.waitFor(); assert.equal(await records.getAttribute('aria-expanded'),'false');
  await records.click(); await page.locator('.agent-work-timeline .agent-tool').first().waitFor(); assert.equal(await page.locator('.agent-work-timeline .agent-tool').count(),2); await records.click();
  await page.waitForTimeout(220); await page.locator('.agent-panel').screenshot({path:path.join(output,'completed-light.png')});
  await page.evaluate(()=>document.documentElement.dataset.theme='dark'); await picker.click();
  await page.waitForTimeout(160); await page.screenshot({path:path.join(output,'model-dark.png')}); await page.keyboard.press('Escape');
  await page.setViewportSize({width:320,height:780}); await picker.click();
  await page.getByRole('option',{name:`备用连接 · ${longModel}`,exact:true}).click();
  await page.waitForFunction(model=>document.querySelector('.agent-model-select')?.textContent===model,longModel);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  measurements.longPickerWidth=(await picker.boundingBox()).width;assert(measurements.longPickerWidth<=180);
  await picker.click(); const compactMenu=await page.locator('.agent-model-menu').boundingBox();
  assert(compactMenu.x>=12 && compactMenu.x+compactMenu.width<=308);
  await page.waitForTimeout(160); await page.screenshot({path:path.join(output,'model-compact-dark.png')}); await page.keyboard.press('Escape');
  await page.getByRole('textbox',{name:'向 GeoD 助手发送消息'}).fill('长消息\n'.repeat(35));
  assert(await input.evaluate(element=>element.getBoundingClientRect().height<=150));
  const send=await page.getByRole('button',{name:'发送消息',exact:true}).boundingBox();assert(send.y+send.height<780);
  const counts=await page.evaluate(()=>({send:window.calls.filter(call=>call.method==='agent_send').length,stop:window.calls.filter(call=>call.method==='agent_interrupt').length}));
  assert.deepEqual(counts,{send:1,stop:1});
  await page.evaluate(()=>window.unmount()); assert.equal(await page.evaluate(()=>window.listenerCount()),0);
  assert.deepEqual(errors,[]);
  await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',scope:'Controlled native IPC and actual shared React UI; not a live model or native WebView recording',usedUserDesktop:false,modelCalls:0,savedConnectionsChanged:false,viewports:[{width:1000,height:840,panelWidths:[260,320,460,493]},{width:320,height:780}],measurements,checks:['whole reference composer and toolbar order','cold narrow mount and subsequent resize','empty and long input height','toolbar center alignment and no overlap','reference plus menu and Lucide icons','native permission request','unknown and reported context usage','privacy inside context popup','model management from composer','compact model name and bounded popup','keyboard Escape and focus restoration','Chinese IME','stream chunks and caret','reader scroll preserved','jump to latest','stop freezes output','copy original answer','collapsed execution log','long draft input','event listener cleanup'],screenshots:['composer-light.png','composer-dark.png','attachment-menu.png','model-light.png','stream-light.png','completed-light.png','model-dark.png','model-compact-dark.png']},null,2));
  console.log(JSON.stringify({status:'passed',output,measurements}));
} finally {await browser?.close();await server.close();}
