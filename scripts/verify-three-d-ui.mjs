// Headless browser acceptance against an actual isolated Rust service.
// This does not claim installed Tauri/WebView or native file-picker acceptance.
import {chromium} from 'playwright';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';

const root=fileURLToPath(new URL('../',import.meta.url));
const options=new Map();
for(let i=2;i<process.argv.length;i++){
  const key=process.argv[i];
  if(['--production','--capture-engine-state','--archive-roundtrip','--local-scenes'].includes(key))options.set(key,true);
  else if(['--ui','--server','--output','--channel'].includes(key))options.set(key,process.argv[++i]);
  else throw new Error(`Unknown option: ${key}`);
}
const ui=options.get('--ui')||'http://127.0.0.1:4317';
const server=options.get('--server')||'http://127.0.0.1:4380';
for(const address of[ui,server]){const u=new URL(address);assert.equal(u.protocol,'http:');assert.equal(u.hostname,'127.0.0.1');assert.equal(u.pathname,'/');}
const output=path.resolve(options.get('--output')||path.join(root,'.verification/three-d-ui'));
await mkdir(output,{recursive:true});
const config=JSON.parse(await readFile(path.join(root,'src-tauri/tauri.conf.json'),'utf8'));
const response=await fetch(server+'/three-d/packages');assert.ok(response.ok);
const packages=await response.json();
const assets=['tileset','gltf'].map(kind=>packages.find(p=>p.origin==='public-https'&&p.resources.find(r=>r.id===p.entry)?.kind===kind));
assert.ok(assets.every(Boolean),'Save the real nested-tiles and externally textured glTF samples first');
if(options.get('--local-scenes')){
  const local=packages.find(p=>p.origin==='local-files'&&p.name==='QA local BoxTextured unchanged files'&&p.resources.find(r=>r.id===p.entry)?.kind==='gltf');
  assert.ok(local,'Run the actual local-input removal acceptance first');
  assets.push(local);
}
const production=Boolean(options.get('--production'));
const captureEngine=Boolean(options.get('--capture-engine-state'));
assert.ok(!production||!captureEngine,'Engine instrumentation is available only in development source');
const browser=await chromium.launch({channel:options.get('--channel')||'msedge',headless:true,args:['--enable-webgl','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const reports=[];
const cspProbes=[];
const cardContrast=[];
let archiveReport;
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
async function textContrast(page,selector){
  const values=await page.evaluate(selector=>{
    const context=document.createElement('canvas').getContext('2d',{willReadFrequently:true});
    const pixel=color=>{context.clearRect(0,0,1,1);context.fillStyle=color;context.fillRect(0,0,1,1);return[...context.getImageData(0,0,1,1).data];};
    const luminance=rgb=>rgb.slice(0,3).map(v=>{v/=255;return v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4;}).reduce((sum,v,i)=>sum+v*[0.2126,0.7152,0.0722][i],0);
    return[...document.querySelectorAll(selector)].map(element=>{
      const fg=pixel(getComputedStyle(element).color);let parent=element,bg;
      while(parent){bg=pixel(getComputedStyle(parent).backgroundColor);if(bg[3]===255)break;parent=parent.parentElement;}
      if(!parent)throw new Error('Contrast check needs an opaque rendered background');
      const a=luminance(fg),b=luminance(bg);return(Math.max(a,b)+0.05)/(Math.min(a,b)+0.05);
    });
  },selector);
  assert.ok(values.length>0,'Expected rendered text for contrast acceptance');
  const minimum=Math.min(...values);assert.ok(minimum>=4.5,`Small text contrast ${minimum.toFixed(2)} must meet 4.5:1`);return minimum;
}
async function nativeRequest(url,method,body,headers){
  const u=new URL(url);assert.equal(u.origin,'http://127.0.0.1:4318');
  const payload=body?.base64?Buffer.from(body.base64,'base64'):body;
  const result=await fetch(server+u.pathname+u.search,{method,body:['GET','HEAD'].includes(method)?undefined:payload,headers});
  return{status:result.status,headers:Object.fromEntries(result.headers),body:Buffer.from(await result.arrayBuffer()).toString('base64')};
}
try{
  for(const[width,locale,theme]of[[1440,'en','light'],[1024,'zh-CN','dark'],[900,'en','dark']]){
    const context=await browser.newContext({viewport:{width,height:960}});
    const calls=[],errors=[],remote=[],security=[];
    await context.addInitScript(({locale,theme})=>{
      localStorage.setItem('geod-global-locale',locale);
      localStorage.setItem('geod-design-theme',JSON.stringify(theme));
      document.addEventListener('securitypolicyviolation',event=>{window.__CSP_ERRORS??=[];window.__CSP_ERRORS.push({directive:event.violatedDirective,blocked:event.blockedURI});});
    },{locale,theme});
    if(production){
      // Only the loopback HTTP adapter is bridged by the harness. Renderer fetch,
      // workers, scripts, textures and Blob resources obey the exact desktop CSP.
      await context.exposeBinding('__native3dHttp',async(_source,request)=>{
        const u=new URL(request.url);calls.push({method:request.method,path:u.pathname});
        return nativeRequest(request.url,request.method,request.body,request.headers);
      });
      await context.addInitScript(()=>{
        const original=window.fetch.bind(window);
        window.fetch=async(input,init={})=>{
          const url=String(input instanceof Request?input.url:input);
          if(!url.startsWith('http://127.0.0.1:4318/'))return original(input,init);
          if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
          const body=init.body instanceof Blob?{base64:btoa(Array.from(new Uint8Array(await init.body.arrayBuffer()),v=>String.fromCharCode(v)).join(''))}:init.body;
          const result=await window.__native3dHttp({url,method:init.method||'GET',body,headers:Object.fromEntries(new Headers(init.headers))});
          if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
          return new Response(Uint8Array.from(atob(result.body),c=>c.charCodeAt(0)),{status:result.status,headers:result.headers});
        };
      });
    }
    await context.route('**/*',async route=>{
      const request=route.request(),u=new URL(request.url());
      if(u.origin==='http://127.0.0.1:4318'){
        calls.push({method:request.method(),path:u.pathname});
        const result=await nativeRequest(u.href,request.method(),request.postDataBuffer(),{'Content-Type':request.headers()['content-type']||'application/json','X-GeoD-Client':'geod-global'});
        return route.fulfill({status:result.status,contentType:result.headers['content-type'],headers:{'Access-Control-Allow-Origin':ui},body:Buffer.from(result.body,'base64')});
      }
      if(u.origin!==ui){remote.push(u.href);return route.abort();}
      if(production&&u.pathname==='/'){
        const result=await route.fetch();return route.fulfill({response:result,headers:{...result.headers(),'Content-Security-Policy':config.app.security.csp}});
      }
      if(captureEngine&&u.pathname.endsWith('/three-d-viewer.jsx')){
        const result=await route.fetch(),source=await result.text();
        assert.ok(source.includes('readyTimer = setTimeout('),'Instrumentation marker changed');
        return route.fulfill({response:result,body:source.replace('readyTimer = setTimeout(','window.__THREE_D_QA = {widget, primitive, scene}; readyTimer = setTimeout(')});
      }
      return route.continue();
    });
    const page=await context.newPage();
    page.on('pageerror',e=>errors.push(e.message));
    page.on('requestfailed',r=>errors.push(r.url()+' '+r.failure()?.errorText));
    page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
    const label=(en,zh)=>locale==='en'?en:zh;
    await page.goto(ui+'/#My%20Data?view=3d');
    await page.getByRole('button',{name:label('View 3D scene','查看三维场景'),exact:true}).first().waitFor();
    await page.waitForTimeout(500);
    assert.equal(await page.evaluate(()=>[...document.querySelectorAll('.three-d-asset')].every(card=>card.getBoundingClientRect().right<=innerWidth)),true,'Asset cards must fit the viewport, not a clipped parent');
    assert.equal(await page.evaluate(()=>{const nav=document.querySelector('.data-library-heading .bui-segmented'),active=nav?.querySelector('[data-state=on]');return Boolean(active)&&active.getBoundingClientRect().right<=nav.getBoundingClientRect().right+1;}),true,'The selected library category must be visible');
    assert.equal(await page.evaluate(()=>{const heights=[...document.querySelectorAll('.three-d-asset')].map(card=>card.getBoundingClientRect().height);return Math.max(...heights)-Math.min(...heights)<2;}),true,'Collapsed 3D asset cards must have consistent heights');
    cardContrast.push({width,locale,theme,minimum:await textContrast(page,'.three-d-asset-meta>span:not([data-slot=badge])')});
    await page.screenshot({path:path.join(output,`library-${width}-${locale}.png`)});
    const viewing=[...assets];
    if(options.get('--archive-roundtrip')&&!archiveReport){
      const parent=assets[1],card=page.locator('.three-d-asset').filter({has:page.getByRole('heading',{name:parent.name,exact:true})});
      const waiting=page.waitForEvent('download');await card.getByRole('button',{name:'Export offline 3D package',exact:true}).click();const download=await waiting;
      const file=path.join(output,'ui-export.zip');await download.saveAs(file);const exported=await readFile(file),control=Buffer.from(await(await fetch(server+`/three-d/packages/${parent.id}/export`)).arrayBuffer());assert.deepEqual(exported,control,'UI export must return the actual complete native ZIP');
      await page.getByRole('button',{name:'Add 3D assets',exact:true}).click();
      const dialog=page.getByRole('dialog');await dialog.getByRole('radio',{name:'Local scene',exact:true}).click();
      const name='Offline archive round-trip '+Date.now().toString(36);
      for(const[field,value]of[['Asset name',name],['License or permission',parent.rights.license],['Attribution',parent.rights.attribution]])await dialog.getByRole('textbox',{name:field,exact:true}).fill(value);
      await dialog.getByRole('checkbox').click();await dialog.locator('input[type=file]').setInputFiles(file);
      await dialog.waitFor({state:'detached',timeout:30000});
      const beforeIds=new Set(packages.map(p=>p.id));
      const saved=await(await fetch(server+'/three-d/packages')).json(),copy=saved.find(p=>p.name===name&&!beforeIds.has(p.id));assert.ok(copy);
      assert.deepEqual(copy.resources,parent.resources,'Archive import must retain exact original bytes and dependency locators');assert.equal(copy.importedFrom.sourceReceiptSha256,parent.receiptSha256);
      archiveReport={assetId:copy.id,parentId:parent.id,parentReceiptSha256:parent.receiptSha256,exportSha256:sha(exported),exactOriginalResourcesRetained:true};viewing.push(copy);
      const imported=page.locator('.three-d-asset').filter({has:page.getByRole('heading',{name,exact:true})});await imported.getByRole('button',{name:'3D source details',exact:true}).click();await imported.getByText(parent.source,{exact:true}).waitFor();await imported.getByRole('button',{name:'3D source details',exact:true}).click();
    }
    for(const asset of viewing){
      const kind=asset.resources.find(r=>r.id===asset.entry).kind;
      const card=page.locator('.three-d-asset').filter({has:page.getByRole('heading',{name:asset.name,exact:true})});
      const readsBefore=calls.filter(c=>c.path.includes('/resources/')).length;
      await card.getByRole('button',{name:label('View 3D scene','查看三维场景'),exact:true}).click();
      try{await page.locator('.three-d-render-state[data-state=ready]').waitFor({timeout:40000});}
      catch(e){await page.screenshot({path:path.join(output,`failure-${kind}-${width}-${locale}.png`)});await writeFile(path.join(output,'failure.json'),JSON.stringify({errors,remote,html:await page.locator('.three-d-viewer-dialog').textContent()},null,2));throw e;}
      const canvas=page.locator('.three-d-canvas canvas');assert.equal(await canvas.count(),1);
      const suffix=asset.origin==='local-archive'?'-imported':asset.origin==='local-files'?'-local':'';
      await page.screenshot({path:path.join(output,`viewer-${kind}-${width}-${locale}${suffix}.png`)});
      const still=await canvas.screenshot();
      const minimumTextContrast=await textContrast(page,'.three-d-viewer-toolbar>span:not(.three-d-render-state),.three-d-attribution');
      await writeFile(path.join(output,`canvas-${kind}-${width}-${locale}${suffix}.png`),still);
      const bounds=await canvas.boundingBox();
      await page.mouse.move(bounds.x+bounds.width/2,bounds.y+bounds.height/2);
      await page.mouse.down();await page.mouse.move(bounds.x+bounds.width*0.63,bounds.y+bounds.height*0.54,{steps:12});await page.mouse.up();
      await page.waitForTimeout(1000);
      const rotated=await canvas.screenshot();assert.notEqual(sha(rotated),sha(still),'Drag must change the rendered geometry');
      await page.mouse.wheel(0,-150);await page.waitForTimeout(1000);
      const zoomed=await canvas.screenshot();assert.notEqual(sha(zoomed),sha(rotated),'Wheel must change the rendered geometry');
      await page.getByRole('button',{name:label('Reset 3D camera','重置三维视角'),exact:true}).click();
      await page.waitForTimeout(1000);
      const reset=await canvas.screenshot();assert.notEqual(sha(reset),sha(zoomed),'Reset must change the rendered geometry');
      let engine;
      if(captureEngine)engine=await page.evaluate(()=>{const p=window.__THREE_D_QA?.primitive;return p?{ready:p.ready??p.tilesLoaded,meshTriangles:p._statistics?.trianglesLength,selectedTiles:p._selectedTiles?.length}:null;});
      if(engine)assert.ok(engine.ready,'Renderer must finish preparing the actual mesh');
      security.push(...await page.evaluate(()=>window.__CSP_ERRORS||[]));
      assert.deepEqual(errors,[]);assert.deepEqual(remote,[]);assert.deepEqual(security,[]);
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
      const resourceReads=calls.filter(c=>c.path.includes('/resources/')).length-readsBefore;
      assert.equal(resourceReads,asset.resources.length);
      await page.locator('.three-d-viewer-dialog').getByRole('button',{name:label('Close','关闭'),exact:true}).click();
      await page.locator('.three-d-viewer-dialog').waitFor({state:'detached'});
      assert.equal(await page.locator('.three-d-canvas canvas').count(),0);
      reports.push({width,locale,theme,asset:asset.id,origin:asset.origin,kind,sourceReceiptSha256:asset.receiptSha256,resourceReads,minimumTextContrast,dragChangesFrame:true,wheelChangesFrame:true,resetChangesFrame:true,canvasSha256:sha(still),disposedCanvas:true,...(engine?{instrumentedEngine:engine}:{})});
    }
    if(production){
      // Run a normal same-origin script, not a privileged page.evaluate eval.
      await page.route(ui+'/three-d-csp-probe.js',route=>route.fulfill({contentType:'text/javascript',body:`(async()=>{const result={javascriptEvalBlocked:false,localWasmAllowed:false};try{Function('return 1')();}catch{result.javascriptEvalBlocked=true;}try{await WebAssembly.compile(new Uint8Array([0,97,115,109,1,0,0,0]));result.localWasmAllowed=true;}catch{}window.__THREE_D_CSP_PROBE=result;})();`}));
      await page.addScriptTag({url:ui+'/three-d-csp-probe.js'});await page.waitForFunction(()=>window.__THREE_D_CSP_PROBE);
      const probe=await page.evaluate(()=>window.__THREE_D_CSP_PROBE);assert.deepEqual(probe,{javascriptEvalBlocked:true,localWasmAllowed:true});cspProbes.push({width,...probe});
    }
    await context.close();
  }
}finally{await browser.close();}
const report={verifiedAt:new Date().toISOString(),runtime:'actual Rust HTTP service; isolated QA storage',renderer:'headless Microsoft Edge with software WebGL',productionBuild:production,exactDesktopRendererCsp:production,nativeWebViewVerified:false,remoteRequests:[],errors:[],cspProbes,cardContrast,...(archiveReport?{archiveRoundtrip:archiveReport}:{}),results:reports};
await writeFile(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report));
