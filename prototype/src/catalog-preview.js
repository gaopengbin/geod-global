import { MODIS_CRS, MODIS_ASSET_NAMES, modisAssetIdentity } from './modis.js';
import { RADAR_KEYS, radarAssetIdentity } from './radar.js';
import { vegetationPreview } from './vegetation-preview.js';

// Display capabilities are independent of original-download eligibility. These
// modes never enter the optical RGB COG loader or turn a preview into a download.
export function catalogPreviewKind(provider) {
  const id = typeof provider === 'string' ? provider : provider?.id;
  return ({'planetary-vegetation':'vegetation','planetary-modis':'reflectance','planetary-radar':'radar',
    'copernicus-dem':'elevation','copernicus-dem-90':'elevation'})[id] || null;
}
export function catalogPreviewChannel(scene, preferred) {
  const kind = catalogPreviewKind(scene?.provider);
  if (kind === 'vegetation') return preferred === 'evi' ? 'evi' : 'ndvi';
  if (kind === 'radar') return RADAR_KEYS.filter(key => scene.assets?.[key]).find(key => key === preferred)
    || RADAR_KEYS.find(key => scene.assets?.[key]);
  return kind === 'reflectance' ? 'rgb' : kind === 'elevation' ? 'height' : undefined;
}
export function validPreviewBounds(bounds) {
  return Array.isArray(bounds) && bounds.length === 4 && bounds.every(Number.isFinite)
    && bounds[0] >= -180 && bounds[2] <= 180 && bounds[1] >= -90 && bounds[3] <= 90
    && bounds[0] < bounds[2] && bounds[1] < bounds[3];
}
function itemPreview(scene, channel, parameters) {
  const params = new URLSearchParams({collection:scene.collection,item:scene.id,
    unscale:'false',resampling:'nearest',reproject:'nearest',return_mask:'true'});
  parameters.forEach(([key,value]) => params.append(key,value));
  return {url:`https://planetarycomputer.microsoft.com/api/data/v1/item/tiles/WebMercatorQuad/{z}/{x}/{y}.png?${params}`,
    browseURL:`https://planetarycomputer.microsoft.com/api/data/v1/item/preview.png?${params}&max_size=128`,
    bounds:scene.bbox.slice(),channel,itemId:scene.id};
}
export function modisReflectancePreview(scene) {
  if (scene?.provider !== 'planetary-modis' || scene.collection !== 'modis-09A1-061'
    || scene.crs !== MODIS_CRS || !validPreviewBounds(scene.bbox)
    || ['red','green','blue'].some(key => {
      const asset = scene.assets?.[key], band = asset?.rasterBand;
      return modisAssetIdentity(asset?.href,key)?.id !== scene.id || band?.dataType !== 'int16'
        || band.scale !== 0.0001 || band.offset !== 0 || band.nodata !== -28672 || band.spatialResolution !== 500;
    })) throw new Error('This scene has no verified MODIS reflectance preview source.');
  // Signed DN 0..3000 corresponds to reflectance 0..0.3. Gamma is display only.
  return itemPreview(scene,'rgb',[
    ...['red','green','blue'].map(key => ['assets',MODIS_ASSET_NAMES[key]]),
    ['rescale','0,3000'],['color_formula','Gamma RGB 2.2'],['nodata','-28672'],
  ]);
}
export function radarPreview(scene, channel) {
  const asset = scene?.assets?.[channel], band = asset?.['raster:bands']?.[0];
  if (scene?.provider !== 'planetary-radar' || scene.collection !== 'sentinel-1-rtc' || !RADAR_KEYS.includes(channel)
    || !radarAssetIdentity(asset?.href,channel)?.ids.includes(scene.id) || asset?.['raster:bands']?.length !== 1
    || band?.data_type !== 'float32' || band.nodata !== -32768 || band.spatial_resolution !== 10
    || (band.scale ?? 1) !== 1 || (band.offset ?? 0) !== 0 || !validPreviewBounds(scene.bbox)) {
    throw new Error('This scene has no verified radar polarization preview source.');
  }
  // Original linear gamma0 is retained. Valid zero intensity is shown at the
  // dark end of the display range; only the source's -32768 mask is NoData.
  return itemPreview(scene,channel,[['assets',channel],['asset_as_band','true'],
    ['expression',`where(${channel}>0,10*log10(${channel}),-30)`],['rescale','-30,0'],['nodata','-32768']]);
}
export function catalogItemPreview(scene, channel) {
  const kind = catalogPreviewKind(scene?.provider);
  if (kind === 'vegetation') return vegetationPreview(scene,channel);
  if (kind === 'reflectance' && channel === 'rgb') return modisReflectancePreview(scene);
  if (kind === 'radar') return radarPreview(scene,channel);
  throw new Error('This scene has no supported online preview source.');
}
export function catalogBrowsePreview(scene) {
  try { return catalogItemPreview(scene,catalogPreviewChannel(scene)).browseURL; }
  catch { return scene?.thumbnail || null; }
}
