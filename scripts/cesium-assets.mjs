// Cesium's workers, decoders and credits are bundled locally. No CDN or ion
// token is involved. Only these four reviewed directories can be served.
import { createReadStream } from 'node:fs';
import { cp, realpath, stat, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const source=fileURLToPath(new URL('../node_modules/cesium/Build/Cesium/',import.meta.url));
const packageRoot=fileURLToPath(new URL('../node_modules/cesium/',import.meta.url));
const noticeFiles=['LICENSE.md','ThirdParty.json','ThirdParty.extra.json'];
const directories=['Workers','ThirdParty','Assets','Widgets'];
const types={'.js':'text/javascript','.json':'application/json','.wasm':'application/wasm','.png':'image/png','.jpg':'image/jpeg','.css':'text/css','.svg':'image/svg+xml','.ktx2':'image/ktx2'};
export function cesiumAssets(){let output;return {name:'geod-local-cesium-assets',configResolved(config){output=config.command==='build'?path.resolve(config.root,config.build.outDir):null;},configureServer(server){server.middlewares.use('/cesium',async(req,res,next)=>{try{const raw=decodeURIComponent((req.url||'').split('?')[0]);const parts=raw.replace(/^\//,'').split('/');const notice=parts.length===2&&parts[0]==='notices'&&noticeFiles.includes(parts[1]);if((!directories.includes(parts[0])&&!notice)||parts.some(p=>!p||p==='.'||p==='..')||raw.includes('\\')){res.statusCode=404;res.end();return;}const base=await realpath(notice?packageRoot:source),candidate=await realpath(notice?path.join(packageRoot,parts[1]):path.join(source,...parts));if(!candidate.startsWith(base+path.sep)||(await stat(candidate)).isDirectory()){res.statusCode=404;res.end();return;}res.setHeader('Content-Type',types[path.extname(candidate)]||'application/octet-stream');res.setHeader('Cache-Control','public, max-age=3600');createReadStream(candidate).on('error',()=>res.destroy()).pipe(res);}catch{next();}});},async closeBundle(){if(!output)return;for(const folder of directories)await cp(path.join(source,folder),path.join(output,'cesium',folder),{recursive:true});await mkdir(path.join(output,'cesium/notices'),{recursive:true});for(const name of noticeFiles)await cp(path.join(packageRoot,name),path.join(output,'cesium/notices',name));}};}
