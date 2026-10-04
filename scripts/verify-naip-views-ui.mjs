// Production renderer + exact desktop CSP + independently accepted NAIP bytes.
// Headless Edge, GET-only native bridge, offline store; no user desktop control.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {chromium} from 'playwright';

const args=process.argv.slice(2),positional=args.filter(arg=>!arg.startsWith('--'));
const resume=args.find(arg=>arg.startsWith('--resume-from='))?.slice('--resume-from='.length);
assert(args.filter(arg=>arg.startsWith('--')).every(arg=>arg.startsWith('--resume-from=')));
const root=path.resolve(positional[0]),port=Number(positional[1]||4614),uiPort=Number(positional[2]||4615);
assert.equal(path.dirname(root),path.resolve('.verification'));assert(path.basename(root).startsWith('naip-views-'));
const sourceRaw=await readFile(path.join(root,'views-verification.json')),source=JSON.parse(sourceRaw);
assert.equal(source.status,'passed');assert([undefined,'030-100','060'].includes(source.cohort));
const fileCount=source.cohort==='060'?4:8;assert.equal(source.entries.length,fileCount);
if(source.cohort==='060')assert(source.entries.every(entry=>entry.group==='0.6m'));
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const binary=path.join(root,`runtime-${source.nativeBinarySha256.slice(0,16)}.exe`);
assert.equal(hash(await readFile(binary)),source.nativeBinarySha256);
const output=path.join(root,'ui');await mkdir(output,{recursive:true});
const base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
const csp=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8')).app.security.csp;
const report={schema:'geod-naip-views-ui/v1',status:'running',checkedAt:new Date().toISOString(),
  nativeBinarySha256:source.nativeBinarySha256,independentReceiptSha256:hash(sourceRaw),
  renderer:'Production build with exact desktop CSP; real GET-only native API bridge',
  nativeWindowTested:false,usedUserDesktop:false,entries:[],errors:[],remoteRequests:[]};
const frontendFiles=new Map();
if(resume){
  const file=path.resolve(root,resume);assert(file.startsWith(root+path.sep));
  const bytes=await readFile(file),saved=JSON.parse(bytes);
  assert.equal(saved.schema,'geod-naip-views-ui-subset/v1');assert.equal(saved.status,'passed');
  assert.equal(saved.nativeBinarySha256,source.nativeBinarySha256);assert.equal(saved.independentReceiptSha256,hash(sourceRaw));
  assert.equal(hash(await readFile(path.join(root,saved.sourceReport.file))),saved.sourceReport.sha256);
  const ids=new Set();
  for(const entry of saved.entries){
    const pin=source.entries.find(item=>item.jobId===entry.jobId);assert(pin&&!ids.has(entry.jobId));ids.add(entry.jobId);
    assert.equal(entry.sha256,pin.sha256);assert.equal(entry.group,pin.group);assert.equal(entry.case,pin.case);
    for(const key of ['rawPixelUnchanged','panAndZoomRetained','cacheHitForThreeRevisits','exactCirChannelsEqual','exactNirGrayEqual','exactAlphaEqual'])assert.equal(entry[key],true);
    assert.deepEqual(entry.previews,pin.views.map(item=>({view:item.view,sha256:item.pngSha256})));
    assert(entry.calls.every(call=>call.method==='GET'&&call.status===200));
    report.entries.push(entry);
  }
  for(const frame of saved.frames){assert.equal(hash(await readFile(path.join(root,frame.file))),frame.sha256);}
  for(const entry of saved.frontendFiles){assert.equal(hash(await readFile(path.join('prototype/dist',entry.file))),entry.sha256);frontendFiles.set(entry.file,entry.sha256);}
  report.resumedAcceptedCases=saved.entries.length;report.acceptedSubset={file:resume,sha256:hash(bytes)};
}
let browser,runtime,currentPage,currentPin,runtimeError='';
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve('prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.resolve('prototype/dist')+path.sep));
    const bytes=await readFile(file),key=path.relative(path.resolve('prototype/dist'),file).replaceAll(path.sep,'/'),digest=hash(bytes);
    assert(!frontendFiles.has(key)||frontendFiles.get(key)===digest,'Production renderer changed during acceptance');frontendFiles.set(key,digest);
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.woff2':'font/woff2','.woff':'font/woff'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':csp});res.end(bytes);
  }catch{res.writeHead(404);res.end();}
});
async function api(route){const response=await fetch(base+route,{signal:AbortSignal.timeout(120000)});assert(response.ok);return response.json();}
async function ready(){const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(root));}
async function write(){
  report.frontendFiles=[...frontendFiles].sort(([a],[b])=>a.localeCompare(b)).map(([file,sha256])=>({file,sha256}));
  await writeFile(path.join(output,'views-verification.json'),JSON.stringify(report,null,2)+'\n');
}
async function stable(page){
  await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});
  await page.waitForFunction(()=>[...document.querySelectorAll('.wm-layer')].every(el=>{for(let parent=el;parent;parent=parent.parentElement)if(Number(getComputedStyle(parent).opacity)<.999)return false;return true;}));
}
async function painted(page,previous){
  const deadline=performance.now()+60000;
  for(;;){
    const ready=await page.evaluate(async previous=>{
      const raster=window.__NAIP_RASTER_FRAME;if(!raster)return false;
      let opaque=0;for(let i=3;i<raster.rgba.length;i+=4)if(raster.rgba[i])opaque++;
      if(opaque<100)return false;
      const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',raster.rgba))].map(v=>v.toString(16).padStart(2,'0')).join('');
      return digest!==previous;
    },previous||'');
    if(ready)break;
    assert(performance.now()<deadline,'The observed raster draw did not arrive');await new Promise(resolve=>setTimeout(resolve,50));
  }
  await page.evaluate(()=>new Promise((resolve,reject)=>{
    const deadline=performance.now()+5000;let prior='',stable=0;
    async function frame(){
      const raster=window.__NAIP_RASTER_FRAME,state=[];
      if(raster){
        const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',raster.rgba))].map(v=>v.toString(16).padStart(2,'0')).join('');
        state.push([raster.width,raster.height,raster.transform,digest]);
      }
      const current=JSON.stringify(state);
      stable=current===prior?stable+1:0;prior=current;
      if(stable>=8)return resolve();if(performance.now()>deadline)return reject(new Error('Map transform did not settle'));
      requestAnimationFrame(frame);
    }requestAnimationFrame(frame);
  }));
  return page.evaluate(()=>{
    const raster=window.__NAIP_RASTER_FRAME,pixels=raster.rgba,canvas=document.createElement('canvas');
    canvas.width=raster.width;canvas.height=raster.height;canvas.getContext('2d').putImageData(new ImageData(pixels,raster.width,raster.height),0,0);
    let text='';for(let i=0;i<pixels.length;i+=32768)text+=String.fromCharCode(...pixels.subarray(i,i+32768));
    return{width:raster.width,height:raster.height,transform:raster.transform,rgba:btoa(text),png:canvas.toDataURL().split(',')[1]};
  });
}
try{
  let occupied=false;try{await ready();occupied=true;}catch(error){assert(error.cause?.code==='ECONNREFUSED',String(error));}
  assert(!occupied,'The science verifier must release its isolated store before UI acceptance');
  runtime=spawn(binary,['serve','--data-dir',root,'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  runtime.stdout.on('data',()=>{});runtime.stderr.on('data',part=>runtimeError+=part);
  for(let attempt=0;attempt<100;attempt++){try{await ready();break;}catch{assert(runtime.exitCode===null&&runtime.signalCode===null,runtimeError);assert(attempt<99);await new Promise(resolve=>setTimeout(resolve,100));}}
  assert.deepEqual(await api('/proxy'),{mode:'custom',url:'http://127.0.0.1:9'});
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [index,pin]of source.entries.entries()){
    if(report.entries.some(entry=>entry.jobId===pin.jobId))continue;
    currentPin={jobId:pin.jobId,group:pin.group,case:pin.case};
    const [width,locale,theme]=[[1440,'en','light'],[1024,'zh-CN','dark'],[900,'en','dark']][index%3];
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[],pixels=[],previews=[];
    const label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.addInitScript(()=>{
      // OpenLayers can reuse the raster canvas for vector outlines and pixel
      // markers. Observe the real image draw before those overlays; delegate
      // unchanged arguments first, and never alter the rendered canvas.
      const original=CanvasRenderingContext2D.prototype.drawImage;
      CanvasRenderingContext2D.prototype.drawImage=function(...args){
        const result=Reflect.apply(original,this,args),image=args[0],canvas=this.canvas;
        // The pinned ImageStatic decoder returns an ImageBitmap in Edge.
        // Small vector symbol canvases are not raster source draws.
        const raster=(image instanceof HTMLImageElement&&image.src.startsWith('data:image/png;base64,'))||image instanceof HTMLCanvasElement||(typeof ImageBitmap!=='undefined'&&image instanceof ImageBitmap);
        if(raster&&image.width>100&&image.height>100&&canvas.closest('.wm-map')){
          window.__NAIP_RASTER_FRAME={width:canvas.width,height:canvas.height,transform:canvas.style.transform,sourceKind:image.constructor.name,rgba:this.getImageData(0,0,canvas.width,canvas.height).data};
        }
        return result;
      };
    });
    await context.exposeBinding('__naipViewNative',async(_context,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');assert.equal(request.method,'GET');
      const call={method:'GET',path:url.pathname,query:url.search};calls.push(call);
      const started=performance.now();
      const response=await fetch(base+url.pathname+url.search,{signal:AbortSignal.timeout(120000)}),body=Buffer.from(await response.arrayBuffer());
      call.status=response.status;call.elapsedMs=Math.round(performance.now()-started);
      if(response.ok&&url.pathname===`/jobs/${pin.jobId}/raster`){
        const metadata=JSON.parse(body),view=url.searchParams.get('aerialView')||'rgb';
        const expected=pin.views.find(item=>item.view===view);assert(expected);
        assert.equal(metadata.sha256,pin.sha256);assert.deepEqual(metadata.aerial.displayBands,expected.displayBands);
        const png=Buffer.from(metadata.previewDataUrl.split(',')[1],'base64');assert.equal(hash(png),expected.pngSha256);
        previews.push({view,sha256:hash(png)});
      }
      if(response.ok&&url.pathname.endsWith('/pixel'))pixels.push(JSON.parse(body));
      return{status:response.status,headers:Object.fromEntries(response.headers),body:body.toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{
      const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
      const result=await window.__naipViewNative({url,method:init.method||'GET'});
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
      return new Response(Uint8Array.from(atob(result.body),v=>v.charCodeAt(0)),{status:result.status,headers:result.headers});
    };});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push({origin:url.origin,path:url.pathname});return route.abort();}return route.continue();});
    const page=await context.newPage();currentPage=page;page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    async function readPoint(){
      const expected=pixels.length+1;
      await page.getByRole('application').focus();await page.keyboard.press('Enter');
      // A full-file integrity check precedes every raw read. Use the client's
      // actual 60 s limit, and require a fresh native reply rather than an old
      // inspector value that happens to remain in the DOM.
      const deadline=performance.now()+60000;
      while(pixels.length<expected){assert(performance.now()<deadline,'Fresh native raw pixel response timed out');await new Promise(resolve=>setTimeout(resolve,100));}
      await page.locator('.wm-pixel-value').waitFor({timeout:60000});
      assert.equal(pixels.length,expected);return pixels.at(-1);
    }
    await page.goto(origin+'/#Workspace?file='+pin.jobId);
    const select=page.getByRole('combobox',{name:label('Aerial display','航空影像显示方式'),exact:true});
    await select.waitFor({timeout:60000});await stable(page);
    await page.getByRole('button',{name:label('Zoom in','放大'),exact:true}).click();
    await stable(page);
    // OpenLayers may share the image canvas with the blue inspection marker.
    // Compare unmarked raster frames first, then separately verify raw reads.
    const frames={rgb:await painted(page)},loadingObserved=[];
    const prefix=`${pin.group}-${pin.case}-${index}-${width}-${locale}`;
    await page.screenshot({path:path.join(output,prefix+'-rgb.png')});
    for(const [view,en,zh]of [['cir','Colour infrared','红外假彩色'],['nir','Near infrared','近红外']]){
      const before=hash(Buffer.from(frames[view==='cir'?'rgb':'cir'].rgba,'base64'));
      await select.click();await page.getByRole('option',{name:label(en,zh),exact:true}).click();
      loadingObserved.push(await select.isDisabled());
      await page.waitForFunction(({name,label})=>{const el=[...document.querySelectorAll('[role=combobox]')].find(el=>el.getAttribute('aria-label')===name);return el&&!el.disabled&&el.textContent.includes(label);},{name:label('Aerial display','航空影像显示方式'),label:label(en,zh)},{timeout:60000});
      frames[view]=await painted(page,before);
      await page.screenshot({path:path.join(output,prefix+'-'+view+'.png')});
    }
    const rgb=Buffer.from(frames.rgb.rgba,'base64'),cir=Buffer.from(frames.cir.rgba,'base64'),nir=Buffer.from(frames.nir.rgba,'base64');
    for(const [view,frame]of Object.entries(frames))await writeFile(path.join(output,prefix+'-'+view+'-canvas.png'),Buffer.from(frame.png,'base64'));
    for(const frame of Object.values(frames)){assert.equal(frame.width,frames.rgb.width);assert.equal(frame.height,frames.rgb.height);assert.equal(frame.transform,frames.rgb.transform);}
    assert.equal(rgb.length,cir.length);assert.equal(rgb.length,nir.length);
    const differences=[];
    for(let i=0;i<rgb.length;i+=4){
      if(cir[i]!==nir[i]||cir[i+1]!==rgb[i]||cir[i+2]!==rgb[i+1]||nir[i]!==nir[i+1]||nir[i]!==nir[i+2]||cir[i+3]!==rgb[i+3]||nir[i+3]!==rgb[i+3])differences.push({pixel:i/4,rgb:[...rgb.subarray(i,i+4)],cir:[...cir.subarray(i,i+4)],nir:[...nir.subarray(i,i+4)]});
    }
    if(differences.length)await writeFile(path.join(output,prefix+'-canvas-differences.json'),JSON.stringify({pixels:rgb.length/4,count:differences.length,opaqueDifferences:differences.filter(p=>p.rgb[3]===255).length,samples:differences.slice(0,30)},null,2));
    for(let i=0;i<rgb.length;i+=4){
      assert.equal(cir[i],nir[i]);assert.equal(cir[i+1],rgb[i]);assert.equal(cir[i+2],rgb[i+1]);
      assert.equal(nir[i],nir[i+1]);assert.equal(nir[i],nir[i+2]);assert.equal(cir[i+3],rgb[i+3]);assert.equal(nir[i+3],rgb[i+3]);
    }
    const baselinePixel=await readPoint();assert.equal(pixels.length,1);
    for(const view of ['cir','nir','rgb']){
      const options={cir:['Colour infrared','红外假彩色'],nir:['Near infrared','近红外'],rgb:['Natural colour','自然色']};
      const before=hash(Buffer.from((await painted(page)).rgba,'base64'));
      await select.click();await page.getByRole('option',{name:label(...options[view]),exact:true}).click();
      await painted(page,before);
      assert((await page.locator('.wm-pixel-value').textContent()).includes('NIR '+baselinePixel.nearInfrared));
      assert.deepEqual(await readPoint(),baselinePixel);
    }
    await page.screenshot({path:path.join(output,prefix+'-inspector-rgb.png')});
    assert.deepEqual(previews.map(item=>item.view),['rgb','cir','nir']); // Returning to any prior view uses layer memory.
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
    assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.entries.push({jobId:pin.jobId,sha256:pin.sha256,group:pin.group,case:pin.case,width,locale,theme,
      pixel:baselinePixel,rawPixelUnchanged:true,panAndZoomRetained:true,cacheHitForThreeRevisits:true,loadingObserved,
      renderedCanvasPixelsCompared:rgb.length/4,renderedWithoutInteractionOverlay:true,exactCirChannelsEqual:true,exactNirGrayEqual:true,exactAlphaEqual:true,
      rasterDrawObservation:'Captured immediately after the original drawImage; the renderer and vector overlays were left unchanged.',
      canvasSha256:Object.fromEntries(Object.entries(frames).map(([view,frame])=>[view,hash(Buffer.from(frame.rgba,'base64'))])),previews,calls});
    await write();await context.close();console.log(JSON.stringify({stage:'naip-view-ui-case',group:pin.group,case:pin.case,width,locale}));
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);
  report.frontendFiles=[...frontendFiles].sort(([a],[b])=>a.localeCompare(b)).map(([file,sha256])=>({file,sha256}));
  report.status='passed';report.completedAt=new Date().toISOString();await write();
  console.log(JSON.stringify({stage:'naip-views-ui-complete',status:'passed',files:fileCount,nativeWindowTested:false,usedUserDesktop:false}));
}catch(error){
  // A rejected preflight does not own the store and must not overwrite an
  // existing observer's checkpoint while that process is still closing.
  if(!runtime)throw error;
  report.status='failed';report.failure={message:String(error),case:currentPin,checkedAt:new Date().toISOString()};
  if(currentPage&&!currentPage.isClosed()){
    await currentPage.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
    report.failure.visibleText=await currentPage.locator('body').innerText().catch(()=>null);
  }
  await write();throw error;
}finally{
  await browser?.close();server.close();
  if(runtime&&runtime.exitCode===null&&runtime.signalCode===null){assert(!(await api('/jobs')).some(job=>['queued','running'].includes(job.status)));const ended=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await ended;}
}
