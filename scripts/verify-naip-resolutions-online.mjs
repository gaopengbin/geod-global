// Live official catalogue + real original COG ranges in the production UI.
// This proves online display only, never whole-original acceptance.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {chromium} from 'playwright';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),nativePort=Number(process.argv[3]||4608),uiPort=Number(process.argv[4]||4609);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('naip-resolutions-'));
const source=JSON.parse(await readFile(path.join(root,'native-resolution-verification.json'),'utf8'));
const base=`http://127.0.0.1:${nativePort}`,origin=`http://127.0.0.1:${uiPort}`;
const health=await(await fetch(base+'/health')).json();assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));
const output=path.join(root,'online');await mkdir(output,{recursive:true});
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.woff2':'font/woff2'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(await readFile(file));
  }catch{res.writeHead(404);res.end();}
});
const report={schema:'geod-naip-resolution-online/v1',checkedAt:new Date().toISOString(),nativeBinarySha256:source.nativeBinarySha256,status:'in_progress',
  wholeOriginalAcceptance:false,renderer:'production renderer with the exact desktop CSP',readbackInstrumentation:'preserve the WebGL draw buffer for actual raster pixel acceptance',usedUserDesktop:false,cases:[]};
const clean=value=>String(value).replace(/https?:\/\/\S+/g,'[URL]');
let browser;
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [group,width,locale,theme]of[['1m',1440,'en','light'],['0.3m',1024,'zh-CN','dark']]){
    const project=source.cases.find(entry=>entry.group===group&&entry.kind==='single').project;
    const context=await browser.newContext({viewport:{width,height:940}}),log={group,projectId:project.id,width,locale,theme,catalog:[],cog:[],errors:[],rejectedRemote:[]},responses=[];
    let releaseCatalog;const gate=new Promise(resolve=>releaseCatalog=resolve);
    await context.addInitScript(({locale,theme})=>{
      localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];
      document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));
      const original=HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext=function(kind,options){return original.call(this,kind,/^(webgl2?|experimental-webgl)$/.test(kind)?{...options,preserveDrawingBuffer:true}:options);};
      window.__NAIP_RASTER_PROBE=()=>[...document.querySelectorAll('canvas.explore-base-layer, .explore-base-layer canvas')].map(canvas=>{
        const gl=canvas.getContext('webgl2')||canvas.getContext('webgl');
        if(!gl||!canvas.width||!canvas.height)return{width:canvas.width,height:canvas.height,opaqueSamples:0,distinctRgb:0};
        const rgba=new Uint8Array(canvas.width*canvas.height*4);gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,rgba);
        const colors=new Set();let opaqueSamples=0;
        for(let i=0;i<rgba.length;i+=52){if(rgba[i+3]>0){opaqueSamples++;colors.add((rgba[i]>>3)*1024+(rgba[i+1]>>3)*32+(rgba[i+2]>>3));}}
        const rect=canvas.getBoundingClientRect(),style=getComputedStyle(canvas);
        return{width:canvas.width,height:canvas.height,opaqueSamples,distinctRgb:colors.size,glError:gl.getError(),displayWidth:rect.width,displayHeight:rect.height,opacity:style.opacity,transform:style.transform};
      });
    },{locale,theme});
    await context.exposeBinding('__naipOnlineNative',async(_context,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');assert.equal(request.method,'GET');
      const response=await fetch(base+url.pathname+url.search,{headers:request.headers,signal:AbortSignal.timeout(120000)});
      return{status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(await response.arrayBuffer()).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{
      const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
      const response=await window.__naipOnlineNative({url,method:init.method||'GET',headers:Object.fromEntries(new Headers(init.headers))});
      return new Response(Uint8Array.from(atob(response.body),value=>value.charCodeAt(0)),{status:response.status,headers:response.headers});
    };});
    await context.route('**/*',async route=>{
      const url=new URL(route.request().url());
      if(url.origin===origin||['https://planetarycomputer.microsoft.com','https://naipeuwest.blob.core.windows.net'].includes(url.origin)){
        if(url.hostname==='planetarycomputer.microsoft.com'&&url.pathname==='/api/stac/v1/search'){
          try{const response=await route.fetch({timeout:45000,maxRetries:2});await gate;return route.fulfill({response});}
          catch(error){log.errors.push(clean(error.message));return route.abort();}
        }
        return route.continue();
      }
      log.rejectedRemote.push({origin:url.origin,path:url.pathname});return route.abort();
    });
    const page=await context.newPage();page.on('pageerror',error=>log.errors.push(clean(error.message)));page.on('console',message=>{if(message.type()==='error')log.errors.push(clean(message.text()));});
    page.on('response',response=>{
      const url=new URL(response.url());
      if(url.hostname==='planetarycomputer.microsoft.com'&&url.pathname==='/api/stac/v1/search'){
        const entry={status:response.status(),path:url.pathname};log.catalog.push(entry);
        responses.push(response.json().then(body=>{entry.items=body.features?.map(item=>item.id)||[];}).catch(error=>log.errors.push(clean(error.message))));
      }
      if(url.hostname==='naipeuwest.blob.core.windows.net'&&url.pathname.endsWith('.tif'))log.cog.push({status:response.status(),path:url.pathname,range:response.request().headers().range||null});
    });
    try{
      await page.goto(origin+'/#Explore?project='+project.id);
      const waiting=page.getByRole('button',{name:locale==='en'?'Reading imagery metadata…':'正在读取影像元数据…',exact:true});
      await waiting.waitFor({timeout:15000});assert.equal(await waiting.isDisabled(),true);log.restoredGridWaitGuard=true;releaseCatalog();
      await page.waitForFunction(()=>document.querySelector('.explore-map-target')?.getAttribute('aria-label')?.includes(document.documentElement.lang==='en'?'true-color':'真彩色')
        &&document.querySelector('.explore-map-root')?.dataset.mapReady==='true',null,{timeout:90000});
      await page.waitForFunction(()=>!document.querySelector('.explore-map-progress'),null,{timeout:90000});
      await page.waitForFunction(()=>window.__NAIP_RASTER_PROBE().some(probe=>probe.opaqueSamples>100&&probe.distinctRgb>40&&probe.glError===0),null,{timeout:60000,polling:250});
      log.rasterPixels=await page.evaluate(()=>window.__NAIP_RASTER_PROBE());
      await Promise.all(responses);
      assert(log.catalog.some(entry=>entry.status===200&&entry.items.includes(project.scenes[0].itemId)));
      assert(log.cog.some(entry=>entry.status===206&&/^bytes=\d+-\d+$/.test(entry.range||'')));
      assert(log.cog.some(entry=>entry.path===new URL(project.scenes[0].assets.aerial.href).pathname));
      assert.deepEqual(log.errors,[]);assert.deepEqual(log.rejectedRemote,[]);
      await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation=>animation.finished.catch(()=>{})));});
      assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
      const body=await page.locator('body').innerText();assert(!/missing a supported|缺少受支持|could not load|无法加载/i.test(body));
      log.actualItemId=project.scenes[0].itemId;log.gridSpacing=group==='1m'?1:.3;log.status='passed';
      await page.screenshot({path:path.join(output,`explore-${group}-${width}-${locale}.png`)});
      report.cases.push(log);console.log(JSON.stringify({stage:'online-display',group,status:log.status,catalogRequests:log.catalog.length,cogRanges:log.cog.length,wholeOriginalAcceptance:false}));
    }catch(error){
      log.status='failed';log.failure=clean(error.message);log.rasterPixels=await page.evaluate(()=>window.__NAIP_RASTER_PROBE());
      log.canvases=await page.locator('.explore-map-target canvas').evaluateAll(elements=>elements.map(canvas=>({className:canvas.className,width:canvas.width,height:canvas.height})));
      report.cases.push(log);report.status='failed';
      await page.screenshot({path:path.join(output,`failure-${group}.png`)});throw error;
    }finally{releaseCatalog();await context.close();}
  }
  report.status='passed';
}finally{
  await browser?.close();server.close();
  await writeFile(path.join(output,'resolution-online-verification.json'),JSON.stringify(report,null,2)+'\n');
}
