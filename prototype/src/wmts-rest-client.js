import {validServiceUrl} from './features-client.js';
const fail=message=>{throw new Error(message);};
export function wmtsRestTemplate(raw,root,time) {
  if(typeof raw!=='string'||!raw||raw.length>2048||!/^[\x21-\x7e]+$/.test(raw)||/[%\\?#]/.test(raw))fail('Unsupported WMTS REST tile template');
  const names=new Set();let probe=raw;
  for(const match of raw.matchAll(/\{([^{}]+)\}/g)) {
    const key=match[1];if(names.has(key)||!['Layer','Style','TileMatrixSet','TileMatrix','TileRow','TileCol',...(time?[time]:[])].includes(key))fail('Unsupported WMTS REST tile placeholder');
    names.add(key);probe=probe.replace(match[0],'0');
  }
  if(/[{}]/.test(probe)||['TileMatrix','TileRow','TileCol',...(time?[time]:[])].some(k=>!names.has(k)))fail('WMTS REST template must include tile coordinates and every supported dimension');
  const target=new URL(probe,root),canonical=new URL(raw,root).href.replaceAll('%7B','{').replaceAll('%7D','}');
  const path=canonical.split('://')[1]?.slice(canonical.split('://')[1].indexOf('/')+1);
  if(!validServiceUrl(target.href)||target.origin!==new URL(root).origin||target.search||canonical.includes('%')||[...names].some(k=>!path?.includes(`{${k}}`))||target.hostname==='tile.openstreetmap.org'||target.hostname.endsWith('.tile.openstreetmap.org'))fail('WMTS REST placeholders require an allowed public URL path');
  return canonical;
}
const segment=raw=>{
  if(typeof raw!=='string'||!raw||raw.trim()!==raw||raw.length>256||/[\x00-\x1f\x7f]/.test(raw)||['.','..'].includes(raw))fail('Unsupported WMTS REST identifier');
  return encodeURIComponent(raw).replace(/[!'()*]/g,c=>`%${c.charCodeAt(0).toString(16).toUpperCase()}`);
};
export function wmtsRestTileUrl(source,snapshot,row,col) {
  let raw=wmtsRestTemplate(snapshot.resourceUrl,source.serviceUrl,snapshot.timeIdentifier);
  const values={Layer:source.layerName,Style:source.style,TileMatrixSet:snapshot.matrixSet,TileMatrix:snapshot.matrix.id,TileRow:String(row),TileCol:String(col)};
  if(snapshot.timeIdentifier)values[snapshot.timeIdentifier]=source.time;
  for(const[key,value]of Object.entries(values))if(raw.includes(`{${key}}`))raw=raw.replace(`{${key}}`,segment(value));
  return new URL(raw).href;
}
