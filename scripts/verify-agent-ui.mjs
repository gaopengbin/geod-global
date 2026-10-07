// Headless renderer acceptance using a recorded, genuinely completed native
// Agent transcript and a copied real raster. No model call or user desktop is
// involved here; installed WebView/window controls remain separate acceptance.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, cp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { AgentService } from '../agent/service.mjs';

const workspace = process.cwd(), scienceWorkflow=process.argv.includes('--science'), projectWorkflow=process.argv.includes('--project'), workflow = scienceWorkflow || projectWorkflow || process.argv.includes('--workflow');
const accountWorkflow=process.argv.includes('--accounts');
const deliveryWorkflow=process.argv.includes('--delivery');
if(deliveryWorkflow) assert(scienceWorkflow,'Delivery acceptance currently uses the real scientific RGB receipt.');
const accountLatest=accountWorkflow ? JSON.parse(await readFile('.verification/agent-accounts-latest.json','utf8')) : null;
const accountReceipt=accountWorkflow ? JSON.parse(await readFile(path.join(accountLatest.directory,'native-acceptance.json'),'utf8')) : null;
if(accountWorkflow)assert.equal(accountReceipt.status,'passed');
const latest = workflow ? JSON.parse(await readFile(scienceWorkflow ? '.verification/agent-science-latest.json' : projectWorkflow ? '.verification/agent-project-latest.json' : '.verification/agent-workflow-latest.json','utf8')) : null;
const accepted = workflow ? latest.directory.replace(/^\\\\\?\\/,'') : path.join(workspace, '.verification/agent-native-20261004');
const receipt = JSON.parse(await readFile(path.join(accepted, 'native-acceptance.json'), 'utf8'));
assert.equal(receipt.status, 'passed');
const output = path.join(workspace, '.verification', `agent-ui-${Date.now()}`);
const core = path.join(output, 'core'), history = path.join(output, 'history');
await mkdir(output); await mkdir(path.join(output, 'screenshots'));
await cp(path.join(accepted, 'core'), core, { recursive: true });
await mkdir(history); await cp(path.join(accepted, 'sessions/sessions.json'), path.join(history, 'sessions.json'));
let jobRecord = await readFile(path.join(core, 'jobs.json'), 'utf8');
const jobs = JSON.parse(jobRecord);
// Change managed file locations only. Re-serializing arbitrary provenance with
// JS changes JSON numeric types (30.0 -> 30), invalidating the TIFF's exact
// embedded native metadata contract even though the displayed numbers match.
for (const job of Object.values(jobs)) {
  if(job.outputPath) jobRecord=jobRecord.replaceAll(JSON.stringify(job.outputPath),JSON.stringify(path.join(core,'assets',path.basename(job.outputPath))));
  if(job.manifestPath) jobRecord=jobRecord.replaceAll(JSON.stringify(job.manifestPath),JSON.stringify(path.join(core,'assets',path.basename(job.manifestPath))));
}
await writeFile(path.join(core, 'jobs.json'), jobRecord);
const replay = await new AgentService({ home: history, callTool() { throw Error('Transcript review never runs tools.'); } }).open();
const fullEntries = workflow ? structuredClone(replay.snapshot().selected.entries) : null;
let confirmationReplayed = false, confirmCalls = 0, phase='project';
let accountReplayEntry=null;
const scienceConfirmed=new Set();
if (workflow) {
  const boundary = fullEntries.findIndex((entry,index) => index > 0 && entry.type === 'user');
  replay.sessions.find(session => session.id === replay.selectedId).entries = fullEntries.slice(0,boundary < 0 ? fullEntries.length : boundary);
}
const phaseFrames=projectWorkflow ? {project:receipt.pendingProject,download:receipt.pendingDownload,processing:receipt.pendingProcessing,complete:receipt.snapshot} : null;
const phaseSnapshot = () => scienceWorkflow ? {...structuredClone(confirmationReplayed ? receipt.snapshot : receipt.pending),runtimeAvailable:true,configured:false,model:null,
  plans:(confirmationReplayed ? receipt.snapshot : receipt.pending).plans.map(plan=>scienceConfirmed.has(plan.planId) ? structuredClone(receipt.snapshot.plans.find(p=>p.planId===plan.planId)) : structuredClone(plan))}
  : projectWorkflow ? {...structuredClone(phaseFrames[phase]),runtimeAvailable:true,configured:false,model:null} : ({ ...replay.snapshot(), runtimeAvailable: true,
  ...(workflow ? {plans:[structuredClone(confirmationReplayed ? receipt.snapshot.plans[0] : receipt.plan)]} : {}) });
const snapshot=()=>{
  const value=phaseSnapshot();
  if(accountReplayEntry && value.selected) {
    value.selected.entries=[...value.selected.entries,structuredClone(accountReplayEntry)];
    value.revision++;
  }
  return value;
};
const originalId = snapshot().selected.id;
assert.equal(snapshot().selected.status, 'completed');
const native = spawn(path.join(workspace, 'target/debug/geod-runtime.exe'), ['serve', '--data-dir', core, '--port', '4768'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
native.stdout.on('data', () => {}); let nativeError = ''; native.stderr.on('data', bytes => nativeError += bytes);
const origin = 'http://127.0.0.1:4769', base = 'http://127.0.0.1:4768';
const csp = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8')).app.security.csp;
const renderer = path.join(workspace, 'prototype/dist');
const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, origin), filename = path.resolve(renderer, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    assert(filename.startsWith(renderer + path.sep));
    const bytes = await readFile(filename);
    res.writeHead(200, { 'content-type': ({ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.json': 'application/json' })[path.extname(filename)] || 'application/octet-stream', 'content-security-policy': csp }).end(bytes);
  } catch { res.writeHead(404).end(); }
});
const routes = { health: '/health', list_jobs: '/jobs', list_projects: '/projects', list_recipes: '/recipes', get_proxy_settings: '/proxy', list_provider_accounts: '/accounts', list_vectors: '/vectors', list_feature_services: '/feature-services', list_map_services: '/map-services', list_map_images: '/map-images', list_tile_sources: '/tile-sources', list_tile_packages: '/tile-packages', list_stac_connections: '/stac/connections', list_wcs_connections: '/wcs/connections', list_three_d: '/three-d/packages' };
async function api(route, options) { const response = await fetch(base + route, options); const value = await response.json(); assert(response.ok, JSON.stringify(value)); return value; }
const report = { schema: 'geod-agent-renderer-acceptance/v1', nativeReadToolsVerifiedSeparately: receipt.nativeTools, agentTranscript: 'Recorded actual model/native-tool result; replay only during renderer review.', nativeWindowsTested: false, modelCalls: 0, usedUserDesktop: false, cases: [], errors: [], cspErrors: [], status: 'pending' };
if (workflow) report.confirmation = 'UI click replays separately verified native approval; this renderer review creates no downloads.';
if(deliveryWorkflow) {report.delivery='Explicit UI action builds actual verified ZIPs from the copied native files; no model approval or new processing job.';report.nativeDeliveryPackages=[];report.nativeDeliveryRejections=[];}
let browser, page;
async function capture(name, locale, width) {
  await page.evaluate(async () => Promise.all(document.getAnimations().filter(animation => Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation => animation.finished.catch(() => {}))));
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  const file = `${name}-${width}-${locale}.png`;
  await page.screenshot({ path: path.join(output, 'screenshots', file) });
  report.cases.push({ name, locale, width, screenshot: file });
}
try {
  await new Promise(resolve => server.listen(4769, '127.0.0.1', resolve));
  for (let attempt = 0; attempt < 100; attempt++) {
    try { await api('/health'); break; } catch (error) { assert.equal(native.exitCode, null, nativeError); if (attempt === 99) throw error; await new Promise(resolve => setTimeout(resolve, 100)); }
  }
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  for (const [width, height, locale, theme] of [[1440, 960, 'en', 'light'], [900, 720, 'zh-CN', 'dark']]) {
    confirmationReplayed = false;
    accountReplayEntry=null;
    scienceConfirmed.clear();
    phase='project';
    if (workflow) {
      const boundary = fullEntries.findIndex((entry,index) => index > 0 && entry.type === 'user');
      replay.sessions.find(session => session.id === originalId).entries = fullEntries.slice(0,boundary < 0 ? fullEntries.length : boundary);
    }
    await replay.select(originalId);
    const context = await browser.newContext({ viewport: { width, height } });
    const label = (en, zh) => locale === 'en' ? en : zh;
    await context.exposeBinding('__agentReviewNative', async (_, command, args = {}) => {
      if (command === 'activate_desktop_frame') return 'custom';
      if (['set_desktop_locale', 'set_desktop_appearance'].includes(command)) return null;
      if (routes[command]) return api(routes[command]);
      if (command === 'get_project') return api(`/projects/${args.id}`);
      if (command === 'file_thumbnail') return api(`/jobs/${args.id}/thumbnail`);
      if (command === 'inspect_raster') return api(`/jobs/${args.id}/raster`);
      if (command === 'sample_raster') return api(`/jobs/${args.id}/pixel?${new URLSearchParams({x:String(args.x),y:String(args.y)})}`);
      if (command === 'inspect_scientific_rgb') return api(`/jobs/${args.id}/rgb`);
      if (command === 'sample_scientific_rgb') return api(`/jobs/${args.id}/rgb/pixel?${new URLSearchParams({x:String(args.x),y:String(args.y)})}`);
      if(command==='prepare_artifact' && deliveryWorkflow) {
        const plan=receipt.snapshot.plans.find(plan=>plan.kind==='rgb' && plan.jobs.some(job=>job.id===args.id));
        assert(plan && scienceConfirmed.has(plan.planId));
        const before=(await api('/jobs')).map(job=>job.id);
        const response=await fetch(`${base}/jobs/${args.id}/package`,{method:'POST',headers:{'X-GeoD-Client':'geod-global'}});
        const result=await response.json();assert.deepEqual((await api('/jobs')).map(job=>job.id),before);
        if(!response.ok) {report.nativeDeliveryRejections.push({locale,jobId:args.id,error:result.error});throw Error(result.error);}
        assert.equal(result.jobId,args.id);
        report.nativeDeliveryPackages.push({locale,...result});return result;
      }
      if (command === 'agent_snapshot') return snapshot();
      if (command === 'agent_select') { await replay.select(args.id); return snapshot(); }
      if (command === 'agent_interrupt') { await replay.interrupt(); return snapshot(); }
      if (command === 'agent_approve_plan' && workflow) {
        if(scienceWorkflow) {
          const pending=receipt.pending.plans.find(p=>p.planId===args.planId);
          assert(pending);assert.equal(args.sessionId,originalId);assert.equal(args.planHash,pending.planHash);assert(!scienceConfirmed.has(args.planId));
          scienceConfirmed.add(args.planId);confirmCalls++;confirmationReplayed=scienceConfirmed.size===receipt.pending.plans.length;return snapshot();
        }
        if(projectWorkflow) {
          const pending=phaseFrames[phase].plans.find(plan=>plan.status==='pending');
          assert(pending);assert.equal(args.sessionId,originalId);assert.equal(args.planId,pending.planId);assert.equal(args.planHash,pending.planHash);
          confirmCalls++;phase=({project:'download',download:'processing',processing:'complete'})[phase];assert(phase);return snapshot();
        }
        assert.equal(args.sessionId, originalId); assert.equal(args.planId, receipt.plan.planId); assert.equal(args.planHash,receipt.plan.planHash);
        assert.equal(confirmationReplayed,false); confirmCalls++; confirmationReplayed = true;
        replay.sessions.find(session => session.id === originalId).entries = structuredClone(fullEntries);
        replay.revision++; return snapshot();
      }
      throw Error('Unavailable in recorded review: ' + command);
    });
    await context.addInitScript(({ locale, theme }) => {
      localStorage.setItem('geod-global-locale', locale); localStorage.setItem('geod-design-theme', JSON.stringify(theme));
      window.__TAURI__ = { core: { invoke: (command, args) => window.__agentReviewNative(command, args) } };
      window.__CSP_ERRORS = []; document.addEventListener('securitypolicyviolation', event => window.__CSP_ERRORS.push(event.violatedDirective));
    }, { locale, theme });
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ error: 'Remote catalog is outside this renderer review.' }) }));
    page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message));
    await page.goto(origin + (workflow ? '/#Tasks' : '/#My%20Data'));
    await page.getByRole('button', { name: label('Open Agent', '打开 Agent'), exact: true }).waitFor();
    await page.getByRole('button', { name: label('Open Agent', '打开 Agent'), exact: true }).click();
    await page.locator('.agent-message-assistant').last().waitFor();
    assert.equal(await page.locator('.app-header').count(), 1);
    if (scienceWorkflow) {
      assert.equal(await page.locator('.agent-plan').count(),3);
      for (const plan of receipt.pending.plans) {
        const card=page.locator(`[data-plan-id="${plan.planId}"]`);
        await card.scrollIntoViewIfNeeded();await capture(`pending-${plan.kind}-${plan.files[0].assetKey}`,locale,width);
        assert((await card.locator('.agent-plan-processing').textContent()).length>0);
        await card.getByRole('button',{name:label(plan.kind==='rgb'?'Confirm RGB':'Confirm processing',plan.kind==='rgb'?'确认合成':'确认处理'),exact:true}).click();
      }
      const rgbCard=page.getByRole('region',{name:label('Scientific RGB','科学 RGB 合成'),exact:true});
      await rgbCard.getByRole('link',{name:label('Open in workspace','在工作空间打开'),exact:true}).waitFor();
      const controls=await rgbCard.locator('.agent-plan-job-heading [data-slot=button]').evaluateAll(items=>items.map(item=>{const {x,y,width,height}=item.getBoundingClientRect();return {x,y,width,height};}));
      assert.equal(controls.length,4);for(const c of controls){assert.equal(c.width,32);assert.equal(c.height,32);assert.equal(c.y,controls[0].y);}
      await rgbCard.scrollIntoViewIfNeeded();await capture('scientific-results',locale,width);
      if(deliveryWorkflow) {
        const button=rgbCard.getByRole('button',{name:label('Prepare delivery package','准备交付包'),exact:true});
        assert.equal(report.nativeDeliveryPackages.filter(value=>value.locale===locale).length,0);
        for(let attempt=0;attempt<2;attempt++) {
          await button.click();const modal=page.getByRole('dialog',{name:label('Verified delivery package','已核验的交付包'),exact:true});await modal.waitFor();
          await capture(`native-delivery-${attempt}`,locale,width);
          await modal.getByRole('button',{name:label('Package checksum and contents','交付包校验值与内容'),exact:true}).click();
          const result=report.nativeDeliveryPackages.at(-1);
          assert((await modal.textContent()).includes(result.sha256));assert((await modal.textContent()).includes(result.filename));
          await modal.getByRole('button',{name:label('Close','关闭'),exact:true}).click();
        }
        const packages=report.nativeDeliveryPackages.filter(value=>value.locale===locale);
        assert.equal(packages.length,2);assert.equal(packages[0].sha256,packages[1].sha256);assert.equal(packages[0].path,packages[1].path);
        const result=packages[0], zipPath=path.resolve(core,'exports',result.filename);
        assert.equal(path.resolve(result.path.replace(/^\\\\\?\\/,'')),zipPath);
        const original=await readFile(zipPath), changed=Buffer.from(original);changed[changed.length-1]^=1;
        try {
          await writeFile(zipPath,changed);await button.click();
          const failure=page.getByRole('dialog',{name:label('Delivery package unavailable','无法准备交付包'),exact:true});await failure.waitFor();
          assert.equal(await page.getByRole('dialog',{name:label('Verified delivery package','已核验的交付包'),exact:true}).count(),0);
          assert.deepEqual(await readFile(zipPath),changed);
          assert.equal(report.nativeDeliveryRejections.at(-1).error,'The existing RGB delivery package changed; it will not be overwritten');
          await failure.getByRole('button',{name:label('Technical details','技术详情'),exact:true}).click();
          await capture('native-delivery-changed',locale,width);await failure.getByRole('button',{name:label('Close','关闭'),exact:true}).click();
        } finally {await writeFile(zipPath,original);}
      }
    } else if (projectWorkflow) {
      for(const [kind,en,zh] of [['project','Confirm project','确认创建工程'],['download','Confirm download','确认下载'],['processing','Confirm processing','确认处理']]) {
        const confirm=page.getByRole('button',{name:label(en,zh),exact:true});await confirm.waitFor();await confirm.scrollIntoViewIfNeeded();
        await capture(`pending-${kind}`,locale,width);await confirm.click();
      }
      await page.getByRole('button',{name:label('Confirm processing','确认处理'),exact:true}).waitFor({state:'detached'});
      const processingCard=page.getByRole('region',{name:label('Project processing','工程处理'),exact:true});
      const resultControls=await processingCard.locator('.agent-plan-job-heading [data-slot=button]').evaluateAll(controls=>controls.map(control=>{const {x,y,width,height}=control.getBoundingClientRect();return {x,y,width,height};}));
      assert.equal(resultControls.length,3);
      for(const control of resultControls) {assert.equal(control.width,32);assert.equal(control.height,32);assert.equal(control.y,resultControls[0].y);}
      for(let index=1;index<resultControls.length;index++) assert(resultControls[index].x-resultControls[index-1].x-resultControls[index-1].width<=8);
      await processingCard.scrollIntoViewIfNeeded();await capture('processed-project',locale,width);
      await processingCard.getByRole('button',{name:label('Open project','打开工程'),exact:true}).click();
      await page.locator('.projects-library-focused').waitFor();
      assert(new URL(page.url()).hash.includes(receipt.projectId));await capture('native-project-link',locale,width);
    } else if (workflow) {
      const card = page.locator('.agent-plan').first(); await card.scrollIntoViewIfNeeded();
      assert.equal(confirmCalls, locale === 'en' ? 0 : 1);
      await capture('pending-plan',locale,width);
      await page.getByRole('button',{name:label('Confirm download','确认下载'),exact:true}).click();
      await page.locator('.agent-plan-jobs').waitFor();
      assert.equal(await page.getByRole('button',{name:label('Confirm download','确认下载'),exact:true}).count(),0);
      await page.locator('.agent-plan').first().scrollIntoViewIfNeeded();
      await capture('confirmed-plan',locale,width);
    }
    await capture('transcript', locale, width);
    if(accountWorkflow) {
      // Actual native result, recorded without a model call. Only this UI tool
      // entry is assembled for replay; its data and checked time are unaltered.
      const result=accountReceipt.result;
      const summary={kind:'sources',count:result.sources.length,checkedAt:result.checkedAt,accounts:result.accountSources.map(source=>({provider:source.id,...source.authorization}))};
      accountReplayEntry={id:`native-accounts-${locale}`,type:'tool',name:'geod_sources_list',status:'completed',references:[],summary};
      const tool=page.locator('.agent-tool').filter({hasText:label('Check data source capabilities','查询数据源能力')}).last();
      await tool.getByRole('button').first().click();
      await tool.locator('.agent-account-actions').waitFor();
      assert.equal(await tool.locator('.bui-badge').count(),2);
      assert.equal(await tool.locator('.agent-account-actions a').count(),2);
      await page.evaluate(async()=>Promise.all(document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{}))));
      // Opening the result must reveal its actions without a manual scroll.
      await page.waitForFunction(()=>{
        const content=[...document.querySelectorAll('.agent-account-actions')].at(-1), viewport=document.querySelector('.agent-conversation');
        if(!content || !viewport) return false;
        const body=content.getBoundingClientRect(), frame=viewport.getBoundingClientRect();
        return body.top>=frame.top && body.bottom<=frame.bottom-8;
      });
      const accountBounds=await tool.locator('.agent-account-actions').boundingBox(), transcriptBounds=await page.locator('.agent-conversation').boundingBox();
      const accountControls=await tool.locator('.agent-account-actions a').evaluateAll(links=>links.map(link=>{const {x,y,width,height}=link.getBoundingClientRect();return {x,y,width,height};}));
      assert(accountBounds.y>=transcriptBounds.y && accountBounds.y+accountBounds.height<=transcriptBounds.y+transcriptBounds.height,JSON.stringify({accountBounds,transcriptBounds,accountControls}));
      await capture('native-account-status',locale,width);
      const accountLink=tool.getByRole('link',{name:label('Manage NASA Earthdata authorization','管理 NASA Earthdata 授权'),exact:true});
      assert.equal(await accountLink.getAttribute('href'),'#Settings?account=nasa-earthdata');
      await accountLink.click();
      const token=page.getByLabel(label('Earthdata user token','Earthdata 用户令牌'),{exact:true});
      await token.waitFor();assert.equal(await token.inputValue(),'');
      await capture('native-account-entry',locale,width);
      await page.getByRole('dialog').getByRole('button',{name:label('Cancel','取消'),exact:true}).click();
      await page.evaluate(()=>{window.location.hash='Tasks';});
    }
    const separator = page.getByRole('separator', { name: label('Resize Agent panel', '调整 Agent 面板宽度'), exact: true });
    const initialWidth = await page.locator('.agent-panel').evaluate(element => element.getBoundingClientRect().width);
    await separator.focus(); await page.keyboard.press('ArrowRight'); await page.keyboard.press('ArrowRight');
    const resizedWidth = await page.locator('.agent-panel').evaluate(element => element.getBoundingClientRect().width);
    assert(Math.abs(resizedWidth - initialWidth) > 1);
    report.cases.push({ name: 'keyboard-panel-resize', locale, initialWidth, resizedWidth });
    if (workflow) {
      const card=scienceWorkflow ? page.getByRole('region',{name:label('Scientific RGB','科学 RGB 合成'),exact:true}) : projectWorkflow ? page.getByRole('region',{name:label('Project processing','工程处理'),exact:true}) : page.locator('.agent-plan');
      await card.getByRole('button',{name:label('Open task','打开任务'),exact:true}).first().click();
    }
    else {
      const jobTool = page.locator('.agent-tool').filter({ hasText: label('Check task status', '检查任务状态') }).first();
      await jobTool.getByRole('button').first().click();
      await jobTool.locator('.agent-tool-results button').first().click();
    }
    await page.locator('[data-highlighted="true"]').waitFor();
    const target=scienceWorkflow ? receipt.rgbId : projectWorkflow ? receipt.outputId : receipt.jobId;
    assert(new URL(page.url()).hash.includes(`job=${target}`));
    await capture('native-task-link', locale, width);
    if (projectWorkflow || scienceWorkflow) {
      const outputCard=page.getByRole('region',{name:label(scienceWorkflow?'Scientific RGB':'Project processing',scienceWorkflow?'科学 RGB 合成':'工程处理'),exact:true});
      await outputCard.getByRole('link',{name:label('Open in workspace','在工作空间打开'),exact:true}).click();
      await page.waitForFunction(()=>document.querySelector('.wm-layer.active') || document.querySelector('.wm-error'));
      if(await page.locator('.wm-error').count()) {
        const detail=page.locator('.wm-error').getByRole('button',{name:label('Technical details','技术详情'),exact:true});
        if(await detail.count())await detail.click();
        throw Error(await page.locator('.wm-error').textContent());
      }
      await page.locator('.wm-layer.active').waitFor();
      await page.locator('.wm-map canvas').waitFor();
      assert(new URL(page.url()).hash.includes(`file=${target}`));
      const projectId=scienceWorkflow ? receipt.pending.plans.find(p=>p.kind==='rgb').project.id : receipt.projectId;
      assert(new URL(page.url()).hash.includes(`project=${projectId}`));
      assert.equal(await page.locator('.wm-layer').count(),1);
      await page.getByRole('button',{name:label('Read centre pixel','读取中心像元'),exact:true}).click();
      await page.locator('.wm-pixel-value').waitFor();
      assert.equal(await page.locator('.wm-error').count(),0);
      await capture('native-output-workspace',locale,width);
    }
    await page.getByRole('button', { name: label('Agent model connection', 'Agent 模型连接'), exact: true }).click();
    await page.getByRole('dialog').waitFor();
    assert.equal(await page.locator('.agent-model-form input[type=password]').inputValue(), '');
    await capture('model-connection', locale, width);
    for(const [provider,protocol,endpoint] of [['Anthropic','anthropic-messages','https://api.anthropic.com/v1'],['Google','google-generative-ai','https://generativelanguage.googleapis.com/v1beta']]) {
      await page.getByRole('combobox',{name:label('Agent model provider','Agent 模型供应商'),exact:true}).click();
      await page.getByRole('option',{name:provider,exact:true}).click();
      assert.equal(await page.locator('.agent-endpoint-field input').inputValue(),endpoint);
      assert(await page.getByRole('combobox',{name:label('Agent model protocol','Agent 模型协议'),exact:true}).isDisabled());
      assert.equal(await page.locator('.agent-model-form input[type=password]').inputValue(),'');
      await capture(`model-${protocol}`,locale,width);
    }
    await page.getByRole('button', { name: label('Cancel', '取消'), exact: true }).click();
    await page.getByRole('button', { name: label('Conversation history', '历史会话'), exact: true }).click();
    assert(await page.locator('.agent-history button').count() > 0);
    await page.getByRole('button', { name: label('Conversation history', '历史会话'), exact: true }).click();
    await page.getByRole('button', { name: label('New conversation', '新会话'), exact: true }).click();
    await page.getByRole('heading', { name: label('Connect your model', '连接模型'), exact: true }).waitFor();
    await capture('unconfigured', locale, width);
    await page.getByRole('button', { name: label('Close Agent', '关闭 Agent'), exact: true }).first().click();
    assert.equal(await page.locator('.agent-panel').count(), 0);
    report.cspErrors.push(...await page.evaluate(() => window.__CSP_ERRORS));
    await context.close();
  }
  assert.deepEqual(report.errors, []); assert.deepEqual(report.cspErrors, []); report.status = 'passed';
  if (workflow) assert.equal(confirmCalls,scienceWorkflow || projectWorkflow ? 6 : 2);
} catch (error) { report.status = 'failed'; report.failure = error.stack; await page?.screenshot({ path: path.join(output, 'screenshots/failure.png') }).catch(() => {}); throw error; }
finally {
  await browser?.close(); await replay.close(); native.kill(); await new Promise(resolve => server.close(resolve));
  await writeFile(path.join(output, 'verification.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ status: report.status, output, cases: report.cases.length, modelCalls: 0, usedUserDesktop: false }));
}
