// Actual saved public TMS images in the production desktop renderer and CSP.
// No user window, synthetic pixels, public tile requests or native writes.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {chromium} from 'playwright';

const root=path.resolve(process.argv[2]),binary=path.resolve(process.argv[3]);
const port=Number(process.argv[4]||4614),uiPort=Number(process.argv[5]||4615),workspace=process.cwd();
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('tms-public-'));
const native=JSON.parse(await readFile(path.join(root,'evidence/report.json'),'utf8'));
assert.equal(native.status,'passed');assert.equal(createHash('sha256').update(await readFile(binary)).digest('hex'),native.nativeBinarySha256);
const output=path.join(root,'ui');await mkdir(output,{recursive:true});
const base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`;
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const report={schema:'geod-tms-public-ui/v1',status:'running',nativeBinarySha256:native.nativeBinarySha256,
  usedUserDesktop:false,nativeWindowTested:false,readOnly:true,renderer:'production build and exact desktop CSP; actual native API via loopback bridge',
  cases:[],errors:[],remoteRequests:[]};
const files=new Map();let runtime,browser,runtimeError='';
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    const raw=await readFile(file);files.set(path.relative(path.join(workspace,'prototype/dist'),file).replaceAll(path.sep,'/'),createHash('sha256').update(raw).digest('hex'));
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.woff2':'font/woff2'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(raw);
  }catch{res.writeHead(404);res.end();}
});
async function api(route){const response=await fetch(base+route,{signal:AbortSignal.timeout(10000)});const value=await response.json();assert(response.ok,JSON.stringify(value));return value;}
async function owner(){const health=await api('/health');assert.equal(path.toNamespacedPath(path.resolve(health.storageRoot)),path.toNamespacedPath(path.join(root,'store')));}
async function paint(page){
  await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})));});
}
try{
  try{await owner();throw new Error('Use a stopped verification store');}
  catch(error){assert(error instanceof TypeError&&error.cause?.code==='ECONNREFUSED',String(error));}
  runtime=spawn(binary,['serve','--data-dir',path.join(root,'store'),'--port',String(port)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  runtime.stdout.on('data',()=>{});runtime.stderr.on('data',bytes=>runtimeError+=bytes);
  for(let i=0;i<100;i++){try{await owner();break;}catch(error){assert(runtime.exitCode===null&&runtime.signalCode===null,runtimeError);if(i===99)throw error;await new Promise(resolve=>setTimeout(resolve,100));}}
  assert.equal((await api('/proxy')).mode,'custom');
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,locale,theme]of [[1440,'en','light'],[1024,'zh-CN','dark']]){
    const label=(en,zh)=>locale==='en'?en:zh,context=await browser.newContext({viewport:{width,height:940}}),calls=[];
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.exposeBinding('__tmsNative',async(_context,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');assert.equal(request.method,'GET');
      calls.push({method:request.method,path:url.pathname});const response=await fetch(base+url.pathname+url.search,{signal:AbortSignal.timeout(30000)});
      return{status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(await response.arrayBuffer()).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{
      const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
      const result=await window.__tmsNative({url,method:init.method||'GET'});
      if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
      return new Response(Uint8Array.from(atob(result.body),char=>char.charCodeAt(0)),{status:result.status,headers:result.headers});
    };});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push({origin:url.origin,path:url.pathname});return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    try{
      await page.goto(origin+'/#My%20Data?view=maps');await page.locator('.wms-file').first().waitFor();
      await page.waitForFunction(count=>{const images=[...document.querySelectorAll('.wms-thumbnail img')];return images.length===count&&images.every(image=>image.complete&&image.naturalWidth>0);},native.cases.length);
      await paint(page);await page.screenshot({path:path.join(output,`${width}-library.png`)});
      await page.getByRole('button',{name:label('Get map imagery','获取地图影像'),exact:true}).click();
      const dialog=page.getByRole('dialog');await dialog.getByRole('button',{name:label('Add map service','添加地图服务')}).click();
      await dialog.getByRole('combobox',{name:label('Map service type','服务类型')}).click();await page.getByRole('option',{name:'TMS',exact:true}).click();
      const template=native.tileMapUrl+'/{z}/{x}/{y}.png';
      const beforeExample=calls.length;
      await dialog.getByRole('button',{name:label('Use DLR Basemap','填入 DLR 底图'),exact:true}).click();
      assert.equal(calls.length,beforeExample,'Example must only fill the draft');
      assert.equal(await dialog.getByRole('combobox',{name:label('Map service type','服务类型')}).textContent(),'TMS');
      assert.equal(await dialog.getByRole('textbox',{name:label('Connection name','连接名称')}).inputValue(),'DLR EOC Basemap');
      assert.equal(await dialog.getByRole('textbox',{name:label('Tile URL template','瓦片地址模板')}).inputValue(),template);
      assert.equal(await dialog.getByRole('spinbutton',{name:label('Maximum level','最大级别')}).inputValue(),'16');
      assert.equal(await dialog.getByRole('button',{name:label('Save connection','保存来源'),exact:true}).isEnabled(),true);
      assert.equal(await dialog.getByRole('alert').count(),0);assert.match(await dialog.innerText(),/Rows from bottom|行号从底部开始/);
      await paint(page);await page.screenshot({path:path.join(output,`${width}-configuration.png`)});
      await dialog.getByRole('button',{name:label('Close','关闭'),exact:true}).click();await dialog.waitFor({state:'hidden'});
      const checked=[];
      for(const [index,asset]of native.cases.entries()){
        await page.goto(origin+'/#Workspace?map='+asset.imageId);await page.locator('.wms-map-attribution').waitFor();
        await page.waitForFunction(()=>[...document.querySelectorAll('.vector-map canvas')].some(canvas=>{
          const ctx=canvas.getContext('2d');if(!ctx||!canvas.width||!canvas.height)return false;
          const rgba=ctx.getImageData(0,0,canvas.width,canvas.height).data;let painted=0;const colours=new Set();
          for(let i=0;i<rgba.length;i+=400)if(rgba[i+3]>0){painted++;colours.add(`${rgba[i]},${rgba[i+1]},${rgba[i+2]}`);}
          return painted>100&&colours.size>40;
        }),null,{timeout:30000});
        assert.match(await page.locator('.vector-inspector').innerText(),/TMS/);
        await paint(page);await page.screenshot({path:path.join(output,`${width}-${index}-workspace.png`)});
        await page.getByRole('button',{name:label('Map source details','影像来源详情'),exact:true}).click();
        await page.getByText(label('Configured tile grid','已配置的瓦片网格'),{exact:true}).waitFor();
        assert.match(await page.locator('.vector-properties').innerText(),/Rows from bottom|行号从底部开始/);
        assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
        await paint(page);await page.screenshot({path:path.join(output,`${width}-${index}-source.png`)});
        checked.push({imageId:asset.imageId,sha256:asset.sha256,paintedRaster:true,sourceBottomOriginVisible:true});
      }
      assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);assert(calls.every(call=>call.method==='GET'));
      report.cases.push({width,locale,theme,actualLocalThumbnails:native.cases.length,officialEncodedTemplateAccepted:true,verifiedTmsPresetOnlyFillsDraft:true,
        configuredGridVisible:true,noHorizontalOverflow:true,workspaceImages:checked,calls});
      console.log(JSON.stringify({stage:'tms-ui',width,locale,images:checked.length}));
    }catch(error){await page.screenshot({path:path.join(output,`${width}-failed.png`)});await writeFile(path.join(output,`${width}-failed.json`),JSON.stringify({error:String(error),body:await page.locator('body').innerText(),calls,errors:report.errors},null,2));throw error;}
    finally{await context.close();}
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);
  report.status='passed';report.frontendFiles=[...files].sort(([a],[b])=>a.localeCompare(b)).map(([file,sha256])=>({file,sha256}));
}finally{
  await browser?.close();server.close();
  if(runtime&&runtime.exitCode===null&&runtime.signalCode===null){await owner();assert(!(await api('/jobs')).some(job=>['running','queued'].includes(job.status)));const closed=new Promise(resolve=>runtime.once('close',resolve));runtime.kill();await closed;}
  await writeFile(path.join(output,'runtime.stderr.log'),runtimeError);
  await writeFile(path.join(output,'ui-verification.json'),JSON.stringify(report,null,2)+'\n');
}
