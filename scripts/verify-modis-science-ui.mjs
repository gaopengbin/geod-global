// Production renderer, exact desktop CSP, actual files and native processing.
// Runs in a private store and headless Edge, leaving the user's desktop alone.
import {chromium} from 'playwright';
import {spawn,spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,copyFile,cp,readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import {MODIS_SCIENCE,MODIS_SCIENCE_KEYS} from '../prototype/src/modis-science-layers.js';
import zh from '../prototype/src/locales/modis-science.zh-CN.js';
const workspace=process.cwd(),acceptedRoot=path.resolve(process.argv[2]),root=path.resolve(process.argv[3]);
const port=Number(process.argv[4]||4644),uiPort=Number(process.argv[5]||4645),base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('modis-science-'));
await mkdir(root,{recursive:false});await mkdir(path.join(root,'assets'));await mkdir(path.join(root,'ui'));
const hash=b=>createHash('sha256').update(b).digest('hex'),fileHash=async p=>hash(await readFile(p));
const accepted=JSON.parse(await readFile(path.join(acceptedRoot,'verification.json'),'utf8'));assert.equal(accepted.status,'passed');
const binaryName=`runtime-${accepted.nativeBinarySha256.slice(0,16)}.exe`,exe=path.join(root,binaryName);await copyFile(path.join(acceptedRoot,binaryName),exe);assert.equal(await fileHash(exe),accepted.nativeBinarySha256);
const prefix=String.fromCharCode(92,92,63,92),local=v=>path.resolve(v.startsWith(prefix)?v.slice(4):v);
const entries=[...accepted.originals,...accepted.outputs],jobs={},projects=Object.fromEntries(accepted.cases.map(e=>[e.project.id,e.project]));
assert.equal(entries.length,70);
for(const e of entries){const j=structuredClone(e.job),file=local(j.outputPath),destination=path.join(root,'assets',j.id+'.tif');assert.equal(await fileHash(file),j.sha256);await copyFile(file,destination);j.outputPath=destination;
 if(j.manifestPath){const m=path.join(root,'assets',j.id+'.metadata.json');await copyFile(local(j.manifestPath),m);j.manifestPath=m;}jobs[j.id]=j;}
await writeFile(path.join(root,'jobs.json'),JSON.stringify(jobs,null,2));await writeFile(path.join(root,'projects.json'),JSON.stringify(projects,null,2));await writeFile(path.join(root,'proxy-settings.json'),JSON.stringify({mode:'custom',url:'http://127.0.0.1:9'}));
const renderer=path.join(root,'renderer');await cp(path.join(workspace,'prototype/dist'),renderer,{recursive:true,errorOnExist:true,force:false});
async function files(dir,prefix=''){const rows=[];for(const entry of await readdir(dir,{withFileTypes:true})){const name=prefix+entry.name;if(entry.isDirectory())rows.push(...await files(path.join(dir,entry.name),name+'/'));else rows.push({path:name,sha256:await fileHash(path.join(dir,entry.name))});}return rows.sort((a,b)=>a.path.localeCompare(b.path));}
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const report={schema:'geod-modis-science-ui/v1',nativeBinarySha256:accepted.nativeBinarySha256,sourceReceiptSha256:await fileHash(path.join(acceptedRoot,'verification.json')),rendererFiles:await files(renderer),csp:config.app.security.csp,nativeWindowTested:false,usedUserDesktop:false,upstreamBlocked:true,cases:[],createdByUi:[],errors:[],remoteRequests:[],status:'pending'};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';runtime.stderr.on('data',d=>stderr+=d);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{try{const url=new URL(req.url,origin),p=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));assert(p.startsWith(renderer+path.sep));const ext=path.extname(p),body=await readFile(p);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);}catch{res.writeHead(404);res.end();}});
let browser,currentPage;
async function api(route){const r=await fetch(base+route);assert(r.ok,await r.clone().text());return r.json();}
async function settle(id){for(let i=0;i<600;i++){const j=await api('/jobs/'+id);assert(!['failed','cancelled','interrupted'].includes(j.status),JSON.stringify(j));if(j.status==='succeeded'&&j.settled)return j;await new Promise(r=>setTimeout(r,100));}throw Error('UI processing did not settle');}
async function motion(page){await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});}
async function paintedAndSettled(page){
 // OpenLayers animates the map view on animation frames, not through DOM animations.
 await page.waitForFunction(()=>{
  const canvas=[...document.querySelectorAll('.wm-map canvas')].find(c=>c.width&&c.height);if(!canvas)return false;
  const ctx=canvas.getContext('2d');if(!ctx)return false;const pixels=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0,hash=0;
  for(let n=0;n<pixels.length;n+=400){hash=(Math.imul(hash,31)+pixels[n]+pixels[n+1]*3+pixels[n+2]*5+pixels[n+3]*7)|0;if(pixels[n+3]>0)count++;}
  const signature=[canvas.width,canvas.height,canvas.style.transform,hash].join('|'),old=window.__scienceMapFrame;
  window.__scienceMapFrame={signature,frames:old?.signature===signature?old.frames+1:0};return count>100&&window.__scienceMapFrame.frames>=8;
 },null,{timeout:60000});
}
try{
 await new Promise(resolve=>server.listen(uiPort,'127.0.0.1',resolve));
 for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){assert.equal(runtime.exitCode,null,stderr);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
 assert.equal((await api('/proxy')).url,'http://127.0.0.1:9');browser=await chromium.launch({channel:'msedge',headless:true});
 const layouts=[['single',1440,'en','light'],['mosaic',1024,'zh-CN','dark'],['polygon',900,'en','dark'],['temporal',1440,'zh-CN','light']];
 for(const [i,key] of MODIS_SCIENCE_KEYS.entries()){
  const [caseName,width,locale,theme]=layouts[i%4],project=accepted.cases.find(c=>c.name===caseName).project,recorded=accepted.outputs.find(e=>e.case===caseName&&e.job.assetKey===key);
  const label=(en,ch)=>locale==='en'?en:ch,t=en=>locale==='en'?en:zh[en]||en,calls=[],pixels=[],created=[],context=await browser.newContext({viewport:{width,height:960}});
  report.activeRun={case:caseName,key,calls,pixels};
  await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',e=>window.__CSP_ERRORS.push(e.violatedDirective));},{locale,theme});
  await context.exposeBinding('__scienceNative',async(_s,req)=>{const u=new URL(req.url);assert.equal(u.origin,'http://127.0.0.1:4318');calls.push({method:req.method,path:u.pathname});const r=await fetch(base+u.pathname+u.search,{method:req.method,body:req.body,headers:req.headers}),body=Buffer.from(await r.arrayBuffer());if(u.pathname.endsWith('/pixel')&&r.ok)pixels.push(JSON.parse(body));if(u.pathname.endsWith('/mosaics')&&req.method==='POST'&&r.ok)created.push(JSON.parse(body));return{status:r.status,headers:Object.fromEntries(r.headers),body:body.toString('base64')};});
  await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__scienceNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
  await context.route('**/*',route=>{const u=new URL(route.request().url());if(u.origin!==origin){report.remoteRequests.push(u.href);return route.abort();}return route.continue();});
  const page=await context.newPage();currentPage=page;page.on('pageerror',e=>report.errors.push(e.message));page.on('console',m=>{if(m.type()==='error')report.errors.push(m.text());});
  await page.goto(origin+'/#My%20Data?project='+project.id);await page.getByRole('heading',{name:project.name,exact:true}).waitFor({timeout:60000});
  const controls=page.locator('.modis-science-tools');assert.equal(await controls.count(),1);await controls.getByRole('combobox',{name:t('Scientific layer')}).click();assert.equal(await page.getByRole('option').count(),10);await page.getByRole('option',{name:t(MODIS_SCIENCE[key].label),exact:true}).click();
  const action=controls.getByRole('button',{name:t(caseName==='single'?'Clip layer':'Mosaic and clip layer'),exact:true});assert(await action.isEnabled());await action.click();
  for(let n=0;!created.length&&n<300;n++)await new Promise(r=>setTimeout(r,100));assert.equal(created.length,1);const j=await settle(created[0].id);
  assert.equal(j.assetKey,key);assert.equal(j.mosaic.projectId,project.id);assert.equal(j.sha256,recorded.job.sha256);assert.deepEqual(j.mosaicOutput,recorded.job.mosaicOutput);assert.deepEqual(j.mosaic.sources,recorded.job.mosaic.sources);
  const metadata=await api(`/jobs/${j.id}/raster`);assert.equal(metadata.previewDataUrl,recorded.metadata.previewDataUrl);assert.deepEqual(metadata.science,recorded.metadata.science);
  report.createdByUi.push({case:caseName,key,id:j.id,sha256:j.sha256,sourcePinsIdentical:true,outputBytesIdentical:true,planIdentical:true,displayIdentical:true});
  const open=page.locator(`[data-layout=files] a[href*="file=${j.id}"]`);await open.waitFor({timeout:60000});
  const source=accepted.originals.find(e=>e.job.assetKey===key&&project.scenes.some(s=>s.itemId===e.job.itemId));
  const row=page.locator('[data-layout=files] [data-slot=task-row]').filter({has:page.locator(`a[href*="file=${source.job.id}"]`)});
  await row.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();await row.getByRole('button',{name:label('Inspect raster','检查栅格'),exact:true}).click();
  const dialog=page.getByRole('dialog');await dialog.getByRole('img',{name:t('Verified local scientific layer preview'),exact:true}).waitFor({timeout:60000});
  await page.waitForFunction(()=>{const img=document.querySelector('[role=dialog] .runtime-raster-image img');return img?.complete&&img.naturalWidth>0;});assert((await dialog.innerText()).includes(recorded.metadata.dataType));assert.equal(await dialog.getByRole('heading',{name:label('Scene classes','场景分类'),exact:true}).count(),0);
  await dialog.getByRole('button',{name:t(MODIS_SCIENCE[key].label),exact:true}).click();assert((await dialog.innerText()).includes(t('Preview samples')));await motion(page);await page.screenshot({path:path.join(root,'ui',`inspection-${key}-${width}-${locale}.png`)});
  await dialog.getByRole('button',{name:label('Close raster inspection','关闭栅格检查'),exact:true}).click();await dialog.waitFor({state:'hidden'});
  // Collapsed cards must keep the shared height and full-height thumbnail at every width.
  await row.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();
  assert.equal(await row.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).getAttribute('aria-expanded'),'false');await motion(page);
  const cards=await page.evaluate(()=>[...document.querySelectorAll('[data-layout=files] [data-slot=task-row]')].map(e=>{const r=e.getBoundingClientRect(),img=e.querySelector('.runtime-file-thumbnail');return{height:r.height,right:r.right,thumbnailHeight:img?.getBoundingClientRect().height};}));
  report.activeRun.cards=cards;
  assert(cards.length>0&&cards.every(c=>c.right<=width+1)&&Math.max(...cards.map(c=>c.height))-Math.min(...cards.map(c=>c.height))<2);assert(cards.every(c=>c.thumbnailHeight>=100));
  await page.getByRole('heading',{name:project.name,exact:true}).scrollIntoViewIfNeeded();await motion(page);await page.screenshot({path:path.join(root,'ui',`project-${key}-${width}-${locale}.png`)});
  await open.click();await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
  await paintedAndSettled(page);assert.equal(await page.locator('.wm-map-caption').getByText(label('Georeferenced SCL overview · nearest-neighbour display','已配准 SCL 概览 · 最近邻显示'),{exact:true}).count(),0);
  const map=page.getByRole('application'),box=await map.boundingBox();await map.click({position:{x:box.width/2+7.25,y:box.height/2+3.375}});await page.locator('[data-science-pixel]').waitFor({timeout:60000});assert.equal(pixels.length,1);
  const pixel=pixels[0],oracle=spawnSync('python',['-X','utf8','scripts/verify-modis-science-pixel.py',local(j.outputPath),key,...pixel.coordinate.map(String)],{windowsHide:true,encoding:'utf8'});assert.equal(oracle.status,0,oracle.stderr);const expected=JSON.parse(oracle.stdout);
  assert.deepEqual(pixel.pixel,expected.pixel);assert(pixel.center.every((v,n)=>Math.abs(v-expected.center[n])<1e-7));assert.equal(pixel.value,expected.value);assert.equal(pixel.isNoData,expected.isNoData);assert.equal(pixel.science.convertedValue,expected.convertedValue);assert.equal(pixel.science.date,expected.date);assert((await page.locator('[data-science-pixel]').innerText()).includes('DN'));
  await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(c=>{const ctx=c.getContext('2d');if(!ctx||!c.width||!c.height)return false;const rgba=ctx.getImageData(0,0,c.width,c.height).data;let count=0;for(let n=3;n<rgba.length;n+=400)if(rgba[n]>0)count++;return count>100;}),null,{timeout:60000});await motion(page);await page.screenshot({path:path.join(root,'ui',`workspace-${key}-${width}-${locale}.png`)});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
  report.cases.push({case:caseName,key,width,locale,theme,paintedRaster:true,pixelInteraction:'actual interior map click',independentPixel:pixel,gdalPixel:expected,sampledScienceMetadataVisible:true,cardCount:cards.length,cardHeight:cards[0].height,thumbnailsFillUniformCards:true,calls});
  await context.close();currentPage=null;console.log(JSON.stringify({case:caseName,key,width,locale,sha256:j.sha256}));
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);delete report.activeRun;report.status='passed';console.log(JSON.stringify({status:'passed',cases:report.cases.length,created:report.createdByUi.length,rendererFiles:report.rendererFiles.length}));
}catch(e){report.status='failed';report.failure=e.stack;await currentPage?.screenshot({path:path.join(root,'ui','failure.png')}).catch(()=>{});throw e;}
finally{await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(r=>runtime.once('exit',r));}await writeFile(path.join(root,'ui','verification.json'),JSON.stringify(report,null,2));await writeFile(path.join(root,'ui','runtime.stderr.log'),stderr);}
