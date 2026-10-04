import {validQueryBounds,validServiceUrl} from './features-client.js';
const digest=/^[a-f0-9]{64}$/,uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const fail=message=>{throw new Error(message);};
function rootUrl(raw){
 if(!validServiceUrl(raw))fail('Invalid ArcGIS service URL.');const u=new URL(raw);
 if(u.search||u.hash||u.pathname.includes('%')||!/^.*\/rest\/services\/.+\/(MapServer|ImageServer)$/.test(u.pathname))fail('Invalid ArcGIS service URL.');return u;
}
function response(raw,max){
 if(typeof raw!=='string'||!raw||new TextEncoder().encode(raw).length>max)fail('Invalid ArcGIS response receipt.');let v;try{v=JSON.parse(raw);}catch{fail('Invalid ArcGIS response receipt.');}
 if(!v||typeof v!=='object'||Array.isArray(v)||Object.hasOwn(v,'error'))fail('ArcGIS service returned an error.');return v;
}
function imageUrl(root,raw){
 if(!validServiceUrl(raw))fail('Invalid ArcGIS output URL.');const u=new URL(raw),[base,tail]=root.pathname.split('/rest/services/');
 if(u.origin!==root.origin||u.search||u.hash||u.pathname.includes('%'))fail('Invalid ArcGIS output URL.');
 const folder=tail.replaceAll('/','_'),dirs=`${base}/rest/directories/`,output=`${base}/arcgisoutput/`;let parts;
 if(u.pathname.startsWith(dirs)){parts=u.pathname.slice(dirs.length).split('/');if(![2,3].includes(parts.length)||!/^[-\w]+$/.test(parts[0])||(parts.length===3&&parts[1]!==folder))fail('Invalid ArcGIS output URL.');}
 else if(u.pathname.startsWith(output)||u.pathname.startsWith('/arcgisoutput/')){parts=u.pathname.slice(u.pathname.startsWith(output)?output.length:'/arcgisoutput/'.length).split('/');if(parts.length>2||(parts.length===2&&parts[0]!==folder))fail('Invalid ArcGIS output URL.');}
 else fail('Invalid ArcGIS output URL.');
 if(!/^_ags_[\w.-]+\.png$/.test(parts.at(-1))||parts.at(-1).length>180)fail('Invalid ArcGIS output URL.');return u;
}
function discovery(s){
 const root=rootUrl(s.url),c=s.arcgis,v=response(c?.metadata,2*1024*1024),kind=root.pathname.endsWith('/MapServer')?'MapServer':'ImageServer';
 if(s.wmts||!c||c.serviceType!==kind||s.version!==String(v.currentVersion)||!Number.isFinite(v.currentVersion)||v.currentVersion<10||v.currentVersion>=100||s.mapUrl!==`${root}/${kind==='MapServer'?'export':'exportImage'}`||!v.capabilities?.split(',').map(n=>n.trim()).includes(kind==='MapServer'?'Map':'Image')||v.hasMultidimensions||v.hasMultidimensionalInfo||!Array.isArray(c.excludedLayers)||c.excludedLayers.some(e=>typeof e.name!=='string'||typeof e.reason!=='string'))fail('Invalid saved ArcGIS service.');
 if(![s.maxWidth,s.maxHeight].every((n,i)=>Number.isInteger(n)&&n>0&&n<=2048&&n===Math.min(v[i?'maxImageHeight':'maxImageWidth'],2048)))fail('Invalid ArcGIS image size limits.');
 if(kind==='MapServer'){
  const d=response(c.layersMetadata,2*1024*1024);if(!Array.isArray(v.layers)||!Array.isArray(d.layers)||!v.supportedImageFormatTypes?.split(',').some(f=>f.trim().toUpperCase()==='PNG32'))fail('Invalid ArcGIS layer receipt.');
  for(const l of s.layers){const a=v.layers.find(x=>String(x.id)===l.name),b=d.layers.filter(x=>String(x.id)===l.name);if(!a||b.length!==1||a.name!==b[0].name||l.title!==a.name||a.subLayerIds?.length||b[0].type==='Group Layer')fail('ArcGIS layer does not match its discovery receipt.');}
 }else if(c.layersMetadata!==null||s.layers.length!==1||s.layers[0].name!=='image')fail('Invalid ArcGIS image layer receipt.');
 return root;
}
export function validateArcgisMapService(s){
 if(!s||!uuid.test(s.id)||typeof s.name!=='string'||!s.name.trim()||typeof s.title!=='string'||!digest.test(s.capabilitiesSha256)||!Number.isFinite(Date.parse(s.connectedAt))||!Array.isArray(s.layers)||!s.layers.length||s.layers.length>4096)fail('Invalid saved ArcGIS service.');
 const seen=new Set();for(const l of s.layers){if(!l||typeof l.name!=='string'||seen.has(l.name)||typeof l.title!=='string'||l.crs!=='EPSG:4326'||l.wmts||!Array.isArray(l.styles)||l.styles.length||(l.bounds&&!validQueryBounds(l.bounds))||(l.time&&(typeof l.time.values!=='string'||l.time.values.split('/').length!==2||l.time.values.split('/').some(t=>!Number.isFinite(Date.parse(t))))))fail('Invalid saved ArcGIS layer.');seen.add(l.name);}
 discovery(s);return s;
}
export function validateArcgisMapImage(a){
 const s=a?.source,p=s?.arcgis;
 if(!a||!uuid.test(a.id)||typeof a.name!=='string'||!validQueryBounds(a.bounds)||a.imageExtent||a.crs!=='EPSG:4326'||![a.width,a.height].every(n=>Number.isInteger(n)&&n>0&&n<=2048)||!Number.isInteger(a.bytes)||a.bytes<=0||a.bytes>16*1024*1024||!digest.test(a.sha256)||!s||s.wmts||s.style!==''||s.requestCrs!=='EPSG:4326'||s.selection!=='bbox-rendered-map'||!digest.test(s.capabilitiesSha256)||!Number.isFinite(Date.parse(s.requestedAt))||!p||!validQueryBounds(p.requestedBounds)||!digest.test(p.exportSha256)||!uuid.test(p.requestId))fail('Invalid saved ArcGIS map image.');
 const root=rootUrl(s.serviceUrl),c=p.capabilities,metadata=response(c?.metadata,2*1024*1024),kind=root.pathname.endsWith('/MapServer')?'MapServer':'ImageServer';
 if(c.serviceType!==kind||s.version!==String(metadata.currentVersion)||s.mapEndpoint!==`${root}/${kind==='MapServer'?'export':'exportImage'}`)fail('Invalid ArcGIS service receipt.');
 const e=response(p.exportMetadata,64*1024),b=['xmin','ymin','xmax','ymax'].map(k=>e.extent?.[k]);
 if(e.width!==a.width||e.height!==a.height||e.extent?.spatialReference?.wkid!==4326||(e.extent.spatialReference.latestWkid!==undefined&&e.extent.spatialReference.latestWkid!==4326)||e.extent.spatialReference.wkt!==undefined||!validQueryBounds(b)||b.some((n,i)=>n!==a.bounds[i])||e.href!==p.imageUrl)fail('ArcGIS image grid does not match its export receipt.');
 imageUrl(root,e.href);const r=p.requestedBounds,eps=1e-9;
 if(b[0]>r[0]+eps||b[1]>r[1]+eps||b[2]<r[2]-eps||b[3]<r[3]-eps||Math.abs(b[0]+b[2]-r[0]-r[2])>eps||Math.abs(b[1]+b[3]-r[1]-r[3])>eps)fail('Unexpected ArcGIS returned extent.');
 const u=new URL(s.mapEndpoint),q=u.searchParams;for(const [k,v]of Object.entries({f:'json',_geodRequest:p.requestId,bbox:r.join(','),bboxSR:'4326',imageSR:'4326',size:`${a.width},${a.height}`,format:'png32',transparent:'true',...(kind==='MapServer'?{layers:`show:${s.layerName}`,dpi:'96',rotation:'0'}:{adjustAspectRatio:'true'})}))q.set(k,v);
 if(s.time!==null&&s.time!==undefined){if(!Number.isFinite(Date.parse(s.time)))fail('Invalid ArcGIS time.');q.set('time',String(Date.parse(s.time)));}
 const actual=new URL(s.requestUrl),pairs=[...actual.searchParams];
 if(actual.origin!==u.origin||actual.pathname!==u.pathname||actual.hash||actual.username||actual.password||pairs.length!==[...q].length||new Set(pairs.map(([k])=>k)).size!==pairs.length||pairs.some(([k,v])=>!q.has(k)||(k==='bbox'?v.split(',').length!==4||v.split(',').some((n,i)=>Number(n)!==r[i]):v!==q.get(k))))fail('ArcGIS request does not match its saved grid.');
 return a;
}
export const mapProtocol=s=>s?.xyz?(s.xyz.scheme||s.xyz.configuration?.scheme):s?.arcgis?'ArcGIS':s?.wmts?'WMTS':'WMS';
