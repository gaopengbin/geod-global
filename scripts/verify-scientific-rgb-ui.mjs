// Headless QA of the built renderer against an isolated, real Rust task service.
// The loopback HTTP adapter is bridged; this is not a native WebView/window test.
import {chromium} from 'playwright';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';

const root=process.cwd(),qa=path.resolve(process.argv[2]||'.verification/scientific-rgb-20261003c');
assert.equal(path.dirname(qa),path.join(root,'.verification'));
const output=path.join(qa,'ui');await mkdir(output,{recursive:true});
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const fixture=JSON.parse(await readFile(path.join(qa,'ui-fixture.json'),'utf8'));
const server='http://127.0.0.1:4599',origin='http://127.0.0.1:4597';
const resultName='QA · UI scientific RGB · '+randomUUID().slice(0,8);
const runtime=spawn(path.join(root,'target/debug/geod-runtime.exe'),['serve','--data-dir',qa,'--port','4599'],{windowsHide:true,stdio:['ignore','pipe','pipe']});
let runtimeError='';runtime.stderr.on('data',d=>runtimeError+=d);
const staticServer=createServer(async(req,res)=>{
  try{
    const u=new URL(req.url,origin),p=path.resolve(root,'prototype/dist','.'+decodeURIComponent(u.pathname==='/'?'/index.html':u.pathname));
    assert.ok(p.startsWith(path.join(root,'prototype/dist')+path.sep));
    const body=await readFile(p),ext=path.extname(p);res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml'})[ext]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(body);
  }catch{res.writeHead(404);res.end();}
});
let browser;
const report={schema:'geod-scientific-rgb-ui/v1',renderer:'production built renderer with exact desktop CSP',nativeWindowTested:false,cases:[],remoteRequests:[],errors:[]};
async function api(path,body){const r=await fetch(server+path,{method:body?'POST':'GET',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:body?JSON.stringify(body):undefined});assert.ok(r.ok,await r.clone().text());return r.json();}
try{
  await new Promise((resolve,reject)=>{staticServer.once('error',reject);staticServer.listen(4597,'127.0.0.1',resolve);});
  for(let i=0;i<100;i++){try{await api('/health');break;}catch(e){if(i===99||runtime.exitCode!==null)throw new Error(runtimeError||e.message);await new Promise(r=>setTimeout(r,100));}}
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const[width,locale,theme]of[[1440,'en','light'],[1024,'zh-CN','dark'],[900,'en','dark']]){
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[];const label=(en,zh)=>locale==='en'?en:zh;
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',e=>window.__CSP_ERRORS.push(e.violatedDirective));},{locale,theme});
    await context.exposeBinding('__rgbNative',async(_source,request)=>{
      const u=new URL(request.url);assert.equal(u.origin,'http://127.0.0.1:4318');calls.push({method:request.method,path:u.pathname});
      const r=await fetch(server+u.pathname+u.search,{method:request.method,body:request.body,headers:request.headers});return{status:r.status,headers:Object.fromEntries(r.headers),body:Buffer.from(await r.arrayBuffer()).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');const r=await window.__rgbNative({url,method:init.method||'GET',body:init.body,headers:Object.fromEntries(new Headers(init.headers))});if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');return new Response(Uint8Array.from(atob(r.body),c=>c.charCodeAt(0)),{status:r.status,headers:r.headers});};});
    await context.route('**/*',route=>{const u=new URL(route.request().url());if(u.origin!==origin){report.remoteRequests.push(u.href);return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',e=>report.errors.push(e.message));page.on('console',m=>{if(m.type()==='error')report.errors.push(m.text());});
    await page.goto(origin+'/#Workspace?file='+fixture.derived.id);
    await page.getByText(fixture.derived.title,{exact:true}).first().waitFor({timeout:60000});
    await page.getByRole('button',{name:label('Layer details','图层详情'),exact:true}).first().waitFor();
    await page.locator('.wm-map canvas').first().waitFor();
    await page.getByRole('application').focus();await page.keyboard.press('Enter');
    await page.locator('.wm-pixel-value').waitFor({timeout:60000});
    assert.ok(calls.some(c=>c.path===`/jobs/${fixture.derived.id}/rgb/pixel`));
    await page.waitForFunction(()=>[...document.querySelectorAll('.wm-map canvas')].some(canvas=>{
      const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;
      const pixels=ctx.getImageData(0,0,canvas.width,canvas.height).data;
      let painted=0;for(let i=3;i<pixels.length;i+=400)if(pixels[i]!==0)painted++;
      return painted>100;
    }),null,{timeout:60000});
    await page.screenshot({path:path.join(output,`saved-rgb-${width}-${locale}.png`)});
    if(width===1440){
      await page.goto(origin+'/#Workspace?rgb='+fixture.sourceIds[0]+'&project='+fixture.projectId);
      await page.getByRole('button',{name:'Create scientific RGB',exact:true}).waitFor({timeout:60000});await page.getByRole('button',{name:'Create scientific RGB',exact:true}).click();
      const name=page.getByRole('textbox',{name:'Result name'});await name.waitFor({timeout:60000});await name.fill(resultName);
      await page.screenshot({path:path.join(output,'rgb-review-dialog.png')});await page.getByRole('button',{name:'Create RGB file',exact:true}).click();
      await page.getByRole('dialog').waitFor({state:'hidden',timeout:60000});
      let job;for(let i=0;i<1200;i++){job=(await api('/jobs')).find(j=>j.title===resultName);if(job&&job.status==='succeeded')break;assert.ok(!job||!['failed','cancelled','interrupted'].includes(job.status),JSON.stringify(job));await new Promise(r=>setTimeout(r,100));}
      assert.equal(job?.status,'succeeded');report.createdByUi={id:job.id,sha256:job.sha256,bytes:job.bytesDownloaded};
    }
    await page.goto(origin+'/#My%20Data?view=files');await page.getByText(resultName,{exact:true}).first().waitFor({timeout:60000});
    const row=page.getByRole('listitem').filter({has:page.getByText(resultName,{exact:true})});
    await row.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();
    await row.getByRole('button',{name:label('Prepare delivery package','准备交付包'),exact:true}).click();
    await row.getByText(label('Verified delivery package','已核验的交付包'),{exact:true}).waitFor({timeout:60000});
    await row.getByRole('button',{name:label('File details and provenance','文件详情与来源'),exact:true}).click();
    await row.locator('.bui-task-expanded > [data-slot=disclosure-content]').waitFor({state:'hidden'});
    await page.screenshot({path:path.join(output,`library-${width}-${locale}.png`)});
    const geometry=await page.evaluate(()=>[...document.querySelectorAll('[data-slot=task-row]')].map(e=>{const r=e.getBoundingClientRect();return {height:r.height,right:r.right,bottom:r.bottom};}));
    assert.ok(geometry.length>0&&geometry.every(g=>g.right<=width+1));assert.ok(Math.max(...geometry.map(g=>g.height))-Math.min(...geometry.map(g=>g.height))<2,'Collapsed cards need equal heights');
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
    assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({width,locale,theme,savedFileOpenedWithoutParents:true,paintedRaster:true,pixelRead:true,deliveryPackaged:true,cards:geometry.length,cardHeight:geometry[0].height,calls});await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status='passed';await writeFile(path.join(output,'verification.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,cases:report.cases.length,createdByUi:report.createdByUi,errors:report.errors,remoteRequests:report.remoteRequests}));
}finally{await browser?.close();staticServer.close();if(runtime.exitCode===null){runtime.kill();await new Promise(resolve=>runtime.once('exit',resolve));}}
