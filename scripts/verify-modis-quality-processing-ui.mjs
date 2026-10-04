// Real processed MODIS rasters, offline restart, persistent previews and UI actions.
// Uses a private runtime and headless Edge; never opens or controls a user window.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,appendFile,readdir,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4603),uiPort=Number(process.argv[4]||4604);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('modis-quality-processing-'));
const source=JSON.parse(await readFile(path.join(root,'native-processing-verification.json'),'utf8'));
const independent=JSON.parse(await readFile(path.join(root,'independent-processing-verification.json'),'utf8'));
assert.equal(source.status,'passed');assert.equal(independent.status,'passed');
const output=path.join(root,'ui');await mkdir(output,{recursive:true});
const origin=`http://127.0.0.1:${uiPort}`,base=`http://127.0.0.1:${port}`,config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const exe=path.join(workspace,'target/debug/geod-runtime.exe');
assert.equal(createHash('sha256').update(await readFile(exe)).digest('hex'),source.nativeBinarySha256);
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let runtimeError='';runtime.stderr.on('data',d=>runtimeError+=d);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),p=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(p.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    const ext=path.extname(p),body=await readFile(p);
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);
  }catch{res.writeHead(404);res.end();}
});
const report={schema:'geod-modis-quality-processing-ui/v1',nativeBinarySha256:source.nativeBinarySha256,
  renderer:'production build; exact desktop CSP; real native API via loopback bridge',nativeWindowTested:false,usedUserDesktop:false,
  cases:[],createdByUi:[],remoteRequests:[],errors:[]};
let browser;
async function api(route){const response=await fetch(base+route);assert(response.ok,await response.clone().text());return response.json();}
async function settled(id){
  for(let i=0;i<600;i++){
    const job=await api('/jobs/'+id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(job.status==='succeeded'&&job.settled)return job;
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  throw new Error('UI processing did not settle: '+id);
}
async function finishMotion(page){await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});}
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(error){assert.equal(runtime.exitCode,null,runtimeError);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
  const proxy=await api('/proxy');assert.equal(proxy.mode,'custom');assert.equal(proxy.url,'http://127.0.0.1:9');
  const projects=await api('/projects');for(const entry of source.cases)assert.deepEqual(projects.find(p=>p.id===entry.project.id),entry.project);
  for(const job of [...source.originals,...source.outputs.map(entry=>entry.job)]){
    const value=job.outputPath,managed=path.resolve(value.startsWith(String.fromCharCode(92,92,63,92))?value.slice(4):value);
    assert.equal(path.dirname(managed),path.join(root,'assets'));
    const bytes=await readFile(managed);assert.equal(bytes.length,job.bytesDownloaded);assert.equal(createHash('sha256').update(bytes).digest('hex'),job.sha256);
  }
  const checked=[...source.qualityOriginals.map(q=>({...q,job:{id:q.jobId}})),...source.outputs];
  const cacheDirectory=path.join(root,'cache','thumbnails','v1'),cacheBefore=new Map();
  for(const filename of await readdir(cacheDirectory)){
    if(!filename.endsWith('.json'))continue;
    const file=path.join(cacheDirectory,filename),data=await readFile(file),entry=JSON.parse(data);
    if(checked.some(item=>item.job.id===entry.preview.jobId)){
      const identity=await stat(file,{bigint:true});
      cacheBefore.set(entry.preview.jobId,{file,data,inode:identity.ino,created:identity.birthtimeNs});
    }
  }
  assert.equal(cacheBefore.size,21,'Every checked preview must already exist on disk before restart reads');
  for(const entry of checked){
    assert.deepEqual(await api(`/jobs/${entry.job.id}/raster`),entry.metadata);
    assert.deepEqual(await api(`/jobs/${entry.job.id}/thumbnail`),entry.thumbnail);
  }
  for(const cached of cacheBefore.values()){
    assert.deepEqual(await readFile(cached.file),cached.data);
    // Hits update the eviction timestamp. Atomic regeneration would change file identity.
    const identity=await stat(cached.file,{bigint:true});
    assert.equal(identity.ino,cached.inode,'A restarted hit must retain the existing disk entry');
    assert.equal(identity.birthtimeNs,cached.created,'A restarted hit must not regenerate the entry');
  }
  const pinned=source.outputs.find(entry=>entry.case==='polygon'&&entry.key==='modis_qc');
  const value=pinned.job.outputPath,managed=path.resolve(value.startsWith(String.fromCharCode(92,92,63,92))?value.slice(4):value);
  assert.equal(path.dirname(managed),path.join(root,'assets'));const bytes=await readFile(managed);
  try{
    await appendFile(managed,Buffer.from([0]));
    for(const route of ['thumbnail','raster'])assert.equal((await fetch(base+`/jobs/${pinned.job.id}/${route}`)).ok,false,'Changed derived raster must not use a cached preview');
  }finally{await writeFile(managed,bytes);}
  assert.deepEqual(await api(`/jobs/${pinned.job.id}/thumbnail`),pinned.thumbnail);
  report.offlineRestart={projectsRestored:3,originalFilesIdentical:15,outputFilesIdentical:15,
    previewsIdentical:21,thumbnailsIdentical:21,diskCacheEntriesReusedWithoutRegeneration:21,
    changedDerivedRejected:true,restoredDerivedCacheIdentical:true};
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [caseName,key,width,locale,theme]of[['single','modis_qc',1440,'en','light'],['mosaic','modis_state',1024,'zh-CN','dark'],['polygon','modis_qc',900,'en','dark']]){
    const recorded=source.outputs.find(entry=>entry.case===caseName&&entry.key===key),project=source.cases.find(entry=>entry.name===caseName).project;
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],created=[],label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',e=>window.__CSP_ERRORS.push(e.violatedDirective));},{locale,theme});
    await context.exposeBinding('__qualityNative',async(_source,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');calls.push({method:request.method,path:url.pathname});
      const response=await fetch(base+url.pathname+url.search,{method:request.method,body:request.body,headers:request.headers});const body=Buffer.from(await response.arrayBuffer());
      if(url.pathname.endsWith('/pixel')&&response.ok)pixels.push(JSON.parse(body));
      if(url.pathname.endsWith('/mosaics')&&request.method==='POST'&&response.ok)created.push(JSON.parse(body));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__qualityNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#My%20Data?project='+project.id);
    await page.getByRole('heading',{name:project.name,exact:true}).waitFor({timeout:60000});
    const asset=page.locator('.project-asset-quality').nth(key==='modis_qc'?0:1);
    const button=asset.getByRole('button',{name:label(caseName==='single'?'Clip quality to project area':'Mosaic and clip quality',caseName==='single'?'裁剪质量层':'拼接并裁剪质量层'),exact:true});
    await button.waitFor();assert.equal(await button.isEnabled(),true);await button.click();
    for(let i=0;!created.length&&i<300;i++)await new Promise(resolve=>setTimeout(resolve,100));
    assert.equal(created.length,1);const job=await settled(created[0].id);
    assert.equal(job.assetKey,key);assert.equal(job.mosaic.projectId,project.id);assert.equal(job.sha256,recorded.job.sha256);
    assert.deepEqual(job.mosaicOutput,recorded.job.mosaicOutput);assert.deepEqual(job.mosaic.sources,recorded.job.mosaic.sources);
    report.createdByUi.push({case:caseName,key,id:job.id,sha256:job.sha256,pinnedSourcesIdentical:true,outputBytesIdentical:true,planIdentical:true});
    const openResult=page.locator(`[data-layout=files] a[href*="file=${job.id}"]`);
    await openResult.waitFor({timeout:60000});
    await page.locator('[data-layout=files]').scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>{const images=[...document.querySelectorAll('[data-layout=files] [data-slot=task-row] img')];return images.length>0&&images.every(image=>image.complete&&image.naturalWidth>0);},null,{timeout:60000});
    const cards=await page.evaluate(()=>[...document.querySelectorAll('[data-layout=files] [data-slot=task-row]')].map(element=>{const rect=element.getBoundingClientRect();return{height:rect.height,right:rect.right};}));
    assert(cards.length>0&&cards.every(card=>card.right<=width+1)&&Math.max(...cards.map(card=>card.height))-Math.min(...cards.map(card=>card.height))<2);
    await finishMotion(page);await page.screenshot({path:path.join(output,`project-${caseName}-${key}-${width}-${locale}.png`)});
    await openResult.click();
    await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('application').focus();await page.keyboard.press('Enter');await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.equal(pixels.length,1);assert.equal(pixels[0].quality.layer,recorded.metadata.quality.layer);assert(pixels[0].quality.fields.length>0);
    const [x,y]=pixels[0].coordinate;assert.deepEqual(pixels[0],await api(`/jobs/${job.id}/pixel?x=${x}&y=${y}`));
    await page.getByRole('button',{name:label('Decode quality flags','解码质量标记'),exact:true}).click();
    await page.getByText(pixels[0].quality.hex,{exact:false}).first().waitFor();
    assert.equal(await page.getByRole('button',{name:label('MODLAND quality','MODLAND 产品质量'),exact:true}).count(),key==='modis_qc'?1:0);
    assert(await page.getByText(key==='modis_qc'?label('Adjacency correction','邻近效应校正'):label('Salt pan','盐滩'),{exact:true}).count()>0);
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0;for(let i=3;i<rgba.length;i+=400)if(rgba[i]>0)count++;return count>100;}),null,{timeout:60000});
    await finishMotion(page);await page.screenshot({path:path.join(output,`workspace-${caseName}-${key}-${width}-${locale}.png`)});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({case:caseName,key,width,locale,theme,paintedRaster:true,pixel:pixels[0],bitFieldsVisible:true,
      projectCards:cards.length,projectCardHeight:cards[0].height,calls});await context.close();
    console.log(JSON.stringify({case:caseName,key,width,locale,createdOutputSha256:job.sha256}));
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status='passed';
  await writeFile(path.join(output,'processing-verification.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors,remoteRequests:report.remoteRequests}));
}finally{
  await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}
  await writeFile(path.join(output,'runtime.stderr.log'),runtimeError);
}
