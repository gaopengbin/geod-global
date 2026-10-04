import { vegetationAssetIdentity, VEGETATION_KEYS } from './vegetation.js';
import { MODIS_CRS } from './modis.js';

export const VEGETATION_PREVIEW_COLORS = ['#a50026', '#d73027', '#f46d43', '#fdae61', '#fee08b', '#ffffbf', '#d9ef8b', '#a6d96a', '#66bd63', '#1a9850', '#006837'];

// This is an item-specific display service, separate from original COG access.
// The tiler reprojects the sinusoidal grid and colors signed DN on a fixed
// scientific range. It does not apply a VI quality/reliability mask.
export function vegetationPreview(scene, index) {
  const asset = scene?.assets?.[index];
  const identity = vegetationAssetIdentity(asset?.href, index);
  const band = asset?.rasterBand;
  if (!VEGETATION_KEYS.includes(index) || scene?.provider !== 'planetary-vegetation'
    || scene.collection !== 'modis-13Q1-061' || scene.crs !== MODIS_CRS
    || identity?.id !== scene.id || band?.dataType !== 'int16'
    || band.scale !== 0.0001 || band.offset !== 0 || band.nodata !== -3000
    || !Array.isArray(scene.bbox) || scene.bbox.length !== 4
    || !scene.bbox.every(Number.isFinite) || scene.bbox[0] < -180 || scene.bbox[2] > 180
    || scene.bbox[1] < -90 || scene.bbox[3] > 90
    || scene.bbox[0] >= scene.bbox[2] || scene.bbox[1] >= scene.bbox[3]) {
    throw new Error('This scene has no verified NDVI/EVI preview source.');
  }
  const parameters = new URLSearchParams({
    collection: scene.collection, item: scene.id, assets: `250m_16_days_${index.toUpperCase()}`,
    rescale: '-2000,10000', colormap_name: 'rdylgn', nodata: '-3000', unscale: 'false',
    resampling: 'nearest', reproject: 'nearest', return_mask: 'true',
  });
  return {
    url: `https://planetarycomputer.microsoft.com/api/data/v1/item/tiles/WebMercatorQuad/{z}/{x}/{y}.png?${parameters}`,
    browseURL: `https://planetarycomputer.microsoft.com/api/data/v1/item/preview.png?${parameters}&max_size=128`,
    bounds: scene.bbox.slice(), index, itemId: scene.id,
  };
}

export function vegetationBrowsePreview(scene) {
  try { return vegetationPreview(scene, 'ndvi').browseURL; }
  catch { return null; }
}

// Only the documented out-of-bounds response means an empty tile. A missing
// item, authorization error, server error or malformed image must stay visible.
export async function vegetationTileBlob(url, signal, fetchImpl = fetch) {
  const response = await fetchImpl(url, { signal, credentials: 'omit', redirect: 'error' });
  if (response.status === 404) {
    const body = await response.json().catch(() => null);
    if (/^Tile\(x=\d+, y=\d+, z=\d+\) is outside bounds$/.test(body?.detail || '')) return null;
  }
  if (!response.ok || !response.headers.get('content-type')?.startsWith('image/png')) {
    throw new Error('The index preview tiles could not load. Check your connection or retry.');
  }
  return response.blob();
}
