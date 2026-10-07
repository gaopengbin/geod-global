import { PROVIDERS, providerById, isSupportedAsset } from './providers.js';
import { normalizeScene } from './catalog.js';
import { validPreviewBounds } from './catalog-preview.js';

export function validatePlanMapPreview(value) {
  const fail=()=>{throw Error('The task map preview is invalid.');};
  const keys=(v,list)=>v && typeof v==='object' && !Array.isArray(v) && Object.keys(v).every(k=>list.includes(k));
  if(!keys(value,['planId','planHash','provider','bounds','geometry','selections'])
    || !/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(value.planId)
    || !/^[a-f0-9]{64}$/.test(value.planHash) || !PROVIDERS.some(p=>p.id===value.provider)
    || !validPreviewBounds(value.bounds) || !Array.isArray(value.selections) || !value.selections.length || value.selections.length>32) fail();
  const ids=new Set();
  for(const item of value.selections) {
    if(!keys(item,['itemId','assets']) || typeof item.itemId!=='string' || !/^[A-Za-z0-9_.-]{1,300}$/.test(item.itemId)
      || ids.has(item.itemId) || !item.assets || Array.isArray(item.assets) || typeof item.assets!=='object'
      || !Object.keys(item.assets).length || Object.keys(item.assets).length>32
      || Object.entries(item.assets).some(([key,href])=>typeof href!=='string' || !isSupportedAsset(href,key))) fail();
    ids.add(item.itemId);
  }
  if(value.geometry!=null) {
    const g=value.geometry;
    if(!keys(g,['type','coordinates']) || !['Polygon','MultiPolygon'].includes(g.type) || !Array.isArray(g.coordinates)) fail();
    const polygons=g.type==='Polygon'?[g.coordinates]:g.coordinates;let points=0;
    if(!polygons.length) fail();
    for(const polygon of polygons) {
      if(!Array.isArray(polygon)||!polygon.length) fail();
      for(const ring of polygon) {
        if(!Array.isArray(ring)||ring.length<4 || ring.some(p=>!Array.isArray(p)||p.length!==2||!p.every(Number.isFinite)||Math.abs(p[0])>180||Math.abs(p[1])>90)
          || ring[0][0]!==ring.at(-1)[0] || ring[0][1]!==ring.at(-1)[1] || (points+=ring.length)>100000) fail();
      }
    }
  }
  return value;
}

async function itemJSON(response) {
  if(!response.ok) throw Error('Imagery metadata could not be loaded.');
  const reader=response.body.getReader(), chunks=[];let size=0;
  try {
    while(true) { const {done,value}=await reader.read();if(done)break;
      size+=value.byteLength;if(size>4*1024*1024)throw Error('Imagery metadata could not be loaded.');chunks.push(value);
    }
  } finally { await reader.cancel(); }
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
  return JSON.parse(new TextDecoder().decode(bytes));
}

// Load only these exact catalog items. Never repeat a search or replace a review's files.
export async function loadPlanPreviewScenes(preview,{signal,fetcher=fetch}={}) {
  validatePlanMapPreview(preview);
  const provider=providerById(preview.provider), results=new Array(preview.selections.length);let next=0;
  const requestSignal=signal?AbortSignal.any([signal,AbortSignal.timeout(30000)]):AbortSignal.timeout(30000);
  const worker=async()=>{while(next<preview.selections.length){const index=next++, selection=preview.selections[index];
    try {
      const url=`${provider.catalog}collections/${encodeURIComponent(provider.collection||'sentinel-2-l2a')}/items/${encodeURIComponent(selection.itemId)}`;
      const item=await itemJSON(await fetcher(url,{signal:requestSignal,credentials:'omit',headers:{Accept:'application/geo+json, application/json'}}));
      if(item.id!==selection.itemId)throw Error('The imagery metadata differs from this task.');
      const scene=normalizeScene(item,provider.id);
      if(!validPreviewBounds(scene.bbox))throw Error('The imagery metadata differs from this task.');
      for(const [key,href] of Object.entries(selection.assets)) {
        // SAFE is not a remotely viewable raster; its public footprint is still useful.
        if(key==='product')continue;
        const actual=scene.assets[key]?.href;
        if(!actual || new URL(actual).origin+new URL(actual).pathname!==new URL(href).origin+new URL(href).pathname)
          throw Error('The imagery metadata differs from this task.');
      }
      results[index]={scene};
    } catch(error) { if(signal?.aborted)throw error;results[index]={error}; }
  }};
  await Promise.all(Array.from({length:Math.min(4,preview.selections.length)},worker));
  return {scenes:results.flatMap(r=>r.scene?[r.scene]:[]),failed:results.filter(r=>r.error).length};
}
