// Asset export only; optional renderers are installed outside product dependencies.
import {createRequire} from 'node:module';
import {readFile,writeFile,mkdir,copyFile} from 'node:fs/promises';
import path from 'node:path';
const root=process.cwd();
const require=createRequire(path.resolve(process.argv[2]||'.verification/brand-tools','package.json'));
const {Resvg}=require('@resvg/resvg-js');
const sharp=require('sharp');
const bitmapSource=process.argv[4];
const version=process.argv[5]||'v1';
if(!/^v[1-9][0-9]*$/.test(version))throw Error('Invalid brand version');
const teal=version==='v3';
const emerald=version==='v4';
const edge=version==='v5';
const theme=source=>emerald?source.replaceAll('#75C7FF','#81DB98').replaceAll('#237BB5','#158A42').replaceAll('#102A43','#163B25').replaceAll('#F3FAFF','#F2FAF1').replaceAll('#071423','#0B1D14').replaceAll('#0C2035','#102E1E').replaceAll('#D6F4FF','#DCF4D9').replaceAll('#3EABDF','#159447'):teal?source.replaceAll('#75C7FF','#7AE3C9').replaceAll('#237BB5','#087F8C').replaceAll('#102A43','#0D343B').replaceAll('#F3FAFF','#F0FBF7').replaceAll('#071423','#071F24').replaceAll('#0C2035','#0C2C31').replaceAll('#D6F4FF','#D6F6EC').replaceAll('#3EABDF','#079E9C'):source;
const bitmap=bitmapSource?await readFile(bitmapSource):null;
const out=path.join(root,'design/brand/global-'+version);
await mkdir(path.join(out,'icons'),{recursive:true});
const defs=`<defs>
 <linearGradient id="ribbon" x1=".15" y1=".9" x2=".8" y2=".1"><stop stop-color="#32F0ED"/><stop offset=".43" stop-color="#00B7EF"/><stop offset="1" stop-color="#0562F5"/></linearGradient>
 <linearGradient id="fold" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#05329D"/><stop offset=".55" stop-color="#0759EA"/><stop offset="1" stop-color="#00BAF1"/></linearGradient>
 <linearGradient id="crossbar" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#00CDF0"/><stop offset="1" stop-color="#0751DC"/></linearGradient>
 <linearGradient id="orbit" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#37E8ED"/><stop offset="1" stop-color="#0775FA"/></linearGradient>
 </defs>`;
const contours=`
 <path d="M122 665 C24 800 124 866 374 805 C613 746 876 536 919 311 C942 190 844 173 735 214" fill="none" stroke="url(#orbit)" stroke-width="17" stroke-linecap="round"/>
 <path d="M813 264 C724 146 612 105 483 126 C262 160 133 335 148 551 C160 745 311 889 497 889 C691 889 854 744 854 551 C854 510 819 479 778 479 H608 C569 479 541 507 541 543 C541 579 569 607 608 607 H700 C674 693 599 745 511 745 C384 745 287 649 287 524 C287 398 384 296 511 296 C591 296 658 330 711 385 C740 414 777 404 803 381 L838 349 C866 324 839 290 813 264 Z" fill="url(#ribbon)"/>
 <path d="M813 264 C713 211 604 228 514 271 C376 337 298 441 287 524 C270 402 352 264 483 223 C610 183 732 204 813 264 Z" fill="url(#fold)"/>
 <path d="M608 479 H778 C819 479 854 510 854 551 C854 699 761 833 626 875 C706 828 761 739 761 658 C761 626 737 607 700 607 H608 C569 607 541 579 541 543 C541 507 569 479 608 479 Z" fill="url(#crossbar)"/>
 <path d="M700 607 C781 626 789 686 752 748 C712 809 647 848 582 855 C530 861 470 848 468 817 C465 775 607 759 662 683 C681 658 693 632 700 607 Z" fill="url(#fold)" opacity=".85"/>
 <path d="M122 665 C24 800 124 866 374 805 C499 774 616 708 718 617" fill="none" stroke="url(#orbit)" stroke-width="17" stroke-linecap="round"/>
 <circle cx="914" cy="270" r="26" fill="url(#crossbar)"/>
`;
const symbol=mono=>`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024" role="img" aria-label="GeoD Global">${bitmap?`<image width="1024" height="1024" href="data:image/png;base64,${bitmap.toString('base64')}"/>`:`${defs}<g${mono?` fill="${mono}"`:''}>${mono?contours.replace(/url\(#[a-z]+\)/g,mono).replace(' opacity=".85"',''):contours}</g>`}</svg>`;
const svg=(w,h,content)=>theme(`<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${edge?'<defs><linearGradient id="global-word-gradient"><stop stop-color="#087BEA"/><stop offset=".5" stop-color="#00BBDF"/><stop offset="1" stop-color="#21C885"/></linearGradient></defs>':''}${content}</svg>`);
const markData=(bitmap||Buffer.from(symbol())).toString('base64');
const mark=(x,y,size)=>`<image x="${x}" y="${y}" width="${size}" height="${size}" href="data:${bitmap?'image/png':'image/svg+xml'};base64,${markData}"/>`;
const word=(x,y,light,scale=1)=>`<g transform="translate(${x} ${y}) scale(${scale})" font-family="Segoe UI, Arial, sans-serif"><text y="0" font-size="76" font-weight="700" letter-spacing="-2" fill="${light?'#F3F7FF':'#102A43'}">GeoD</text><text x="205" y="0" font-size="76" font-weight="400" letter-spacing="-2" fill="${edge?'url(#global-word-gradient)':light?'#75C7FF':'#237BB5'}">Global</text></g>`;
async function asset(name,source,width){await writeFile(path.join(out,name+'.svg'),source);const png=new Resvg(source,{font:{loadSystemFonts:true},...(width?{fitTo:{mode:'width',value:width}}:{})}).render().asPng();await writeFile(path.join(out,name+'.png'),png);return png;}
await asset('geod-global-symbol',symbol());
if(!bitmap){await asset('geod-global-symbol-white',symbol('#FFFFFF'));await asset('geod-global-symbol-ink',symbol('#102A43'));}
await asset('geod-global-logo-on-light',svg(780,190,mark(0,0,190)+word(214,120,false)));
await asset('geod-global-logo-on-dark',svg(780,190,mark(0,0,190)+word(214,120,true)));
await asset('geod-global-logo-stacked',svg(600,640,mark(135,14,330)+word(78,458,false,.99)));
const iconSvg=dark=>svg(1024,1024,`<rect width="1024" height="1024" rx="224" fill="${dark?'#071423':'#F3FAFF'}"/>${mark(62,62,900)}`);
const icoSizes=[16,24,32,48,64,128,256];
const icoPng=[];
for(const dark of [false,true])for(const size of [...icoSizes,512,1024]){
 const png=new Resvg(iconSvg(dark),{fitTo:{mode:'width',value:size}}).render().asPng();
 await writeFile(path.join(out,'icons',`app-${dark?'dark':'light'}-${size}.png`),png);
 if(dark&&icoSizes.includes(size))icoPng.push({size,png});
}
const ico=Buffer.alloc(6+16*icoPng.length+icoPng.reduce((n,{png})=>n+png.length,0));ico.writeUInt16LE(1,2);ico.writeUInt16LE(icoPng.length,4);let offset=6+16*icoPng.length;
for(const [i,{size,png}] of icoPng.entries()){const p=6+i*16;ico[p]=size===256?0:size;ico[p+1]=ico[p];ico.writeUInt16LE(1,p+4);ico.writeUInt16LE(32,p+6);ico.writeUInt32LE(png.length,p+8);ico.writeUInt32LE(offset,p+12);png.copy(ico,offset);offset+=png.length;}
await writeFile(path.join(out,'icons','geod-global.ico'),ico);
const backgroundSource=process.argv[3];
if(backgroundSource)await copyFile(backgroundSource,path.join(out,'background-space.png'));
const bgPng=await readFile(path.join(out,'background-space.png'));
await sharp(bgPng).webp({quality:88}).toFile(path.join(out,'background-space.webp'));
const lightBg=svg(1920,1080,`<defs><radialGradient id="wash"><stop stop-color="#D6F4FF"/><stop offset="1" stop-color="#F7FAFD"/></radialGradient></defs><rect width="1920" height="1080" fill="#F7FAFD"/><ellipse cx="260" cy="1000" rx="980" ry="740" fill="url(#wash)"/><g fill="none" stroke="#3EABDF" stroke-width="2" opacity=".10"><ellipse cx="300" cy="1090" rx="820" ry="770"/><ellipse cx="300" cy="1090" rx="620" ry="770"/><ellipse cx="300" cy="1090" rx="330" ry="770"/><ellipse cx="300" cy="1090" rx="820" ry="500"/><ellipse cx="300" cy="1090" rx="820" ry="280"/><path d="M-180 900 C420 510 980 240 1710 395"/></g>`);
await asset('background-light-orbit',lightBg,1920);
const bgData=bgPng.toString('base64');
const text=(x,y,value,size=24,fill='#8EABC3',weight=400)=>`<text x="${x}" y="${y}" font-family="Segoe UI, Microsoft YaHei, sans-serif" font-size="${size}" font-weight="${weight}" fill="${fill}">${value}</text>`;
const colors=edge?['#087BEA','#00BBDF','#21C885','#071423','#F3FAFF']:emerald?['#159447','#35B85A','#A3E89D','#0B1D14','#F2FAF1']:teal?['#087F8C','#079E9C','#58E0BC','#071F24','#F0FBF7']:['#0759EA','#00B7EF','#32F0ED','#071423','#F3FAFF'];
const boardIcon=async(dark,x)=>`<image x="${x}" y="1000" width="180" height="180" href="data:image/png;base64,${(await readFile(path.join(out,'icons',`app-${dark?'dark':'light'}-512.png`))).toString('base64')}"/>`;
const board=svg(2000,1520,`<rect width="2000" height="1520" fill="#071423"/><image width="2000" height="780" preserveAspectRatio="xMidYMid slice" href="data:image/png;base64,${bgData}"/><rect width="2000" height="780" fill="#071423" opacity=".28"/>
${text(90,96,'GEOD GLOBAL / BRAND SYSTEM',24,'#9BBBD8',600)}${mark(90,155,360)}${word(466,365,true,1.85)}${text(476,433,'One agent. A world of geospatial data.',30,'#AEC8DE')}
<rect x="70" y="600" width="920" height="290" rx="28" fill="#F3FAFF"/><rect x="1010" y="600" width="920" height="290" rx="28" fill="#0C2035"/>${mark(114,639,204)}${word(345,767,false,1.08)}${mark(1054,639,204)}${word(1285,767,true,1.08)}${text(110,856,'LIGHT / 深色文字',20,'#59748B')}${text(1050,856,'DARK / 浅色文字',20,'#93B3CE')}
${text(90,962,'APPLICATION ICONS',22,'#A1BAD0',600)}${await boardIcon(true,90)}${await boardIcon(false,310)}${text(90,1220,'Windows · Desktop · Avatar',22)}${text(700,962,'BRAND PALETTE',22,'#A1BAD0',600)}${colors.map((c,i)=>`<rect x="${700+i*246}" y="1000" width="214" height="152" rx="18" fill="${c}"/>${text(700+i*246,1194,c,23,'#AFC3D4')}`).join('')}
<path d="M90 1310H1910" stroke="#253B50"/>${text(90,1375,edge?'蓝 → 青 → 绿 · 立体折带 G · 轨道地球':emerald?'翠绿立体折带 G · 地图植被色 · 轨道地球':teal?'蓝绿立体折带 G · 轨道地球 · 半球太空视角':bitmap?'新版立体折带 G · 轨道地球 · 半球太空视角':'蓝青折带 G · 全球轨道 · 半球太空视角',30,'#D6E9FA',600)}${text(90,1430,'Logo / Icon / Light &amp; Dark / Background · GeoD Global · '+version,23)}
`);
await asset('brand-board',board);
await copyFile(path.join(root,'prototype/public/brand/LICENSE'),path.join(out,'LICENSE'));
console.log(JSON.stringify({output:out,icons:18,icoSizes,background:await sharp(bgPng).metadata(),board:'brand-board.png'},null,2));
