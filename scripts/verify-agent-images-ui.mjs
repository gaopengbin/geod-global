// Hidden renderer review of recorded live image conversation and native ingestion.
// Browser callbacks replay these receipts; they never invoke a model or user desktop.
import {chromium} from 'playwright';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=process.cwd(),source=path.resolve(process.env.GEOD_AGENT_IMAGE_CONVERSATION),fixture=path.resolve(process.env.GEOD_AGENT_IMAGE_FIXTURE);
const organize=process.argv.includes('--context');
const responses=process.argv.includes('--responses');assert(!responses || !organize);
const dialogFooterOnly=process.argv.includes('--dialog-footer-only');assert(!dialogFooterOnly || responses);
const accepted=JSON.parse(await readFile(path.join(source,responses?'native-acceptance.json':'acceptance.json'),'utf8'));assert.equal(accepted.status,'passed');if(!responses)assert.equal(accepted.mode,'live-model-route');
const recorded=responses?accepted.snapshots[0]:JSON.parse(await readFile(path.join(source,`openai-compatible/turn-${organize?1:2}.json`),'utf8'));
if(responses){assert.equal(recorded.model.protocol,'openai-responses');assert.equal(recorded.model.capabilities.encryptedReasoning,true);}
const organized=organize?JSON.parse(await readFile(path.join(source,'openai-compatible/organized.json'),'utf8')):null;
if(organize){assert.equal(organized.selected.contextState.count,1);assert.deepEqual(organized.selected.entries,recorded.selected.entries);}
assert.equal(recorded.selected.status,'completed');
const ingestion=JSON.parse(await readFile(path.join(fixture,'ingestion.json'),'utf8'));assert.equal(ingestion.status,'passed');
const png=await readFile(path.join(fixture,'images',`${ingestion.image.id}.png`));assert.equal(createHash('sha256').update(png).digest('hex'),ingestion.image.id);
const original=await readFile(path.join(fixture,'source.png'));
const output=path.join(root,'.verification',`agent-images-ui-${Date.now()}`);await mkdir(path.join(output,'screenshots'),{recursive:true});
const report={schema:'geod-agent-images-renderer/v1',status:'running',usedUserDesktop:false,nativeWindowTested:false,modelCalls:0,
  callbacks:responses?'Recorded actual desktop/private IPC and native reads; upstream generation and vault were controlled fixtures. Dialog edits and send transitions are renderer callbacks only, with no model or vault calls.':'Recorded native image ingestion and actual live-model transcript; UI replay only. Context start/refusal transitions are controlled callbacks; completed state is the actual recorded native checkpoint.',conversationReceipt:path.relative(root,source),cases:[],errors:[],cspErrors:[],uploads:0,sends:0,contextOrganizations:0};
const probe=createServer();await new Promise(resolve=>probe.listen(0,'127.0.0.1',resolve));const nativePort=probe.address().port;await new Promise(resolve=>probe.close(resolve));
const native=spawn(path.join(root,'target/debug/geod-runtime.exe'),['serve','--data-dir',path.join(output,'core'),'--port',String(nativePort)],{windowsHide:true,stdio:['ignore','ignore','ignore']});
const base=`http://127.0.0.1:${nativePort}`;
const csp=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8')).app.security.csp;
const renderer=path.join(root,'prototype/dist');
const server=createServer(async(request,response)=>{
  try {const url=new URL(request.url,'http://127.0.0.1'),file=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));assert(file.startsWith(renderer+path.sep));
    const bytes=await readFile(file);response.writeHead(200,{'content-type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[path.extname(file)]??'application/octet-stream','content-security-policy':csp}).end(bytes);
  }catch {response.writeHead(404).end();}
});
const routes={health:'/health',list_jobs:'/jobs',list_projects:'/projects',list_recipes:'/recipes',get_proxy_settings:'/proxy',list_provider_accounts:'/accounts',list_vectors:'/vectors',list_feature_services:'/feature-services',list_map_services:'/map-services',list_map_images:'/map-images',list_tile_sources:'/tile-sources',list_tile_packages:'/tile-packages',list_stac_connections:'/stac/connections',list_wcs_connections:'/wcs/connections',list_three_d:'/three-d/packages'};
let browser,page;
try {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const origin=`http://127.0.0.1:${server.address().port}`;
  for(let i=0;i<100;i++){try{const response=await fetch(base+'/health');assert(response.ok);break;}catch{assert.equal(native.exitCode,null);if(i===99)throw Error('Owned native service did not start.');await new Promise(r=>setTimeout(r,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,height,locale,theme] of (dialogFooterOnly?[[900,720,'zh-CN','dark']]:[[1440,960,'en','light'],[900,720,'zh-CN','dark']])) {
    const context=await browser.newContext({viewport:{width,height}});const label=(en,zh)=>locale==='en'?en:zh;
    const snapshot={...structuredClone(recorded),runtimeAvailable:true};let refuse=true,refuseContext=true,contextTimer;
    await context.exposeBinding('__imageNativeReview',async(_,command,args={})=>{
      if(command==='activate_desktop_frame')return 'custom';if(['set_desktop_locale','set_desktop_appearance'].includes(command))return null;
      if(routes[command]) {const response=await fetch(base+routes[command]);assert(response.ok);return response.json();}
      if(command==='agent_snapshot')return structuredClone(snapshot);
      if(command==='agent_image_preview') {assert.equal(args.id,ingestion.image.id);return {image:ingestion.image,dataUrl:'data:image/png;base64,'+png.toString('base64')};}
      if(command==='agent_attach_image') {assert.equal(createHash('sha256').update(Buffer.from(args.encoded,'base64')).digest('hex'),createHash('sha256').update(original).digest('hex'));report.uploads++;return ingestion.image;}
      if(command==='agent_send') {assert.deepEqual(args.images,[ingestion.image.id]);assert.equal(args.text,'');report.sends++;if(refuse){refuse=false;throw Error('Owned UI send refusal');}return structuredClone(snapshot);}
      if(command==='agent_compact' && organize){assert.equal(args.sessionId,snapshot.selected.id);if(refuseContext){refuseContext=false;throw Error('Owned UI context refusal');}
        report.contextOrganizations++;snapshot.busy=true;snapshot.selected.status='starting';snapshot.selected.contextState={...(snapshot.selected.contextState??{count:0,lastCompletedAt:null,usedTokens:null,windowTokens:null}),status:'organizing'};
        contextTimer=setTimeout(()=>Object.assign(snapshot,structuredClone(organized),{runtimeAvailable:true}),1300);return structuredClone(snapshot);}
      throw Error('Unavailable in image renderer review: '+command);
    });
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__TAURI__={core:{invoke:(command,args)=>window.__imageNativeReview(command,args)}};window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.fulfill({status:200,contentType:'application/json',body:'{"error":"Remote catalog outside image review"}'}));
    page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));await page.goto(origin+'/#My%20Data');
    await page.getByRole('button',{name:label('Open Agent','打开 Agent'),exact:true}).click();await page.locator('.agent-message-user img').waitFor();
    await page.waitForFunction(()=>[...document.querySelectorAll('.agent-image img')].every(image=>image.complete && image.naturalWidth>0));
    const capture=async name=>{await page.evaluate(async()=>Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{}))));assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
      const screenshot=`${name}-${width}-${locale}.png`;await page.screenshot({path:path.join(output,'screenshots',screenshot)});report.cases.push({name,width,locale,screenshot});};
    if(!dialogFooterOnly){await page.locator('.agent-message-user .agent-image').first().scrollIntoViewIfNeeded();await capture('persisted-image');}
    if(responses){
      const protocol=()=>page.getByRole('combobox',{name:label('Agent model protocol','Agent 模型协议'),exact:true});
      await page.getByRole('button',{name:label('Agent model connection','Agent 模型连接'),exact:true}).click();
      assert((await protocol().textContent()).includes(label('OpenAI Responses','OpenAI 原生 Responses')));
      await page.getByRole('button',{name:label('Connection details','连接说明'),exact:true}).click();
      const actionVisible=async()=>{
        const box=await page.getByRole('button',{name:label('Save connection','保存连接'),exact:true}).boundingBox();
        assert(box && box.y>=0 && box.y+box.height<=height);
        assert(await page.locator('.agent-model-dialog .bui-dialog-footer').count()===1);
      };
      await actionVisible();
      await capture('responses-saved-connection');
      if(dialogFooterOnly){
        const body=page.locator('.agent-model-dialog .bui-dialog-body');
        await body.evaluate(element=>{element.scrollTop=element.scrollHeight;});await actionVisible();
        await page.getByRole('button',{name:label('Cancel','取消'),exact:true}).click();
        report.cspErrors.push(...await page.evaluate(()=>window.__CSP_ERRORS));await context.close();continue;
      }
      await page.getByRole('combobox',{name:label('Agent model provider','Agent 模型供应商'),exact:true}).click();
      await page.getByRole('option',{name:'OpenAI',exact:true}).click();
      await protocol().click();await page.getByRole('option',{name:label('OpenAI-compatible Chat Completions','OpenAI 兼容接口'),exact:true}).click();
      assert((await protocol().textContent()).includes(label('OpenAI-compatible Chat Completions','OpenAI 兼容接口')));await capture('responses-compatible-choice');
      await page.getByRole('combobox',{name:label('Saved connection','已保存的连接'),exact:true}).click();
      await page.getByRole('option',{name:label('Add model connection','添加模型连接'),exact:true}).click();
      assert((await protocol().textContent()).includes(label('OpenAI Responses','OpenAI 原生 Responses')));await actionVisible();await capture('responses-new-connection');
      await page.getByRole('button',{name:label('Cancel','取消'),exact:true}).click();
      assert(!(await page.locator('body').textContent()).includes('controlled-opaque-'));
    }
    if(organize){const draft=page.getByRole('textbox',{name:label('Message GeoD Agent','向 GeoD 助手发送消息'),exact:true});await draft.fill('Unsent draft stays local');
      const action=page.getByRole('button',{name:label('Organize context','整理上下文'),exact:true});await action.click();await page.getByText('Owned UI context refusal',{exact:true}).waitFor();assert.equal(await draft.inputValue(),'Unsent draft stays local');await capture('context-refused-history-retained');
      await action.click();await page.getByText(label('Organizing context…','正在整理上下文…'),{exact:true}).waitFor();assert(await action.isDisabled());assert(await page.getByRole('button',{name:label('Send message','发送消息'),exact:true}).count()===0);await capture('context-organizing');
      await page.getByRole('button',{name:label('Send message','发送消息'),exact:true}).waitFor();assert.equal(await draft.inputValue(),'Unsent draft stays local');assert.equal(await page.locator('.agent-message-user img').count(),1);await capture('context-ready-image-and-draft-retained');clearTimeout(contextTimer);
    }
    await page.getByRole('button',{name:label('Preview image visual-check.png','预览图片 visual-check.png'),exact:true}).first().click();await page.locator('.agent-image-full').waitFor();await capture('full-image-preview');
    await page.getByRole('button',{name:label('Close','关闭'),exact:true}).click();
    await page.getByRole('button',{name:label('New conversation','新会话'),exact:true}).click();
    await page.locator('input[type=file]').setInputFiles(path.join(fixture,'source.png'));await page.locator('.agent-draft-images img').waitFor();
    await page.waitForFunction(()=>document.querySelector('.agent-draft-images img')?.naturalWidth>0);await capture('image-draft');
    await page.getByRole('button',{name:label('Send message','发送消息'),exact:true}).click();await page.getByText('Owned UI send refusal',{exact:true}).waitFor();assert.equal(await page.locator('.agent-draft-images img').count(),1);await capture('send-refused-draft-retained');
    await page.getByRole('button',{name:label('Send message','发送消息'),exact:true}).click();await page.locator('.agent-message-user img').waitFor();assert.equal(await page.locator('.agent-draft-images').count(),0);await capture('recorded-send-clears-draft');
    await page.getByRole('button',{name:label('New conversation','新会话'),exact:true}).click();await page.locator('input[type=file]').setInputFiles(path.join(fixture,'source.png'));await page.locator('.agent-draft-images img').waitFor();
    await page.getByRole('button',{name:label('Remove image visual-check.png','移除图片 visual-check.png'),exact:true}).click();assert.equal(await page.locator('.agent-draft-images').count(),0);assert(await page.getByRole('button',{name:label('Send message','发送消息'),exact:true}).isDisabled());
    await capture('draft-removed');report.cspErrors.push(...await page.evaluate(()=>window.__CSP_ERRORS));await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.cspErrors,[]);assert.equal(report.uploads,dialogFooterOnly?0:4);assert.equal(report.sends,dialogFooterOnly?0:4);assert.equal(report.contextOrganizations,organize?2:0);report.status='passed';
}catch(error){report.status='failed';report.failure=error.message;await page?.screenshot({path:path.join(output,'screenshots/failure.png')}).catch(()=>{});throw error;}
finally {await browser?.close();native.kill();await new Promise(resolve=>server.close(resolve));await writeFile(path.join(output,'verification.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({output,status:report.status,cases:report.cases.length,modelCalls:0}));}
