import { NAIP_HOST, isSupportedAsset, naipMatchesItem, naipPixelSize } from './providers.js';

const VIEW_BANDS = Object.freeze({rgb:Object.freeze([1,2,3]), cir:Object.freeze([4,1,2]), nir:Object.freeze([4,4,4])});
export const AERIAL_VIEWS = Object.freeze(Object.keys(VIEW_BANDS));
export function aerialView(metadata) {
  const bands=metadata?.aerial?.displayBands;
  return AERIAL_VIEWS.find(view => Array.isArray(bands) && bands.length === 3 && bands.every((band,index)=>band===VIEW_BANDS[view][index]));
}
export function aerialPixelColor(pixel, view) {
  if (!pixel || !VIEW_BANDS[view]) return pixel?.color;
  const samples=[...(pixel.values || []), pixel.nearInfrared];
  if (samples.length !== 4 || samples.some(value=>!Number.isInteger(value) || value<0 || value>255)) return pixel.color;
  return '#' + VIEW_BANDS[view].map(band=>samples[band-1].toString(16).padStart(2,'0')).join('');
}
export function verifyAerialView(previous, next, view) {
  const unchanged=['width','height','bandCount','dataType','crs','bounds','pixelSize','nodata','sha256','previewWidth','previewHeight','classes'];
  const profile=['product','bands','pixelInterpretation','coverageMask','sourceExtraSample'];
  if (!validAerialMetadata(previous) || !validAerialMetadata(next) || aerialView(next)!==view
    || unchanged.some(key=>JSON.stringify(previous[key])!==JSON.stringify(next[key]))
    || profile.some(key=>JSON.stringify(previous.aerial[key])!==JSON.stringify(next.aerial[key]))) {
    throw new Error('The aerial display does not match the requested channels or source grid.');
  }
  return next;
}
export function aerialViewLabel(view) {
  return {rgb:'Natural colour',cir:'Colour infrared',nir:'Near infrared'}[view];
}
export function aerialViewCaption(view) {
  return {rgb:'Local aerial RGB overview · original RGB + NIR available',cir:'Local colour-infrared overview · original RGB + NIR available',nir:'Local NIR overview · original RGB + NIR available'}[view];
}

export function aerialTitle(itemId, date) {
  const match=naipPixelSize(itemId) && /^([a-z]{2})_m_(\d{7})_(nw|ne|sw|se)_\d{2}_(?:030|060|100|1)_(\d{4})(\d{2})(\d{2})(?:_\d{8})?$/.exec(itemId);
  return match ? `${date(`${match[4]}-${match[5]}-${match[6]}`)} · ${match[1].toUpperCase()} ${match[2]} ${match[3].toUpperCase()}` : itemId;
}

export function validAerialMetadata(data) {
  const info = data?.aerial;
  return Boolean(info?.product === 'naip' && info.bands?.join(',') === 'red,green,blue,nir'
    && aerialView(data) !== undefined && info.pixelInterpretation === 'PixelIsArea'
    && (info.coverageMask === undefined || info.coverageMask === 'internal-1bit')
    && (info.sourceExtraSample === undefined || info.sourceExtraSample === 2 && info.coverageMask === undefined
      && data.pixelSize?.length === 2 && data.pixelSize.every(value => value === 1))
    && data.bandCount === 4 && data.dataType === 'UInt8' && data.nodata === null
    && /^EPSG:269(0[1-9]|1[0-9]|2[0-3])$/.test(data.crs) && data.classes?.length === 0
    && data.reflectance === undefined && data.elevation === undefined);
}
export function aerialMatchesJob(job, metadata) {
  if (!validAerialMetadata(metadata) || job.assetKey !== 'aerial') return false;
  if (job.kind === 'raster_mosaic') {
    const plan = job.mosaicOutput;
    return Boolean(job.mosaic?.assetKey === 'aerial' && job.mosaic.sources?.length
      && metadata.aerial.coverageMask === 'internal-1bit' && plan?.aerial?.coverageMask === 'internal-1bit'
       && validAerialMetadata({...metadata, aerial:plan.aerial}) && plan.aerial.displayBands.join(',') === '1,2,3' && plan.bandCount === 4
      && plan.width === metadata.width && plan.height === metadata.height && plan.crs === metadata.crs
      && ['bounds','pixelSize'].every(key => Array.isArray(plan[key]) && plan[key].length === metadata[key]?.length && plan[key].every((v,i) => v === metadata[key][i]))
      && plan.calibration === undefined && plan.elevation === undefined);
  }
  if (job.kind !== 'download' || metadata.aerial.coverageMask !== undefined || !isSupportedAsset(job.href, 'aerial')) return false;
  const url = new URL(job.href), parts = job.itemId?.split('_');
  return url.hostname === NAIP_HOST && naipMatchesItem(url.pathname, job.itemId)
    && (metadata.aerial.sourceExtraSample === undefined || parts[5] === '1')
    && metadata.crs === `EPSG:${26900 + Number(parts[4])}`
    && metadata.pixelSize?.length === 2 && metadata.pixelSize.every(value => Math.abs(value - naipPixelSize(job.itemId)) < 1e-8);
}
export function validAerialPixel(result, metadata) {
  return Array.isArray(result.values) && result.values.length === 3
    && [...result.values, result.nearInfrared].every(value => Number.isInteger(value) && value >= 0 && value <= 255)
    && result.value === result.values[0] && result.label === 'RGB + NIR'
    && (result.isNoData === false || result.isNoData === true && metadata?.aerial?.coverageMask === 'internal-1bit'
      && [...result.values, result.nearInfrared].every(v => v === 0))
    && result.color?.toLowerCase() === '#' + result.values.map(value => value.toString(16).padStart(2, '0')).join('');
}

// Validate actual TIFF headers before showing remote source pixels. Preview
// endpoints are never used as a substitute for georeferenced original COGs.
export function validateNaipImages(sources, scene) {
  const image = [...(sources?.[0] || [])].sort((a,b) => b.getWidth() - a.getWidth())[0], keys = image?.getGeoKeys?.();
  const fd = image?.fileDirectory, tag = name => fd?.getValue(name);
  const shape = scene.grid?.shape, affine = scene.grid?.transform;
  const extra = Array.from(tag('ExtraSamples') || []).join(',');
  const legacyNir = scene.id?.split('_')[5] === '1' && naipPixelSize(scene.id) === 1
    && isSupportedAsset(scene.assets?.aerial?.href, 'aerial') && naipMatchesItem(new URL(scene.assets.aerial.href).pathname, scene.id)
    && scene.assets.aerial['eo:bands']?.map(band => band.common_name).join(',') === 'red,green,blue,nir';
  if (sources?.length !== 1 || !image || keys?.GTModelTypeGeoKey !== 1 || keys.GTRasterTypeGeoKey !== 1
    || `EPSG:${keys.ProjectedCSTypeGeoKey}` !== scene.crs || !/^EPSG:269(0[1-9]|1[0-9]|2[0-3])$/.test(scene.crs)
    || image.getSamplesPerPixel() !== 4 || tag('PhotometricInterpretation') !== 2
    || Array.from(tag('BitsPerSample') || []).join(',') !== '8,8,8,8'
    || !(extra === '0' || extra === '2' && legacyNir)
    || (tag('SampleFormat') && Array.from(tag('SampleFormat')).some(value => value !== 1))
    || (tag('PlanarConfiguration') ?? 1) !== 1 || (tag('Orientation') ?? 1) !== 1
    || tag('ModelTransformation')
    || image.getGDALNoData() !== null || image.getWidth() !== shape?.[1] || image.getHeight() !== shape?.[0]
    || !affine || Math.abs(image.getResolution()[0] - affine[0]) > 1e-8
    || Math.abs(image.getResolution()[1] - affine[4]) > 1e-8
    || image.getOrigin().slice(0,2).some((value,index) => Math.abs(value - affine[index ? 5 : 2]) > 1e-7)) throw new Error('NAIP COG metadata does not match the selected RGB + NIR source.');
}
