// Actual shared React UI with controlled IPC. No model or remote data requests.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'vite';
import { chromium } from 'playwright';
import { validateAgentSnapshot } from '../prototype/src/agent-client.js';

const root=process.cwd(), output=path.join(root,'.verification',`agent-decisions-ui-${Date.now()}`);
await mkdir(output,{recursive:true});
const fsUrl=file=>'/@fs/'+file.replaceAll('\\','/');
const sessionId='a1234567-1234-1234-1234-123456789abc',decisionId='b1234567-1234-1234-1234-123456789abc',planId='c1234567-1234-1234-1234-123456789abc';
const decision={version:1,id:decisionId,title:'确定区域和交付方案',status:'pending',questions:[
  {id:'boundary',prompt:'需要按哪种区域边界交付？',recommendedOptionId:'polygon',options:[{id:'polygon',label:'行政区多边形',description:'保留已选择的真实区域边界，后续按边界裁剪。'},{id:'rectangle',label:'外接矩形',description:'保留区域周围的影像，文件范围会更大。'}]},
  {id:'quality',prompt:'如何平衡质量与观测数量？',recommendedOptionId:'strict',options:[{id:'strict',label:'优先可靠观测',description:'使用该产品支持的严格质量规则，允许出现缺测。'},{id:'more',label:'保留更多观测',description:'保留边缘质量观测，并在交付时明确说明。'}]},
]};
const request='按已选择的行政区下载最近一个月的影像，保留质量图层，后续按区域裁剪并检查成果。';
const snapshot={version:1,revision:1,runtimeAvailable:true,configured:true,busy:false,mode:'review-first',
  model:{label:'受控 UI 验收',model:'test-model',protocol:'openai-compatible',baseUrl:'https://example.test/v1'},
  execution:{mode:'confirm-each',defaultMode:'confirm-each',scope:'managed-projects-and-files',modelCanChangePermission:false},
  sessions:[{id:sessionId,title:'交互验收',status:'completed',compatible:true}],
  selected:{id:sessionId,status:'completed',entries:[{id:'request',type:'user',status:'completed',text:request},{id:'decision',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision}]},plans:[]};
const nativePlan={planId,planHash:'a'.repeat(64),kind:'download',status:'pending',approvalRequired:true,expiresAt:'2026-10-08T00:00:00Z',source:'Earth Search · Sentinel-2 L2A',bounds:[1,2,3,4],start:'2026-09-01',end:'2026-09-30',expectedBytes:42000000,
  files:[{itemId:'Native scene A',assetKey:'visual',bytes:32000000},{itemId:'Native scene A',assetKey:'scl',bytes:10000000}],notes:['目录相交不代表整个区域都有有效观测；后续裁剪需单独检查。'],jobs:[],polygon:{sha256:'b'.repeat(64),bounds:[1,2,3,4]}};
validateAgentSnapshot({...snapshot,plans:[nativePlan]});
const entry=path.join(output,'entry.jsx'),html=path.join(output,'index.html');
await writeFile(entry,`import React from 'react';import{createRoot}from'react-dom/client';
import{I18nProvider}from'${fsUrl(path.join(root,'prototype/src/i18n.jsx'))}';
import'${fsUrl(path.join(root,'prototype/src/ui/foundation.css'))}';
import'${fsUrl(path.join(root,'prototype/src/styles.css'))}';
import'${fsUrl(path.join(root,'prototype/src/catalog.css'))}';
import'${fsUrl(path.join(root,'prototype/src/runtime.css'))}';
import{AgentPanel}from'${fsUrl(path.join(root,'prototype/src/agent-panel.jsx'))}';
createRoot(document.getElementById('root')).render(<I18nProvider><AgentPanel onClose={()=>{}} onOpenProject={()=>{}} onOpenTasks={()=>{}}/></I18nProvider>);`);
await writeFile(html,`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><base href="/"><title>Owned decision UI check</title><style>body{margin:0;background:var(--canvas);color:var(--ink)}#root{width:min(560px,100vw);height:100vh;margin:auto;background:var(--surface);border-inline:1px solid var(--line);box-sizing:border-box}</style></head><body><div id="root"></div><script type="module" src="${fsUrl(entry)}"></script></body></html>`);
const server=await createServer({configFile:path.join(root,'prototype/vite.config.mjs'),root:path.join(root,'prototype'),logLevel:'error',server:{host:'127.0.0.1',port:0,strictPort:false,fs:{allow:[root]}}});
let browser;const errors=[],measurements=[];
try{
  await server.listen();const origin=`http://127.0.0.1:${server.httpServer.address().port}`;
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,theme,locale] of [[560,'light','zh-CN'],[360,'dark','zh-CN'],[360,'light','en']]){
    const page=await browser.newPage({viewport:{width,height:1000}});
    page.on('pageerror',error=>errors.push(error.message));
    await page.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
    await page.addInitScript(({snapshot,nativePlan,theme,locale})=>{
      localStorage.setItem('geod-global-locale',locale);
      window.state=snapshot;window.calls=[];const listeners=new Set();
      const signal=()=>{window.state.revision++;for(const listener of listeners)listener({payload:window.state.revision});};
      window.__TAURI__={event:{listen:async(_,callback)=>{listeners.add(callback);return()=>listeners.delete(callback);}},core:{invoke:async(method,args)=>{
        window.calls.push({method,args});const state=window.state;
        if(method==='set_desktop_locale')return;
        if(method==='agent_send'){
          const record=state.selected.entries.find(entry=>entry.decision);
          if(!args.decisionAnswer||args.text!==''||args.decisionAnswer.decisionId!==record.decision.id)throw Error('Expected actual card answer');
          record.decision.status='answered';record.decision.answers=args.decisionAnswer.answers;
          const choices=record.decision.questions.map(question=>{const answer=record.decision.answers.find(value=>value.questionId===question.id);return{decisionId:record.decision.id,questionId:question.id,prompt:question.prompt,answer:answer.text??question.options.find(option=>option.id===answer.optionId).label};});
          state.selected.entries.push({id:'review',type:'tool',name:'geod_download_plan',status:'completed',references:[{kind:'plan',id:nativePlan.planId,label:'Download plan'}],taskContext:{requestText:state.selected.entries[0].text,choices}});
          state.plans=[nativePlan];signal();
        }else if(method==='agent_approve_plan'){
          if(args.planId!==nativePlan.planId||args.planHash!==nativePlan.planHash)throw Error('Expected reviewed native identity');
          state.plans[0].status='submitted';signal();
        }else if(method!=='agent_snapshot')throw Error('Unexpected controlled IPC: '+method);
        return structuredClone(state);
      }}};
    },{snapshot,nativePlan,theme,locale});
    await page.goto(origin+fsUrl(html));
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    const card=page.locator('.agent-decision');await card.waitFor();
    await page.waitForFunction(()=>document.querySelector('.agent-decision-option')?.disabled===false);
    const submit=page.getByRole('button',{name:locale==='en'?'Submit choices':'提交选择'});
    assert(await submit.isDisabled());assert.equal(await card.locator('[aria-pressed=true]').count(),0);
    await card.screenshot({path:path.join(output,`choices-${width}-${theme}-${locale}.png`)});
    const options=card.locator('.agent-decision-options');
    await options.nth(0).getByRole('button',{name:/行政区多边形/}).click();
    assert(await submit.isDisabled());
    await options.nth(1).getByRole('button',{name:/优先可靠观测/}).click();
    const colors=await options.nth(0).evaluate(node=>[...node.children].map(button=>({border:getComputedStyle(button).borderColor,background:getComputedStyle(button).backgroundColor})));
    assert.notEqual(colors[0].border,colors[1].border);assert.notEqual(colors[0].background,colors[1].background);
    await card.screenshot({path:path.join(output,`selected-${width}-${theme}-${locale}.png`)});
    await submit.click();
    const review=page.locator(`[data-plan-id="${planId}"]`);await review.waitFor();
    assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.method==='agent_approve_plan').length),0);
    assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.method==='agent_send').length),1);
    assert((await review.innerText()).includes('VISUAL · Native scene A'));
    assert((await review.innerText()).includes('SCL · Native scene A'));
    await review.screenshot({path:path.join(output,`task-${width}-${theme}-${locale}.png`)});
    const measured=await page.evaluate(()=>({width:innerWidth,documentWidth:document.documentElement.scrollWidth,
      overflow:[...document.querySelectorAll('.agent-decision,.agent-plan,.agent-decision-option')].filter(node=>node.scrollWidth>node.clientWidth+1).length}));
    assert.equal(measured.documentWidth,width);assert.equal(measured.overflow,0);measurements.push({theme,locale,...measured});
    await page.reload();await page.locator('.agent-decision').waitFor(); // Pending snapshot restored; never silently answers it.
    assert(await page.getByRole('button',{name:locale==='en'?'Submit choices':'提交选择'}).isDisabled());
    await page.close();
  }
  assert.deepEqual(errors,[]);
  const result={status:'passed',scope:'Actual React UI and controlled IPC; no live model, native window or downloaded data',modelCalls:0,measurements,screenshots:['choices-560-light-zh-CN.png','choices-360-dark-zh-CN.png','choices-360-light-en.png','selected-360-dark-zh-CN.png','task-560-light-zh-CN.png','task-360-dark-zh-CN.png','task-360-light-en.png']};
  await writeFile(path.join(output,'result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({output,...result}));
}finally{await browser?.close();await server.close();}
