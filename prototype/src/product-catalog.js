import { SOURCE_DIRECTORY } from './source-directory.js';

// Only declared product families share a picker entry. Scene ids, source ids,
// asset receipts and download validation remain specific to the chosen channel.
// SAFE packages, derived composites and different resolutions stay separate.
const MULTICHANNEL = {
  'earth-search': { id:'sentinel-2-l2a-cog', name:'Sentinel-2 L2A · COG', detail:'10 m true color · 20 m scene classification' },
  'planetary-computer': { id:'sentinel-2-l2a-cog', name:'Sentinel-2 L2A · COG', detail:'10 m true color · 20 m scene classification' },
};
export const productIdentity = source => MULTICHANNEL[source.id] || {id:source.id};
export function catalogProducts(sources = SOURCE_DIRECTORY) {
  const groups = new Map();
  for (const source of sources) {
    const identity=productIdentity(source), current=groups.get(identity.id);
    if(current) current.channels.push(source);
    else groups.set(identity.id,{...source,...identity,channels:[source]});
  }
  return [...groups.values()];
}
export function productForSource(id) {
  return catalogProducts().find(product=>product.channels.some(source=>source.id===id));
}
export const channelLabel = source => source.id==='earth-search'?'Earth Search':source.id==='planetary-computer'?'Planetary Computer':source.platform||source.name||source.id;

const PREFS_KEY='geod-global-product-preferences-v1';
export function readProductPreferences(storage) {
  try {
    storage ??= globalThis.localStorage;
    const value=JSON.parse(storage?.getItem(PREFS_KEY)||'{}');
    const valid=new Set(catalogProducts().map(product=>product.id));
    const ids=list=>Array.isArray(list)?[...new Set(list.filter(id=>typeof id==='string'&&valid.has(id)))].slice(0,40):[];
    const preferred={};
    for(const product of catalogProducts()) if(product.channels.some(source=>source.id===value?.preferred?.[product.id]))preferred[product.id]=value.preferred[product.id];
    return {favorites:ids(value?.favorites),recent:ids(value?.recent).slice(0,8),preferred};
  } catch { return {favorites:[],recent:[],preferred:{}}; }
}
export function writeProductPreferences(value, storage) {
  try { (storage ?? globalThis.localStorage)?.setItem(PREFS_KEY,JSON.stringify(value)); } catch { /* Discovery remains usable without storage. */ }
}
