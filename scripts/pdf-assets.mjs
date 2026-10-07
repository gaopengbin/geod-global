// Only reviewed PDF.js runtime resources are served, all from this repository.
import {createReadStream} from 'node:fs';
import {cp,mkdir,realpath,stat} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const source=fileURLToPath(new URL('../node_modules/pdfjs-dist/',import.meta.url));
const directories=['cmaps','standard_fonts','wasm','iccs'];
export function pdfAssets(){let output;return {name:'geod-local-pdf-assets',configResolved(config){output=config.command==='build'?path.resolve(config.root,config.build.outDir):null;},configureServer(server){server.middlewares.use('/pdfjs',async(req,res,next)=>{
  try{const raw=decodeURIComponent((req.url||'').split('?')[0]),parts=raw.replace(/^\//,'').split('/');
    if(!directories.includes(parts[0]) || parts.some(part=>!part || part==='.' || part==='..') || raw.includes('\\')){res.statusCode=404;res.end();return;}
    const base=await realpath(source),candidate=await realpath(path.join(base,...parts));if(!candidate.startsWith(base+path.sep) || !(await stat(candidate)).isFile()){res.statusCode=404;res.end();return;}
    res.setHeader('Content-Type',path.extname(candidate)==='.wasm'?'application/wasm':'application/octet-stream');createReadStream(candidate).on('error',()=>res.destroy()).pipe(res);
  }catch{next();}
});},async closeBundle(){if(!output)return;for(const directory of directories)await cp(path.join(source,directory),path.join(output,'pdfjs',directory),{recursive:true});await mkdir(path.join(output,'pdfjs/notices'),{recursive:true});await cp(path.join(source,'LICENSE'),path.join(output,'pdfjs/notices/LICENSE'));}};}
