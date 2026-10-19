import index from '../../crates/geod-runtime/src/datasets/enmap-reviewed-index.json' with {type:'json'};
import {validQueryBounds} from './features-client.js';
const canonical=v=>v&&typeof v==='object'?(Array.isArray(v)?v.map(canonical):Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])]))):v;
const same=(a,b)=>JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
export const ENMAP=index;
export const enmapRecipe={archiveBytes:index.archiveBytes,archiveSha256:index.archiveSha256,member:index.main.name,memberBytes:index.main.bytes,memberSha256:index.main.sha256};
export function enmapEncoding(){return {kind:'enmap-reviewed-envi-bsq/v1',originalMemberSha256:index.main.sha256,headerSha256:index.header.sha256,
  interleave:'bsq',byteOrder:'little-endian',wavelengthUnit:'nm',wavelengths:index.profile.wavelengths,fwhm:index.profile.fwhm,
  gain:0.0001,offset:0,nodata:-32768,quantity:'surface reflectance',processingMode:'land'};}
export function enmapSelection(asset){
  const p=asset?.metadata?.datasetPreset,r=p?.request,b=index.profile.sourceBounds;
  return asset?.key==='raster'&&asset.mediaType==='application/zip'&&asset.href===index.url&&asset.metadata?.href===index.url
    &&asset.metadata.kind==='reviewed-raster-archive'&&p?.schemaVersion===2&&p.dataset?.id==='enmap'
    &&p.sourceId==='enmap-nevada-l2a-20230707'&&p.layer?.id==='spectral-cube'&&p.layer.scale===0.0001&&p.layer.offset===0&&p.layer.nodata===-32768
    &&p.declarationKind==='reviewed-client-preset; publisher file bytes and embedded metadata verified separately'
    &&r?.datasetId==='enmap'&&r.layer==='spectral-cube'&&['year','month','country','region','season'].every(k=>r[k]==null)
    &&validQueryBounds(r.bounds)&&r.bounds[0]<b[2]&&r.bounds[2]>b[0]&&r.bounds[1]<b[3]&&r.bounds[3]>b[1]
    &&same(p.sourceBounds,b)&&same(asset.metadata.reviewedArchive,enmapRecipe)
    &&asset.metadata['file:size']===index.archiveBytes&&asset.metadata['file:checksum']===`1220${index.archiveSha256}`?enmapRecipe:null;
}
export function reviewedEnmapInspection(data){
  const r=data?.archiveOriginal,p=index.profile;
  return Boolean(index.wholeArchiveVerified&&index.wholeAllMembersCrcVerified&&index.wholeCubeDecodedBandByBand
    &&data?.sha256===index.main.sha256&&data.width===p.width&&data.height===p.height&&data.storageLayout==='bsq'
    &&data.crs==='EPSG:32611'&&data.horizontalCrs===data.crs&&data.verticalCrs==null&&data.pixelInterpretation==='area'
    &&data.sourceModelPixelScale==null&&data.sourceModelTiepoint==null&&data.sampleEncoding==null&&data.categoricalEncoding==null
    &&same(data.transform,p.transform)&&same(data.bounds,p.bounds)&&same(data.spectralEncoding,enmapEncoding())
    &&Array.isArray(data.bands)&&data.bands.length===224&&data.bands.every((b,i)=>b.index===i+1&&b.dataType==='Int16'&&b.nodata===-32768)
    &&Number.isInteger(data.displayBand)&&data.displayBand>=1&&data.displayBand<=224
    &&r?.schemaVersion===1&&r.jobId===data.jobId&&r.archiveUrl===index.url&&r.archiveFile===`${data.jobId}.zip`
    &&r.memberFile===`${data.jobId}.member.bsq`&&same(r.selection,enmapRecipe)
    &&r.sourcePin?.assetKey==='raster'&&/^[a-f0-9]{64}$/.test(r.sourcePin.snapshotId)
    &&enmapSelection({key:'raster',href:index.url,mediaType:'application/zip',metadata:{kind:'reviewed-raster-archive',href:index.url,
      datasetPreset:r.sourceProvenance,reviewedArchive:r.selection,'file:size':index.archiveBytes,'file:checksum':`1220${index.archiveSha256}`}}));
}
export function reviewedEnmapPixel(value){return value?.sha256===index.main.sha256&&Number.isInteger(value.column)&&value.column>=0&&value.column<index.profile.width
  &&Number.isInteger(value.row)&&value.row>=0&&value.row<index.profile.height&&Array.isArray(value.values)&&value.values.length===224
  &&Array.isArray(value.noData)&&value.noData.length===224&&value.values.every((v,i)=>Number.isInteger(v)&&v>=-32768&&v<=32767&&value.noData[i]===(v===-32768));}
