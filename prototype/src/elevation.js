import { DEM_PRODUCTS, demProduct, demCell, srtmCell, srtmHref, isSupportedAsset } from './providers.js';

export const isElevationKey = key => ['elevation', 'srtm'].includes(key);
export const elevationSourceCheck = 'Transfer size, GeoTIFF signature and SHA-256. Inspect original Float32 heights and the geographic grid in Workspace; terrain accuracy is not assessed.';
export const elevationProductLabel = data => data?.elevation?.product === 'srtmgl1-v003' ? 'NASA SRTMGL1 v003'
  : `Copernicus DEM ${Object.values(DEM_PRODUCTS).find(product => product.product === data?.elevation?.product)?.label || ''}`.trim();
export const heightReference = data => data?.elevation?.product === 'srtmgl1-v003' ? 'EGM96 · EPSG:5773' : 'EGM2008 · EPSG:3855';
export const elevationNotice = job => job.assetKey === 'srtm'
  ? job.kind === 'raster_mosaic'
    ? 'This GeoTIFF keeps the original Int16 heights above EGM96 and the Point grid. No resampling, reprojection or height conversion is applied; gaps and masks are -32768 NoData.'
    : 'Heights above EGM96 come from the verified original HGT ZIP. The grayscale preview changes display only; original Int16 values remain unchanged.'
  : 'Surface height above EGM2008. Grayscale is for display only; original Float32 values are unchanged.';

export function validElevationMetadata(data) {
  const info = data?.elevation;
  if (info?.product === 'srtmgl1-v003') return Boolean(data.dataType === 'Int16' && data.bandCount === 1 && data.crs === 'EPSG:4326'
    && data.reflectance === undefined && data.aerial === undefined && info.heightUnit === 'metre' && info.coordinateUnit === 'degree'
    && info.verticalReference === 'EPSG:5773' && info.pixelInterpretation === 'PixelIsPoint' && info.nodataIsNan === false && data.nodata === -32768
    && Number.isSafeInteger(data.width) && data.width > 0 && Number.isSafeInteger(data.height) && data.height > 0
    && data.pixelSize?.length === 2 && data.pixelSize.every(v => Math.abs(v-1/3600) < 1e-12)
    && Array.isArray(data.classes) && data.classes.length === 0 && Number.isSafeInteger(info.sampleCount) && info.sampleCount === data.previewWidth * data.previewHeight
    && Number.isSafeInteger(info.validSampleCount) && info.validSampleCount >= 0 && info.validSampleCount <= info.sampleCount
    && Array.isArray(info.displayRange) && info.displayRange.length === 2 && info.displayRange.every(v => Number.isInteger(v) && v >= -32767 && v <= 32767)
    && info.displayRange[0] <= info.displayRange[1] && (info.validSampleCount !== 0 || info.displayRange.every(v => v === 0)));
  const product = Object.values(DEM_PRODUCTS).find(product => product.product === info?.product);
  return Boolean(product && data.dataType === 'Float32' && data.bandCount === 1 && data.crs === 'EPSG:4326'
    && data.reflectance === undefined && data.aerial === undefined
    && info.heightUnit === 'metre' && info.coordinateUnit === 'degree' && info.verticalReference === 'EPSG:3855'
    && info.pixelInterpretation === 'PixelIsPoint' && typeof info.nodataIsNan === 'boolean'
    && (data.nodata === null || Number.isFinite(data.nodata) && Math.fround(data.nodata) === data.nodata)
    && (!info.nodataIsNan || data.nodata === null)
    && Number.isSafeInteger(data.width) && data.width > 0 && Number.isSafeInteger(data.height) && data.height > 0
    && Array.isArray(data.pixelSize) && data.pixelSize.length === 2
    && product.widths.some(width => Math.abs(data.pixelSize[0] - 1 / width) < 1e-12)
    && Math.abs(data.pixelSize[1] - 1 / product.height) < 1e-12
    && Array.isArray(data.classes) && data.classes.length === 0
    && Number.isSafeInteger(info.sampleCount) && info.sampleCount === data.previewWidth * data.previewHeight
    && Number.isSafeInteger(info.validSampleCount) && info.validSampleCount >= 0 && info.validSampleCount <= info.sampleCount
    && Array.isArray(info.displayRange) && info.displayRange.length === 2
    && info.displayRange.every(value => Number.isFinite(value) && Math.fround(value) === value)
    && info.displayRange[0] <= info.displayRange[1]
    && (info.validSampleCount !== 0 || info.displayRange.every(value => value === 0)));
}
export function elevationMatchesJob(job, data) {
  const srtm = job?.assetKey === 'srtm';
  if (!isElevationKey(job?.assetKey) || !validElevationMetadata(data) || !isSupportedAsset(job.href, job.assetKey)) return false;
  const product = srtm ? null : demProduct(new URL(job.href).pathname.split('/')[1]);
  if (data.elevation.product !== (srtm ? 'srtmgl1-v003' : product?.product)) return false;
  if (job.kind === 'raster_mosaic') {
    const plan = job.mosaicOutput, profile = plan?.elevation;
    return job.mediaType === 'image/tiff' && job.itemId === `project:${job.mosaic?.projectId}` && job.mosaic?.assetKey === job.assetKey
      && Array.isArray(job.mosaic.sources) && job.mosaic.sources.length > 0
      && ['product', 'heightUnit', 'coordinateUnit', 'verticalReference', 'pixelInterpretation'].every(key => profile?.[key] === data.elevation[key])
      && plan.calibration === undefined && plan.aerial === undefined
      && (srtm ? data.nodata === -32768 && !data.elevation.nodataIsNan : data.nodata === null && data.elevation.nodataIsNan)
      && plan.width === data.width && plan.height === data.height && plan.bandCount === 1 && plan.crs === data.crs
      && Array.isArray(plan.bounds) && plan.bounds.length === 4 && plan.bounds.every((value,index) => Number.isFinite(value) && Math.abs(value-data.bounds[index]) < 1e-10)
      && Array.isArray(plan.pixelSize) && plan.pixelSize.length === 2 && plan.pixelSize.every((value,index) => Number.isFinite(value) && Math.abs(value-data.pixelSize[index]) < 1e-12);
  }
  if (job?.assetKey === 'srtm') {
    const cell = srtmCell(job.itemId), half = 1 / 7200;
    if (!cell || job.kind !== 'download' || data.width !== 3601 || data.height !== 3601 || !validElevationMetadata(data) || data.elevation.product !== 'srtmgl1-v003'
      || !isSupportedAsset(job.href, 'srtm') || job.href !== srtmHref(job.itemId)) return false;
    const expected = [cell[0]-half, cell[1]-half, cell[0]+1+half, cell[1]+1+half];
    return data.bounds?.length === 4 && data.bounds.every((v,i) => Number.isFinite(v) && Math.abs(v-expected[i]) < 1e-10);
  }
  if (job?.assetKey !== 'elevation' || !validElevationMetadata(data) || !isSupportedAsset(job.href, 'elevation')) return false;
  const cell = demCell(job?.itemId);
  if (!cell || job?.kind !== 'download' || job.assetKey !== 'elevation' || !validElevationMetadata(data)
    || !product.widths.includes(data.width) || data.height !== product.height || Math.abs(data.pixelSize[0] - 1 / data.width) > 1e-12
    || job.href !== `https://${product.host}/${job.itemId}/${job.itemId}.tif`) return false;
  const expected = [cell[0] - data.pixelSize[0] / 2, cell[1] + data.pixelSize[1] / 2,
    cell[0] + 1 - data.pixelSize[0] / 2, cell[1] + 1 + data.pixelSize[1] / 2];
  return Array.isArray(data.bounds) && data.bounds.length === 4 && data.bounds.every((value, index) => Number.isFinite(value) && Math.abs(value - expected[index]) < 1e-10);
}
export function validElevationPixel(result, metadata) {
  if (!validElevationMetadata(metadata) || result.values !== undefined || result.reflectance !== undefined || typeof result.isNoData !== 'boolean') return false;
  if (result.value === null) return metadata.elevation.nodataIsNan && result.isNoData;
  if (metadata.elevation.product === 'srtmgl1-v003') return Number.isInteger(result.value) && result.value >= -32768 && result.value <= 32767
    && result.nearInfrared === undefined && result.isNoData === (result.value === -32768);
  return Number.isFinite(result.value) && Math.fround(result.value) === result.value
    && result.isNoData === (metadata.nodata !== null && result.value === metadata.nodata);
}
