// Export the selected mark unchanged. Optional renderers are export tools only.
import {createRequire} from 'node:module';
import {readFile,writeFile,mkdir,copyFile,readdir,stat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
const root=process.cwd();
const require=createRequire(path.resolve(process.argv[2]||'.verification/brand-tools','package.json'));
const {Resvg}=require('@resvg/resvg-js');
const sharp=require('sharp');
const version=process.argv[5]||'1.0';
if(!/^\d+\.\d+$/.test(version))throw Error('Invalid brand kit version');
const input=path.resolve(root,process.argv[3]||'design/brand/global-v5');
const out=path.resolve(root,process.argv[4]||'design/brand/geod-global-brand-kit-1.0');
const generation=JSON.parse(await readFile(path.join(input,'source/PROVENANCE.json'),'utf8'));
for(const dir of ['logos','icons/windows','icons/macos','icons/png','social','covers','backgrounds','source','guidelines'])await mkdir(path.join(out,dir),{recursive:true});
const source=await readFile(path.join(input,'source/geod-global-generated.png'));
const earth=await readFile(path.join(input,'background-space.png'));
const markUrl='data:image/png;base64,'+source.toString('base64');
const earthUrl='data:image/png;base64,'+earth.toString('base64');
const font='Segoe UI, Arial, sans-serif';
const defs='<defs><linearGradient id="brand-gradient"><stop stop-color="#087BEA"/><stop offset=".5" stop-color="#00BBDF"/><stop offset="1" stop-color="#21C885"/></linearGradient></defs>';
const svg=(w,h,body)=>`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${defs}${body}</svg>`;
const text=(x,y,s,size=28,color='#A6BACF',weight=400)=>`<text x="${x}" y="${y}" font-family="${font}" font-size="${size}" fill="${color}" font-weight="${weight}">${s}</text>`;
const mark=(x,y,size)=>`<image x="${x}" y="${y}" width="${size}" height="${size}" href="${markUrl}"/>`;
const word=(x,y,scale=1,dark=false)=>`<g transform="translate(${x} ${y}) scale(${scale})">${text(0,0,'GeoD',76,dark?'#F3F7FF':'#102A43',700)}${text(217,0,'Global',76,'url(#brand-gradient)')}</g>`;
const lockup=(x,y,scale=1,dark=false)=>`<g transform="translate(${x} ${y}) scale(${scale})">${mark(0,0,190)}${word(214,120,1,dark)}</g>`;
const records=[];
async function save(name,bytes){const target=path.join(out,name);await mkdir(path.dirname(target),{recursive:true});await writeFile(target,bytes);}
async function render(name,xml,widths=[],webp=false){
 if(!name.startsWith('icons/png/'))await save(name+'.svg',xml);
 const png=new Resvg(xml,{font:{loadSystemFonts:true}}).render().asPng();
 await save(name+'.png',png);
 const meta=await sharp(png).metadata();records.push({file:name+'.png',width:meta.width,height:meta.height,alpha:meta.hasAlpha});
 if(webp)await save(name+'.webp',await sharp(png).webp({quality:90}).toBuffer());
 for(const multiple of widths){const scaled=new Resvg(xml,{font:{loadSystemFonts:true},fitTo:{mode:'width',value:meta.width*multiple}}).render().asPng();await save(`${name}@${multiple}x.png`,scaled);}
 return png;
}
await save('source/approved-symbol.png',source);
await save('source/approved-generation.json',await readFile(path.join(input,'source/PROVENANCE.json')));
await save('LICENSE',await readFile(path.join(input,'LICENSE')));
await render('logos/symbol',svg(1280,1280,mark(0,0,1280)));
for(const dark of [false,true]){
 const suffix=dark?'dark':'light';
 await render('logos/horizontal-on-'+suffix,svg(780,190,lockup(0,0,1,dark)),[2,4],true);
 await render('logos/stacked-on-'+suffix,svg(660,610,mark(165,0,330)+word(72,466,1.06,dark)),[2],true);
 await render('logos/wordmark-on-'+suffix,svg(600,130,word(28,94,1.07,dark)),[2]);
 const solid=dark?'#FFFFFF':'#102A43';
 await render('logos/wordmark-solid-'+(dark?'white':'ink'),svg(600,130,`<g transform="translate(28 94) scale(1.07)">${text(0,0,'GeoD',76,solid,700)}${text(217,0,'Global',76,solid)}</g>`),[2]);
}
for(const size of [16,24,32,48,64,96,128,180,192,256,512,1024]){
 await render(`icons/png/symbol-${size}`,svg(size,size,mark(0,0,size)));
 for(const dark of [false,true])await render(`icons/png/app-${dark?'dark':'light'}-${size}`,svg(size,size,`<rect width="${size}" height="${size}" rx="${size*.22}" fill="${dark?'#071423':'#F3FAFF'}"/>${mark(size*.06,size*.06,size*.88)}`));
}
function ico(images){
 const header=6+16*images.length;const data=Buffer.alloc(header+images.reduce((n,i)=>n+i.png.length,0));data.writeUInt16LE(1,2);data.writeUInt16LE(images.length,4);let offset=header;
 for(const [i,{size,png}]of images.entries()){const p=6+i*16;data[p]=size===256?0:size;data[p+1]=data[p];data.writeUInt16LE(1,p+4);data.writeUInt16LE(32,p+6);data.writeUInt32LE(png.length,p+8);data.writeUInt32LE(offset,p+12);png.copy(data,offset);offset+=png.length;}
 return data;
}
for(const type of ['symbol','app-dark','app-light']){
 const images=[];for(const size of [16,24,32,48,64,128,256])images.push({size,png:await readFile(path.join(out,`icons/png/${type}-${size}.png`))});
 await save(`icons/windows/geod-global-${type}.ico`,ico(images));
}
const favicon=[];for(const size of [16,32,48])favicon.push({size,png:await readFile(path.join(out,`icons/png/symbol-${size}.png`))});
await save('icons/favicon.ico',ico(favicon));
await copyFile(path.join(out,'icons/png/symbol-32.png'),path.join(out,'icons/favicon-32.png'));
await copyFile(path.join(out,'icons/png/app-dark-180.png'),path.join(out,'icons/apple-touch-icon-180.png'));
for(const type of ['app-dark','app-light']){
 const blocks=[];for(const [kind,size]of [['ic08',256],['ic09',512],['ic10',1024]]){const png=await readFile(path.join(out,`icons/png/${type}-${size}.png`));const block=Buffer.alloc(8+png.length);block.write(kind);block.writeUInt32BE(block.length,4);png.copy(block,8);blocks.push(block);}
 const header=Buffer.alloc(8);header.write('icns');header.writeUInt32BE(8+blocks.reduce((n,b)=>n+b.length,0),4);
 await save(`icons/macos/geod-global-${type}.icns`,Buffer.concat([header,...blocks]));
}
for(const dark of [false,true]){
 const suffix=dark?'dark':'light';
 const body=`<rect width="1024" height="1024" fill="${dark?'#071423':'#F3FAFF'}"/>${mark(82,82,860)}`;
 await render(`social/avatar-${suffix}`,svg(1024,1024,body),[],true);
 await render(`social/avatar-circle-${suffix}`,svg(1024,1024,`<circle cx="512" cy="512" r="512" fill="${dark?'#071423':'#F3FAFF'}"/>${mark(105,105,814)}`));
}
const backdrop=(w,h)=>`<rect width="${w}" height="${h}" fill="#071423"/><image width="${w}" height="${h}" preserveAspectRatio="xMidYMid slice" href="${earthUrl}"/><rect width="${w}" height="${h}" fill="#071423" opacity=".4"/>`;
const campaign=(w,h,scale,title,subtitle)=>svg(w,h,backdrop(w,h)+lockup(w*.055,h*.15,scale,true)+text(w*.065,h*.71,title,h*.065,'#EEF5FD',600)+text(w*.065,h*.83,subtitle,h*.033,'#AFC8DF'));
await render('covers/open-graph-1200x630',campaign(1200,630,1.24,'One agent. A world of geospatial data.','Explore · Preview · Download · Process'),[],true);
await render('covers/github-social-1280x640',campaign(1280,640,1.33,'Geospatial data, guided by one agent.','GeoD Global · Open source'),[],true);
await render('covers/readme-header-1600x520',campaign(1600,520,1.28,'One agent. A world of geospatial data.','Explore · Preview · Download · Process'),[],true);
await render('covers/website-hero-1920x1080',campaign(1920,1080,1.9,'From a place to the data you need.','Explore · Preview · Download · Process'),[],true);
await render('covers/social-landscape-1600x900',campaign(1600,900,1.65,'Geospatial data, guided by one agent.','Explore · Preview · Download · Process'),[],true);
await render('backgrounds/desktop-dark-1920x1080',svg(1920,1080,backdrop(1920,1080)+lockup(62,50,.68,true)),[],true);
const orbit=svg(1920,1080,`<defs><radialGradient id="wash"><stop stop-color="#DFF5F7"/><stop offset="1" stop-color="#F3FAFF"/></radialGradient></defs><rect width="1920" height="1080" fill="#F3FAFF"/><ellipse cx="250" cy="1030" rx="970" ry="720" fill="url(#wash)"/><g fill="none" stroke="url(#brand-gradient)" stroke-width="2" opacity=".16"><ellipse cx="250" cy="1100" rx="830" ry="770"/><ellipse cx="250" cy="1100" rx="590" ry="770"/><ellipse cx="250" cy="1100" rx="300" ry="770"/><ellipse cx="250" cy="1100" rx="830" ry="480"/><ellipse cx="250" cy="1100" rx="830" ry="240"/></g>`);
await render('backgrounds/clean-light-1920x1080',orbit,[],true);
await render('backgrounds/desktop-light-1920x1080',orbit.replace('</svg>',lockup(62,50,.68,false)+'</svg>'),[],true);
await save('backgrounds/clean-space-original.png',earth);
await save('backgrounds/clean-space-original.webp',await readFile(path.join(input,'background-space.webp')));
const tokens={name:'GeoD Global',version,colors:{blue:'#087BEA',cyan:'#00BBDF',green:'#21C885',ink:'#102A43',space:'#071423',paper:'#F3FAFF'},gradient:{angle:90,stops:[{color:'#087BEA',position:0},{color:'#00BBDF',position:.5},{color:'#21C885',position:1}]},fonts:{primary:'Segoe UI',fallbacks:['Arial','sans-serif']},usage:{minimumSymbolPx:24,minimumHorizontalLogoHeightPx:32,minimumClearSpace:'12% of the symbol canvas width',gradientText:'Decorative brand wordmark only; use solid ink/white for body text'}};
await save('guidelines/brand-tokens.json',JSON.stringify(tokens,null,2));
await save('guidelines/brand-tokens.css',`:root {\n  --geod-blue: #087BEA;\n  --geod-cyan: #00BBDF;\n  --geod-green: #21C885;\n  --geod-ink: #102A43;\n  --geod-space: #071423;\n  --geod-paper: #F3FAFF;\n  --geod-gradient: linear-gradient(90deg, #087BEA 0%, #00BBDF 50%, #21C885 100%);\n  --geod-brand-font: "Segoe UI", Arial, sans-serif;\n}\n`);
// The approved source stays unchanged; SVG artwork only arranges it with editable type.
const proof={version,approvedVariant:generation.variant||'global-v5',sourceSha256:createHash('sha256').update(source).digest('hex'),symbolPreserved:true,newImageGeneration:generation.tool==='image_gen',liveAssetsReplaced:false,rasterMark:true,svgContainerEmbedsPng:true,backgroundOriginal:{width:1672,height:941},imageExports:records};
await save('source/PROVENANCE.json',JSON.stringify(proof,null,2));
const thumb=async(file,x,y,w,h)=>`<image x="${x}" y="${y}" width="${w}" height="${h}" preserveAspectRatio="xMidYMid meet" href="data:image/png;base64,${(await readFile(path.join(out,file))).toString('base64')}"/>`;
let overview=`<rect width="2200" height="2000" fill="#071423"/>${text(80,92,'GeoD Global / Brand Kit ${version}',40,'#F2F7FF',700)}${text(80,144,'Approved blue → cyan → green · Logo / Icon / Cover / Background',25)}
<rect x="65" y="190" width="1010" height="330" rx="26" fill="#F3FAFF"/><rect x="1100" y="190" width="1035" height="330" rx="26" fill="#0C2035"/>${lockup(100,225,1.16,false)}${lockup(1140,225,1.16,true)}${text(90,565,'LOGO · LIGHT / DARK',24)}
<rect x="65" y="605" width="660" height="390" rx="24" fill="#10263A"/><rect x="755" y="605" width="660" height="390" rx="24" fill="#10263A"/><rect x="1445" y="605" width="690" height="390" rx="24" fill="#10263A"/>${await thumb('social/avatar-dark.png',110,640,270,270)}${await thumb('social/avatar-light.png',415,640,270,270)}${text(100,960,'AVATARS · SQUARE / CIRCLE',23)}${await thumb('icons/png/app-dark-256.png',785,630,265,265)}${await thumb('icons/png/app-light-256.png',1110,630,265,265)}${text(790,960,'PNG · ICO · ICNS · FAVICON',23)}
${['#087BEA','#00BBDF','#21C885'].map((c,i)=>`<rect x="${1480+i*210}" y="690" width="178" height="190" rx="18" fill="${c}"/>${text(1480+i*210,925,c,23)}`).join('')}
<rect x="65" y="1045" width="1010" height="505" rx="24" fill="#10263A"/><rect x="1100" y="1045" width="1035" height="505" rx="24" fill="#10263A"/>${await thumb('covers/open-graph-1200x630.png',85,1065,970,460)}${await thumb('covers/readme-header-1600x520.png',1120,1065,995,460)}${text(90,1605,'SOCIAL &amp; LINK COVER · 1200 × 630',23)}${text(1125,1605,'README HEADER · 1600 × 520',23)}
${await thumb('backgrounds/desktop-dark-1920x1080.png',65,1650,630,290)}${await thumb('backgrounds/clean-light-1920x1080.png',745,1650,630,290)}${text(1465,1730,'GUIDELINES &amp; EDITABLE SOURCES',25,'#E8F2FC',600)}${text(1465,1790,'Palette · Typography · Safe area',25)}${text(1465,1850,'SVG containers · PNG masters',25)}${text(1465,1910,'Manifest · Provenance · Usage guide',25)}`;
await render('overview',svg(2200,2000,overview));
async function walk(dir){let files=[];for(const name of await readdir(dir)){const p=path.join(dir,name);if((await stat(p)).isDirectory())files.push(...await walk(p));else files.push(p);}return files;}
const entries=[];for(const file of(await walk(out)).filter(file=>path.basename(file)!=='MANIFEST.json').sort()){const bytes=await readFile(file);entries.push({path:path.relative(out,file).replaceAll('\\','/'),bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')});}
await save('MANIFEST.json',JSON.stringify({product:'GeoD Global',version,files:entries},null,2));
console.log(JSON.stringify({out,files:entries.length+1,approvedSourceSha256:proof.sourceSha256,unchangedReference:true}));
