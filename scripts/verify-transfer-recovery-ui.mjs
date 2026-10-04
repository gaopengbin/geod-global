// Production renderer + actual live native tasks in an owned verification store.
// Read-only headless Edge; no user browser, desktop window or credentials.
import {chromium} from 'playwright';
import {createServer} from 'node:http';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {formatBytes} from '../prototype/src/runtime-client.js';

const workspace=process.cwd(),root=path.resolve(process.argv[2]),port=Number(process.argv[3]||4605),uiPort=Number(process.argv[4]||4607);
assert.equal(path.dirname(root),path.join(workspace,'.verification'));assert(path.basename(root).startsWith('radar-polarizations-'));
const proof=JSON.parse(await readFile(path.join(root,'resume-live-verification.json'),'utf8'));
assert.equal(proof.rangeAccepted,true);assert.equal(proof.independentBoundary.exactSourceBytes,true);
const base=`http://127.0.0.1:${port}`,origin=`http://127.0.0.1:${uiPort}`,output=path.join(root,'recovery-ui');await mkdir(output,{recursive:true});
const config=JSON.parse(await readFile('src-tauri/tauri.conf.json','utf8'));
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,origin),file=path.resolve(workspace,'prototype/dist','.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    assert(file.startsWith(path.join(workspace,'prototype/dist')+path.sep));
    res.writeHead(200,{'Content-Type':({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.svg':'image/svg+xml','.woff2':'font/woff2'})[path.extname(file)]||'application/octet-stream','Content-Security-Policy':config.app.security.csp});res.end(await readFile(file));
  }catch{res.writeHead(404);res.end();}
});
const report={schema:'geod-transfer-recovery-ui/v1',checkedAt:new Date().toISOString(),renderer:'production build; exact desktop CSP; real native API via loopback bridge',nativeWindowTested:false,usedUserDesktop:false,accountAuthorizationSubmitted:false,cases:[],errors:[],remoteRequests:[]};
let browser;
try{
  const live=await (await fetch(base+'/jobs/'+proof.jobId)).json();assert.equal(live.transfer.mode,'resumed');assert.equal(live.transfer.resumedBytes,proof.verifiedPrefixBytes);
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(uiPort,'127.0.0.1',resolve);});
  browser=await chromium.launch({channel:'msedge',headless:true});
  for(const [width,locale,theme]of[[1440,'en','light'],[1024,'zh-CN','dark']]){
    const context=await browser.newContext({viewport:{width,height:960}}),calls=[];
    await context.addInitScript(({locale,theme})=>{localStorage.setItem('geod-global-locale',locale);localStorage.setItem('geod-design-theme',JSON.stringify(theme));window.__CSP_ERRORS=[];document.addEventListener('securitypolicyviolation',event=>window.__CSP_ERRORS.push(event.violatedDirective));},{locale,theme});
    await context.exposeBinding('__transferNative',async(_source,request)=>{
      const url=new URL(request.url);assert.equal(url.origin,'http://127.0.0.1:4318');assert.equal(request.method,'GET');calls.push(url.pathname);
      const response=await fetch(base+url.pathname+url.search,{headers:request.headers}),body=await response.arrayBuffer();
      return{status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(body).toString('base64')};
    });
    await context.addInitScript(()=>{const original=window.fetch.bind(window);window.fetch=async(input,init={})=>{const url=String(input instanceof Request?input.url:input);if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);const result=await window.__transferNative({url,method:init.method||'GET',headers:Object.fromEntries(new Headers(init.headers))});return new Response(Uint8Array.from(atob(result.body),value=>value.charCodeAt(0)),{status:result.status,headers:result.headers});};});
    await context.route('**/*',route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.remoteRequests.push(url.href);return route.abort();}return route.continue();});
    const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));page.on('console',message=>{if(message.type()==='error')report.errors.push(message.text());});
    await page.goto(origin+'/#Tasks');
    const size=formatBytes(proof.verifiedPrefixBytes,locale),label=locale==='en'?'Resumed':'已续传';
    const platform=proof.itemId.split('_')[0].replace('S1','Sentinel-1');
    const card=page.locator('[data-slot=task-row]').filter({hasText:label}).filter({hasText:platform}).first();await card.waitFor();
    await page.evaluate(async()=>{
      await document.fonts.ready;
      await Promise.all(document.getAnimations()
        .filter(animation=>Number.isFinite(animation.effect?.getComputedTiming().endTime))
        .map(animation=>animation.finished.catch(()=>{})));
    });
    await page.waitForFunction(()=>[...document.querySelectorAll('[data-slot=task-row]')].every(card=>{
      for(let element=card;element;element=element.parentElement){
        if(Number(getComputedStyle(element).opacity)<0.999)return false;
      }
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
    const layout=await page.evaluate(()=>({width:innerWidth,overflow:document.documentElement.scrollWidth-innerWidth,cards:[...document.querySelectorAll('[data-slot=task-row]')].map(element=>{const rect=element.getBoundingClientRect();return{height:rect.height,right:rect.right};})}));
    const polarizationLabels=await page.locator('[data-slot=task-row]').evaluateAll(cards=>cards.map(card=>card.textContent.match(/RTC · (VV|VH|HH|HV)/)?.[1]));
    const actualJobs=await(await fetch(base+'/jobs')).json();
    const polarizations=actualJobs.filter(job=>['queued','running'].includes(job.status)&&job.kind==='download').map(job=>job.assetKey.toUpperCase());
    assert.deepEqual([...polarizationLabels].sort(),polarizations.sort());
    await writeFile(path.join(output,`layout-${width}-${locale}.json`),JSON.stringify(layout,null,2));
    const screenshot=`tasks-${width}-${locale}.png`;await page.screenshot({path:path.join(output,screenshot)});
    assert.equal(layout.width,width);assert(layout.overflow<=1);assert(layout.cards.every(row=>row.right<=width+1));assert(Math.max(...layout.cards.map(row=>row.height))-Math.min(...layout.cards.map(row=>row.height))<2);
    await card.getByRole('button',{name:locale==='en'?'Task details':'任务详情',exact:true}).click();
    assert((await card.textContent()).includes(locale==='en'?`Resumed ${size} of verified source bytes.`:`从已校验的 ${size} 源文件数据继续下载。`));
    assert.deepEqual(await page.evaluate(()=>window.__CSP_ERRORS),[]);
    report.cases.push({width,locale,theme,screenshot,layout,polarizationLabels,stablePaintVerified:true,verifiedResumeLabel:true,detailsDescribeActualSavedBytes:true,nativeReadCalls:calls});await context.close();
  }
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.remoteRequests,[]);report.status='passed';
  await writeFile(path.join(output,'verification.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,cases:report.cases.length,nativeWindowTested:false,usedUserDesktop:false}));
}finally{if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
