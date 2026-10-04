// Real RTC files, private restart, disk-cache identity and production UI actions.
// Headless Edge is private: no user window or desktop controls.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {createReadStream} from 'node:fs';
import {readFile,writeFile,mkdir,appendFile,readdir,stat,rename} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4605),uiPort=Number(process.argv[4]||4606);
const partial=process.argv.includes('--partial');
const completedProjects=process.argv.includes('--completed-projects'),liveOwner=partial||completedProjects;
const resume=process.argv.includes('--resume');
assert(!(partial&&completedProjects),'Use either read-only originals or processing-button subset checks');
assert(!resume||completedProjects,'Resume is limited to live-owner project checks');
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('radar-polarizations-'));
const sourceBytes=await readFile(path.join(root,completedProjects?'early-projects-native-verification.json':'native-polarizations-verification.json'));
const independentBytes=await readFile(path.join(root,completedProjects?'early-projects-independent-verification.json':'independent-polarizations-verification.json'));
const source=JSON.parse(sourceBytes),independent=JSON.parse(independentBytes);
assert(liveOwner||source.status==='passed'&&independent.status==='passed');
assert.equal(source.nativeBinarySha256,independent.nativeBinarySha256);
const accepted=new Map([...independent.originals,...independent.outputs].map(entry=>[entry.jobId,entry.sha256]));
const checked=[...source.originals,...source.outputs].filter(entry=>accepted.get(entry.job.id)===entry.job.sha256);
assert(checked.length>0,'No independently accepted real radar files are available');
const output=path.join(root,completedProjects?'ui-completed':'ui');await mkdir(output,{recursive:true});
const origin=`http://127.0.0.1:${uiPort}`,base=`http://127.0.0.1:${port}`,config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const exe=path.join(workspace,'target/debug/geod-runtime.exe');
assert.equal(createHash('sha256').update(await readFile(exe)).digest('hex'),source.nativeBinarySha256);
let runtime,runtimeError='';
const frontendFiles=new Map();
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    const body=await readFile(file),ext=path.extname(file);
    frontendFiles.set(path.relative(path.join(workspace,'prototype/dist'),file).replaceAll(path.sep,'/'),createHash('sha256').update(body).digest('hex'));
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);
  }catch{res.writeHead(404);res.end();}
});
const report={schema:'geod-radar-polarizations-ui/v1',nativeBinarySha256:source.nativeBinarySha256,
  nativeReceiptSha256:createHash('sha256').update(sourceBytes).digest('hex'),
  independentReceiptSha256:createHash('sha256').update(independentBytes).digest('hex'),
  checkedAt:new Date().toISOString(),readOnly:partial,offlineRestart:null,
  renderer:'production build; exact desktop CSP; real private native API via loopback bridge',
  nativeWindowTested:false,usedUserDesktop:false,cases:[],createdByUi:[],remoteRequests:[],errors:[],pendingCase:null};
if(partial)report.scope='Read-only production renderer of independently accepted actual originals in the active store; no restart or processing-button acceptance.';
if(completedProjects)report.scope='Actual processing buttons for independently accepted ready projects in the active store; acquisition owner is untouched and full-matrix restart remains pending.';
const checkpointFile=path.join(output,'polarizations-verification.json');
let browser;
async function hash(file){const digest=createHash('sha256');for await(const chunk of createReadStream(file))digest.update(chunk);return digest.digest('hex');}
function managed(job){const file=path.resolve(job.outputPath.startsWith(String.fromCharCode(92,92,63,92))?job.outputPath.slice(4):job.outputPath);assert.equal(path.dirname(file),path.join(root,'assets'));return file;}
async function api(route){const response=await fetch(base+route,{signal:AbortSignal.timeout(120000)});assert(response.ok,await response.clone().text());return response.json();}
async function ready(){const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));}
async function checkpoint(){
  report.checkedAt=new Date().toISOString();
  report.frontendFiles=[...frontendFiles].sort(([a],[b])=>a.localeCompare(b)).map(([file,sha256])=>({file,sha256}));
  await writeFile(checkpointFile+'.tmp',JSON.stringify(report,null,2));await rename(checkpointFile+'.tmp',checkpointFile);
}
async function settled(id){
  const deadline=Date.now()+15*60*1000;
  while(Date.now()<deadline){
    const job=await api('/jobs/'+id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(job.status==='succeeded'&&job.settled)return job;
    await new Promise(resolve=>setTimeout(resolve,1000));
  }
  throw new Error('Existing UI processing is still active: '+id);
}
async function finishMotion(page){
  await page.evaluate(async()=>{
    await document.fonts.ready;
    await Promise.all(document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{})));
  });
  await page.waitForFunction(()=>[...document.querySelectorAll('[data-slot=task-row]')].every(card=>{
    for(let element=card;element;element=element.parentElement)if(Number(getComputedStyle(element).opacity)<0.999)return false;
    return true;
  }));
  await page.evaluate(()=>new Promise((resolve,reject)=>{
    const deadline=performance.now()+5000;let previous='',stable=0;
    const frame=()=>{
      const positions=JSON.stringify([...document.querySelectorAll('[data-slot=task-row]')].map(element=>{
        const rect=element.getBoundingClientRect();return[rect.x,rect.y,rect.width,rect.height];
      }));
      stable=positions===previous?stable+1:0;previous=positions;
      if(stable>=6)return resolve();
      if(performance.now()>deadline)return reject(new Error('Task cards did not reach a stable paint'));
      requestAnimationFrame(frame);
    };requestAnimationFrame(frame);
  }));
}
try{
  try{await ready();assert(liveOwner,'The completed acquisition service must close before full UI/restart verification');}
  catch(error){
    if(liveOwner||!(error instanceof TypeError&&error.cause?.code==='ECONNREFUSED'))throw error;
    runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
    runtime.stderr.on('data',data=>runtimeError+=data);runtime.stdout.on('data',()=>{});
    for(let i=0;i<100;i++){try{await ready();break;}catch(error){assert(runtime.exitCode===null&&runtime.signalCode===null,runtimeError);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
  }
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  if(!liveOwner)assert.deepEqual(await api('/proxy'),{mode:'custom',url:'http://127.0.0.1:9'});
  const projects=await api('/projects');for(const entry of source.cases)assert.deepEqual(projects.find(project=>project.id===entry.project.id),entry.project);
  if(!liveOwner)assert.equal(checked.length,15);
  for(const entry of checked){const file=managed(entry.job);assert.equal((await stat(file)).size,entry.job.bytesDownloaded);assert.equal(await hash(file),entry.job.sha256);}
  if(resume){
    const saved=await readFile(checkpointFile,'utf8').then(JSON.parse).catch(()=>null);
    if(saved?.schema===report.schema&&saved.nativeBinarySha256===report.nativeBinarySha256
      &&saved.nativeReceiptSha256===report.nativeReceiptSha256&&saved.independentReceiptSha256===report.independentReceiptSha256
      &&saved.frontendFiles?.length){
      assert.deepEqual(saved.errors,[]);assert.deepEqual(saved.remoteRequests,[]);
      assert.equal(saved.usedUserDesktop,false);assert.equal(saved.readOnly,false);assert.equal(saved.offlineRestart,null);
      for(const entry of saved.frontendFiles){
        const file=path.resolve(workspace,'prototype/dist',entry.file);
        assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));assert.equal(await hash(file),entry.sha256);
        frontendFiles.set(entry.file,entry.sha256);
      }
      report.cases=saved.cases;report.createdByUi=saved.createdByUi.filter(entry=>saved.cases.some(scene=>scene.jobId===entry.id));
      report.pendingCase=saved.pendingCase||null;report.resumedCompletedCases=saved.cases.length;
      console.log(JSON.stringify({stage:'resume-ui-checkpoint',completedCases:saved.cases.length,pendingJob:report.pendingCase?.jobId||null}));
    }
  }
  if(!liveOwner){
  const cacheDirectory=path.join(root,'cache','thumbnails','v1'),cacheBefore=new Map();
  for(const filename of await readdir(cacheDirectory)){
    if(!filename.endsWith('.json'))continue;
    const file=path.join(cacheDirectory,filename),data=await readFile(file),entry=JSON.parse(data);
    if(checked.some(item=>item.job.id===entry.preview.jobId)){
      const identity=await stat(file,{bigint:true});cacheBefore.set(entry.preview.jobId,{file,data,inode:identity.ino,created:identity.birthtimeNs});
    }
  }
  assert.equal(cacheBefore.size,15);
  for(const entry of checked){
    assert.deepEqual(await api(`/jobs/${entry.job.id}/raster`),entry.metadata);
    assert.deepEqual(await api(`/jobs/${entry.job.id}/thumbnail`),entry.thumbnail);
  }
  for(const cached of cacheBefore.values()){
    assert.deepEqual(await readFile(cached.file),cached.data);
    // Hits update the LRU timestamp; atomic regeneration would replace identity.
    const identity=await stat(cached.file,{bigint:true});assert.equal(identity.ino,cached.inode);assert.equal(identity.birthtimeNs,cached.created);
  }
  const pinned=source.outputs.find(entry=>entry.case==='polygon'&&entry.key==='hv'),file=managed(pinned.job),bytes=await readFile(file);
  try{
    await appendFile(file,Buffer.from([0]));
    for(const route of ['thumbnail','raster'])assert.equal((await fetch(base+`/jobs/${pinned.job.id}/${route}`)).ok,false);
  }finally{await writeFile(file,bytes);}
  assert.deepEqual(await api(`/jobs/${pinned.job.id}/thumbnail`),pinned.thumbnail);
  report.offlineRestart={projectsRestored:6,originalFilesIdentical:6,outputFilesIdentical:9,previewsIdentical:15,
    thumbnailsIdentical:15,diskCacheEntriesReusedWithoutRegeneration:15,changedDerivedRejected:true,restoredDerivedCacheIdentical:true};
  }
  browser=await chromium.launch({channel:'msedge',headless:true});
  const scenarios=partial?checked.filter(entry=>entry.job.kind==='download').flatMap((entry,index)=>[[entry,1440,'en','light',index],[entry,1024,'zh-CN','dark',index]]):
    completedProjects?checked.filter(entry=>entry.job.kind==='raster_mosaic').flatMap((entry,index)=>[[entry,1440,'en','light',index],[entry,1024,'zh-CN','dark',index]]):
    source.outputs.map((entry,index)=>[entry,...[[1440,'en','light'],[1024,'zh-CN','dark'],[900,'en','dark']][index%3],index]);
  assert(scenarios.length>0);
  for(const [recorded,width,locale,theme,index]of scenarios){
    const caseName=recorded.case||'original',key=recorded.key||recorded.job.assetKey;
    const project=partial?source.cases.find(entry=>entry.keys.includes(key)&&entry.project.scenes.some(scene=>scene.itemId===recorded.job.itemId)).project:
      source.cases.find(entry=>entry.kind===caseName&&entry.keys.includes(key)).project;
    const saved=report.cases.find(entry=>entry.case===caseName&&entry.key===key&&entry.width===width&&entry.locale===locale&&entry.theme===theme);
    if(saved){
      const job=await settled(saved.jobId);assert.equal(saved.sha256,recorded.job.sha256);assert.equal(job.sha256,recorded.job.sha256);
      assert.equal(job.mosaic.projectId,project.id);assert.deepEqual(job.mosaic.sources,recorded.job.mosaic.sources);
      assert.deepEqual(job.mosaicOutput,recorded.job.mosaicOutput);assert.equal(await hash(managed(job)),recorded.job.sha256);
      assert(saved.paintedRaster&&saved.linearGamma0AndDbRetained&&saved.pixel.sha256===recorded.job.sha256);
      assert(report.createdByUi.some(entry=>entry.id===saved.jobId&&entry.case===caseName&&entry.key===key&&entry.sha256===saved.sha256));
      continue;
    }
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],created=[],label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.exposeBinding('__radarNative',async(_context,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');if(partial)assert.equal(request.method,'GET');calls.push({method:request.method,path:url.pathname});
      if(request.method!=='GET'){
        assert.equal(request.method,'POST');assert.equal(url.pathname,`/projects/${project.id}/mosaics`);
        assert.deepEqual(JSON.parse(request.body),{assetKey:key});
      }
      const response=await fetch(base+url.pathname+url.search,{method:request.method,body:request.body,headers:request.headers});const body=Buffer.from(await response.arrayBuffer());
      if(url.pathname.endsWith('/pixel')&&response.ok)pixels.push(JSON.parse(body));
      if(url.pathname.endsWith('/mosaics')&&request.method==='POST'&&response.ok)created.push(JSON.parse(body));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const response=await window.__radarNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(response.body),character=>character.charCodeAt(0)),{status:response.status,headers:response.headers});};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.origin+url.pathname);return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#My%20Data?project='+project.id);
    await page.getByRole('heading',{name:project.name,exact:true}).waitFor({timeout:60000});
    let job=recorded.job;
    let reusedUiAction=false;
    if(!partial){
    const asset=page.locator('.project-asset').filter({has:page.getByText(label('Radar backscatter · '+key.toUpperCase(),'雷达后向散射 · '+key.toUpperCase()),{exact:true})});
    assert.equal(await asset.count(),1);
    const button=asset.getByRole('button',{name:label(caseName==='single'?'Clip radar to project area':'Mosaic and clip radar',caseName==='single'?'按工程区域裁剪雷达':'拼接并裁剪雷达'),exact:true});
    const pending=report.pendingCase;
    if(pending&&pending.case===caseName&&pending.key===key&&pending.width===width&&pending.locale===locale&&pending.theme===theme){
      assert.equal(pending.projectId,project.id);assert.equal(pending.expectedSha256,recorded.job.sha256);
      assert(pending.calls.some(call=>call.method==='POST'&&call.path===`/projects/${project.id}/mosaics`));
      calls.unshift(...pending.calls);job=await settled(pending.jobId);assert.equal(created.length,0);reusedUiAction=true;
    }else{
      assert.equal(pending,null,'The saved UI action is outside this scenario');
      // The project can render before the independently fetched job list is ready.
      // Click waits for the real button to become enabled, without forcing it.
      await button.click({timeout:60000});
      for(let i=0;!created.length&&i<300;i++)await new Promise(resolve=>setTimeout(resolve,100));
      assert.equal(created.length,1);
      report.pendingCase={case:caseName,key,width,locale,theme,jobId:created[0].id,projectId:project.id,expectedSha256:recorded.job.sha256,calls:[...calls]};
      report.status='partial';await checkpoint();job=await settled(created[0].id);
    }
    assert.equal(job.assetKey,key);assert.equal(job.mosaic.projectId,project.id);assert.equal(job.sha256,recorded.job.sha256);
    assert.deepEqual(job.mosaicOutput,recorded.job.mosaicOutput);assert.deepEqual(job.mosaic.sources,recorded.job.mosaic.sources);
    report.createdByUi.push({case:caseName,key,id:job.id,sha256:job.sha256,pinnedSourcesIdentical:true,outputBytesIdentical:true,planIdentical:true,reusedAfterObserverResume:reusedUiAction});
    }
    const result=page.locator(`[data-layout=files] a[href*="file=${job.id}"]`);await result.waitFor({timeout:60000});
    await page.locator('[data-layout=files]').scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>{const images=[...document.querySelectorAll('[data-layout=files] [data-slot=task-row] img')];return images.length>0&&images.every(image=>image.complete&&image.naturalWidth>0);},null,{timeout:60000});
    const cards=await page.evaluate(()=>[...document.querySelectorAll('[data-layout=files] [data-slot=task-row]')].map(element=>{const rect=element.getBoundingClientRect();return{height:rect.height,right:rect.right};}));
    assert(cards.length>0&&cards.every(card=>card.right<=width+1)&&Math.max(...cards.map(card=>card.height))-Math.min(...cards.map(card=>card.height))<2);
    if(partial){
      const fileCard=page.locator('[data-layout=files] [data-slot=task-row]').filter({has:page.locator(`a[href*="file=${job.id}"]`)});
      assert.equal(await fileCard.getByText(new RegExp('Sentinel-1[ABC] · '+key.toUpperCase()+'$')).count(),1);
    }
    await finishMotion(page);
    const assetAlignment=await page.locator('.project-assets').evaluateAll(groups=>groups.map(group=>[...group.children].filter(child=>child.matches('.project-asset')).map(card=>({
      top:card.getBoundingClientRect().top,
      progress:card.querySelector('[data-slot=progress]')?.getBoundingClientRect().top,
      actions:card.querySelector('.project-actions')?.getBoundingClientRect().top,
    }))));
    for(const row of assetAlignment){
      if(row.length<2||Math.max(...row.map(card=>card.top))-Math.min(...row.map(card=>card.top))>1)continue;
      for(const field of ['progress','actions']){
        assert(row.every(card=>Number.isFinite(card[field])),'Missing project asset '+field);
        assert(Math.max(...row.map(card=>card[field]))-Math.min(...row.map(card=>card[field]))<=1,'Misaligned project asset '+field);
      }
    }
    await page.screenshot({path:path.join(output,`project-${caseName}-${key}-${index}-${width}-${locale}.png`)});
    await result.click();await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('application').focus();await page.keyboard.press('Enter');await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.equal(pixels.length,1);const [x,y]=pixels[0].coordinate;assert.deepEqual(pixels[0],await api(`/jobs/${job.id}/pixel?x=${x}&y=${y}`));
    assert.equal(pixels[0].sha256,recorded.job.sha256);
    if(!pixels[0].isNoData){assert.equal(pixels[0].label,'Gamma0 · '+key.toUpperCase());assert(await page.locator('.wm-pixel-value').getByText(/dB/).count()>0);}
    assert(await page.getByText(label('Local radar · dB display only · original gamma0 available','本地雷达 · dB 仅用于显示 · 可查询原始 γ⁰'),{exact:true}).count()>0);
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0;for(let i=3;i<rgba.length;i+=400)if(rgba[i]>0)count++;return count>100;}),null,{timeout:60000});
    await finishMotion(page);await page.screenshot({path:path.join(output,`workspace-${caseName}-${key}-${index}-${width}-${locale}.png`)});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({case:caseName,key,jobId:job.id,sha256:job.sha256,width,locale,theme,paintedRaster:true,pixel:pixels[0],linearGamma0AndDbRetained:true,
      projectCards:cards.length,projectCardHeight:cards[0].height,assetAlignment,calls});await context.close();
    report.pendingCase=null;report.status='partial';await checkpoint();
    console.log(JSON.stringify({case:caseName,key,width,locale,sha256:job.sha256}));
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status=liveOwner?'partial':'passed';
  await checkpoint();
  console.log(JSON.stringify({status:report.status,cases:report.cases.length,errors:report.errors,remoteRequests:report.remoteRequests}));
}finally{
  try{
    if(browser){report.createdByUi=report.createdByUi.filter(entry=>report.cases.some(scene=>scene.jobId===entry.id));await checkpoint();}
  }finally{
    await browser?.close();server.close();
    if(runtime&&runtime.exitCode===null&&runtime.signalCode===null){
      const jobs=await api('/jobs').catch(()=>null);
      if(Array.isArray(jobs)&&!jobs.some(job=>['queued','running'].includes(job.status))){const ended=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await ended;}
    }
    await writeFile(path.join(output,'runtime.stderr.log'),runtimeError);
  }
}
