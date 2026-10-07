// Headless renderer contract fixtures. No real credentials, model request,
// download, preflight, approval, native execution, or desktop automation.
// Native revision correctness is tested separately in geod-runtime.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { MODEL_CAPABILITIES } from '../prototype/src/agent-model-registry.js';
import { validateAgentSnapshot, validatePlanRevisionDraft } from '../prototype/src/agent-client.js';

const workspace=process.cwd(), renderer=path.join(workspace,'prototype/dist');
const output=path.join(workspace,'.verification',`agent-revision-renderer-${Date.now()}`);
await mkdir(path.join(output,'screenshots'),{recursive:true});
const csp=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8')).app.security.csp;
const sessionId='a1234567-1234-1234-1234-123456789abc', projectId='b1234567-1234-1234-1234-123456789abc';
const uuid=n=>`${String(n).padStart(8,'0')}-1234-1234-1234-123456789abc`;
const hash=n=>n.toString(16).padStart(64,'0');
const report={schema:'geod-agent-revision-renderer-acceptance/v1',evidence:'Renderer contract fixture: synthetic native IPC responses, not native execution or model availability acceptance.',
  nativeRevisionExecutionTestedHere:false,nativeWindowsTested:false,usedUserDesktop:false,modelCalls:0,externalRequests:0,nativeJobsCreated:0,cases:[],interactions:[],errors:[],cspErrors:[],status:'pending'};
const commands=[];
const sourceBounds=[-122.55,37.65,-122.4,37.85];
const sceneIds=['S2A_10SEG_20260905_0_L2A','S2B_10SEG_20260910_0_L2A'];
const items=sceneIds.map((id,index)=>({id,date:`2026-09-${index?10:'05'}T00:00:00Z`,locked:false}));
const processing={product:'landsat-c2-l2',dataType:'UInt16',channels:3,rawBytes:36,requiredDiskBytes:8388700,
  quality:{product:'landsat-c2-l2',policy:'cloud_free_conservative',excludeSnow:true,coupled:false,sceneCount:1,sourceCount:5,sourceSha256:hash(90)}};
function makePlan(kind,index){
  const plan={planId:uuid(index),planHash:hash(index),kind,status:'pending',approvalRequired:true,source:'Renderer contract fixture',bounds:sourceBounds,boundsCrs:'EPSG:4326',
    files:[{itemId:sceneIds[0],assetKey:kind==='project'?'scene':'visual',bytes:kind==='project'?null:1000,width:3,height:2}],
    expectedBytes:kind==='project'?null:1000,expiresAt:'2099-01-01T00:00:00Z',notes:[],jobs:[]};
  if(kind==='vector') {
    const {files,...review}=plan;
    return {...review,expectedBytes:null,vector:null,vectorReview:{serviceId:uuid(600),serviceSha256:hash(600),collectionId:'lakes',collectionTitle:'Lakes',protocol:'OGC API Features',name:'Vector review fixture',pageSize:2,responseFormat:null,selection:'bbox-full-features',liveAvailabilityChecked:false,clipped:false}};
  }
  if(['project','mosaic','rgb'].includes(kind))plan.project={id:projectId,name:'Coastal monitoring',mode:kind==='project'?'create':'existing',committed:false,sceneCount:2};
  if(kind==='project'||kind==='download')plan.files=sceneIds.map(itemId=>({...plan.files[0],itemId}));
  if(kind==='clip')plan.polygon={sha256:hash(99),bounds:sourceBounds};
  if(kind==='rgb'){plan.files=['red','green','blue'].map(assetKey=>({...plan.files[0],assetKey}));plan.expectedBytes=null;plan.processing=processing;}
  if(kind==='mosaic'){plan.files[0].assetKey='ndvi';plan.processing={...processing,product:'modis-13q1-v061',dataType:'Int16',channels:1,rawBytes:12,
    quality:{...processing.quality,product:'modis-13q1-v061',policy:'good',excludeSnow:false,coupled:true,sourceCount:4}};}
  return plan;
}
const plans=['clip','download','project','mosaic','rgb'].map((kind,index)=>makePlan(kind,index+10));
for (const [kind,index] of [['project',20],['download',21]]) {
  const plan=makePlan(kind,index);
  plan.source='Selected custom raster assets';
  plan.project={id:projectId,name:'Custom original assets',mode:kind==='project'?'create':'existing',committed:false,sceneCount:2};
  plan.files=['science-band-a','science-band-b'].map((originalAssetKey,n)=>({itemId:'Same native catalog item',assetKey:kind==='project'?'scene':'stac_asset',originalAssetKey,snapshotId:hash(200+n),referenceId:hash(300+n),date:'2026-09-05T00:00:00Z',bytes:kind==='project'?null:1000}));
  plans.push(plan);
}
plans.push(makePlan('vector',22));
const coveragePlans=[];
for(const [kind,index] of [['project',23],['download',24]]) {
  const plan=makePlan(kind,index);plan.source='Selected WCS coverage subsets';plan.expectedBytes=null;
  plan.project={id:projectId,name:'WCS native coverage',mode:kind==='project'?'create':'existing',committed:false,sceneCount:1};
  plan.files=[401,402].map(index=>({itemId:`native:coverage-${index}`,assetKey:kind==='project'?'scene':'wcs_coverage',referenceId:hash(index),coveragePlanId:hash(index),bytes:null,width:48,height:48,crs:'EPSG:4326',requestedBounds:sourceBounds,alignedBounds:sourceBounds,selection:'bbox-native-grid'}));
  coveragePlans.push(plan);
}
function draftFor(plan){
  const common={planId:plan.planId,planHash:plan.planHash,kind:plan.kind};
  if (plan.files?.[0]?.referenceId) {
    const parameters={kind:plan.kind,itemIds:plan.files.map(file=>file.referenceId)};
    const fields={items:plan.files.map(file=>({id:file.referenceId,label:file.coveragePlanId?file.itemId:`${file.itemId} · ${file.originalAssetKey}`,date:file.date,locked:false}))};
    if(plan.kind==='project'){Object.assign(parameters,{name:plan.project.name,bounds:sourceBounds,keepPolygon:false});Object.assign(fields,{name:true,bounds:true,polygon:false});}
    return validatePlanRevisionDraft({...common,parameters,fields,boundsCrs:'EPSG:4326',projectId:plan.kind==='download'?projectId:null});
  }
  if(plan.kind==='vector')return validatePlanRevisionDraft({...common,parameters:{kind:'vector',name:plan.vectorReview.name,bounds:sourceBounds,keepPolygon:false},fields:{name:true,bounds:true,polygon:false},boundsCrs:'EPSG:4326'});
  const shapes={
    clip:{parameters:{kind:'clip',name:'Coastal crop',bounds:sourceBounds,keepPolygon:true},fields:{name:true,bounds:true,polygon:true},boundsCrs:'EPSG:4326'},
    download:{parameters:{kind:'download',itemIds:sceneIds},fields:{items:items.map(item=>({...item,assetKey:'visual'}))},projectId:null},
    project:{parameters:{kind:'project',name:'Coastal monitoring',bounds:sourceBounds,itemIds:sceneIds,keepPolygon:false},fields:{name:true,bounds:true,polygon:false,items},boundsCrs:'EPSG:4326',projectId:null},
    mosaic:{parameters:{kind:'mosaic',assetKey:'ndvi',qualityPolicy:'good'},fields:{assetKeys:['ndvi','evi'],qualityPolicies:['good','usable']},projectId},
    rgb:{parameters:{kind:'rgb',name:'Scientific RGB',qualityPolicy:'cloud_free_conservative',excludeSnow:true},fields:{name:true,snow:true,qualityPolicies:['cloud_free','cloud_free_conservative']},projectId},
  };
  return validatePlanRevisionDraft(structuredClone({...common,...shapes[plan.kind]}));
}
const toolNames={vector:'geod_vector_extract_plan',clip:'geod_clip_plan',download:'geod_download_plan',project:'geod_project_plan',mosaic:'geod_project_mosaic_plan',rgb:'geod_scientific_rgb_plan'};
const connections=[{id:uuid(1),provider:'openai',label:'OpenAI saved',model:'renderer-test-model',baseUrl:'https://api.openai.com/v1'},
  {id:uuid(2),provider:'deepseek',label:'DeepSeek saved',model:'renderer-test-model',baseUrl:'https://api.deepseek.com'},
  {id:uuid(3),provider:'custom',label:'Local compatible',model:'renderer-test-model',baseUrl:'http://127.0.0.1:9988/v1'}]
  .map(connection=>({...connection,protocol:'openai-compatible',capabilities:{...MODEL_CAPABILITIES},verification:'not-verified'}));
function fixture(){
  const value={version:1,revision:1,runtimeAvailable:true,mode:'review-first',configured:true,busy:false,model:structuredClone(connections[0]),
    registry:{version:1,selectedId:connections[0].id,connections:structuredClone(connections)},
    sessions:[{id:sessionId,title:'Coastal analysis · renderer contract fixture',status:'completed',compatible:true,modelConnectionId:connections[0].id,modelProvider:'openai',modelLabel:'OpenAI saved',modelId:'renderer-test-model'}],
    selected:{id:sessionId,status:'completed',entries:[{id:'question',type:'user',status:'completed',text:'Review the selected coastal scenes and processing parameters.'},
      {id:'vector-fixture',type:'tool',status:'completed',name:'geod_vector_inspect',summary:{kind:'vector',count:25,verified:true,sha256:hash(501)},references:[{kind:'vector',id:uuid(501),label:'Coastal boundary · renderer contract fixture'}]},
      ...plans.map(plan=>({id:plan.planId,type:'tool',status:'completed',name:plan.files?.[0]?.coveragePlanId?`geod_wcs_${plan.kind==='project'?'project':'download'}_plan`:plan.files?.[0]?.referenceId?`geod_stac_${plan.kind==='project'?'project':'download'}_plan`:toolNames[plan.kind],summary:{kind:'plan',status:'pending'},references:[{kind:'plan',id:plan.planId,label:plan.kind}]})),
      {id:'answer',type:'assistant',status:'completed',text:'The reviews are ready. You can adjust their parameters before confirming. Source files remain in the local workspace.'}]},plans:structuredClone(plans)};
  return validateAgentSnapshot(value);
}
let backend, browser, page, origin;
const snapshot=()=>validateAgentSnapshot(structuredClone(backend));
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin), filename=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(filename.startsWith(renderer+path.sep));
    const bytes=await readFile(filename);
    res.writeHead(200,{'content-type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[path.extname(filename)]||'application/octet-stream','content-security-policy':csp}).end(bytes);
  }catch{res.writeHead(404).end();}
});
const listCommands=['list_jobs','list_projects','list_recipes','list_vectors','list_feature_services','list_map_services','list_map_images','list_tile_sources','list_tile_packages','list_stac_connections','list_wcs_connections','list_three_d'];
async function invoke(command,args={}){
  commands.push({command,...(command==='agent_approve_plan'?{args:structuredClone(args)}:{})});
  if(command==='activate_desktop_frame')return 'custom';
  if(['set_desktop_locale','set_desktop_appearance'].includes(command))return null;
  if(listCommands.includes(command))return [];
  if(command==='health')return {status:'ok',desktop:true};
  if(command==='get_proxy_settings')return {mode:'system',url:null};
  if(command==='list_provider_accounts')return ['nasa-earthdata','copernicus'].map(provider=>({provider,status:'not-connected',expiresAt:null,verifiedAt:null}));
  if(command==='agent_snapshot')return snapshot();
  if(command==='agent_select'){assert.equal(args.id,sessionId);return snapshot();}
  if(command==='agent_save_model'){
    const request=args.request;
    if(request.action==='select'){
      const selected=backend.registry.connections.find(connection=>connection.id===request.id);assert(selected);
      backend.registry.selectedId=selected.id;backend.model=selected;backend.sessions[0].compatible=selected.id===connections[0].id;
    }else if(request.action==='delete'){
      backend.registry.connections=backend.registry.connections.filter(connection=>connection.id!==request.id);
      assert(backend.registry.connections.length);backend.model=backend.registry.connections[0];backend.registry.selectedId=backend.model.id;
      backend.sessions[0].compatible=backend.model.id===connections[0].id;
    }else if(request.action==='save'){
      const connection={id:request.id??uuid(4),provider:request.provider,label:request.label,protocol:request.protocol,baseUrl:request.baseUrl,model:request.model,
        capabilities:{...MODEL_CAPABILITIES},verification:'not-verified'};
      backend.registry.connections=backend.registry.connections.filter(entry=>entry.id!==connection.id).concat(connection);backend.registry.selectedId=connection.id;backend.model=connection;
      backend.sessions[0].compatible=connection.id===connections[0].id;
    }else throw Error('Unexpected registry fixture action');
    backend.revision++;return snapshot();
  }
  if(command==='agent_approve_plan'){
    assert.equal(args.sessionId,sessionId);
    const original=backend.plans.find(plan=>plan.planId===args.planId);assert(original);assert.equal(original.planHash,args.planHash);
    assert(['pending','expired'].includes(original.status));
    if(args.action==='draft')return draftFor(original);
    assert.equal(args.action,'revise','Renderer review must never submit a real or fixture approval.');
    const expected={
      vector:{kind:'vector',name:'Revised coastal review',bounds:[sourceBounds[0],sourceBounds[1],-122.45,sourceBounds[3]],keepPolygon:false},
      clip:{kind:'clip',name:'Revised coastal review',bounds:[sourceBounds[0],sourceBounds[1],-122.45,sourceBounds[3]],keepPolygon:true},
      download:{kind:'download',itemIds:[sceneIds[0]]},
      project:{kind:'project',name:'Revised coastal review',bounds:[sourceBounds[0],sourceBounds[1],-122.45,sourceBounds[3]],keepPolygon:false,itemIds:[sceneIds[0]]},
      mosaic:{kind:'mosaic',assetKey:'evi',qualityPolicy:'usable'},
      rgb:{kind:'rgb',name:'Revised coastal review',qualityPolicy:'cloud_free',excludeSnow:false},
    }[original.kind];
    if(original.files?.[0]?.referenceId)expected.itemIds=[original.files?.[0]?.referenceId];
    assert.deepEqual(args.revision,expected,'Form must submit exactly the selected editable contract fields.');
    const next=structuredClone(original);
    next.planId=uuid(Number.parseInt(original.planId.slice(0,8),10)+100);next.planHash=hash(Number.parseInt(next.planId.slice(0,8),10));
    if(args.revision.bounds)next.bounds=args.revision.bounds;
    if(args.revision.itemIds)next.files=next.files.filter(file=>args.revision.itemIds.includes(file.referenceId || file.itemId));
    if(args.revision.assetKey)next.files=next.files.map(file=>({...file,assetKey:args.revision.assetKey}));
    if(args.revision.name&&next.kind==='vector')next.vectorReview.name=args.revision.name;
    if(args.revision.name&&next.project&&next.kind==='project')next.project.name=args.revision.name;
    if(args.revision.qualityPolicy)next.processing.quality.policy=args.revision.qualityPolicy;
    if(args.revision.excludeSnow!==undefined)next.processing.quality.excludeSnow=args.revision.excludeSnow;
    if(args.revision.keepPolygon===false)delete next.polygon;
    // The native snapshot bounds review cards to ten; retain this predecessor
    // for its comparison and omit previously checked superseded fixture cards.
    backend.plans=backend.plans.filter(plan=>plan.planId===original.planId||plan.status!=='superseded');
    original.status='superseded';original.replacedBy=next.planId;backend.plans.push(next);backend.revision++;
    backend.selected.entries.push({id:`revision-${next.planId}`,type:'tool',status:'completed',name:'geod_plan_status',
      summary:{kind:'plan',status:'pending',revisionOf:original.planId},references:[{kind:'plan',id:next.planId,label:'Revised review'}]});
    report.interactions.push({kind:original.kind,rendererContractFixture:true,action:'revise',submitted:structuredClone(args.revision),originalPlanId:original.planId,replacementPlanId:next.planId});
    return snapshot();
  }
  throw Error('Unavailable in closed renderer contract fixture: '+command);
}
async function settle(){await page.evaluate(async()=>Promise.all(document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{}))));}
async function capture(name,locale,width,theme){
  await settle();
  const overflow=await page.evaluate(()=>({document:document.documentElement.scrollWidth-innerWidth,
    agent:[...document.querySelectorAll('.agent-panel,.agent-composer,.agent-model-form,.agent-revision-form,[role=dialog]')].map(element=>({className:element.className,overflow:element.scrollWidth-element.clientWidth}))}));
  assert(overflow.document<=1,JSON.stringify(overflow));for(const check of overflow.agent)assert(check.overflow<=1,JSON.stringify(check));
  const dialogs=await page.locator('[role=dialog]').evaluateAll(elements=>elements.map(element=>{const r=element.getBoundingClientRect();return{x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:r.width,height:r.height};}));
  const viewport=page.viewportSize();for(const dialog of dialogs)assert(dialog.x>=0&&dialog.y>=0&&dialog.right<=viewport.width+1&&dialog.bottom<=viewport.height+1,JSON.stringify(dialog));
  const file=`${name}-${width}-${locale}-${theme}.png`;await page.screenshot({path:path.join(output,'screenshots',file)});
  report.cases.push({name,locale,width,theme,screenshot:file,overflow,dialogs});
}
async function chooseSelect(locator,option){await locator.click();await page.getByRole('option',{name:option,exact:true}).click();}
async function alignedButtons(locator,{sameHeight=true}={}){
  const bounds=await locator.evaluateAll(elements=>elements.map(element=>{const r=element.getBoundingClientRect();const icon=element.querySelector('svg')?.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height,iconOffset:icon?Math.abs(icon.y+icon.height/2-r.y-r.height/2):0};}));
  assert(bounds.length>=2);for(const r of bounds){if(sameHeight)assert(Math.abs(r.height-bounds[0].height)<1,JSON.stringify(bounds));assert(Math.abs(r.y+r.height/2-bounds[0].y-bounds[0].height/2)<1,JSON.stringify(bounds));assert(r.iconOffset<1,JSON.stringify(bounds));}
  return bounds;
}
try{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin=`http://127.0.0.1:${server.address().port}`;
  browser=await chromium.launch({channel:'msedge',headless:true});
  // Four complete combinations exercise both locales and themes at both widths.
  for(const [width,height,locale,theme] of [[1440,960,'en','light'],[900,720,'zh-CN','dark'],[1440,960,'zh-CN','light'],[900,720,'en','dark']]){
    backend=fixture();const commandStart=commands.length;
    const context=await browser.newContext({viewport:{width,height}});
    const label=(en,zh)=>locale==='en'?en:zh;
    await context.exposeBinding('__agentRendererFixture',(_,command,args)=>invoke(command,args));
    await context.addInitScript(({locale,theme})=>{
      localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));
      window.__TAURI__={core:{invoke:(command,args)=>window.__agentRendererFixture(command,args)}};
      window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));
    },{locale,theme});
    await context.route('**/*',route=>{
      if(new URL(route.request().url()).origin===origin)return route.continue();
      report.externalRequests++;return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({error:'External requests blocked during renderer contract review.'})});
    });
    page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));
    await page.goto(origin+'/#Tasks');await page.getByRole('button',{name:label('Open Agent','打开 Agent'),exact:true}).click();
    await page.locator('.agent-message-assistant').waitFor();
    assert.equal(await page.locator('.agent-plan').count(),plans.length);
    const headerBounds=await alignedButtons(page.locator('.agent-header-actions button'));
    report.interactions.push({locale,width,theme,action:'header-icons-aligned',bounds:headerBounds});
    await capture('conversation',locale,width,theme);
    await page.getByRole('button',{name:new RegExp(label('Verify vector file','校验矢量文件'))}).click();
    const vectorLink=page.getByRole('link',{name:'Coastal boundary · renderer contract fixture',exact:true});await vectorLink.waitFor();
    assert.equal(await vectorLink.getAttribute('href'),`#Workspace?vector=${uuid(501)}`);
    await capture('verified-vector-result',locale,width,theme);
    await page.getByRole('combobox',{name:label('Agent model selection','Agent 模型选择')}).click();
    for(const group of ['OpenAI','DeepSeek',label('Custom compatible connection','自定义兼容连接')])await page.getByRole('group',{name:group,exact:true}).waitFor();
    await capture('connection-groups',locale,width,theme);await page.keyboard.press('Escape');
    await page.getByRole('button',{name:label('Agent model connection','Agent 模型连接'),exact:true}).click();
    const connection=page.getByRole('dialog',{name:label('Agent model connection','Agent 模型连接'),exact:true});await connection.waitFor();
    assert.equal(await connection.locator('input[type=password]').inputValue(),'');
    await capture('connection-dialog',locale,width,theme);
    const formBounds=await connection.locator('input:not([type=password]),.bui-select').evaluateAll(elements=>elements.map(element=>{const r=element.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height};}));
    for(const r of formBounds)assert(r.width>0&&r.height>=28);
    await chooseSelect(connection.getByRole('combobox',{name:label('Saved connection','已保存的连接')}),'DeepSeek saved · renderer-test-model');
    assert.equal(await connection.getByLabel(label('API endpoint','接口地址'),{exact:true}).inputValue(),'https://api.deepseek.com');
    assert.equal(await connection.locator('input[type=password]').inputValue(),'');
    const saveBefore=commands.filter(entry=>entry.command==='agent_save_model').length;
    await connection.getByRole('button',{name:label('Remove connection','移除连接'),exact:true}).click();
    await connection.getByRole('button',{name:label('Confirm removal','确认移除'),exact:true}).waitFor();
    assert.equal(commands.filter(entry=>entry.command==='agent_save_model').length,saveBefore);
    await capture('connection-removal-prompt',locale,width,theme);
    await connection.getByRole('button',{name:label('Cancel','取消'),exact:true}).click();
    assert.equal(backend.registry.connections.length,3);
    for(const original of [...plans,...coveragePlans]){
      if(original.files?.[0]?.coveragePlanId){
        backend.plans=[structuredClone(original)];backend.selected.entries.push({id:original.planId,type:'tool',status:'completed',name:`geod_wcs_${original.kind==='project'?'project':'download'}_plan`,references:[{kind:'plan',id:original.planId,label:'WCS review fixture'}]});backend.revision++;
      }
      const card=page.locator(`[data-plan-id="${original.planId}"]`);
      await card.scrollIntoViewIfNeeded();await alignedButtons(card.locator('.agent-plan-actions button'),{sameHeight:false});
      await card.getByRole('button',{name:label('Edit review parameters','修改计划参数'),exact:true}).click();
      let dialog=page.getByRole('dialog',{name:label('Edit review parameters','修改计划参数'),exact:true});await dialog.waitFor();
      const expected=draftFor(original);
      if(expected.fields.name)assert.equal(await dialog.getByLabel(label('Output name','输出名称'),{exact:true}).inputValue(),expected.parameters.name);
      if(expected.fields.bounds)assert.equal(await dialog.locator('input[type=number]').count(),4);
      if(expected.fields.items)assert.equal(await dialog.getByRole('switch').count(),expected.fields.items.length);
      if(expected.fields.qualityPolicies)await dialog.getByRole('combobox',{name:label('Quality policy','质量规则')}).waitFor();
      await alignedButtons(dialog.locator('.agent-model-actions button'));
      await capture(`revision-${original.files?.[0]?.coveragePlanId?'coverage-':original.files?.[0]?.referenceId?'custom-':''}${original.kind}-draft`,locale,width,theme);
      await dialog.getByRole('button',{name:label('Cancel','取消'),exact:true}).click();
      assert.equal(backend.plans.find(plan=>plan.planId===original.planId).status,'pending');
      assert.equal(commands.slice(commandStart).filter(entry=>entry.command==='agent_approve_plan'&&entry.args.action==='revise'&&entry.args.planId===original.planId).length,0);
      await card.getByRole('button',{name:label('Edit review parameters','修改计划参数'),exact:true}).click();
      dialog=page.getByRole('dialog',{name:label('Edit review parameters','修改计划参数'),exact:true});await dialog.waitFor();
      if(expected.fields.name)await dialog.getByLabel(label('Output name','输出名称'),{exact:true}).fill('Revised coastal review');
      if(expected.fields.bounds)await dialog.getByLabel(label('Maximum X','最大 X'),{exact:true}).fill('-122.45');
      if(expected.fields.items)await dialog.getByRole('switch',{name:expected.fields.items[1].label || sceneIds[1],exact:true}).click();
      if(expected.fields.assetKeys)await chooseSelect(dialog.getByRole('combobox',{name:label('Source asset','源数据类型')}),'EVI');
      if(expected.fields.qualityPolicies){
        const policy=original.kind==='rgb'?label('Landsat standard QA','Landsat 标准 QA'):label('VI good + marginal observations','VI 优质与可用观测');
        await chooseSelect(dialog.getByRole('combobox',{name:label('Quality policy','质量规则')}),policy);
      }
      if(expected.fields.snow)await dialog.getByRole('switch',{name:label('Exclude snow','剔除积雪'),exact:true}).click();
      await dialog.getByRole('button',{name:label('Save and review again','保存并重新审查'),exact:true}).click();
      await dialog.waitFor({state:'detached'});
      const replacement=backend.plans.find(plan=>plan.planId===backend.plans.find(plan=>plan.planId===original.planId).replacedBy);assert(replacement);
      await card.getByText(label('Replaced by revised review','已由修正计划替代'),{exact:true}).waitFor();
      assert.equal(await card.getByRole('button',{name:label('Edit review parameters','修改计划参数'),exact:true}).count(),0);
      assert.equal(await card.locator('button[data-variant=primary]').count(),0);
      const revised=page.locator(`[data-plan-id="${replacement.planId}"]`);await revised.waitFor();await revised.scrollIntoViewIfNeeded();
      await capture(`revision-${original.files?.[0]?.coveragePlanId?'coverage-':original.files?.[0]?.referenceId?'custom-':''}${original.kind}-superseded`,locale,width,theme);
      assert.equal(commands.filter(entry=>entry.command==='agent_approve_plan'&&entry.args.action!=='draft'&&entry.args.action!=='revise').length,0);
    }
    // Explicitly synthetic authorization states. These checks never open an
    // account connection, use a credential or execute a protected download.
    for(const [index,format,assetKey,provider] of [[701,'SAFE ZIP','product','copernicus'],[702,'HGT ZIP','srtm','nasa-earthdata'],[703,'HDF5','viirs','nasa-earthdata'],[704,'GeoTIFF','red','nasa-earthdata']]) {
      const protectedPlan={...makePlan('download',index),source:'Protected source · renderer fixture',format,
        files:[{itemId:'Native product identity · renderer fixture',assetKey,bytes:null}],expectedBytes:null,
        authorization:{provider,status:'not-connected',expiresAt:null,verifiedAt:null,downloadEnabled:false,entitlement:'not-checked'}};
      backend.plans=[protectedPlan];backend.selected.entries.push({id:protectedPlan.planId,type:'tool',status:'completed',name:'geod_project_download_plan',references:[{kind:'plan',id:protectedPlan.planId,label:'Protected review fixture'}]});backend.revision++;
      snapshot();
      const card=page.locator(`[data-plan-id="${protectedPlan.planId}"]`);await card.waitFor();await card.scrollIntoViewIfNeeded();
      const confirm=card.getByRole('button',{name:label('Confirm download','确认下载'),exact:true});
      await card.getByText(label('Authorization required','需要授权'),{exact:true}).waitFor();
      assert(await confirm.isDisabled());assert(!(await card.textContent()).includes('NaN'));
      assert((await card.locator('.agent-plan-facts').first().textContent()).includes(format));
      await card.getByText(label('Size available after download','下载后显示大小'),{exact:true}).waitFor();
      const accountAction=card.getByRole('link',{name:provider==='copernicus'?label('Manage Copernicus authorization','管理 Copernicus 授权'):label('Manage NASA Earthdata authorization','管理 NASA Earthdata 授权'),exact:true});
      assert.equal(await accountAction.getAttribute('href'),'#Settings?account='+provider);
      await capture(`protected-${assetKey}-not-connected-fixture`,locale,width,theme);
      if(assetKey==='viirs')for(const [state,expiresAt,blocked] of [['saved','2099-01-01T00:00:00Z',false],['expired','2000-01-01T00:00:00Z',true]]) {
        Object.assign(protectedPlan.authorization,{status:'saved',downloadEnabled:true,verifiedAt:'2026-10-01T00:00:00Z',expiresAt});backend.revision++;snapshot();
        await card.getByText(blocked?label('Authorization required','需要授权'):label('Authorization saved','已保存授权'),{exact:true}).waitFor();
        assert.equal(await confirm.isDisabled(),blocked);
        await capture(`protected-viirs-${state}-fixture`,locale,width,theme);
      }
      assert.equal(commands.filter(entry=>entry.command==='agent_approve_plan'&&entry.args.action!=='draft'&&entry.args.action!=='revise').length,0);
    }
    const submitted=structuredClone(plans.find(plan=>plan.kind==='vector'));backend.plans=[submitted];backend.selected.entries.push({id:'vector-result-fixture',type:'tool',status:'completed',name:'geod_plan_status',references:[{kind:'plan',id:submitted.planId,label:'Vector result fixture'}]});
    submitted.status='submitted';
    submitted.vector={id:uuid(601),name:submitted.vectorReview.name,format:'geojson',bytes:2000,featureCount:25,coordinateCount:100,sourceSha256:hash(601),geojsonSha256:hash(602),verified:true};
    backend.revision++;
    const vectorResult=page.getByRole('link',{name:label('Open in workspace','在工作空间打开'),exact:true});
    await vectorResult.waitFor();await vectorResult.scrollIntoViewIfNeeded();
    assert.equal(await vectorResult.getAttribute('href'),'#Workspace?vector='+uuid(601));
    await capture('vector-submitted-result-fixture',locale,width,theme);
    for(const [protocol,format,pageSize,responseFormat,selection] of [
      ['WFS 2','wfs-snapshot',1,'gml32','wfs-bbox-full-features'],
      ['ArcGIS','geojson',2,null,'bbox-full-features'],
      ['Overpass','overpass-json',null,null,'overpass-bbox-full-geometry'],
    ]){
      Object.assign(submitted.vectorReview,{protocol,pageSize,responseFormat,selection});
      submitted.vector.format=format;backend.revision++;
      snapshot();
      const protocolCard=page.locator(`[data-plan-id="${submitted.planId}"]`);
      await protocolCard.locator('.agent-plan-source').getByText(new RegExp(protocol+'$')).waitFor();
      await protocolCard.getByText(label('File saved','已保存'),{exact:true}).waitFor();
      await protocolCard.scrollIntoViewIfNeeded();
      assert.equal(await protocolCard.locator('button[data-variant=primary]').count(),0);
      assert.equal(await protocolCard.getByRole('link',{name:label('Open in workspace','在工作空间打开'),exact:true}).getAttribute('href'),'#Workspace?vector='+uuid(601));
      await capture(`vector-${format}-${protocol.replaceAll(' ','-')}-result-fixture`,locale,width,theme);
    }
    // A connection switch clears the active transcript, while its saved history remains visible.
    await chooseSelect(page.getByRole('combobox',{name:label('Agent model selection','Agent 模型选择')}),'DeepSeek saved · renderer-test-model');
    await page.locator('.agent-plan').waitFor({state:'detached'});
    assert.equal(backend.model.provider,'deepseek');assert.equal(backend.sessions.length,1);
    await page.getByRole('button',{name:label('Conversation history','历史会话'),exact:true}).click();
    await page.locator('.agent-history button').first().waitFor();
    assert((await page.locator('.agent-history button').first().textContent()).includes('Coastal analysis · renderer contract fixture'));
    await capture('connection-switched-history',locale,width,theme);
    report.cspErrors.push(...await page.evaluate(()=>window.__CSP_ERRORS));
    report.interactions.push({locale,width,theme,action:'fixture-complete',commands:commands.slice(commandStart).map(({command,args})=>({command,...(args?{action:args.action,kind:args.revision?.kind}:{})}))});
    await context.close();
  }
  assert.equal(report.errors.length,0,report.errors.join('\n'));assert.equal(report.cspErrors.length,0,report.cspErrors.join('\n'));
  report.status='passed';await writeFile(path.join(workspace,'.verification','agent-revision-renderer-latest.json'),JSON.stringify({directory:output,status:report.status},null,2));
  console.log(JSON.stringify({status:report.status,evidence:report.evidence,cases:report.cases.length,output}));
}catch(error){report.status='failed';report.failure=error.stack;if(page)await page.screenshot({path:path.join(output,'screenshots','failure.png')}).catch(()=>{});throw error;
}finally{
  await browser?.close();await new Promise(resolve=>server.close(resolve));
  await writeFile(path.join(output,'verification.json'),JSON.stringify(report,null,2));
}
