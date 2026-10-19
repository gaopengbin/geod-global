import glc from '../../crates/geod-runtime/src/datasets/glc-fcs10-reviewed-index.json' with {type:'json'};
import glc30 from '../../crates/geod-runtime/src/datasets/glc-fcs30d-reviewed-index.json' with {type:'json'};
import sino from '../../crates/geod-runtime/src/datasets/sinolc-reviewed-index.json' with {type:'json'};
import clcd from '../../crates/geod-runtime/src/datasets/clcd-reviewed-index.json' with {type:'json'};
import ihu from '../../crates/geod-runtime/src/datasets/merit-ihu-reviewed-index.json' with {type:'json'};
import tid from '../../crates/geod-runtime/src/datasets/gebco-tid-reviewed-index.json' with {type:'json'};
import {validQueryBounds} from './features-client.js';
import {enmapSelection,reviewedEnmapInspection} from './enmap-client.js';

const same=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
const canonical=v=>v&&typeof v==='object'?(Array.isArray(v)?v.map(canonical):Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])]))):v;
const fabUrl='https://data.bris.ac.uk/datasets/s5hqmjcdj8yo2ibzi9b4ew3sn/N00E050-N10E060_FABDEM_V1-2.zip';
const fab={archiveBytes:21627700,archiveSha256:'be348eecf2fb835792c1d446d5528a34956a8509c37004da9b72101c313cacfe'};
const fabTiles={
  'N08E050_FABDEM_V1-2':{member:'N08E050_FABDEM_V1-2.tif',memberBytes:5592127,memberSha256:'6a8988122a922f0f11a059829c4ecfc5451364eca1b38708d02bf789668b7837',bounds:[49.99986111111111,8.000138888888891,50.99986111111111,9.00013888888889]},
  'N09E050_FABDEM_V1-2':{member:'N09E050_FABDEM_V1-2.tif',memberBytes:16035307,memberSha256:'c8d2ffb9777dddabefa856636a072af81563eb9fed3bf3cfcfebfe95a9ed9928',bounds:[49.99986111111111,9.000138888888891,50.99986111111111,10.00013888888889]},
};
export function reviewedArchiveSelection(asset) {
  if(asset?.metadata?.datasetPreset?.dataset?.id==='enmap')return enmapSelection(asset);
  const p=asset?.metadata?.datasetPreset,r=p?.request;
  if(asset?.key!=='raster'||asset.mediaType!=='application/zip'||asset.metadata.kind!=='reviewed-raster-archive'
    ||asset.metadata.href!==asset.href||p?.schemaVersion!==2||r?.datasetId!==p.dataset?.id||r.layer!==p.layer?.id
    ||p.declarationKind!=='reviewed-client-preset; publisher file bytes and embedded metadata verified separately')return null;
  let expected,bounds;
  if(p.dataset.id==='fabdem') {
    const t=fabTiles[p.sourceId];if(!t||asset.href!==fabUrl||r.layer!=='elevation'||r.year!=null||r.month!=null||r.country!=null||r.region!=null||r.season!=null||p.layer.scale!==1||p.layer.offset!==0||p.layer.nodata!==-9999)return null;
    bounds=t.bounds;expected={...fab,member:t.member,memberBytes:t.memberBytes,memberSha256:t.memberSha256};
  } else if(p.dataset.id==='glc-fcs10') {
    const t=glc.tiles.find(t=>t.name.replace(/\.tif$/,'')===p.sourceId);
    if(!t||asset.href!==glc.archiveUrl||r.layer!=='classes'||r.year!==2023||r.month!=null||r.country!=null||r.region!=null||r.season!=null||p.layer.scale!==1||p.layer.offset!==0||(p.layer.nodata??null)!==null)return null;
    bounds=t.bounds;expected={archiveBytes:glc.archiveBytes,archiveSha256:glc.archiveSha256,member:t.name,memberBytes:t.bytes,memberSha256:t.sha256};
  } else if(p.dataset.id==='glc-fcs30d') {
    const t=glc30.tiles.find(t=>t.name.replace(/\.tif$/,'')===p.sourceId);
    if(!t||asset.href!==glc30.archiveUrl||r.layer!=='classes-1985'||r.year!==1985||r.month!=null||r.country!=null||r.region!=null||r.season!=null||p.layer.scale!==1||p.layer.offset!==0||(p.layer.nodata??null)!==null)return null;
    bounds=t.bounds;expected={archiveBytes:glc30.archiveBytes,archiveSha256:glc30.archiveSha256,member:t.name,memberBytes:t.bytes,memberSha256:t.sha256};
  } else if(p.dataset.id==='sinolc-1') {
    const t=sino.member;
    if(p.sourceId!==t.name.replace(/\.tif$/,'')||asset.href!==sino.archiveUrl||r.layer!=='classes'||r.year!=null||r.month!=null||r.country!=null||r.region!=null||r.season!=null||p.layer.scale!==1||p.layer.offset!==0||p.layer.nodata!==0)return null;
    bounds=t.bounds;expected={archiveBytes:sino.archiveBytes,archiveSha256:sino.archiveSha256,member:t.name,memberBytes:t.bytes,memberSha256:t.sha256};
  } else if(p.dataset.id==='gebco-tid') {
    const v=tid.tiles.find(v=>v.name.replace(/\.tif$/,'')===p.sourceId);
    if(!tid.wholeArchiveVerified||!tid.wholeAllMembersCrcVerified||!tid.allRasterBasePixelsDecoded||!v||asset.href!==tid.archiveUrl
      ||r.layer!=='tid'||['year','month','country','region','season'].some(k=>r[k]!=null)
      ||p.layer.scale!==1||p.layer.offset!==0||p.layer.nodata!==v.nodata||!validQueryBounds(r.bounds)
      ||r.bounds[0]>=v.bounds[2]||r.bounds[2]<=v.bounds[0]||r.bounds[1]>=v.bounds[3]||r.bounds[3]<=v.bounds[1])return null;
    bounds=v.bounds;expected={archiveBytes:tid.archiveBytes,archiveSha256:tid.archiveSha256,member:v.name,memberBytes:v.bytes,memberSha256:v.sha256};
  } else return null;
  return same(p.sourceBounds,bounds)&&same(asset.metadata.reviewedArchive,expected)
    &&asset.metadata['file:size']===expected.archiveBytes&&asset.metadata['file:checksum']===`1220${expected.archiveSha256}`?expected:null;
}

// These are candidates only. A re-search may create a new metadata snapshot,
// so loading must also compare the original pinned source proof below.
export function reviewedArchiveDownloadCandidates(asset,jobs,snapshotId) {
  const expected=reviewedArchiveSelection(asset);
  if(!expected||asset.eligible!==true)return [];
  return jobs.filter(job=>job.status==='succeeded'&&job.href===asset.href&&job.mediaType==='application/zip'
    &&job.sha256===expected.archiveSha256&&job.bytesDownloaded===expected.archiveBytes
    &&/^[a-f0-9]{64}$/.test(job.stacSource?.snapshotId)&&job.stacSource.assetKey===asset.key)
    .sort((a,b)=>Number(b.stacSource.snapshotId===snapshotId)-Number(a.stacSource.snapshotId===snapshotId));
}

export function reviewedArchiveSourceMatches(asset,job,source) {
  const expected=reviewedArchiveSelection(asset),original=source?.assets?.find(a=>a.key===job.stacSource?.assetKey);
  return Boolean(expected&&asset.eligible===true&&original?.eligible===true
    &&source.id===job.stacSource?.snapshotId
    &&reviewedArchiveDownloadCandidates(asset,[job]).length
    &&same(expected,reviewedArchiveSelection(original))
    &&same(asset.metadata.datasetPreset,original.metadata.datasetPreset));
}

export async function inspectReviewedArchiveDownload(asset,candidates,request,signal) {
  for(const job of candidates) {
    const item=await request('snapshot',{id:job.stacSource.snapshotId},signal);
    if(signal?.aborted)return null;
    // Request bounds, product declarations and member identity must match in
    // full; only the metadata retrieval time/snapshot ID may differ.
    if(!reviewedArchiveSourceMatches(asset,job,item))continue;
    const data=await request('inspect',{id:job.id},signal);
    if(signal?.aborted)return null;
    validateOriginalRasterInspection(data,job,item);
    return {job,item,data};
  }
  throw new Error('The original ZIP and TIFF member do not match their saved dual-checksum receipt.');
}

export function sinolcCategoricalEncoding() {
  return {schema:'geod-sinolc-class-palette/v1',datasetId:'sinolc-1',originalMemberSha256:sino.member.sha256,
    unit:'class code',nodata:0,classes:sino.classes,palette:sino.palette,unlistedCodes:[11,250,255],
    legendUrl:sino.legendUrl,legendSha256:sino.legendSha256,
    rendering:'exact original class ColorMap; unlisted codes retain their palette but no class meaning is invented'};
}

// An exception for one immutable, fully reviewed original. Matching dimensions
// alone never relaxes the ordinary raster or generated-subset limits.
export function reviewedLargeRasterInspection(data) {
  if (reviewedClcdInspection(data)) return true;
  const t=sino.member,receipt=data?.archiveOriginal,p=receipt?.sourceProvenance,r=p?.request;
  const expected={archiveBytes:sino.archiveBytes,archiveSha256:sino.archiveSha256,member:t.name,memberBytes:t.bytes,memberSha256:t.sha256};
  return data?.sha256===t.sha256 && data.width===t.width && data.height===t.height && data.storageLayout==='tile'
    && data.crs===t.crs && data.horizontalCrs===t.crs && data.verticalCrs==null && data.pixelInterpretation===t.registration
    && same(data.bands,[{index:1,dataType:'UInt8',nodata:0}]) && same(data.bounds,t.bounds)
    && same(data.sourceModelPixelScale,t.pixelScale) && same(data.sourceModelTiepoint,t.tiepoint)
    && same(data.transform,[t.pixelScale[0],0,t.bounds[0],0,-t.pixelScale[1],t.bounds[3]])
    && receipt?.schemaVersion===1 && receipt.jobId===data.jobId && receipt.archiveUrl===sino.archiveUrl
    && receipt.archiveFile===`${data.jobId}.zip` && receipt.memberFile===`${data.jobId}.member.tif`
    && /^[a-f0-9]{64}$/.test(receipt.sourcePin?.snapshotId) && receipt.sourcePin.assetKey==='raster'
    && same(receipt.selection,expected) && p?.schemaVersion===2 && p.dataset?.id==='sinolc-1'
    && p.declarationKind==='reviewed-client-preset; publisher file bytes and embedded metadata verified separately'
    && p.sourceId===t.name.replace(/\.tif$/,'') && same(p.sourceBounds,t.bounds)
    && p.layer?.id==='classes' && p.layer.scale===1 && p.layer.offset===0 && p.layer.nodata===0
    && r?.datasetId==='sinolc-1' && r.layer==='classes' && ['year','month','country','region','season'].every(k=>r[k]==null)
    && validQueryBounds(r.bounds)
    && r.bounds[0]<t.bounds[2] && r.bounds[2]>t.bounds[0] && r.bounds[1]<t.bounds[3] && r.bounds[3]>t.bounds[1]
    && same(data.categoricalEncoding,sinolcCategoricalEncoding());
}
export function clcdCategoricalEncoding() {
  const p=clcd.profile;
  return {schema:'geod-clcd-class-palette/v1',datasetId:'clcd',originalSha256:clcd.originalSha256,
    referenceYear:2004,unit:'class code',nodata:0,
    palette:Array.from({length:256},(_,i)=>[p.colorMap[i]/257,p.colorMap[256+i]/257,p.colorMap[512+i]/257,i===0?0:255]),
    classes:clcd.legend.classes,legendUrl:clcd.legend.url,legendSha256:clcd.legend.sha256,
    rendering:'exact original ColorMap; NoData 0 transparent; guide meanings are separate from embedded colors'};
}
export function reviewedClcdInspection(data) {
  const p=clcd.profile;
  return Boolean(clcd.wholeFileVerified && clcd.publisherMd5Verified && /^[a-f0-9]{64}$/.test(clcd.originalSha256)
    && data?.sha256===clcd.originalSha256 && data.width===p.width && data.height===p.height && data.storageLayout==='tile'
    && data.crs===p.crs && data.horizontalCrs===p.crs && data.verticalCrs==null && data.pixelInterpretation===p.registration
    && data.archiveOriginal==null && same(data.bands,[{index:1,dataType:'UInt8',nodata:0}]) && same(data.bounds,p.bounds)
    && same(data.sourceModelPixelScale,p.pixelScale) && same(data.sourceModelTiepoint,p.tiepoint)
    && same(data.transform,[30,0,p.bounds[0],0,-30,p.bounds[3]]) && same(data.categoricalEncoding,clcdCategoricalEncoding()));
}
export function reviewedClcdSelection(asset) {
  const p=asset?.metadata?.datasetPreset,r=p?.request;
  return Boolean(clcd.wholeFileVerified && clcd.publisherMd5Verified && /^[a-f0-9]{64}$/.test(clcd.originalSha256)
    && asset.key==='raster' && asset.href===clcd.url && (asset.originalHref==null||asset.originalHref===clcd.url)
    && asset.transportAdapter==null && asset.metadata?.kind==='direct-raster' && asset.metadata.href===clcd.url
    && asset.metadata['file:size']===clcd.originalBytes && asset.metadata['file:checksum']===`1220${clcd.originalSha256}`
    && same(asset.metadata.reviewedOriginal,{bytes:clcd.originalBytes,sha256:clcd.originalSha256})
    && p?.schemaVersion===2 && p.dataset?.id==='clcd' && p.dataset.version===clcd.release
    && same(p.dataset.years,[2004]) && p.dataset.defaultYear===2004 && p.dataset.layers?.length===1
    && p.sourceId===clcd.sourceId && same(p.sourceBounds,clcd.sourceBounds)
    && p.declarationKind==='reviewed-client-preset; publisher file bytes and embedded metadata verified separately'
    && p.layer?.id==='classes' && p.layer.scale===1 && p.layer.offset===0 && p.layer.nodata===0
    && r?.datasetId==='clcd' && r.layer==='classes' && r.year===2004
    && ['month','country','region','season'].every(k=>r[k]==null) && validQueryBounds(r.bounds)
    && r.bounds[0]<clcd.sourceBounds[2] && r.bounds[2]>clcd.sourceBounds[0]
    && r.bounds[1]<clcd.sourceBounds[3] && r.bounds[3]>clcd.sourceBounds[1]);
}
export function reviewedClcdDownloadCandidates(asset,jobs,snapshotId) {
  if(!reviewedClcdSelection(asset)||asset.eligible!==true)return [];
  return jobs.filter(job=>job.status==='succeeded'&&job.href===clcd.url&&job.sha256===clcd.originalSha256
    &&job.bytesDownloaded===clcd.originalBytes&&job.stacSource?.assetKey==='raster'&&/^[a-f0-9]{64}$/.test(job.stacSource.snapshotId))
    .sort((a,b)=>Number(b.stacSource.snapshotId===snapshotId)-Number(a.stacSource.snapshotId===snapshotId));
}
export async function inspectReviewedClcdDownload(asset,candidates,request,signal) {
  for(const job of candidates) {
    const item=await request('snapshot',{id:job.stacSource.snapshotId},signal);
    if(signal?.aborted)return null;
    const original=item.assets.find(a=>a.key===job.stacSource.assetKey);
    if(!reviewedClcdSelection(original)||!same(asset.metadata.datasetPreset,original.metadata.datasetPreset))continue;
    const data=await request('inspect',{id:job.id},signal);
    if(signal?.aborted)return null;
    validateOriginalRasterInspection(data,job,item);
    return {job,item,data};
  }
  throw new Error('The CLCD original differs from its exact saved source, bytes or physical grid.');
}
export function validateOriginalRasterInspection(data,job,snapshot) {
  const asset=snapshot?.assets?.find(a=>a.key===job.stacSource?.assetKey);
  const productId=snapshot?.provenance?.reviewedDataset?.dataset?.id||asset?.metadata?.datasetPreset?.dataset?.id;
  if(productId==='merit-hydro') {
    const o=reviewedIhuSelection(asset);
    if(!o||!reviewedIhuDownloadCandidates(asset,[job]).length||data.jobId!==job.id||data.sha256!==o.sha256
      ||data.width!==o.width||data.height!==o.height||data.crs!=='EPSG:4326'||data.pixelInterpretation!=='area'
      ||data.bands.length!==1||data.bands[0].dataType.toLowerCase()!==o.dataType||data.bands[0].nodata!==o.nodata
      ||!data.bounds?.every((v,i)=>Math.abs(v-o.bounds[i])<1e-10)
      ||!data.transform?.every((v,i)=>Math.abs(v-o.transform[i])<1e-10))throw new Error('The IHU original differs from its saved source, raw data type or original grid.');
  }
  if(productId==='clcd' && (!reviewedClcdSelection(asset) || !reviewedClcdInspection(data)
    ||job.status!=='succeeded'||job.href!==clcd.url||job.sha256!==clcd.originalSha256||job.bytesDownloaded!==clcd.originalBytes
    ||data.jobId!==job.id||snapshot.id!==job.stacSource?.snapshotId||job.stacSource.assetKey!=='raster'))throw new Error('The CLCD original differs from its exact saved source, bytes or physical grid.');
  if((productId==='vantor-open')!==(data.sampleEncoding!=null))throw new Error('The visual component encoding differs from its reviewed original source.');
  const expected=reviewedArchiveSelection(asset),receipt=data?.archiveOriginal;
  if(productId==='enmap') {
    if(!expected||!reviewedEnmapInspection(data)||job.status!=='succeeded'||job.href!==asset.href||job.sha256!==expected.archiveSha256||job.bytesDownloaded!==expected.archiveBytes||data.jobId!==job.id||snapshot.id!==job.stacSource?.snapshotId||!same(receipt.sourcePin,job.stacSource)||!same(receipt.sourceProvenance,asset.metadata.datasetPreset))throw new Error('EnMAP original cube, header or source receipt changed.');
    return data;
  }
  if(!expected) {
    if(receipt!=null||job.sha256&&data.sha256!==job.sha256)throw new Error('The inspected file differs from the saved download.');
    return data;
  }
  if(job.status!=='succeeded'||job.href!==asset.href||job.mediaType!=='application/zip'||job.sha256!==expected.archiveSha256
    ||job.bytesDownloaded!==expected.archiveBytes||data.sha256!==expected.memberSha256||data.jobId!==job.id
    ||receipt?.schemaVersion!==1||receipt.jobId!==job.id||!same(receipt.sourcePin,job.stacSource)
    ||!same(receipt.sourceProvenance,asset.metadata.datasetPreset)||receipt.archiveUrl!==asset.href
    ||receipt.archiveFile!==`${job.id}.zip`||receipt.memberFile!==`${job.id}.member.tif`||!same(receipt.selection,expected)
    ||!same(data.bounds,asset.metadata.datasetPreset.sourceBounds))throw new Error('The original ZIP and TIFF member do not match their saved dual-checksum receipt.');
  if(productId==='gebco-tid') {
    const v=tid.tiles.find(v=>v.sha256===data.sha256);
    if(!v||data.width!==v.width||data.height!==v.height||data.storageLayout!=='strip'||data.crs!=='EPSG:4326'
      ||data.pixelInterpretation!=='area'||data.bands.length!==1||data.bands[0].dataType!=='Int8'
      ||data.bands[0].nodata!==v.nodata||!data.transform?.every((n,i)=>Math.abs(n-v.transform[i])<1e-10))throw new Error('The GEBCO TID original grid or signed source codes changed.');
  }
  return data;
}

// Float64 and no-overview source files use the bounded native reader after a
// complete download. Do not leave the browser waiting on remote tile decoding.
export function reviewedIhuSelection(asset) {
  const p=asset?.metadata?.datasetPreset,r=p?.request;
  const o=ihu.originals.find(o=>o.id===p?.sourceId);
  return o && asset.key==='raster' && asset.href===o.url && asset.metadata.kind==='direct-raster'
    && p.schemaVersion===2 && p.dataset?.id==='merit-hydro' && p.dataset.version===ihu.release
    && p.declarationKind==='reviewed-client-preset; publisher file bytes and embedded metadata verified separately'
    && r?.datasetId==='merit-hydro' && r.layer===o.layer && p.layer?.id===o.layer
    && p.layer.scale===1 && p.layer.offset===0 && p.layer.nodata===o.nodata
    && ['year','month','country','region','season'].every(k=>r[k]==null) && validQueryBounds(r.bounds)
    && same(p.sourceBounds,[-180,-60,180,85]) ? o : null;
}
export function reviewedIhuDownloadCandidates(asset,jobs,snapshotId) {
  const o=reviewedIhuSelection(asset);
  return !o||asset.eligible!==true?[]:jobs.filter(j=>j.status==='succeeded'&&j.href===o.url&&j.sha256===o.sha256
    &&j.bytesDownloaded===o.bytes&&j.stacSource?.assetKey==='raster'&&/^[a-f0-9]{64}$/.test(j.stacSource.snapshotId))
    .sort((a,b)=>Number(b.stacSource.snapshotId===snapshotId)-Number(a.stacSource.snapshotId===snapshotId));
}
export async function inspectReviewedIhuDownload(asset,candidates,request,signal) {
  for(const job of candidates) {
    const item=await request('snapshot',{id:job.stacSource.snapshotId},signal);
    if(signal?.aborted)return null;
    const original=item.assets.find(a=>a.key===job.stacSource.assetKey);
    if(!reviewedIhuSelection(original)||!same(asset.metadata.datasetPreset,original.metadata.datasetPreset))continue;
    const data=await request('inspect',{id:job.id},signal);if(signal?.aborted)return null;
    validateOriginalRasterInspection(data,job,item);return {job,item,data};
  }
  throw new Error('The IHU original differs from its saved source, raw data type or original grid.');
}
