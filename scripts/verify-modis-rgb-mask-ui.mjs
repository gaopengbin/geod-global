// Actual built-renderer controls against a private native service. The HTTP
// bridge runs under the desktop CSP; this does not operate a native window.
import {chromium} from 'playwright';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace=process.cwd(),root=path.resolve(process.argv[2]);
const coupled=['modis-coupled-','landsat-coupled-'].some(prefix=>path.basename(root).startsWith(prefix));
const landsat=['landsat-rgb-mask-','landsat-coupled-'].some(prefix=>path.basename(root).startsWith(prefix));
const policy=landsat?'cloud_free_conservative':'clear_best';
const diagnostic=process.argv[3]?.replace(/^--case=/,'');assert(!diagnostic||(coupled?['fallback','fallback-polygon','polygon']:['original','single','polygon']).includes(diagnostic));
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(coupled||landsat||path.basename(root).startsWith('modis-rgb-mask-'));
const fixture=JSON.parse(await readFile(path.join(root,'ui-fixture.json'),'utf8'));
const native=JSON.parse(await readFile(path.join(root,'native-verification.json'),'utf8'));assert.equal(native.status,'passed');
assert.equal(createHash('sha256').update(await readFile(fixture.nativeBinary)).digest('hex'),native.nativeBinarySha256);
const output=path.join(root,'ui');await mkdir(output,{recursive:true});
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const base='http://127.0.0.1:4625',origin='http://127.0.0.1:4626',dist=path.join(workspace,'prototype/dist');
const runtime=spawn(fixture.nativeBinary,['serve','--data-dir',root,'--port','4625'],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let runtimeError='';runtime.stderr.on('data',d=>runtimeError+=d);runtime.stdout.on('data',()=>{});
const report={schema:landsat&&coupled?'geod-landsat-coupled-ui/v1':landsat?'geod-landsat-rgb-mask-ui/v1':coupled?'geod-modis-coupled-ui/v1':'geod-modis-rgb-mask-ui/v1',qaOnly:true,nativeWindowTested:false,renderer:'built renderer and exact desktop CSP, real native HTTP bridge',nativeBinarySha256:native.nativeBinarySha256,
  nativeReceiptSha256:createHash('sha256').update(await readFile(path.join(root,'native-verification.json'))).digest('hex'),cases:[],errors:[],remoteRequests:[],resources:{},thumbnailResponses:[]};
const staticServer=createServer(async(req,res)=>{
  try {const url=new URL(req.url,origin),file=path.resolve(dist,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));assert(file.startsWith(dist+path.sep));
    const data=await readFile(file);report.resources[path.relative(workspace,file).replaceAll('\\','/')]=createHash('sha256').update(data).digest('hex');
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(data);
  } catch {res.writeHead(404);res.end();}
});
async function api(url,body){const r=await fetch(base+url,{method:body?'POST':'GET',headers:{'X-GeoD-Client':'geod-global','Content-Type':'application/json'},body:body?JSON.stringify(body):undefined});assert(r.ok,await r.clone().text());return r.json();}
async function settled(title){for(let i=0;i<6000;i++){const listed=(await api('/jobs')).find(j=>j.title===title);const j=listed?await api('/jobs/'+listed.id):null;assert(!j||!['failed','cancelled','interrupted'].includes(j.status),JSON.stringify(j));if(j?.status==='succeeded'&&j.settled)return j;await new Promise(r=>setTimeout(r,100));}throw new Error('UI job did not settle');}
async function finishEntryMotion(page){await page.evaluate(async()=>{const finite=document.getAnimations().filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().iterations));await Promise.all(finite.map(animation=>animation.finished.catch(()=>{})));});}
let browser,lastPage,lastCalls;
try {
  await new Promise((resolve,reject)=>{staticServer.once('error',reject);staticServer.listen(4626,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{assert.equal(path.resolve((await api('/health')).storageRoot).replace(/^\\\\\?\\/,''),root);break;}catch(e){if(i===99||runtime.exitCode!==null)throw new Error(runtimeError||e.message);await new Promise(r=>setTimeout(r,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});
  const cases=(coupled?[['fallback',1440,'en','light'],['fallback-polygon',1024,'zh-CN','dark'],['polygon',900,'en','dark']]:[['original',1440,'en','light'],['single',1024,'zh-CN','dark'],['polygon',900,'en','dark']]).filter(c=>!diagnostic||c[0]===diagnostic);
  for (const [kind,width,locale,theme] of cases) {
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[],inspections=new Map();const label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__MASK_CSP=[];document.addEventListener('securitypolicyviolation',e=>window.__MASK_CSP.push(e.violatedDirective));},{locale,theme});
    await context.exposeBinding('__maskNative',async(_s,req)=>{const url=new URL(req.url);assert.equal(url.origin,'http://127.0.0.1:4318');const r=await fetch(base+url.pathname+url.search,{method:req.method,body:req.body,headers:req.headers});
      const body=await r.arrayBuffer();let value;try{value=JSON.parse(Buffer.from(body).toString());}catch{}if(url.pathname.endsWith('/rgb')&&value?.previewDataUrl)inspections.set(value.artifact?.jobId,value);calls.push({method:req.method,path:url.pathname,body:req.body?JSON.parse(req.body):undefined,pixel:url.pathname.endsWith('/rgb/pixel')?value:undefined,
        preflight:url.pathname==='/rasters/rgb/plan'?{status:r.status,error:value?.error,qualityMask:value?.spec?.qualityMask}:undefined});
      if(url.pathname.endsWith('/thumbnail'))report.thumbnailResponses.push({id:url.pathname.split('/')[2],status:r.status,jobId:value?.jobId,sha256:value?.sha256,width:value?.width,height:value?.height,error:value?.error});
      return {status:r.status,headers:Object.fromEntries(r.headers),body:Buffer.from(body).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__maskNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
    await context.addInitScript(()=>{window.__MASK_DRAW_LOG=[];const original=CanvasRenderingContext2D.prototype.drawImage;CanvasRenderingContext2D.prototype.drawImage=function(source,...args){const result=Reflect.apply(original,this,[source,...args]);const raster=source instanceof ImageBitmap||(source instanceof HTMLImageElement&&source.currentSrc.startsWith('data:image/png;base64,'));const inMap=Boolean(this.canvas.closest?.('.wm-map'));if(inMap&&window.__MASK_DRAW_LOG.length<50)window.__MASK_DRAW_LOG.push({width:source.width,height:source.height,kind:source.constructor.name,raster});if(raster&&source.width>20&&source.height>=20){const copy=new OffscreenCanvas(source.width,source.height),ctx=copy.getContext('2d');ctx.drawImage(source,0,0);const frame=this.getImageData(0,0,this.canvas.width,this.canvas.height).data;let painted=0;for(let i=3;i<frame.length;i+=4)painted+=Number(frame[i]>0);window.__MASK_DRAWN={width:source.width,height:source.height,rgba:ctx.getImageData(0,0,source.width,source.height).data,sourceKind:source.constructor.name,painted};}return result;};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();lastPage=page;lastCalls=calls;page.on('pageerror',e=>report.errors.push(e.message));page.on('console',m=>{if(m.type()==='error')report.errors.push(m.text());});
    if (kind===(coupled?'fallback':'original')) {
      const layers=coupled?fixture.cases.fallback.jobs:fixture.originals;
      await page.goto(origin+'/#Workspace?rgb='+layers[0].id);await page.getByRole('button',{name:'Create scientific RGB',exact:true}).waitFor({timeout:60000});await page.getByRole('button',{name:'Create scientific RGB',exact:true}).click();
      const name=page.getByRole('textbox',{name:'Result name'});await name.waitFor({timeout:60000});const title='QA · UI quality-screened RGB · '+randomUUID().slice(0,8);await name.fill(title);
      await page.getByRole('combobox',{name:'Quality screening'}).click();await page.getByRole('option',{name:landsat?'Conservative cloud-free flags':'Clear pixels with best RGB quality',exact:true}).click();
      const snow=page.getByRole('checkbox',{name:'Also exclude snow and ice'});await snow.waitFor();await snow.check();
      await page.getByRole('button',{name:'Create RGB file',exact:true}).waitFor({timeout:60000});await page.waitForFunction(()=>[...document.querySelectorAll('button')].some(b=>b.textContent.includes('Create RGB file')&&!b.disabled));assert.equal(await name.inputValue(),title);
      await finishEntryMotion(page);await page.screenshot({path:path.join(output,'quality-rules-dialog-1440-en.png')});
      await page.getByRole('button',{name:'Create RGB file',exact:true}).click();await page.getByRole('dialog').waitFor({state:'hidden',timeout:60000});
      const job=await settled(title),mask=job.rgbSpec.qualityMask;
      assert.deepEqual([mask.policy,mask.excludeSnow,mask.sources[0].jobId,mask.sources[1].jobId],[policy,true,layers[3].id,layers[4].id]);
      if(coupled){assert.equal(mask.schemaVersion,landsat?'geod-landsat-rgb-mask/v2':'geod-modis-rgb-mask/v2');assert.equal(mask.coupled.scenes.length,2);}
      const reference=native.cases.find(c=>c.case===kind&&c.policy===policy&&c.excludeSnow);
      assert.deepEqual(job.rgbOutput,reference.job.rgbOutput);assert.deepEqual(calls.filter(c=>c.path==='/rasters/rgb').at(-1).body.qualityMask,{...(landsat?{qaPixelJobId:layers[3].id,qaRadsatJobId:layers[4].id}:{qcJobId:layers[3].id,stateJobId:layers[4].id}),policy,excludeSnow:true});
      report.createdByUi={job,sourceSamplesSha256:reference.job.rgbOutput.samplesSha256};
      const preflights=calls.filter(c=>c.preflight),selected=preflights.filter(c=>c.body.qualityMask?.policy===policy&&c.body.qualityMask?.excludeSnow);
      assert(selected.some(c=>c.preflight.status===200));
      report.preflightReadRecovery={busyReplies:preflights.filter(c=>c.preflight.error==='Raster inspection is busy; try again shortly').length,
        latestSelectedRulesRecovered:true,editedNamePreserved:job.title===title,
        attempts:preflights.map(c=>({policy:c.body.qualityMask?.policy||'none',excludeSnow:Boolean(c.body.qualityMask?.excludeSnow),status:c.preflight.status,error:c.preflight.error}))};
    }
    const job=kind===(coupled?'fallback':'original')?report.createdByUi.job:(coupled?native:fixture).cases.find(c=>c.case===kind&&c.policy===policy&&c.excludeSnow).job;
    // Force a fresh document so the newly completed native job is fetched,
    // rather than racing the earlier queued snapshot during a hash-only change.
    await page.goto(origin+'/?case='+kind+'#Workspace?file='+job.id);
    await page.getByText(job.title,{exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor();
    const metadata=await page.locator('.wm-layer.active .wm-layer-metadata').innerText();
    assert.equal(metadata.split(label('Scientific RGB','科学 RGB')).length-1,1);assert(!metadata.includes(' × '));
    await page.locator('.wm-map canvas').first().waitFor();
    await page.getByText(label('Quality-masked RGB · accepted original DN available','质量筛选 RGB · 可检查合格像元的原始 DN'),{exact:true}).waitFor({timeout:60000});
    const inspection=inspections.get(job.id);assert(inspection);
    await page.waitForFunction(({width,height})=>window.__MASK_DRAWN?.width===width&&window.__MASK_DRAWN?.height===height&&window.__MASK_DRAWN.painted>100,{width:inspection.previewWidth,height:inspection.previewHeight},{timeout:diagnostic?15000:60000});
    const actualDraw=await page.evaluate(async url=>{const bytes=Uint8Array.from(atob(url.split(',')[1]),c=>c.charCodeAt(0)),image=await createImageBitmap(new Blob([bytes],{type:'image/png'})),copy=new OffscreenCanvas(image.width,image.height),ctx=copy.getContext('2d');ctx.drawImage(image,0,0);const expected=ctx.getImageData(0,0,image.width,image.height).data,drawn=window.__MASK_DRAWN;const identical=expected.length===drawn.rgba.length&&expected.every((value,i)=>value===drawn.rgba[i]);image.close();return {width:drawn.width,height:drawn.height,sourceKind:drawn.sourceKind,painted:drawn.painted,allSourceRgbaIdentical:identical};},inspection.previewDataUrl);assert(actualDraw.allSourceRgbaIdentical);
    const canvases=await page.locator('.wm-map canvas').evaluateAll(items=>items.map(c=>{const pixels=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let painted=0;for(let i=3;i<pixels.length;i+=4)painted+=Number(pixels[i]>0);return {width:c.width,height:c.height,painted};}));assert(canvases.some(c=>c.painted>10000));
    const drawnPreviewSha256=createHash('sha256').update(Buffer.from(inspection.previewDataUrl.split(',')[1],'base64')).digest('hex');
    assert.equal(drawnPreviewSha256,native.cases.find(c=>c.case===kind&&c.policy===policy&&c.excludeSnow).preview.pngSha256);
    await page.getByRole('application').focus();await page.keyboard.press('Enter');await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    const pixel=calls.filter(c=>c.path===`/jobs/${job.id}/rgb/pixel`).at(-1)?.pixel;assert(pixel);assert.equal(pixel.artifact.sha256,job.sha256);assert.deepEqual(pixel.channelNoData,pixel.values.map(v=>v===(landsat?0:-28672)));
    await page.getByText(label('The saved RGB includes quality screening. Accepted DN are unchanged and rejected pixels are NoData. Display stretching only changes the preview.','此 RGB 已应用质量筛选。合格像元的 DN 保持不变，剔除像元为 NoData；显示拉伸仅改变预览。'),{exact:true}).waitFor();
    assert.equal(await page.locator('.wm-map-caption').count(),0);await finishEntryMotion(page);
    const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);assert(!overflow);assert.deepEqual(await page.evaluate(()=>window.__MASK_CSP),[]);
    const screenshot=`masked-${kind}-${width}-${locale}.png`;await page.screenshot({path:path.join(output,screenshot)});
    let sourceDetails,libraryThumbnail;
    if(coupled||landsat){
      let occupiedWorker;
      if(landsat&&kind==='original'){
        // A real request owns the native worker before the page requests this
        // uncached file. No thumbnail response or busy reply is synthesized.
        occupiedWorker=api(`/jobs/${job.id}/thumbnail`).then(data=>{
          assert.equal(data.jobId,job.id);assert.equal(data.sha256,job.sha256);
          report.busyWorker={id:job.id,sha256:data.sha256,width:data.width,height:data.height,
            pngSha256:createHash('sha256').update(Buffer.from(data.dataUrl.split(',')[1],'base64')).digest('hex')};
        });
        await new Promise(resolve=>setTimeout(resolve,100));
      }
      await page.goto(origin+'/#My%20Data?view=files');
      const card=page.locator('[data-slot="task-row"]').filter({has:page.getByText(job.title,{exact:true})}).first();
      await card.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();
      if(landsat){
      const rules=label('Conservative cloud-free flags','严格晴空标记筛选');
      await card.getByText(rules,{exact:false}).waitFor();
      for(const pin of job.rgbSpec.qualityMask.sources)await card.getByText('SHA-256 · '+pin.sha256,{exact:true}).waitFor();
       const displayedQualityCounts={};
       for(const [key,en,zh] of [['rejectedPixels',coupled?'Pixels without qualified RGB':'Quality-rejected pixels',coupled?'无合格 RGB 的像元':'被质量规则剔除的像元'],['removedValidPixels','Valid pixels removed','剔除的原有效像元']]){
         const term=card.getByText(label(en,zh),{exact:true});await term.waitFor();
         const value=(await term.locator('xpath=following-sibling::dd[1]').innerText()).trim();
         assert.equal(value,new Intl.NumberFormat(locale).format(job.rgbOutput.qualityMask[key]));
         displayedQualityCounts[key]=job.rgbOutput.qualityMask[key];
       }
       sourceDetails={qualityPins:job.rgbSpec.qualityMask.sources.length,selectedPolicy:policy,displayedQualityCounts};
      }
      if(coupled){
      await card.getByRole('button',{name:label('Original scene selection','原始景选择与来源'),exact:true}).click();
      await card.getByText(label('Quality fallback pixels','按质量规则回退的像元'),{exact:true}).waitFor();
      const pins=job.rgbSpec.qualityMask.coupled.scenes.flatMap(s=>s.sources);
      for(const pin of pins)await card.getByText('SHA-256 · '+pin.sha256,{exact:true}).first().waitFor();
      sourceDetails={...sourceDetails,originalLinks:await card.getByRole('link',{name:label('Original source file','原始源文件'),exact:true}).count(),pinnedOriginals:pins.length};
      assert.equal(sourceDetails.originalLinks,pins.length);
       }await finishEntryMotion(page);
       if(landsat){
         const preview=card.locator('.runtime-file-thumbnail img');await preview.waitFor({timeout:120000});
         libraryThumbnail=await preview.evaluate(img=>({complete:img.complete,width:img.naturalWidth,height:img.naturalHeight,dataUrl:img.currentSrc}));
         assert(libraryThumbnail.complete&&libraryThumbnail.width>0&&libraryThumbnail.height>0);
         libraryThumbnail.pngSha256=createHash('sha256').update(Buffer.from(libraryThumbnail.dataUrl.split(',')[1],'base64')).digest('hex');delete libraryThumbnail.dataUrl;
         if(occupiedWorker){
           await occupiedWorker;
           assert(report.thumbnailResponses.some(response=>response.id===job.id&&response.status===400&&response.error==='Preview worker is busy. Try again shortly.'));
           assert.equal(report.busyWorker.pngSha256,libraryThumbnail.pngSha256);
         }
       }
      assert(!(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)));
      await page.screenshot({path:path.join(output,`sources-${kind}-${width}-${locale}.png`)});
    }
    report.cases.push({kind,width,locale,theme,id:job.id,sha256:job.sha256,mask:job.rgbSpec.qualityMask,counts:job.rgbOutput.qualityMask,pixel,canvases,actualDraw,drawnPreviewSha256,screenshot,sourceDetails,libraryThumbnail,horizontalOverflow:false});await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status='passed';report.diagnostic=Boolean(diagnostic);await writeFile(path.join(root,diagnostic?`ui-diagnostic-${diagnostic}.json`:'ui-verification.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({status:'passed',cases:report.cases.length,createdByUi:report.createdByUi?.job.id,resources:Object.keys(report.resources).length}));
} catch(error) {
  report.status='failed';report.failure=error.message;report.lastCalls=lastCalls;
  if(lastPage&&!lastPage.isClosed()){
    const details=lastPage.getByRole('dialog').getByRole('button',{name:/^(Technical details|技术详情)$/}).first();
    if(await details.isVisible())await details.click();
    await lastPage.screenshot({path:path.join(output,'failure.png')});report.visibleFailure=await lastPage.locator('body').innerText();report.drawLog=await lastPage.evaluate(()=>window.__MASK_DRAW_LOG);report.drawnFrame=await lastPage.evaluate(()=>{const d=window.__MASK_DRAWN;return d?{width:d.width,height:d.height,painted:d.painted,kind:d.sourceKind}:null;});}
  if(lastPage&&!lastPage.isClosed())report.mapDiagnosis=await lastPage.evaluate(()=>{const el=document.querySelector('.wm-map');if(!el)return null;let fiber=el[Object.keys(el).find(k=>k.startsWith('__reactFiber$'))];for(let level=0;fiber&&level<30;fiber=fiber.return,level++){let hook=fiber.memoizedState;for(let i=0;hook&&i<100;hook=hook.next,i++){const map=hook.memoizedState?.current;if(typeof map?.getView==='function'&&typeof map?.getLayers==='function')return {view:map.getView().getState(),size:map.getSize(),hints:map.getView().getHints(),frameExtent:map.frameState_?.extent,layers:map.getLayers().getArray().map(l=>{const s=l.getSource(),r=l.getRenderer(),image=s?.image?.getImage?.();return {visible:l.getVisible(),opacity:l.getOpacity(),extent:s?.getImageExtent?.(),imageState:s?.image?.getState?.(),sourceResolution:s?.image?.getResolution?.(),imageDimensions:image?[image.width,image.height]:null,rendererImageState:r.image?.getState?.(),rendererImageResolution:r.image?.getResolution?.(),rendererCanvas:r.context?.canvas?[r.context.canvas.width,r.context.canvas.height]:null,minResolution:l.getMinResolution(),maxResolution:l.getMaxResolution(),minZoom:l.getMinZoom(),maxZoom:l.getMaxZoom()};})};}}return null;});
  await writeFile(path.join(root,'ui-failure-control.json'),JSON.stringify(report,null,2));throw error;
} finally {
  await browser?.close();await new Promise(resolve=>staticServer.close(resolve));if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}
  await writeFile(path.join(root,'ui-runtime.stderr.log'),runtimeError);
}
