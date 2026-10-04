import {Resource} from 'cesium';
// Cesium propagates asset.tilesetVersion as ?v=... to descendant requests.
// Blob URLs cannot carry that query. The version remains in the original and
// localized JSON; only this scene-scoped resource adapter ignores its cache key.
// Never patch Resource globally or broaden the renderer to remote asset URLs.
function localUrl(raw,allowed){
  if(typeof raw!=='string')throw new Error('Unknown offline 3D resource');
  if(/^data:(?:application\/(?:octet-stream|gltf-buffer)|image\/(?:png|jpeg));base64,[A-Za-z0-9+/]*={0,2}$/.test(raw)&&raw.length<=44739308)return raw;
  const u=new URL(raw);if(u.protocol!=='blob:'||u.hash||[...u.searchParams.keys()].some(k=>k!=='v'))throw new Error('Unknown offline 3D resource');
  u.search='';const clean=u.href;if(!allowed.has(clean))throw new Error('Unknown offline 3D resource');return clean;
}
export class OfflineThreeDResource extends Resource {
  constructor(url,allowed){super(localUrl(url,allowed));this.allowed=allowed;}
  clone(result){const out=result instanceof OfflineThreeDResource?result:new OfflineThreeDResource(this.url,this.allowed);super.clone(out);out.allowed=this.allowed;out.url=localUrl(out.url,this.allowed);return out;}
  getDerivedResource(options){const out=super.getDerivedResource(options);out.url=localUrl(out.url,this.allowed);return out;}
}
