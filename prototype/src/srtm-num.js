import {numCell,numHref,isSupportedAsset} from './providers.js';
export const NUM_NOTICE='SRTM NUM retains original source codes on the 1 arc-second Point grid. Codes are not elevations or accuracy scores. No scalar NoData or quality mask is inferred.';
export function numMatchesJob(job,data) {
  const cell=numCell(job?.itemId),spacing=1/3600;
  if(!cell || job.kind!=='download' || job.assetKey!=='srtm_num' || job.href!==numHref(job.itemId) || !isSupportedAsset(job.href,'srtm_num'))return false;
  const bounds=[cell[0]-spacing/2,cell[1]-spacing/2,cell[0]+1+spacing/2,cell[1]+1+spacing/2];
  return Boolean(data?.width===3601 && data.height===3601 && data.bandCount===1 && data.dataType==='UInt8'
    && data.crs==='EPSG:4326' && data.nodata===null && data.bounds?.length===4 && data.bounds.every((x,i)=>Math.abs(x-bounds[i])<1e-10)
    && data.pixelSize?.length===2 && data.pixelSize.every(x=>Math.abs(x-spacing)<1e-12)
    && ['elevation','reflectance','quality','radar','science','aerial','vegetation'].every(key=>data[key]===undefined));
}
export function validNumPixel(result) {
  return Number.isInteger(result?.value) && result.value>=0 && result.value<=255 && result.isNoData===false
    && result.values===undefined && result.reflectance===undefined && result.quality===undefined && result.science===undefined;
}
