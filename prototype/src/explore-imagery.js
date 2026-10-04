import { isSupportedAsset, LANDSAT_BANDS, LANDSAT_HOST } from './providers.js';

export function supportedImageryGrid(scene) {
  const shape = scene?.grid?.shape, transform = scene?.grid?.transform;
  return (scene?.provider === 'planetary-naip' ? /^EPSG:269(0[1-9]|1[0-9]|2[0-3])$/.test(scene.crs) : /^EPSG:(326|327)(0[1-9]|[1-5][0-9]|60)$/.test(scene?.crs || ''))
    && Array.isArray(shape) && shape.length === 2 && shape.every(value => Number.isSafeInteger(value) && value > 0 && value <= (scene.provider === 'planetary-naip' ? 40000 : 20000))
    && Array.isArray(transform) && transform.length === 6 && transform.every(Number.isFinite)
    && transform[0] > 0 && transform[4] < 0 && transform[1] === 0 && transform[3] === 0;
}

export function imageryHrefs(scene) {
  if (!supportedImageryGrid(scene)) return [];
  if (scene.provider === 'planetary-naip') return isSupportedAsset(scene.assets?.aerial?.href, 'aerial') ? [scene.assets.aerial.href] : [];
  if (scene.provider !== 'planetary-landsat') return isSupportedAsset(scene.assets?.visual?.href, 'visual') ? [scene.assets.visual.href] : [];
  if (scene.grid.transform[0] !== 30 || scene.grid.transform[4] !== -30) return [];
  const hrefs = LANDSAT_BANDS.map(key => {
    const asset = scene.assets?.[key];
    if (!asset || !isSupportedAsset(asset.href, key)) return null;
    const url = new URL(asset.href), product = url.pathname.split('/').at(-2), parts = product.split('_');
    const band = { red: 4, green: 3, blue: 2 }[key];
    const metadata = asset.rasterBand || asset['raster:bands']?.[0];
    if (url.hostname !== LANDSAT_HOST || parts.length !== 7 || !['LC08', 'LC09'].includes(parts[0])
      || [parts[0], parts[1], parts[2], parts[3], parts[5], parts[6]].join('_') !== scene.id
      || url.pathname.split('/').at(-1) !== `${product}_SR_B${band}.TIF`
      || (metadata && ((metadata.dataType || metadata.data_type) !== 'uint16' || metadata.scale !== 0.0000275
        || metadata.offset !== -0.2 || metadata.nodata !== 0 || (metadata.spatialResolution || metadata.spatial_resolution) !== 30))) return null;
    return asset.href;
  });
  return hrefs.every(Boolean) ? hrefs : [];
}

// Display-only surface-reflectance stretch. Original downloads and DN inspection
// retain their integer samples, including negative or >1 calibrated values.
export function landsatColorStyle() {
  const channel = band => ['*', 255, ['^', ['clamp', ['/', ['+', ['*', ['band', band], 0.0000275], -0.2], 0.3], 0, 1], 1 / 2.2]];
  return { color: ['color', channel(1), channel(2), channel(3),
    ['case', ['all', ['!=', ['band', 1], 0], ['!=', ['band', 2], 0], ['!=', ['band', 3], 0]], 1, 0]] };
}

export function validateLandsatImages(sources, scene) {
  if (sources.length !== 3 || imageryHrefs(scene).length !== 3) throw new Error('Landsat requires three matching original bands.');
  const [height, width] = scene.grid.shape, t = scene.grid.transform;
  let interpretation;
  for (const images of sources) {
    const image = [...images].sort((a, b) => b.getWidth() - a.getWidth())[0];
    const keys = image?.getGeoKeys(), type = keys?.GTRasterTypeGeoKey;
    const bits = image?.fileDirectory.getValue('BitsPerSample'), sample = image?.fileDirectory.getValue('SampleFormat');
    if (!image || image.getWidth() !== width || image.getHeight() !== height || image.getSamplesPerPixel() !== 1
      || bits?.length !== 1 || bits[0] !== 16 || (sample && (sample.length !== 1 || sample[0] !== 1))
      || image.getGDALNoData() !== 0 || ![1, 2].includes(type) || `EPSG:${keys.ProjectedCSTypeGeoKey}` !== scene.crs
      || image.fileDirectory.getValue('ModelTransformation')) throw new Error('Landsat COG metadata does not match the selected original bands.');
    const origin = image.getOrigin(), resolution = image.getResolution();
    const x = origin[0] - (type === 2 ? resolution[0] / 2 : 0);
    const y = origin[1] - (type === 2 ? resolution[1] / 2 : 0);
    if (resolution[0] !== 30 || resolution[1] !== -30 || Math.abs(x - t[2]) > 1e-7 || Math.abs(y - t[5]) > 1e-7
      || (interpretation !== undefined && interpretation !== type)) throw new Error('Landsat COG geometry differs from the selected scene grid.');
    interpretation = type;
  }
  // OpenLayers maps world coordinates to its unadjusted source coordinates;
  // getView() uses the inverse. PixelIsPoint needs -15,+15 in world space.
  return interpretation === 2 ? [1, 0, 0, 1, 15, -15] : undefined;
}
