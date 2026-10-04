// Real local QA rasters in the production renderer, with exact desktop CSP.
// A headless Edge + loopback bridge is used; no user desktop/window is touched.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,appendFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4601),uiPort=Number(process.argv[4]||4602);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('modis-quality-'));
const source=JSON.parse(await readFile(path.join(root,'native-verification.json'),'utf8'));
const output=path.join(root,'ui');await mkdir(output,{recursive:true});
const origin=`http://127.0.0.1:${uiPort}`,base=`http://127.0.0.1:${port}`,config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const runtime=spawn(path.join(workspace,'target/debug/geod-runtime.exe'),['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let runtimeError='';runtime.stderr.on('data',d=>runtimeError+=d);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),p=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(p.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    const ext=path.extname(p),body=await readFile(p);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);
  }catch{res.writeHead(404);res.end();}
});
const report={schema:'geod-modis-quality-ui/v1',renderer:'production build; exact desktop CSP; real native API via loopback bridge',nativeWindowTested:false,usedUserDesktop:false,cases:[],remoteRequests:[],errors:[]};
let browser;
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{assert((await fetch(base+'/health')).ok);break;}catch(e){assert(runtime.exitCode===null,runtimeError);if(i===99)throw e;await new Promise(r=>setTimeout(r,100));}}
  const proxy=await (await fetch(base+'/proxy')).json();assert.equal(proxy.mode,'custom');assert.equal(proxy.url,'http://127.0.0.1:9');
  const projects=await (await fetch(base+'/projects')).json();assert.deepEqual(projects.find(p=>p.id===source.project.id),source.project);
  assert.deepEqual(projects.find(p=>p.id===source.legacyUpgrade.id).scenes[0].assets,source.legacyUpgrade.assets);
  for(const item of source.cases){
    assert.deepEqual(await (await fetch(base+`/jobs/${item.job.id}/raster`)).json(),item.metadata);
    assert.deepEqual(await (await fetch(base+`/jobs/${item.job.id}/thumbnail`)).json(),item.thumbnail);
  }
  const pinned=source.cases[0],originalPath=pinned.job.outputPath,sourcePath=path.resolve(originalPath.startsWith(String.fromCharCode(92,92,63,92))?originalPath.slice(4):originalPath);assert(sourcePath.startsWith(root+path.sep));const bytes=await readFile(sourcePath);
  try{
    await appendFile(sourcePath,Buffer.from([0]));
    for(const route of ['thumbnail','raster'])assert.equal((await fetch(base+`/jobs/${pinned.job.id}/${route}`)).ok,false,'A changed original cannot use its cached preview');
  }finally{await writeFile(sourcePath,bytes);}
  assert.deepEqual(await (await fetch(base+`/jobs/${pinned.job.id}/thumbnail`)).json(),pinned.thumbnail);
  report.offlineRestart={projectsRestored:true,legacyQualityPinsRestored:true,previewsIdentical:true,thumbnailsIdentical:true,changedOriginalRejected:true};
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [key,width,locale,theme]of[['modis_qc',1440,'en','light'],['modis_state',1024,'zh-CN','dark'],['modis_qc',900,'en','dark']]){
    const recorded=source.cases.find(c=>c.key===key),context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',e=>window.__CSP_ERRORS.push(e.violatedDirective));},{locale,theme});
    await context.exposeBinding('__qualityNative',async(_source,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');calls.push({method:request.method,path:url.pathname});
      const response=await fetch(base+url.pathname+url.search,{method:request.method,body:request.body,headers:request.headers});const body=await response.arrayBuffer();
      if(url.pathname.endsWith('/pixel')&&response.ok)pixels.push(JSON.parse(Buffer.from(body)));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(body).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__qualityNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#My%20Data?project='+source.project.id);
    await page.getByRole('heading',{name:source.project.name,exact:true}).waitFor();
    await page.locator('[data-layout=files]').scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>document.querySelectorAll('[data-layout=files] [data-slot=task-row]').length===2&&document.querySelectorAll('[data-layout=files] [data-slot=task-row] img').length===2&&[...document.querySelectorAll('[data-layout=files] [data-slot=task-row] img')].every(i=>i.complete&&i.naturalWidth>0),null,{timeout:60000});
    const cards=await page.evaluate(()=>[...document.querySelectorAll('[data-layout=files] [data-slot=task-row]')].map(e=>{const r=e.getBoundingClientRect();return{height:r.height,right:r.right};}));
    assert(cards.every(c=>c.right<=width+1)&&Math.max(...cards.map(c=>c.height))-Math.min(...cards.map(c=>c.height))<2);
    await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});
    await page.screenshot({path:path.join(output,`project-${key}-${width}-${locale}.png`)});
    await page.goto(origin+'/#Workspace?file='+recorded.job.id+'&project='+source.project.id);
    await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('application').focus();await page.keyboard.press('Enter');await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.equal(pixels.length,1);assert.equal(pixels[0].quality.layer,recorded.metadata.quality.layer);assert(pixels[0].quality.fields.length>0);
    await page.getByRole('button',{name:label('Decode quality flags','解码质量标记'),exact:true}).click();
    await page.getByText(pixels[0].quality.hex,{exact:false}).first().waitFor();
    assert.equal(await page.getByRole('button',{name:label('MODLAND quality','MODLAND 产品质量'),exact:true}).count(),key==='modis_qc'?1:0);
    const fieldLabel=key==='modis_qc'?label('Adjacency correction','邻近效应校正'):label('Salt pan','盐滩');assert(await page.getByText(fieldLabel,{exact:true}).count()>0);
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0;for(let i=3;i<rgba.length;i+=400)if(rgba[i]>0)count++;return count>100;}),null,{timeout:60000});
    await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});
    await page.screenshot({path:path.join(output,`workspace-${key}-${width}-${locale}.png`)});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({key,width,locale,theme,paintedRaster:true,pixel:pixels[0],bitFieldsVisible:true,projectCardHeight:cards[0].height,calls});await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status='passed';await writeFile(path.join(output,'verification.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors,remoteRequests:report.remoteRequests}));
}finally{await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}}
