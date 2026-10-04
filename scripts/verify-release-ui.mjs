// Headless shared-renderer review. Runtime reads are real; window operations are
// explicit shims, so this never asserts native WebView, chrome or tray acceptance.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,copyFile,cp,readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace=process.cwd(),acceptedRoot=path.resolve(process.argv[2]),root=path.resolve(process.argv[3]);
const port=Number(process.argv[4]||4766),uiPort=Number(process.argv[5]||4767),base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('rc1-ui-'));
await mkdir(root);await mkdir(path.join(root,'assets'));await mkdir(path.join(root,'screenshots'));
const hash=b=>createHash('sha256').update(b).digest('hex'),fileHash=async p=>hash(await readFile(p));
const accepted=JSON.parse(await readFile(path.join(acceptedRoot,'verification.json'),'utf8'));assert.equal(accepted.status,'passed');
const exe=path.join(root,'geod-runtime.exe');await copyFile(path.join(workspace,'.verification/naip-native-target/debug/geod-runtime.exe'),exe);
const prefix=String.fromCharCode(92,92,63,92),local=v=>path.resolve(v.startsWith(prefix)?v.slice(4):v),jobs={};
for(const record of [...accepted.originals,...accepted.outputs]) {
  const j=structuredClone(record.job),original=local(j.outputPath);assert.equal(await fileHash(original),j.sha256);
  j.outputPath=path.join(root,'assets',j.id+'.tif');await copyFile(original,j.outputPath);
  if(j.manifestPath){const dest=path.join(root,'assets',j.id+'.metadata.json');await copyFile(local(j.manifestPath),dest);j.manifestPath=dest;}
  jobs[j.id]=j;
}
await writeFile(path.join(root,'jobs.json'),JSON.stringify(jobs,null,2));
await writeFile(path.join(root,'projects.json'),JSON.stringify(Object.fromEntries(accepted.cases.map(c=>[c.project.id,c.project]))));
await writeFile(path.join(root,'proxy-settings.json'),JSON.stringify({mode:'custom',url:'http://127.0.0.1:9'}));
const renderer=path.join(root,'renderer');await cp(path.join(workspace,'prototype/dist'),renderer,{recursive:true,errorOnExist:true,force:false});
async function files(dir,prefix=''){const out=[];for(const e of await readdir(dir,{withFileTypes:true})){const name=prefix+e.name;if(e.isDirectory())out.push(...await files(path.join(dir,e.name),name+'/'));else out.push({path:name,sha256:await fileHash(path.join(dir,e.name))});}return out.sort((a,b)=>a.path.localeCompare(b.path));}
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const report={schema:'geod-release-ui/v1',version:config.version,runtimeSha256:await fileHash(exe),rendererFiles:await files(renderer),nativeWindowTested:false,usedUserDesktop:false,windowOperations:'shims only',remoteCatalog:'controlled unavailable response; no live catalogue claim',cases:[],commands:[],errors:[],cspErrors:[],status:'pending'};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';runtime.stderr.on('data',d=>stderr+=d);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{try{const u=new URL(req.url,origin),p=path.resolve(renderer,'.'+decodeURIComponent(u.pathname==='/'?'/index.html':u.pathname));assert(p.startsWith(renderer+path.sep));const body=await readFile(p);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[path.extname(p)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);}catch{res.writeHead(404);res.end();}});
async function api(route){const r=await fetch(base+route);const body=await r.json();assert(r.ok,JSON.stringify(body));return body;}
const paths={health:'/health',list_jobs:'/jobs',list_projects:'/projects',list_recipes:'/recipes',get_proxy_settings:'/proxy',list_provider_accounts:'/accounts',list_vectors:'/vectors',list_feature_services:'/feature-services',list_map_services:'/map-services',list_map_images:'/map-images',list_tile_sources:'/tile-sources',list_tile_packages:'/tile-packages',list_stac_connections:'/stac/connections',list_wcs_connections:'/wcs/connections',list_three_d:'/three-d/packages'};
let browser,page;
async function settled(p){await p.evaluate(async()=>await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{}))));}
async function capture(name,locale,width){await settled(page);assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));const filename=`${name}-${width}-${locale}.png`;await page.screenshot({path:path.join(root,'screenshots',filename)});report.cases.push({name,width,locale,screenshot:filename});console.log(JSON.stringify({screen:name,width,locale}));}
try {
 await new Promise(r=>server.listen(uiPort,'127.0.0.1',r));for(let n=0;n<100;n++){try{await api('/health');break;}catch(e){assert.equal(runtime.exitCode,null,stderr);if(n===99)throw e;await new Promise(r=>setTimeout(r,100));}}
 browser=await chromium.launch({channel:'msedge',headless:true});
 for(const [width,height,locale,theme] of [[1440,960,'en','light'],[900,680,'zh-CN','dark']]) {
  const context=await browser.newContext({viewport:{width,height}}),label=(en,ch)=>locale==='en'?en:ch;
  await context.exposeBinding('__releaseNative',async(_s,command,args={})=>{
   report.commands.push({command,args});
   if(command==='activate_desktop_frame')return 'custom';
   if(['set_desktop_locale','set_desktop_appearance'].includes(command))return null;
   if(paths[command])return api(paths[command]);
   if(command==='file_thumbnail')return api(`/jobs/${args.id}/thumbnail`);
   if(command==='inspect_raster')return api(`/jobs/${args.id}/raster`);
   if(command==='sample_raster')return api(`/jobs/${args.id}/pixel?${new URLSearchParams({x:args.x,y:args.y})}`);
   throw Error('Unsupported review command: '+command);
  });
  await context.addInitScript(({locale,theme})=>{
   localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));
   window.__TAURI__={core:{invoke:(command,args)=>window.__releaseNative(command,args)}};
   window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',e=>window.__CSP_ERRORS.push(e.violatedDirective));
  },{locale,theme});
  await context.route('**/*',r=>{const u=new URL(r.request().url());return u.origin===origin?r.continue():r.fulfill({status:200,contentType:'application/json',body:JSON.stringify({error:'Source unavailable during isolated review'})});});
  page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message));page.on('console',m=>{if(m.type()==='error')report.errors.push(m.text());});
  await page.goto(origin+'/#My%20Data');await page.locator('.project-row').first().waitFor();assert.equal(await page.locator('.app-header').count(),1);
  await capture('projects',locale,width);
  await page.goto(origin+'/#My%20Data?view=files');await page.locator('[data-layout=files] [data-slot=task-row]').first().waitFor();
  await page.waitForFunction(()=>[...document.querySelectorAll('.runtime-file-thumbnail')].filter(e=>{const r=e.getBoundingClientRect();return r.top<innerHeight&&r.bottom>0;}).every(e=>e.querySelector('img')?.complete),null,{timeout:60000});
  await settled(page);
  const layout=await page.locator('[data-layout=files] [data-slot=task-row]').evaluateAll(rows=>rows.map(row=>{const r=row.getBoundingClientRect(),t=row.querySelector('.runtime-file-thumbnail').getBoundingClientRect();return {height:r.height,thumbnailHeight:t.height};}));
  report.cases.push({name:'file-card-geometry',width,locale,layout});
  // The outer card includes a one-pixel border on both edges.
  assert(layout.length>=20);assert(layout.every(r=>Math.abs(r.height-layout[0].height)<1&&Math.abs(r.thumbnailHeight-r.height)<=2.01));
  await capture('files',locale,width);
  const scroll=page.locator('.content-page');await scroll.evaluate(e=>e.scrollTop=300);await capture('files-scroll',locale,width);
  for(const [view,name] of [['vectors','vectors-empty'],['maps','maps-empty'],['tiles','tiles-empty'],['3d','three-d-empty']]){await page.goto(origin+'/#My%20Data?view='+view);await page.locator('.data-library-view').waitFor();await capture(name,locale,width);}
  await page.goto(origin+'/#Tasks');await page.getByRole('heading',{name:label('Tasks','任务'),exact:true}).waitFor();await capture('tasks-empty',locale,width);
  await page.locator('.tasks-page [data-slot=segmented-item]').filter({hasText:label('History','历史记录')}).click();await page.locator('.tasks-page [data-slot=task-row]').first().waitFor();await capture('task-history',locale,width);
  await page.goto(origin+'/#Settings');await page.locator('.account-card').first().waitFor();await page.waitForFunction(()=>!document.querySelector('.provider-accounts [role=status]'));
  assert.equal(await page.locator('.account-card').count(),2);await capture('settings',locale,width);
  await page.locator('.account-card').first().getByRole('button',{name:label('Connect account','连接账号'),exact:true}).click();await page.getByRole('dialog').waitFor();await capture('earthdata-entry',locale,width);
  await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'hidden'});
  await page.locator('.account-card').nth(1).getByRole('button',{name:label('Connect account','连接账号'),exact:true}).click();await page.getByRole('dialog').waitFor();await capture('copernicus-entry',locale,width);
  await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'hidden'});
  const out=accepted.outputs[0].job;await page.goto(origin+'/#Workspace?file='+out.id);await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
  await page.waitForFunction(()=>!!document.querySelector('.wm-map canvas')?.width);await capture('workspace',locale,width);
  await page.goto(origin+'/#Explore');await page.getByRole('heading',{name:label('Imagery scenes','影像列表'),exact:true}).waitFor();await page.waitForFunction(()=>!document.querySelector('.catalog-loading'));await capture('explore-unavailable',locale,width);
  const filters=page.getByRole('button',{name:label('Filters','筛选'),exact:true});if(await filters.count()){await filters.click();await page.getByRole('dialog').waitFor();await capture('filters',locale,width);assert.equal(await page.locator('input[type=date]').count(),0);}
  report.cspErrors.push(...await page.evaluate(()=>window.__CSP_ERRORS));await context.close();page=null;
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.cspErrors,[]);assert(!report.commands.some(c=>c.command==='connect_provider_account'||c.command.includes('download')));
 report.status='passed';console.log(JSON.stringify({status:'passed',screens:report.cases.length}));
} catch(e) {report.status='failed';report.failure=e.stack;await page?.screenshot({path:path.join(root,'screenshots','failure.png')}).catch(()=>{});throw e;}
finally {await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(r=>runtime.once('exit',r));}await writeFile(path.join(root,'verification.json'),JSON.stringify(report,null,2));await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
