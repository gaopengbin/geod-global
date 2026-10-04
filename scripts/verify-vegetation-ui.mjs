// Production renderer + exact desktop CSP, actual native files and processing.
// A private seeded store and headless Edge leave the user's desktop untouched.
import {chromium} from 'playwright';
import {spawn,spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,copyFile,cp,readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace=process.cwd(),baseRoot=path.resolve(process.argv[2]),temporalRoot=path.resolve(process.argv[3]),root=path.resolve(process.argv[4]);
const port=Number(process.argv[5]||4633),uiPort=Number(process.argv[6]||4634),base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('modis-vegetation-'));
await mkdir(root,{recursive:false});await mkdir(path.join(root,'assets'));await mkdir(path.join(root,'ui'));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),fileHash=async p=>hash(await readFile(p));
const sources=await Promise.all([baseRoot,temporalRoot].map(async directory=>JSON.parse(await readFile(path.join(directory,'verification.json'),'utf8'))));
assert(sources.every(source=>source.status==='passed'));assert.equal(sources[0].nativeBinarySha256,sources[1].nativeBinarySha256);
const binaryName=`runtime-${sources[0].nativeBinarySha256.slice(0,16)}.exe`,exe=path.join(root,binaryName);await copyFile(path.join(baseRoot,binaryName),exe);
assert.equal(await fileHash(exe),sources[0].nativeBinarySha256);
const prefix=String.fromCharCode(92,92,63,92),local=value=>path.resolve(value.startsWith(prefix)?value.slice(4):value);
const entries=[...sources[0].originals,...sources[0].outputs,...sources[1].outputs],outputs=[...sources[0].outputs,...sources[1].outputs],cases=sources.flatMap(source=>source.cases);
assert.equal(entries.length,14);assert.equal(outputs.length,8);assert.equal(cases.length,4);
const jobs={},projects=Object.fromEntries(cases.map(entry=>[entry.project.id,entry.project]));
for(const entry of entries){
  const job=structuredClone(entry.job),sourceFile=local(job.outputPath),outputPath=path.join(root,'assets',job.id+'.tif');
  assert.equal(await fileHash(sourceFile),job.sha256);await copyFile(sourceFile,outputPath);job.outputPath=outputPath;
  if(job.manifestPath){const destination=path.join(root,'assets',job.id+'.metadata.json');await copyFile(local(job.manifestPath),destination);job.manifestPath=destination;}
  jobs[job.id]=job;
}
await writeFile(path.join(root,'jobs.json'),JSON.stringify(jobs,null,2));await writeFile(path.join(root,'projects.json'),JSON.stringify(projects,null,2));
await writeFile(path.join(root,'proxy-settings.json'),JSON.stringify({mode:'custom',url:'http://127.0.0.1:9'}));
const renderer=path.join(root,'renderer');await cp(path.join(workspace,'prototype/dist'),renderer,{recursive:true,errorOnExist:true,force:false});
async function rendererFiles(directory,prefix=''){
  const rows=[];for(const entry of await readdir(directory,{withFileTypes:true})){
    const name=prefix+entry.name;if(entry.isDirectory())rows.push(...await rendererFiles(path.join(directory,entry.name),name+'/'));
    else rows.push({path:name,sha256:await fileHash(path.join(directory,entry.name))});
  }return rows.sort((a,b)=>a.path.localeCompare(b.path));
}
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const report={schema:'geod-modis-vegetation-ui/v1',nativeBinarySha256:sources[0].nativeBinarySha256,
  sourceReceipts:await Promise.all([baseRoot,temporalRoot].map(async directory=>({path:path.join(directory,'verification.json'),sha256:await fileHash(path.join(directory,'verification.json'))}))),
  renderer:'frozen production build; exact desktop CSP; native API through loopback bridge',rendererFiles:await rendererFiles(renderer),
  csp:config.app.security.csp,nativeWindowTested:false,usedUserDesktop:false,upstreamBlocked:true,cases:[],createdByUi:[],errors:[],remoteRequests:[],status:'pending'};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let stderr='';runtime.stderr.on('data',data=>stderr+=data);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),p=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(p.startsWith(renderer+path.sep));const ext=path.extname(p),body=await readFile(p);
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);
  }catch{res.writeHead(404);res.end();}
});
let browser,currentPage;
async function api(route){const response=await fetch(base+route);assert(response.ok,await response.clone().text());return response.json();}
async function settled(id){
  for(let i=0;i<600;i++){const job=await api('/jobs/'+id);assert(!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));
    if(job.status==='succeeded'&&job.settled)return job;await new Promise(resolve=>setTimeout(resolve,100));}
  throw Error('Actual UI processing did not settle');
}
async function finishMotion(page){await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});}
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(error){assert.equal(runtime.exitCode,null,stderr);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
  assert.equal((await api('/proxy')).url,'http://127.0.0.1:9');
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [caseName,key,width,locale,theme]of[['single','ndvi',1440,'en','light'],['mosaic','evi',1024,'zh-CN','dark'],['polygon','ndvi',900,'en','dark'],['temporal','evi',1440,'zh-CN','light']]){
    const recorded=outputs.find(entry=>entry.case===caseName&&entry.job.assetKey===key),project=cases.find(entry=>entry.name===caseName).project;
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],created=[],label=(en,zh)=>locale==='en'?en:zh;
    report.activeRun={case:caseName,key,calls,pixels};
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.exposeBinding('__vegetationNative',async(_source,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');calls.push({method:request.method,path:url.pathname});
      const response=await fetch(base+url.pathname+url.search,{method:request.method,body:request.body,headers:request.headers}),body=Buffer.from(await response.arrayBuffer());
      if(url.pathname.endsWith('/pixel')&&response.ok)pixels.push(JSON.parse(body));
      if(url.pathname.endsWith('/mosaics')&&request.method==='POST'&&response.ok)created.push(JSON.parse(body));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{
      const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__vegetationNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});
    };});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();currentPage=page;page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#My%20Data?project='+project.id);
    await page.getByRole('heading',{name:project.name,exact:true}).waitFor({timeout:60000});
    const asset=page.locator('.project-asset').nth(key==='ndvi'?0:1),action=asset.getByRole('button').last();
    assert.equal(await action.textContent(),label(caseName==='single'?'Clip band to project area':'Mosaic and clip band',caseName==='single'?'裁剪到工程区域':'拼接并裁剪'));
    assert.equal(await action.isEnabled(),true);await action.click();
    for(let i=0;!created.length&&i<300;i++)await new Promise(resolve=>setTimeout(resolve,100));
    assert.equal(created.length,1);const job=await settled(created[0].id);
    assert.equal(job.assetKey,key);assert.equal(job.mosaic.projectId,project.id);assert.equal(job.sha256,recorded.job.sha256);
    assert.deepEqual(job.mosaicOutput,recorded.job.mosaicOutput);assert.deepEqual(job.mosaic.sources,recorded.job.mosaic.sources);
    const metadata=await api(`/jobs/${job.id}/raster`);assert.equal(metadata.previewDataUrl,recorded.metadata.previewDataUrl);assert.deepEqual(metadata.vegetation,recorded.metadata.vegetation);
    report.createdByUi.push({case:caseName,key,id:job.id,sha256:job.sha256,sourcePinsIdentical:true,outputBytesIdentical:true,planIdentical:true,displayIdentical:true});
    const openResult=page.locator(`[data-layout=files] a[href*="file=${job.id}"]`);await openResult.waitFor({timeout:60000});
    await page.locator('[data-layout=files]').scrollIntoViewIfNeeded();
    // Off-screen cards intentionally defer expensive full-file previews.
    // Visit them once before checking that the entire captured grid is ready.
    for(const thumbnail of await page.locator('[data-layout=files] .runtime-file-thumbnail').all())await thumbnail.scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>{const thumbnails=[...document.querySelectorAll('[data-layout=files] [data-slot=task-row] .runtime-file-thumbnail')];return thumbnails.length>0&&thumbnails.every(thumbnail=>{const image=thumbnail.querySelector('img');return image?.complete&&image.naturalWidth>0;});},null,{timeout:60000});
    const cards=await page.evaluate(()=>[...document.querySelectorAll('[data-layout=files] [data-slot=task-row]')].map(element=>{const rect=element.getBoundingClientRect(),img=element.querySelector('.runtime-file-thumbnail');return{height:rect.height,right:rect.right,thumbnailHeight:img?.getBoundingClientRect().height};}));
    assert(cards.length>0&&cards.every(card=>card.right<=width+1)&&Math.max(...cards.map(card=>card.height))-Math.min(...cards.map(card=>card.height))<2);
    assert(cards.every(card=>card.thumbnailHeight>=100),'Every thumbnail fills the common card height');
    await page.getByRole('heading',{name:project.name,exact:true}).scrollIntoViewIfNeeded();await finishMotion(page);
    await page.screenshot({path:path.join(root,'ui',`project-${caseName}-${key}-${width}-${locale}.png`)});
    const original=entries.find(entry=>entry.job.kind==='download'&&entry.job.assetKey===key&&project.scenes.some(scene=>scene.itemId===entry.job.itemId));
    const sourceRow=page.locator('[data-layout=files] [data-slot=task-row]').filter({has:page.locator(`a[href*="file=${original.job.id}"]`)});
    await sourceRow.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();
    await sourceRow.getByRole('button',{name:label('Inspect raster','检查栅格'),exact:true}).click();
    const dialog=page.getByRole('dialog');await dialog.getByRole('heading',{name:label(key.toUpperCase()+' vegetation index',key.toUpperCase()+' 植被指数'),exact:true}).waitFor({timeout:60000});
    assert.equal(await dialog.getByRole('heading',{name:label('Reflectance display','反射率显示'),exact:true}).count(),0);
    await dialog.getByRole('img',{name:label('Verified local vegetation index preview','已校验的本地植被指数预览'),exact:true}).waitFor();
    await page.waitForFunction(()=>{const img=document.querySelector('[role=dialog] .runtime-raster-image img');return img?.complete&&img.naturalWidth>0;});
    assert((await dialog.innerText()).includes('0.0001'));assert((await dialog.innerText()).includes('Int16'));
    await finishMotion(page);await page.screenshot({path:path.join(root,'ui',`inspection-${caseName}-${key}-${width}-${locale}.png`)});
    await dialog.getByRole('button',{name:label('Close raster inspection','关闭栅格检查'),exact:true}).click();
    await openResult.click();await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor({timeout:60000});
    // Click an interior point away from the exact central grid boundary of
    // even-sized rasters. Division and inverse-affine arithmetic can choose
    // different adjacent cells at a floating-point boundary; this check uses
    // an unambiguous queried coordinate and keeps exact GDAL equality.
    const mapArea=page.getByRole('application'),mapBox=await mapArea.boundingBox();
    await mapArea.click({position:{x:mapBox.width/2+7.25,y:mapBox.height/2+3.375}});await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.equal(pixels.length,1);const pixel=pixels[0],oracle=spawnSync('python',['-X','utf8','scripts/verify-vegetation-pixel.py',local(job.outputPath),...pixel.coordinate.map(String)],{windowsHide:true,encoding:'utf8'});
    assert.equal(oracle.status,0,oracle.stderr);const expected=JSON.parse(oracle.stdout);
    assert.deepEqual(pixel.pixel,expected.pixel);assert(pixel.center.every((value,i)=>Math.abs(value-expected.center[i])<1e-7));
    assert.equal(pixel.value,expected.value);assert.equal(pixel.indexValue,expected.indexValue);assert.equal(pixel.isNoData,expected.isNoData);assert(!('reflectance' in pixel));
    const pixelText=await page.locator('.wm-pixel-value').innerText();assert(pixelText.includes('DN'));
    assert(pixelText.includes(pixel.isNoData?'NoData':key.toUpperCase()));
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let count=0;for(let i=3;i<rgba.length;i+=400)if(rgba[i]>0)count++;return count>100;}),null,{timeout:60000});
    await finishMotion(page);await page.screenshot({path:path.join(root,'ui',`workspace-${caseName}-${key}-${width}-${locale}.png`)});
    await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().click();
    assert((await page.getByRole('dialog').innerText()).includes('MOD13Q1 / MYD13Q1 v061'));
    assert((await page.getByRole('dialog').innerText()).includes('231.656'));
    await page.getByRole('dialog').getByRole('button',{name:label('Close','关闭'),exact:true}).click();
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({case:caseName,key,width,locale,theme,paintedRaster:true,pixelInteraction:'actual interior map click',independentPixel:pixel,gdalPixel:expected,indexMetadataVisible:true,
      thumbnailsFillUniformCards:true,cardCount:cards.length,cardHeight:cards[0].height,calls});
    await context.close();currentPage=null;console.log(JSON.stringify({case:caseName,key,width,locale,sha256:job.sha256}));
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);delete report.activeRun;report.status='passed';
  console.log(JSON.stringify({status:report.status,cases:report.cases.length,created:report.createdByUi.length,rendererFiles:report.rendererFiles.length}));
}catch(error){report.status='failed';report.failure=error.stack;await currentPage?.screenshot({path:path.join(root,'ui','failure.png')}).catch(()=>{});throw error;}
finally{
  await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}
  await writeFile(path.join(root,'ui','verification.json'),JSON.stringify(report,null,2));await writeFile(path.join(root,'ui','runtime.stderr.log'),stderr);
}
