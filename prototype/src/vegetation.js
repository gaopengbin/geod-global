import { MODIS_HOST, MODIS_CRS, MODIS_PIXEL } from './modis.js';
import { MODIS_SCIENCE } from './modis-science-layers.js';
import { viSelectionMatches } from './vegetation-quality.js';
export const VEGETATION_KEYS = ['ndvi','evi'];
export const VEGETATION_PRODUCT = 'modis-13q1-v061';
export const VEGETATION_PIXEL = MODIS_PIXEL / 2;
export function vegetationIdentity(id) {
  const m = /^(MOD13Q1|MYD13Q1)\.A(\d{4})(\d{3})\.h(\d{2})v(\d{2})\.061\.(\d{4})(\d{3})(\d{2})(\d{2})(\d{2})$/.exec(id);
  if (!m || +m[2] < 2000 || +m[2] > 9998 || +m[4] > 35 || +m[5] > 17 || +m[8] > 23 || +m[9] > 59 || +m[10] > 59) return null;
  const day = (year,doy) => { const d = new Date(Date.UTC(+year,0,+doy)); return +doy >= 1 && +doy <= 366 && d.getUTCFullYear() === +year ? d : null; };
  const start = day(m[2],m[3]), production = day(m[6],m[7]);
  if (!start || !production || +m[3] % 16 !== (m[1] === 'MOD13Q1' ? 1 : 9) || production < start) return null;
  const end = new Date(Math.min(+start + 15*86400000,Date.UTC(+m[2],11,31)));
  return {product:m[1],platform:m[1] === 'MOD13Q1' ? 'Terra':'Aqua',h:+m[4],v:+m[5],date:start.toISOString(),endDate:end.toISOString().replace('00:00:00.000Z','23:59:59Z'),directory:`/modis-061-cogs/${m[1]}/${m[4]}/${m[5]}/${m[2]}${m[3]}/`};
}
export function vegetationAssetIdentity(href,key) {
  try {
    const url = new URL(href), suffix = `_${MODIS_SCIENCE[key]?.asset || `250m_16_days_${key?.toUpperCase()}`}.tif`, filename = url.pathname.split('/').at(-1);
    if (!(VEGETATION_KEYS.includes(key) || MODIS_SCIENCE[key]) || url.protocol !== 'https:' || url.hostname !== MODIS_HOST || url.port || url.username || url.password || url.search || url.hash || !filename.endsWith(suffix)) return null;
    const id = filename.slice(0,-suffix.length), info = vegetationIdentity(id);
    return info && url.pathname === info.directory + filename ? {...info,id}:null;
  } catch { return null; }
}
export function validVegetationMetadata(data) {
  const info = data?.vegetation;
  if (info?.product !== VEGETATION_PRODUCT || !VEGETATION_KEYS.includes(info.index)
    || data.reflectance !== undefined || data.quality !== undefined || data.elevation !== undefined || data.aerial !== undefined || data.radar !== undefined || data.composite !== undefined
    || data.bandCount !== 1 || data.dataType !== 'Int16' || data.nodata !== -3000 || data.crs !== MODIS_CRS
    || info.palette !== 'modis-vi-v1' || info.scale !== 0.0001 || info.offset !== 0 || info.pixelInterpretation !== 'PixelIsArea'
    || info.validRange?.join(',') !== '-2000,10000' || info.displayRange?.join(',') !== '-2000,10000'
    || data.pixelSize?.length !== 2 || data.pixelSize.some(v=>Math.abs(v-VEGETATION_PIXEL)>1e-6)
    || data.classes?.length !== 0 || !Number.isSafeInteger(info.sampleCount) || info.sampleCount !== data.previewWidth*data.previewHeight
    || !Number.isSafeInteger(info.validSampleCount) || info.validSampleCount < 0 || info.validSampleCount > info.sampleCount
    || !Number.isSafeInteger(info.outOfRangeSampleCount) || info.outOfRangeSampleCount < 0 || info.outOfRangeSampleCount > info.validSampleCount) return false;
  return true;
}
export function vegetationMatchesJob(job,data) {
  const source = vegetationAssetIdentity(job?.href,job?.assetKey);
  if (!source || !validVegetationMetadata(data) || data.vegetation.index !== job.assetKey || !viSelectionMatches(job,data)) return false;
  if (job.kind === 'download') {
    const size = MODIS_PIXEL*2400, expected = [(source.h-18)*size,(8-source.v)*size,(source.h-17)*size,(9-source.v)*size];
    return source.id === job.itemId && data.width === 4800 && data.height === 4800 && data.bounds?.length === 4 && data.bounds.every((v,i)=>Math.abs(v-expected[i])<0.02);
  }
  const plan = job.mosaicOutput, calibration = plan?.calibration;
  return job.kind === 'raster_mosaic' && job.itemId === `project:${job.mosaic?.projectId}` && job.mosaic?.assetKey === job.assetKey && job.mosaic?.sources?.length > 0
    && calibration?.product === VEGETATION_PRODUCT && calibration.signed === true && calibration.nodata === -3000 && calibration.scale === 0.0001 && calibration.offset === 0
    && plan.bandCount === 1 && plan.width === data.width && plan.height === data.height && plan.crs === data.crs
    && plan.bounds?.length === 4 && plan.bounds.every((v,i)=>v===data.bounds[i]) && plan.pixelSize?.length === 2 && plan.pixelSize.every((v,i)=>v===data.pixelSize[i]);
}
export function validVegetationPixel(result,data) {
  if (data?.vegetation?.product !== VEGETATION_PRODUCT || !Number.isInteger(result.value) || result.value < -32768 || result.value > 32767
    || result.reflectance !== undefined || result.values !== undefined || result.decibels !== undefined || result.quality !== undefined || result.isNoData !== (result.value === -3000)) return false;
  return result.isNoData ? result.indexValue === undefined : Number.isFinite(result.indexValue) && Math.abs(result.indexValue-result.value*0.0001)<1e-12;
}
