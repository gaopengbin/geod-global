// Real catalogues and source pixels in the production renderer and desktop CSP.
// Uses a private empty native store and headless browser; never controls a user's
// desktop window, submits download jobs, saves credentials or publishes files.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createServer as createNetServer } from 'node:net';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
import { PROVIDERS } from '../prototype/src/providers.js';
import { catalogPreviewKind } from '../prototype/src/catalog-preview.js';
import { originalsReleased } from '../prototype/src/release-policy.js';

const workspace=process.cwd(), exe=path.resolve(process.argv[2] || 'target/debug/geod-runtime.exe');
assert(exe.startsWith(workspace+path.sep),'Runtime must belong to this repository.');
const renderer=path.resolve('prototype/dist');
const root=path.join(workspace,'.verification','public-catalog-preview-'+new Date().toISOString().replace(/[:.]/g,'-'));
await mkdir(path.join(root,'evidence'),{recursive:true});
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const sha256=bytes=>createHash('sha256').update(bytes).digest('hex');
const safeURL=value=>{const url=new URL(value);if(url.hostname.endsWith('.blob.core.windows.net'))url.search='';return url.href;};
const freePort=async()=>{const server=createNetServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const port=server.address().port;await new Promise(resolve=>server.close(resolve));return port;};
const runtimePort=await freePort(), base=`http://127.0.0.1:${runtimePort}`;
const report={status:'pending',checkedAt:new Date().toISOString(),runtimeSha256:sha256(await readFile(exe)),
  rendererFiles:[],usedUserDesktop:false,nativeWindowTested:false,originalDownloadsCreated:0,cases:[],requests:[],failedRequests:[],commands:[],errors:[],cspErrors:[]};
const runtime=spawn(exe,['serve','--data-dir',path.join(root,'store'),'--port',String(runtimePort)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let stderr='',origin,browser,page,failTiles=false;
runtime.stdout.on('data',()=>{});runtime.stderr.on('data',bytes=>stderr+=bytes);
const server=createServer(async(req,res)=>{
  try {
    const url=new URL(req.url,origin),file=path.resolve(renderer,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(renderer+path.sep));const bytes=await readFile(file);
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.geojson':'application/json','.woff2':'font/woff2'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(bytes);
  } catch {res.writeHead(404);res.end();}
});
const paths={health:'/health',list_jobs:'/jobs',list_projects:'/projects',list_recipes:'/recipes',get_proxy_settings:'/proxy',list_provider_accounts:'/accounts',list_vectors:'/vectors',list_feature_services:'/feature-services',list_map_services:'/map-services',list_map_images:'/map-images',list_tile_sources:'/tile-sources',list_tile_packages:'/tile-packages',list_stac_connections:'/stac/connections',list_wcs_connections:'/wcs/connections',list_three_d:'/three-d/packages'};
async function api(route){const response=await fetch(base+route),body=await response.json();assert(response.ok);return body;}
async function settled(){await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});}
async function capture(name){await settled();assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.screenshot({path:path.join(root,name+'.png')});}
async function mapSettled(){
  await page.waitForFunction(()=>{
    if(document.querySelector('.explore-map-error'))return true;
    const map=document.querySelector('.explore-map-root');
    const key=[map?.dataset.previewItem,map?.dataset.previewChannel,document.querySelector('.inspector .scene-id')?.textContent].join('|');
    if(map?.dataset.mapReady!=='true'||document.querySelector('.explore-map-progress')){window.__MAP_QUIET=null;return false;}
    const now=performance.now();
    if(window.__MAP_QUIET?.key!==key)window.__MAP_QUIET={key,since:now};
    return now-window.__MAP_QUIET.since>=750;
  },null,{timeout:60000});
  assert.equal(await page.locator('.explore-map-error').count(),0,await page.locator('.explore-map-error').allTextContents());
}
async function visibleMap(imageTiles){
  await mapSettled();
  await settled();
  const canvas=page.locator(imageTiles?'.explore-index-layer canvas':'canvas.explore-base-layer, .explore-base-layer canvas');
  const readPixels=()=>canvas.evaluate(canvas=>{
    const width=canvas.width,height=canvas.height,colors=new Set();let data;
    const gl=canvas.getContext('webgl2')||canvas.getContext('webgl');
    if(gl){data=new Uint8Array(width*height*4);gl.readPixels(0,0,width,height,gl.RGBA,gl.UNSIGNED_BYTE,data);}
    else data=canvas.getContext('2d').getImageData(0,0,width,height).data;
    let valid=0;for(let i=0;i<data.length;i+=4)if(data[i+3]){valid++;colors.add(`${data[i]},${data[i+1]},${data[i+2]}`);}
    return {width,height,valid,colors:colors.size};
  });
  let pixels=await readPixels();
  const until=Date.now()+60000;
  while((pixels.valid<=500||pixels.colors<=8)&&Date.now()<until){
    assert.equal(await page.locator('.explore-map-error').count(),0,await page.locator('.explore-map-error').allTextContents());
    await new Promise(resolve=>setTimeout(resolve,500));pixels=await readPixels();
  }
  await mapSettled();pixels=await readPixels();
  assert(pixels.valid>500&&pixels.colors>8,JSON.stringify(pixels));
  const legend=page.locator('.vegetation-preview-legend');
  if(await legend.count()){
    const geometry=await page.evaluate(()=>({legendBottom:document.querySelector('.vegetation-preview-legend').getBoundingClientRect().bottom,attributionTop:document.querySelector('.map-attribution').getBoundingClientRect().top}));
    assert(geometry.legendBottom<=geometry.attributionTop-8,JSON.stringify(geometry));
  }
  return {...pixels,item:await page.locator('.explore-map-root').getAttribute('data-preview-item'),channel:await page.locator('.explore-map-root').getAttribute('data-preview-channel')};
}
async function choose(source){
  await page.locator('.catalog-source-panel').getByRole('combobox').click();
  await page.getByRole('option',{name:source.name,exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.catalog-fetch-status')?.textContent.includes('目录已取完')||document.querySelector('.catalog-error'),null,{timeout:60000});
  assert.equal(await page.locator('.catalog-error').count(),0,await page.locator('.catalog-error').allTextContents());
  assert(await page.locator('.scene-list .scene').count()>0,source.id+' returned no scenes for this known covered area');
}
async function loadFirst(){
  const first=page.locator('.scene-list .scene').first();
  const item=(await first.getAttribute('aria-label')).trim().split(/\s+/).at(-1);
  await first.click();await page.getByRole('button',{name:/加载所选影像/}).click();
  // Signing is asynchronous. The footprint or previous raster can be ready
  // before the chosen COG exists; require the actual front scene first.
  await page.waitForFunction(item=>document.querySelector('.catalog-error')||document.querySelector('.inspector .scene-id')?.textContent===item,item,{timeout:60000});
  assert.equal(await page.locator('.catalog-error').count(),0,await page.locator('.catalog-error').allTextContents());
  await page.locator('canvas.explore-base-layer, .explore-base-layer canvas').waitFor({timeout:60000});
  return item;
}

try {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin=`http://127.0.0.1:${server.address().port}`;
  for(let n=0;n<80;n++){try{await api('/health');break;}catch{assert.equal(runtime.exitCode,null,stderr);if(n===79)throw new Error('Private runtime unavailable');await new Promise(resolve=>setTimeout(resolve,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});
  const context=await browser.newContext({viewport:{width:1440,height:960}}),receipts=[];
  await context.exposeBinding('__previewNative',async(_source,command)=>{
    report.commands.push(command);if(command==='activate_desktop_frame')return 'custom';
    if(['set_desktop_locale','set_desktop_appearance'].includes(command))return null;
    assert(paths[command],'Unexpected native mutation: '+command);return api(paths[command]);
  });
  await context.exposeBinding('__savePreviewBytes',async(_source,receipt)=>{
    const url=new URL(receipt.url);assert(url.origin==='https://planetarycomputer.microsoft.com'&&url.pathname.startsWith('/api/data/v1/item/tiles/WebMercatorQuad/'));
    const bytes=Buffer.from(receipt.data,'base64'),hash=sha256(bytes);
    if(receipt.status===200)assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
    const file=path.join(root,'evidence',hash+(receipt.status===200?'.png':'.json'));await writeFile(file,bytes);
    report.requests.push({url:url.href,status:receipt.status,path:file,sha256:hash,bytes:bytes.length,capturedFrom:'browser-fetch-response-clone'});
  });
  await context.addInitScript(()=>{
    localStorage.setItem('geod-global-locale','zh-CN');localStorage.setItem('geod-design-theme',JSON.stringify('light'));localStorage.setItem('geod-design-nav-collapsed','true');
    window.__TAURI__={core:{invoke:command=>window.__previewNative(command),convertFileSrc:(item,scheme)=>`http://${scheme}.localhost/${item}`}};
    window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));
    const originalContext=HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext=function(type,options){return originalContext.call(this,type,type.startsWith('webgl')?{...options,preserveDrawingBuffer:true}:options);};
    const originalFetch=window.fetch.bind(window);window.__PREVIEW_RECEIPTS=[];
    window.fetch=async(...args)=>{
      const response=await originalFetch(...args),url=String(args[0] instanceof Request?args[0].url:args[0]);
      if(url.startsWith('https://planetarycomputer.microsoft.com/api/data/v1/item/tiles/WebMercatorQuad/')){
        const save=response.clone().arrayBuffer().then(bytes=>{const data=new Uint8Array(bytes);let binary='';for(let i=0;i<data.length;i+=8192)binary+=String.fromCharCode(...data.subarray(i,i+8192));return window.__savePreviewBytes({url,status:response.status,data:btoa(binary)});}).catch(error=>{if(error.name!=='AbortError')throw error;});
        window.__PREVIEW_RECEIPTS.push(save);
      }
      return response;
    };
  });
  await context.route('**/*',async route=>{
    try {
    const request=route.request(),url=new URL(request.url());if(url.origin===origin)return route.continue();
    if(url.origin==='http://geod-elevation.localhost'){
      assert.equal(request.method(),'GET');assert(!url.search&&/^\/Copernicus_DSM_COG_(10|30)_[NS]\d{2}_00_[EW]\d{3}_00_DEM$/.test(url.pathname));
      const range=request.headers().range;assert(range);
      const response=await fetch(base+'/preview/elevation'+url.pathname,{headers:{Range:range}}),bytes=Buffer.from(await response.arrayBuffer());
      const headers=Object.fromEntries(response.headers);headers['access-control-allow-origin']='*';
      const hash=sha256(bytes),file=path.join(root,'evidence',hash+'.bin');await writeFile(file,bytes);
      report.requests.push({url:url.href,status:response.status,contentRange:headers['content-range']||null,bytes:bytes.length,sha256:hash,path:file,capturedFrom:'actual-native-read-only-elevation-range'});
      return route.fulfill({status:response.status,headers,body:bytes});
    }
    const allowed=['planetarycomputer.microsoft.com','earth-search.aws.element84.com','sentinel-cogs.s3.us-west-2.amazonaws.com','sentinel2l2a01.blob.core.windows.net','landsateuwest.blob.core.windows.net','sentinel1euwestrtc.blob.core.windows.net','naipeuwest.blob.core.windows.net','copernicus-dem-30m.s3.eu-central-1.amazonaws.com','copernicus-dem-90m.s3.eu-central-1.amazonaws.com'];
    assert(request.method()==='GET'&&url.protocol==='https:'&&allowed.includes(url.hostname),'Unexpected remote request: '+safeURL(url.href));
    if(url.pathname.includes('/item/tiles/')&&failTiles)return route.fulfill({status:503,contentType:'application/json',body:'{"detail":"deliberate retry verification"}',headers:{'access-control-allow-origin':'*'}});
    return route.continue();
    }catch(error){report.errors.push(error.message);await route.abort().catch(()=>{});}
  });
  page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message.replace(/https?:\/\/[^\s)]+/g,safeURL)));
  page.on('requestfailed',request=>report.failedRequests.push({url:safeURL(request.url()),error:request.failure()?.errorText}));
  page.on('response',response=>{
    const url=new URL(response.url());
    if(url.origin===origin){if(/\.(js|css)$/.test(url.pathname))report.rendererFiles.push(url.pathname);return;}
    if(url.pathname.includes('/sas/')||url.pathname.includes('/item/tiles/')||url.pathname.includes('/preview.png'))return;
    const save=(async()=>{try{const bytes=await response.body();if(!bytes.length)return;const hash=sha256(bytes),file=path.join(root,'evidence',hash+(url.pathname.endsWith('.tif')||url.pathname.endsWith('.TIF')?'.bin':'.json'));await writeFile(file,bytes);report.requests.push({url:safeURL(url.href),status:response.status(),contentRange:response.headers()['content-range']||null,bytes:bytes.length,sha256:hash,path:file});}catch{/* Source switches intentionally cancel reads. */}})();receipts.push(save);
  });
  await page.goto(origin+'/#Explore');
  const requestedSources=process.argv.slice(3);
  const available=PROVIDERS.filter(source=>originalsReleased(source)&&(!requestedSources.length||requestedSources.includes(source.id)));
  assert(available.length);
  for(const source of available){
    await choose(source);const count=Number(await page.locator('.results-count strong').innerText()),kind=catalogPreviewKind(source);
    const loadedItem=!kind?await loadFirst():null;
    const pixels=await visibleMap(Boolean(kind&&kind!=='elevation'));
    if(loadedItem)assert.equal(await page.locator('.inspector .scene-id').innerText(),loadedItem);
    await capture(source.id+'-1440-zh-light');
    report.cases.push({name:source.id,scenes:count,item:loadedItem||pixels.item,map:pixels,preview:kind||'original-cog',passed:true});
    console.log(JSON.stringify({source:source.id,scenes:count,pixels:pixels.valid,colors:pixels.colors}));
    if(source.id==='planetary-radar'){
      await page.getByRole('radio',{name:'VH',exact:true}).click();const vh=await visibleMap(true);assert.equal(vh.item,pixels.item);await capture('radar-vh-1440');
      report.cases.push({name:'radar-polarization-switch',map:vh,passed:true});
    }
    if(source.id==='planetary-vegetation'){
      await page.getByRole('radio',{name:'EVI',exact:true}).click();const evi=await visibleMap(true);assert.equal(evi.item,pixels.item);report.cases.push({name:'vegetation-evi',map:evi,passed:true});
    }
    if(source.id==='planetary-modis'){
      await page.locator('.scene-list .scene').nth(1).click();const other=await visibleMap(true);assert.notEqual(other.item,pixels.item);report.cases.push({name:'modis-platform-switch',map:other,passed:true});
      failTiles=true;await page.locator('.scene-list .scene').first().click();await page.locator('.explore-map-error').waitFor();assert((await page.locator('.explore-map-error').innerText()).includes('地图预览加载失败'));await capture('modis-failed-request');
      failTiles=false;await page.getByRole('button',{name:'重试地图',exact:true}).click();await visibleMap(true);report.cases.push({name:'modis-error-retry',passed:true});
    }
    if(source.id==='planetary-naip'){
      const clear=page.locator('.catalog-selection-actions').getByRole('button',{name:'清空',exact:true});await clear.click();
      const id='ca_m_3712221_sw_10_.6_20160625_20161004';await page.getByRole('textbox',{name:'搜索影像'}).fill(id);
      assert.equal(await page.locator('.scene-list .scene').count(),1);assert.equal(await loadFirst(),id);const legacy=await visibleMap(false);
      assert.equal(await page.locator('.inspector .scene-id').innerText(),id);assert.equal(await page.locator('.loaded-layer-disclosure').count(),0);
      await capture('naip-legacy-2016-1440');report.cases.push({name:'naip-legacy-h-cog',item:id,map:legacy,passed:true});
    }
  }
  await page.locator('.catalog-source-panel').getByRole('combobox').click();
  assert.equal(await page.getByRole('group',{name:'可下载 · 无需账号 · 9 个数据源'}).getByRole('option').count(),9);await capture('source-capabilities-1440');await page.keyboard.press('Escape');
  await page.setViewportSize({width:900,height:680});await page.getByRole('button',{name:'切换明暗主题',exact:true}).click();
  await choose(PROVIDERS.find(source=>source.id==='planetary-modis'));await visibleMap(true);await capture('modis-900-zh-dark');
  await page.locator('.catalog-source-panel').getByRole('combobox').click();await capture('source-capabilities-900');await page.keyboard.press('Escape');
  report.cspErrors.push(...await page.evaluate(()=>window.__CSP_ERRORS));await Promise.allSettled(receipts);await page.evaluate(()=>Promise.all(window.__PREVIEW_RECEIPTS));
  assert.equal(report.errors.length,0,JSON.stringify(report.errors));assert.equal(report.cspErrors.length,0,JSON.stringify(report.cspErrors));assert.deepEqual(await api('/jobs'),[]);
  report.rendererFiles=[...new Set(report.rendererFiles)];report.status='passed';console.log(JSON.stringify({status:report.status,cases:report.cases.length,root}));
}catch(error){report.status='failed';report.failure=error.stack;await page?.screenshot({path:path.join(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser?.close();server.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}await writeFile(path.join(root,'verification.json'),JSON.stringify(report,null,2));await writeFile(path.join(root,'runtime.stderr.log'),stderr);}
