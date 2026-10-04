// Actual NAIP files in the production renderer, with the desktop CSP.
// Partial mode only reads the running verifier's store. No user windows.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {createReadStream} from 'node:fs';
import {mkdir,readFile,stat,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {chromium} from 'playwright';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4608),uiPort=Number(process.argv[4]||4609);
const partial=process.argv.includes('--partial');
const snapshotIndex=process.argv.indexOf('--snapshot-root'),snapshot=snapshotIndex>=0;
assert(!(partial&&snapshot),'Partial and completed-snapshot modes are distinct');
const dataRoot=snapshot?path.resolve(process.argv[snapshotIndex+1]):root;
assert.equal(path.dirname(root),path.join(workspace,'.verification'));
assert(path.basename(root).startsWith('naip-resolutions-'));
assert.equal(path.dirname(dataRoot),path.join(workspace,'.verification'));
assert(path.basename(dataRoot).startsWith('naip-resolutions-'));
const source=JSON.parse(await readFile(path.join(root,'native-resolution-verification.json'),'utf8'));
const independent=JSON.parse(await readFile(path.join(root,'independent-resolution-verification.json'),'utf8'));
assert(partial||snapshot||source.status==='passed'&&independent.status==='passed');
assert.equal(source.nativeBinarySha256,independent.nativeBinarySha256);
let entries=[...source.originals.filter(entry=>independent.originals.some(proof=>proof.jobId===entry.job.id&&proof.sha256===entry.job.sha256)),
  ...source.outputs.filter(entry=>independent.outputs.some(proof=>proof.jobId===entry.job.id&&proof.sha256===entry.job.sha256))];
if(snapshot){
  assert.notEqual(dataRoot,root,'The snapshot must be separate from the active acquisition store');
  const proof=JSON.parse(await readFile(path.join(root,'subset-cache-verification.json'),'utf8'));
  assert.equal(proof.status,'passed');assert.equal(proof.group,'1m');assert.equal(proof.nativeBinarySha256,source.nativeBinarySha256);
  assert.equal(proof.filesRestored,5);assert.equal(proof.projectsRestored,3);
  entries=entries.filter(entry=>proof.entries.some(pin=>pin.jobId===entry.job.id&&pin.sha256===entry.job.sha256));
  assert.equal(entries.length,5);assert(entries.every(entry=>entry.group==='1m'));
}
assert(entries.length>0,'No independently accepted files are available for UI verification');
const output=path.join(root,snapshot?'ui-snapshot':'ui');await mkdir(output,{recursive:true});
const base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const report={schema:'geod-naip-resolution-ui/v1',checkedAt:new Date().toISOString(),nativeBinarySha256:source.nativeBinarySha256,
  renderer:'production build; exact desktop CSP; real native API via loopback bridge',readOnly:partial,
  nativeWindowTested:false,usedUserDesktop:false,cases:[],createdByUi:[],errors:[],remoteRequests:[]};
if(snapshot)Object.assign(report,{scope:'Actual 1 m files in a verified offline local snapshot; real processing buttons; no new remote download or 0.3 m acceptance.',group:'1m',snapshotFiles:5});
const frontendFiles=new Map();
let runtime,browser,runtimeError='';
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.woff2':'font/woff2','.woff':'font/woff'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});
    const bytes=await readFile(file);frontendFiles.set(path.relative(path.join(workspace,'prototype/dist'),file).replaceAll(path.sep,'/'),createHash('sha256').update(bytes).digest('hex'));
    res.end(bytes);
  }catch{res.writeHead(404);res.end();}
});
async function fileHash(file){const hash=createHash('sha256');for await(const part of createReadStream(file))hash.update(part);return hash.digest('hex');}
async function api(route){const response=await fetch(base+route,{signal:AbortSignal.timeout(120000)});const result=await response.json();assert(response.ok,JSON.stringify(result));return result;}
async function ready(){const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(dataRoot));}
async function settled(id){
  for(let i=0;i<600;i++){
    const job=await api('/jobs/'+id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(job.status==='succeeded'&&job.settled)return job;
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  throw new Error('UI processing remains active: '+id);
}
async function stablePaint(page,selector){
  await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations()
    .filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{})));});
  await page.waitForFunction(selector=>[...document.querySelectorAll(selector)].every(element=>{
    for(let ancestor=element;ancestor;ancestor=ancestor.parentElement)if(Number(getComputedStyle(ancestor).opacity)<.999)return false;
    return true;
  }),selector);
  await page.evaluate(selector=>new Promise((resolve,reject)=>{
    const deadline=performance.now()+5000;let previous='',stable=0;
    const frame=()=>{
      const positions=JSON.stringify([...document.querySelectorAll(selector)].map(element=>{const r=element.getBoundingClientRect();return[r.x,r.y,r.width,r.height];}));
      stable=positions===previous?stable+1:0;previous=positions;
      if(stable>=6)return resolve();
      if(performance.now()>deadline)return reject(new Error('Layout did not reach a stable paint'));
      requestAnimationFrame(frame);
    };requestAnimationFrame(frame);
  }),selector);
}
try{
  try{await ready();assert(partial,'The completed native verifier must be stopped before full UI acceptance');}
  catch(error){
    if(partial||!(error instanceof TypeError&&error.cause?.code==='ECONNREFUSED'))throw error;
    const exe=path.join(dataRoot,`runtime-${source.nativeBinarySha256.slice(0,16)}.exe`);
    assert.equal(await fileHash(exe),source.nativeBinarySha256);
    runtime=spawn(exe,['serve','--data-dir',dataRoot,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
    runtime.stdout.on('data',()=>{});runtime.stderr.on('data',part=>runtimeError+=part);
    for(let i=0;i<100;i++){try{await ready();break;}catch(error){assert(runtime.exitCode===null&&runtime.signalCode===null,runtimeError);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
    assert.deepEqual(await api('/proxy'),{mode:'custom',url:'http://127.0.0.1:9'});
  }
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  const projects=await api('/projects');
  for(const entry of entries){
    if(snapshot){const local=await settled(entry.job.id);assert.equal(local.sha256,entry.job.sha256);assert.equal(local.itemId,entry.job.itemId);entry.job=local;}
    assert.equal((await stat(entry.job.outputPath)).size,entry.job.bytesDownloaded);
    assert.equal(await fileHash(entry.job.outputPath),entry.job.sha256);
  }
  browser=await chromium.launch({channel:'msedge',headless:true});
  // Every available processing variant is distinct; original records are
  // included separately so legacy NIR tags are exercised in the renderer.
  for(const [index,recorded]of entries.entries()){
    const caseName=recorded.case||'original',group=recorded.group;
    const project=caseName==='original'?projects.find(project=>project.scenes.some(scene=>scene.itemId===recorded.job.itemId)):
      projects.find(project=>project.id===recorded.job.mosaic.projectId);
    assert(project);
    const [width,locale,theme]=[[1440,'en','light'],[1024,'zh-CN','dark'],[900,'en','dark']][index%3];
    const label=(en,zh)=>locale==='en'?en:zh,context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],created=[];
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.exposeBinding('__naipNative',async(_context,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');if(partial)assert.equal(request.method,'GET');
      calls.push({method:request.method,path:url.pathname});
      const response=await fetch(base+url.pathname+url.search,{method:request.method,body:request.body,headers:request.headers,signal:AbortSignal.timeout(120000)}),body=Buffer.from(await response.arrayBuffer());
      if(url.pathname.endsWith('/pixel')&&response.ok)pixels.push(JSON.parse(body));
      if(url.pathname.endsWith('/mosaics')&&request.method==='POST'&&response.ok)created.push(JSON.parse(body));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{
      const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
      const result=await window.__naipNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
      return new Response(Uint8Array.from(atob(result.body),value=>value.charCodeAt(0)),{status:result.status,headers:result.headers});
    };});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push({origin:url.origin,path:url.pathname});return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#My%20Data?project='+project.id);
    await page.getByRole('heading',{name:project.name,exact:true}).waitFor({timeout:60000});
    if(snapshot&&index<2){
      const add=page.getByRole('button',{name:label('Add data from another source','添加其他来源数据'),exact:true});
      assert.equal(await add.getAttribute('aria-expanded'),'false');
      assert.equal(await page.locator('.project-source-tools').count(),0);
      await add.click();await stablePaint(page,'.project-add-source');
      const postCount=calls.filter(call=>call.method==='POST').length;
      for(const [buttonTitle,dialogTitle]of [[label('Add raster assets','添加栅格文件'),label('Custom raster sources','自定义栅格来源')],[label('Add coverage subset','添加覆盖数据子集'),label('Coverage services (WCS)','覆盖数据服务（WCS）')]]){
        const launch=page.getByRole('button',{name:buttonTitle,exact:true});await launch.click();
        const dialog=page.getByRole('dialog',{name:dialogTitle,exact:true});await dialog.waitFor();
        await page.waitForFunction(()=>{const close=document.querySelector('[role=dialog] .bui-dialog-header button');return close&&!close.disabled;});
        await page.keyboard.press('Escape');await dialog.waitFor({state:'hidden'});
        await page.waitForFunction(label=>document.activeElement?.textContent?.trim()===label,buttonTitle);
      }
      assert.equal(calls.filter(call=>call.method==='POST').length,postCount);
      await add.click();await stablePaint(page,'.project-add-source');
      assert.equal(await add.getAttribute('aria-expanded'),'false');
      (report.compactSourceAccess??=[]).push({width,locale,collapsedByDefault:true,bothDialogsAccessible:true,focusRestored:true,noSaveOrDownloadRequested:true});
    }
    let job=recorded.job;
    if(!partial&&caseName!=='original'){
      const asset=page.locator('.project-asset').filter({has:page.getByText(label('Aerial imagery · RGB + NIR','航空影像 · RGB + 近红外'),{exact:true})});
      assert.equal(await asset.count(),1);
      const action=asset.getByRole('button',{name:label(project.scenes.length===1?'Clip aerial imagery to project area':'Mosaic and clip aerial imagery',project.scenes.length===1?'按工程区域裁剪航空影像':'拼接并裁剪航空影像'),exact:true});
      assert.equal(await action.isEnabled(),true);await action.click();
      for(let i=0;!created.length&&i<300;i++)await new Promise(resolve=>setTimeout(resolve,100));
      assert.equal(created.length,1);job=await settled(created[0].id);
      assert.equal(job.sha256,recorded.job.sha256);assert.deepEqual(job.mosaicOutput,recorded.job.mosaicOutput);assert.deepEqual(job.mosaic.sources,recorded.job.mosaic.sources);
      report.createdByUi.push({group,case:caseName,id:job.id,sha256:job.sha256,pinnedSourcesIdentical:true,outputBytesIdentical:true,planIdentical:true});
    }
    const result=page.locator(`[data-layout=files] a[href*="file=${job.id}"]`);await result.waitFor({timeout:60000});
    await page.locator('[data-layout=files]').scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>{const images=[...document.querySelectorAll('[data-layout=files] [data-slot=task-row] img')];return images.length>0&&images.every(image=>image.complete&&image.naturalWidth>0);},null,{timeout:60000});
    await stablePaint(page,'[data-layout=files] [data-slot=task-row]');
    const cards=await page.locator('[data-layout=files] [data-slot=task-row]').evaluateAll(elements=>elements.map(element=>{const r=element.getBoundingClientRect();return{height:r.height,right:r.right};}));
    assert(cards.length>0&&cards.every(card=>card.right<=width+1)&&Math.max(...cards.map(card=>card.height))-Math.min(...cards.map(card=>card.height))<2);
    const prefix=`${group}-${caseName}-${index}-${width}-${locale}`;
    await page.screenshot({path:path.join(output,`project-${prefix}.png`)});
    await result.click();await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('application').focus();await page.keyboard.press('Enter');await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.equal(pixels.length,1);const [x,y]=pixels[0].coordinate;
    assert.deepEqual(pixels[0],await api(`/jobs/${job.id}/pixel?x=${x}&y=${y}`));assert.equal(pixels[0].sha256,recorded.job.sha256);
    assert.equal(pixels[0].label,'RGB + NIR');assert.equal(pixels[0].values.length,3);assert(Number.isInteger(pixels[0].nearInfrared));
    assert((await page.locator('.wm-pixel-value').textContent()).includes('NIR '+pixels[0].nearInfrared));
    assert(await page.getByText(label('Local aerial RGB overview · original RGB + NIR available','本地航空 RGB 概览 · 可读取原始 RGB + 近红外'),{exact:true}).count()>0);
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{
      const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;
      const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0;for(let i=3;i<rgba.length;i+=400)if(rgba[i]>0)count++;return count>100;
    }),null,{timeout:60000});
    await stablePaint(page,'.wm-layer');await page.screenshot({path:path.join(output,`workspace-${prefix}.png`)});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({group,case:caseName,jobId:job.id,width,locale,theme,paintedRaster:true,pixel:pixels[0],rawFourChannelsRetained:true,
      sourceExtraSample:recorded.metadata.aerial.sourceExtraSample??0,projectCards:cards.length,projectCardHeight:cards[0].height,calls});
    await context.close();console.log(JSON.stringify({stage:'ui-case',group,case:caseName,width,locale}));
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);
  if(snapshot)assert.equal(report.createdByUi.length,3);
  report.frontendFiles=[...frontendFiles].sort(([a],[b])=>a.localeCompare(b)).map(([file,sha256])=>({file,sha256}));
  report.status=!partial&&(entries.length===8||snapshot&&entries.length===5)?'passed':'partial';
  await writeFile(path.join(output,'resolution-verification.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({stage:'ui-complete',status:report.status,cases:report.cases.length,nativeWindowTested:false,usedUserDesktop:false}));
}finally{
  await browser?.close();server.close();
  if(runtime&&runtime.exitCode===null&&runtime.signalCode===null){
    const jobs=await api('/jobs').catch(()=>null);
    if(Array.isArray(jobs)&&!jobs.some(job=>['queued','running'].includes(job.status))){const ended=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await ended;}
    else runtimeError+='\nPrivate runtime retained for active or unknown jobs: '+runtime.pid;
  }
  await writeFile(path.join(output,'runtime.stderr.log'),runtimeError);
}
