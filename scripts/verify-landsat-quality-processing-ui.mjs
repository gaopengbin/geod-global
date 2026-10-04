// Headless built renderer + desktop CSP + real native bridge. No native window operation.
import {chromium} from 'playwright';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
assert(process.argv[2], 'Usage: node scripts/verify-landsat-quality-processing-ui.mjs .verification/landsat-quality-<cohort>');
const workspace=process.cwd(),root=path.resolve(process.argv[2]),output=path.join(root,'ui');
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('landsat-quality-'));
const nativeReceipt=await readFile(path.join(root,'native-processing-verification.json'));
const oracleReceipt=await readFile(path.join(root,'independent-processing-verification.json'));
const native=JSON.parse(nativeReceipt),oracle=JSON.parse(oracleReceipt);
assert.equal(native.status,'passed');assert.equal(oracle.status,'passed');
const sha=data=>createHash('sha256').update(data).digest('hex');assert.equal(sha(await readFile(native.nativeBinary)),native.nativeBinarySha256);
assert.equal(oracle.nativeReceiptSha256,sha(nativeReceipt));
await mkdir(output,{recursive:true});const dist=path.join(workspace,'prototype/dist'),config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const base='http://127.0.0.1:4635',origin='http://127.0.0.1:4636';
const runtime=spawn(native.nativeBinary,['serve','--data-dir',root,'--port','4635'],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';runtime.stderr.on('data',d=>stderr+=d);runtime.stdout.on('data',()=>{});
const report={schema:'geod-landsat-quality-processing-ui/v1',nativeBinarySha256:native.nativeBinarySha256,nativeReceiptSha256:sha(nativeReceipt),independentReceiptSha256:sha(oracleReceipt),qaOnly:true,nativeWindowTested:false,renderer:'built renderer, exact desktop CSP and real native HTTP bridge',cases:[],errors:[],remoteRequests:[],resources:{}};
const server=createServer(async(req,res)=>{try{const url=new URL(req.url,origin),file=path.resolve(dist,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));assert(file.startsWith(dist+path.sep));const data=await readFile(file);report.resources[path.relative(workspace,file).replaceAll('\\','/')]=sha(data);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(data);}catch{res.writeHead(404);res.end();}});
let browser,lastPage,activeCalls=[];
async function motion(page){await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().iterations)).map(a=>a.finished.catch(()=>{})));});}
try {
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(4636,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{const r=await fetch(base+'/health');assert(r.ok);break;}catch(e){if(i===99||runtime.exitCode!==null)throw e;await new Promise(r=>setTimeout(r,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [caseName,key,width,locale,theme] of [['single','qa_pixel',1440,'en','light'],['polygon','qa_radsat',1024,'zh-CN','dark'],['large','qa_pixel',900,'zh-CN','dark']]) {
    let source=native.outputs.find(c=>c.case===caseName&&c.key===key);const expected=oracle.cases.find(c=>c.jobId===source.job.id),calls=[];activeCalls=calls;
    const context=await browser.newContext({viewport:{width,height:960}});
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__QA_CSP=[];document.addEventListener('securitypolicyviolation',e=>window.__QA_CSP.push(e.violatedDirective));},{locale,theme});
    await context.exposeBinding('__qualityNative',async(_s,req)=>{const url=new URL(req.url);assert.equal(url.origin,'http://127.0.0.1:4318');const r=await fetch(base+url.pathname+url.search,{method:req.method,headers:req.headers,body:req.body});const bytes=await r.arrayBuffer();let body;try{body=JSON.parse(Buffer.from(bytes).toString());}catch{}calls.push({path:url.pathname,method:req.method,status:r.status,error:body?.error,created:url.pathname.endsWith('/mosaics')?body:undefined,pixel:url.pathname.endsWith('/pixel')?body:undefined});return {status:r.status,headers:Object.fromEntries(r.headers),body:Buffer.from(bytes).toString('base64')};});
    await context.addInitScript(()=>{const original=window.fetch;window.fetch=async(input,init)=>{const request=new Request(input,init);if(new URL(request.url).origin!=='http://127.0.0.1:4318')return original(input,init);const result=await window.__qualityNative({url:request.url,method:request.method,headers:Object.fromEntries(request.headers),body:['GET','HEAD'].includes(request.method)?undefined:await request.text()});return new Response(Uint8Array.from(atob(result.body),c=>c.charCodeAt(0)),{status:result.status,headers:result.headers});};});
    await context.addInitScript(()=>{const original=CanvasRenderingContext2D.prototype.drawImage;CanvasRenderingContext2D.prototype.drawImage=function(image,...args){const result=Reflect.apply(original,this,[image,...args]);if((image instanceof ImageBitmap||image instanceof HTMLImageElement&&image.currentSrc.startsWith('data:image/png;base64,'))&&image.width>20&&image.height>20){const copy=new OffscreenCanvas(image.width,image.height),ctx=copy.getContext('2d');ctx.drawImage(image,0,0);const frame=this.getImageData(0,0,this.canvas.width,this.canvas.height).data;let painted=0;for(let i=3;i<frame.length;i+=4)painted+=Number(frame[i]>0);window.__QA_FRAME={width:image.width,height:image.height,rgba:ctx.getImageData(0,0,image.width,image.height).data,painted};}return result;};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();lastPage=page;page.on('pageerror',e=>report.errors.push(e.message));page.on('console',m=>{if(m.type()==='error')report.errors.push(m.text());});
    const label=(en,zh)=>locale==='en'?en:zh;
    if(width===1440){
      const project=native.cases.find(c=>c.name===caseName).project;
      await page.goto(origin+`/#My%20Data?project=${project.id}`);
      const buttons=page.getByRole('button',{name:'Clip quality to project area',exact:true});await buttons.first().waitFor({timeout:60000});assert.equal(await buttons.count(),2);assert(await buttons.first().isEnabled());
      await page.waitForFunction(()=>{const items=[...document.querySelectorAll('.runtime-file-thumbnail')];return items.length>0&&items.every(item=>{const image=item.querySelector('img');return image?.complete&&image.naturalWidth>0;});},{timeout:60000});
      report.projectThumbnailsLoaded=await page.locator('.runtime-file-thumbnail img').count();
      await motion(page);await page.screenshot({path:path.join(output,'quality-project-1440-en.png')});
      await buttons.first().click();
      for(let i=0;i<200&&!calls.some(c=>c.created?.id);i++)await new Promise(r=>setTimeout(r,100));
      const queued=calls.findLast(c=>c.created?.id)?.created;assert(queued,'The project button did not queue a real native job');
      let generated;
      for(let i=0;i<200;i++){const response=await fetch(base+`/jobs/${queued.id}`);generated=await response.json();assert(!['failed','interrupted','cancelled'].includes(generated.status),JSON.stringify(generated));if(generated.status==='succeeded'&&generated.settled)break;await new Promise(r=>setTimeout(r,100));}
      assert.equal(generated.status,'succeeded');assert.equal(generated.sha256,source.job.sha256);assert.deepEqual(generated.mosaic,source.job.mosaic);assert.deepEqual(generated.mosaicOutput,source.job.mosaicOutput);
      report.projectButton={operation:'mosaicProject',projectId:project.id,key,jobId:generated.id,outputSha256:generated.sha256,independentReferenceJobId:source.job.id,identicalVerifiedTiff:true};
      source={...source,job:generated};await page.goto(origin+`/#Workspace?file=${generated.id}`);
    }else await page.goto(origin+`/#Workspace?file=${source.job.id}`);
    await page.waitForFunction(()=>window.__QA_FRAME?.painted>100 || document.querySelector('.wm-error'),{timeout:60000});
    if(await page.locator('.wm-error').count()){const detail=page.locator('.wm-error').getByRole('button',{name:label('Technical details','技术详情')});if(await detail.count())await detail.click();throw new Error(await page.locator('.wm-error').innerText());}
    await page.waitForFunction(({w,h})=>window.__QA_FRAME?.width===w&&window.__QA_FRAME?.height===h&&window.__QA_FRAME.painted>100,{w:source.metadata.previewWidth,h:source.metadata.previewHeight},{timeout:60000});
    const frame=await page.evaluate(()=>({width:window.__QA_FRAME.width,height:window.__QA_FRAME.height,rgba:Array.from(window.__QA_FRAME.rgba),painted:window.__QA_FRAME.painted}));assert.equal(sha(Buffer.from(frame.rgba)),expected.preview.rgbaSha256);
    const map=page.locator('.wm-map');await map.focus();await map.press('Enter');await page.waitForFunction(()=>document.querySelector('.wm-pixel-value'),{timeout:60000});
    const pixel=calls.findLast(c=>c.pixel)?.pixel;assert(pixel&&pixel.quality&&pixel.quality.fields.length>0);assert.equal(pixel.isNoData,!pixel.quality.covered);assert.equal(pixel.value,Number.parseInt(pixel.quality.hex.slice(2),16));
    const decode=page.getByRole('button',{name:label('Decode quality flags','解码质量标记'),exact:true});await decode.click();await motion(page);
    await page.screenshot({path:path.join(output,`${key}-pixel-${width}-${locale}.png`)});
    const legend=page.getByRole('button',{name:label(key==='qa_pixel'?'Pixel quality flags':'Saturation and terrain flags',key==='qa_pixel'?'像元质量标志':'饱和与地形遮挡标志'),exact:true});await legend.click();
    await page.getByRole('button',{name:label('Full-resolution flag counts','完整分辨率标志统计'),exact:true}).click();await motion(page);
    const counts=page.locator('[data-quality-counts]');await counts.waitFor();let binsChecked=0;
    for(const field of source.metadata.quality.flags.fields)for(let code=0;code<field.counts.length;code++)if(field.counts[code]>0){const row=counts.locator(`[data-quality-field=\"${field.name}\"] [data-quality-code=\"${code}\"]`);assert((await row.innerText()).includes(new Intl.NumberFormat(locale).format(field.counts[code])));binsChecked++;}
    await counts.locator('dd').first().scrollIntoViewIfNeeded();
    await page.screenshot({path:path.join(output,`${key}-flags-${width}-${locale}.png`)});
    const link=page.getByRole('link',{name:label('USGS quality flag definitions','USGS 质量标志定义'),exact:true});assert.equal(await link.getAttribute('href'),source.metadata.quality.definition);
    assert.equal(await page.evaluate(()=>window.__QA_CSP.length),0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false);
    report.cases.push({case:caseName,jobId:source.job.id,outputSha256:source.job.sha256,key,width,locale,theme,rgbaSourcePixelsCompared:frame.width*frame.height,mapPaintedPixels:frame.painted,rawPixelValue:pixel.value,decodedFields:pixel.quality.fields.length,flagCountBinsChecked:binsChecked,sourceDefinition:await link.getAttribute('href'),nativeRequests:calls.length});await context.close();
  }
  assert.equal(report.errors.length,0,JSON.stringify(report.errors));assert.equal(report.remoteRequests.length,0,JSON.stringify(report.remoteRequests));report.status='passed';await writeFile(path.join(root,'ui-processing-verification.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,cases:report.cases.length,sourcePixelsCompared:report.cases.reduce((n,c)=>n+c.rgbaSourcePixelsCompared,0),nativeWindowTested:false}));
}catch(error){report.status='failed';report.failure=error.message;report.lastNativeRequests=activeCalls.map(({created,pixel,...c})=>c);if(lastPage){await lastPage.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});await writeFile(path.join(output,'failure-dom.txt'),await lastPage.locator('body').innerText().catch(()=>''));}await writeFile(path.join(root,'ui-processing-verification.json'),JSON.stringify(report,null,2));throw error;}
finally{await browser?.close();await new Promise(r=>server.close(r));if(runtime.exitCode===null){runtime.kill();await new Promise(r=>runtime.once('exit',r));}await writeFile(path.join(output,'native.stderr.log'),stderr);}
