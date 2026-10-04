import { isSupportedAsset, LANDSAT_BANDS, LANDSAT_HOST, NASA_HOST } from './providers.js';

import { MODIS_HOST, MODIS_CRS, MODIS_PIXEL, modisAssetIdentity } from './modis.js';
import { VIIRS_CRS, VIIRS_PIXEL, VIIRS_HOST, viirsAssetIdentity, viirsPreparationScience } from './viirs.js';

import { MODIS_SCIENCE_KEYS } from './modis-science-layers.js';

export const localRasterKeys = ['reflectance_rgb', 'scl', 'visual', ...LANDSAT_BANDS, 'ndvi', 'evi', ...MODIS_SCIENCE_KEYS, 'modis_qc', 'modis_state', 'qa_pixel', 'qa_radsat', 'elevation', 'aerial', 'srtm', 'vv', 'vh', 'hh', 'hv'];
const profiles = {
  'viirs-09a1-v002': { dataType: 'Int16', nodata: -28672, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
  'modis-09a1-v061': { dataType: 'Int16', nodata: -28672, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
  'landsat-c2-l2': { dataType: 'UInt16', nodata: 0, scale: 0.0000275, offset: -0.2, min: 0, max: 65535 },
  'hls-l30-v2': { dataType: 'Int16', nodata: -9999, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
};
export function validReflectanceMetadata(data) {
  const info = data?.reflectance;
  const profile = profiles[info?.product];
  return Boolean(profile && data.vegetation === undefined && data.bandCount === 1 && data.dataType === profile.dataType
    && data.nodata === profile.nodata && LANDSAT_BANDS.includes(info.band)
    && info.scale === profile.scale && info.offset === profile.offset
    && ['PixelIsArea', 'PixelIsPoint'].includes(info.pixelInterpretation)
    && (info.product !== 'modis-09a1-v061' || data.crs === MODIS_CRS && info.pixelInterpretation === 'PixelIsArea')
    && (info.product !== 'viirs-09a1-v002' || data.crs === VIIRS_CRS && info.pixelInterpretation === 'PixelIsArea')
    && Array.isArray(data.pixelSize) && data.pixelSize.length === 2 && data.pixelSize.every(value => Math.abs(value - (info.product === 'viirs-09a1-v002' ? VIIRS_PIXEL : info.product === 'modis-09a1-v061' ? MODIS_PIXEL : 30)) < 1e-6)
    && Array.isArray(data.classes) && data.classes.length === 0
    && Number.isSafeInteger(info.sampleCount) && info.sampleCount === data.previewWidth * data.previewHeight
    && Number.isSafeInteger(info.validSampleCount) && info.validSampleCount >= 0 && info.validSampleCount <= info.sampleCount
    && Array.isArray(info.displayRange) && info.displayRange.length === 2
    && info.displayRange.every(value => Number.isInteger(value) && value >= profile.min && value <= profile.max)
    && info.displayRange[0] <= info.displayRange[1]
    && (info.validSampleCount !== 0 || info.displayRange.every(value => value === 0)));
}
export function reflectanceMatchesJob(job, metadata) {
  if (!validReflectanceMetadata(metadata) || metadata.reflectance.band !== job.assetKey
    || !(metadata.reflectance.product === 'viirs-09a1-v002' ? viirsAssetIdentity(job.href) : isSupportedAsset(job.href, job.assetKey))) return false;
  if (job.kind === 'raster_prepare') {
    const s = viirsPreparationScience(job);
    return Boolean(s && metadata.reflectance.product === 'viirs-09a1-v002' && metadata.width === s.width && metadata.height === s.height
      && s.bounds.every((v,i) => Math.abs(v-metadata.bounds?.[i]) < 1e-8) && s.pixelSize.every((v,i) => v === metadata.pixelSize?.[i]));
  }
  const url = new URL(job.href);
  if (job.kind === 'raster_mosaic') {
    const plan = job.mosaicOutput;
    const calibration = plan?.calibration;
    return job.itemId === `project:${job.mosaic?.projectId}`
      && job.mosaic?.assetKey === job.assetKey && job.mosaic?.sources?.length > 0
      && calibration?.product === metadata.reflectance.product
      && calibration.signed === (metadata.dataType === 'Int16')
      && calibration.scale === metadata.reflectance.scale && calibration.offset === metadata.reflectance.offset
      && calibration.nodata === metadata.nodata && plan.width === metadata.width && plan.height === metadata.height
      && plan.bandCount === 1 && plan.crs === metadata.crs
      && plan.bounds?.every((value, index) => value === metadata.bounds[index])
      && plan.pixelSize?.every((value, index) => value === metadata.pixelSize[index])
      && metadata.reflectance.pixelInterpretation === 'PixelIsArea'
      && url.hostname === (calibration.product === 'viirs-09a1-v002' ? VIIRS_HOST : calibration.product === 'modis-09a1-v061' ? MODIS_HOST : calibration.product === 'hls-l30-v2' ? NASA_HOST : LANDSAT_HOST);
  }
  if (metadata.reflectance.product === 'viirs-09a1-v002') return false;
  if (metadata.reflectance.product === 'modis-09a1-v061') return job.kind === 'download' && metadata.width === 2400 && metadata.height === 2400 && url.hostname === MODIS_HOST && modisAssetIdentity(job.href, job.assetKey)?.id === job.itemId;
  if (metadata.reflectance.product === 'hls-l30-v2') {
    return url.hostname === NASA_HOST && url.pathname.split('/')[3] === job.itemId;
  }
  const product = url.pathname.split('/').at(-2);
  const parts = product?.split('_');
  return url.hostname === LANDSAT_HOST && parts?.length === 7 && ['LC08', 'LC09'].includes(parts[0])
    && [parts[0], parts[1], parts[2], parts[3], parts[5], parts[6]].join('_') === job.itemId;
}
export function validReflectancePixel(result, metadata) {
  const profile = profiles[metadata.reflectance?.product];
  if (!profile || !Number.isInteger(result.value) || result.value < profile.min || result.value > profile.max
    || result.values !== undefined || result.isNoData !== (result.value === profile.nodata)) return false;
  if (result.isNoData) return result.reflectance === undefined;
  const expected = result.value * profile.scale + profile.offset;
  return Number.isFinite(result.reflectance) && Math.abs(result.reflectance - expected) < 1e-12;
}
