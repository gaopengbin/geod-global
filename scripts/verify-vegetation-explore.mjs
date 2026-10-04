// Live public catalogue and previews, frozen production renderer, private store.
// This verifies discovery and the download entry, not another original download.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,copyFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const uiRoot=path.resolve(process.argv[2]),root=path.resolve(process.argv[3]),port=Number(process.argv[4]||4637),uiPort=Number(process.argv[5]||4635);
const science=path.basename(root).startsWith('modis-science-');
assert.equal(path.dirname(root),path.resolve('.verification'));assert(path.basename(root).startsWith('modis-vegetation-')||science);
await mkdir(root,{recursive:false});await mkdir(path.join(root,'evidence'));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const accepted=JSON.parse(await readFile(path.join(uiRoot,'ui','verification.json'),'utf8'));assert.equal(accepted.status,'passed');
const sourcePath=accepted.sourceReceipts?.[0]?.path||process.argv[6];
if(science)assert.equal(hash(await readFile(sourcePath)),accepted.sourceReceiptSha256);
const source=JSON.parse(await readFile(sourcePath,'utf8')),project=source.cases.find(entry=>entry.name==='single').project;
const name=`runtime-${accepted.nativeBinarySha256.slice(0,16)}.exe`,exe=path.join(root,name);await copyFile(path.join(uiRoot,name),exe);assert.equal(hash(await readFile(exe)),accepted.nativeBinarySha256);
await writeFile(path.join(root,'projects.json'),JSON.stringify({[project.id]:project},null,2));
const renderer=path.join(uiRoot,'renderer'),origin=`http://127.0.0.1:${uiPort}`,base=`http://127.0.0.1:${port}`;
const report={schema:science?'geod-modis-science-explore/v1':'geod-modis-vegetation-explore/v1',status:'pending',checkedAt:new Date().toISOString(),nativeBinarySha256:accepted.nativeBinarySha256,
  rendererReceipt:{path:path.join(uiRoot,'ui','verification.json'),sha256:hash(await readFile(path.join(uiRoot,'ui','verification.json')))},
  nativeWindowTested:false,usedUserDesktop:false,liveCatalogRequests:[],providerPreviews:[],previewFailures:[],browserConsoleErrors:[],errors:[],rejectedRequests:[],createdOriginalDownloads:0};
const runtime=spawn(exe,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';runtime.stderr.on('data',data=>stderr+=data);runtime.stdout.on('data',()=>{});
const server=createServer(async(req,res)=>{try{const url=new URL(req.url,origin),p=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));assert(p.startsWith(renderer+path.sep));
  const ext=path.extname(p),body=await readFile(p);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.json':'application/json'})[ext]||'application/octet-stream','Content-Security-Policy':accepted.csp});res.end(body);
}catch{res.writeHead(404);res.end();}});
let browser,page;
async function api(route){const response=await fetch(base+route);assert(response.ok,await response.clone().text());return response.json();}
async function finishMotion(){await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{})));});}
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(error){assert.equal(runtime.exitCode,null,stderr);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});const context=await browser.newContext({viewport:{width:1440,height:960}});
  await context.addInitScript(()=>{localStorage.setItem('geod-global-locale','en');window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));});
  await context.exposeBinding('__vegetationNative',async(_source,request)=>{const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');assert.equal(request.method,'GET','Discovery must not create jobs');
    const response=await fetch(base+url.pathname+url.search,{method:'GET',headers:request.headers}),body=Buffer.from(await response.arrayBuffer());return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};});
  await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
    if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__vegetationNative({url,method:init.method||'GET',headers:Object.fromEntries(new Headers(init.headers))});
    if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
  await context.route('**/*',async route=>{
    const request=route.request(),url=new URL(request.url());if(url.origin===origin)return route.continue();
    const catalog=url.origin==='https://planetarycomputer.microsoft.com'&&url.pathname==='/api/stac/v1/search'&&url.searchParams.get('collections')==='modis-13Q1-061';
    const preview=url.origin==='https://planetarycomputer.microsoft.com'&&url.pathname==='/api/data/v1/item/preview.png'&&url.searchParams.get('collection')==='modis-13Q1-061'&&url.searchParams.get('assets')==='250m_16_days_NDVI';
    if((!catalog&&!preview)||request.method()!=='GET'){report.rejectedRequests.push(url.href);return route.abort();}
    try{
      const response=await fetch(url,{redirect:'error',signal:AbortSignal.timeout(60000)}),bytes=Buffer.from(await response.arrayBuffer());
      const digest=hash(bytes),file=path.join(root,'evidence',`${catalog?'catalog':'preview'}-${digest}.${catalog?'json':response.ok?'png':'txt'}`);await writeFile(file,bytes);
      if(catalog)assert.equal(response.status,200);
      if(catalog){const data=JSON.parse(bytes);assert.equal(data.type,'FeatureCollection');assert(data.features.length>0);assert(data.features.every(item=>item.collection==='modis-13Q1-061'));
        report.liveCatalogRequests.push({url:url.href,status:response.status,features:data.features.length,itemIds:data.features.map(item=>item.id),path:file,sha256:digest});
      }else if(response.ok){assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');report.providerPreviews.push({url:url.href,status:response.status,bytes:bytes.length,path:file,sha256:digest});}
      else{assert(response.status>=400);report.previewFailures.push({url:url.href,status:response.status,bytes:bytes.length,path:file,sha256:digest});}
      return route.fulfill({status:response.status,contentType:response.headers.get('content-type')||'application/octet-stream',body:bytes});
    }catch(error){report.errors.push(String(error));return route.abort();}
  });
  page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.browserConsoleErrors.push({message:message.text(),url:message.location().url});});
  await page.goto(origin+'/#My%20Data?project='+project.id);await page.getByRole('heading',{name:project.name,exact:true}).waitFor();
  await page.getByRole('button',{name:'Explore and add scenes to this project',exact:true}).click();
  await page.getByRole('button',{name:'Return to project details',exact:true}).waitFor();
  await page.waitForFunction(()=>document.querySelector('.catalog-fetch-status')?.textContent.includes('Catalog complete'),null,{timeout:90000});
  assert.equal(report.liveCatalogRequests.length,1);const query=new URL(report.liveCatalogRequests[0].url);
  assert.equal(query.searchParams.get('bbox'),project.bounds.join(','));assert.equal(query.searchParams.get('datetime'),'2025-06-01T00:00:00Z/2025-06-30T23:59:59.999Z');assert.equal(query.searchParams.has('query'),false);
  const picker=page.getByRole('combobox',{name:'Data source',exact:true});assert((await picker.innerText()).includes('MODIS NDVI / EVI'));
  await picker.click();assert.equal(await page.getByRole('option').count(),15);await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Filters',exact:true}).click();const filter=page.getByRole('dialog');assert.equal(await filter.getByRole('slider').count(),0);assert.equal(await filter.locator('input[type=date]').count(),0);
  await filter.getByRole('button',{name:'Close dialog',exact:true}).click();
  await page.waitForFunction(()=>{const images=[...document.querySelectorAll('.scene-list .catalog-thumbnail img')];return images.length>0&&images.every(img=>img.complete&&img.naturalWidth>0);},null,{timeout:90000});
  assert((await page.locator('body').innerText()).includes('Jun 26, 2025 – Jul 11, 2025'));
  await finishMotion();
  await page.screenshot({path:path.join(root,'explore-live.png')});
  report.visibleDownloadActions=await page.locator('.catalog-selection-actions button').allTextContents();
  // The existing project selection is retained. Opening the entry is read-only.
  const candidates=page.getByRole('button',{name:/Add to this project and download/});assert.equal(await candidates.count(),1);await candidates.click();
  const dialog=page.getByRole('dialog');await dialog.waitFor();assert((await dialog.innerText()).includes('NDVI'));assert((await dialog.innerText()).includes('EVI'));
  const text=await dialog.innerText();assert(text.includes(project.name));assert(text.includes('Int16')&&text.includes('sinusoidal'));
  if(science){
    await dialog.getByRole('combobox',{name:'Download content',exact:true}).click();assert.equal(await page.getByRole('option').count(),14);
    await page.getByRole('option',{name:'All MODIS science layers · 12 COGs',exact:true}).click();
    assert((await dialog.innerText()).includes('original types, units and fill values'));
    report.allScienceDownloadChoicesVisible=true;report.restoredScienceLayers=Object.keys(project.scenes[0].assets).filter(key=>key.startsWith('vi_'));
    assert.equal(report.restoredScienceLayers.length,10);
  }
  await finishMotion();await page.screenshot({path:path.join(root,'download-entry.png')});
  await dialog.getByText('Source and file checks · advanced',{exact:true}).click();
  assert((await dialog.innerText()).includes('Official MOD13Q1/MYD13Q1 v061 identity'));
  assert(!(await dialog.innerText()).includes('Inspect local RGB or SCL'));
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('button',{name:'Return to project details',exact:true}).click();await page.getByRole('heading',{name:project.name,exact:true}).waitFor();
  const jobs=await api('/jobs');assert.equal((Array.isArray(jobs)?jobs:jobs.jobs).length,0);
  assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);assert.deepEqual(report.errors,[]);assert.deepEqual(report.rejectedRequests,[]);
  assert(report.browserConsoleErrors.every(error=>report.previewFailures.some(failure=>failure.url===error.url)),JSON.stringify(report.browserConsoleErrors));
  report.unavailableProviderPreviewsExplicitlyRecorded=report.previewFailures.length;
  report.providerCount=15;report.projectRestored=true;report.completePeriodVisible=true;report.noOpticalCloudFilter=true;report.bothDownloadChoicesVisible=true;report.returnToSameProject=true;report.status='passed';
  console.log(JSON.stringify({status:report.status,catalogItems:report.liveCatalogRequests[0].features,previews:report.providerPreviews.length,providerCount:report.providerCount}));
}catch(error){report.status='failed';report.failure=error.stack;await page?.screenshot({path:path.join(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}await writeFile(path.join(root,'verification.json'),JSON.stringify(report,null,2));await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
