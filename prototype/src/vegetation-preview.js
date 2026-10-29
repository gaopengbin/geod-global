import { vegetationAssetIdentity, VEGETATION_KEYS } from './vegetation.js';
import { MODIS_CRS } from './modis.js';

export const VEGETATION_PREVIEW_COLORS = ['#a50026', '#d73027', '#f46d43', '#fdae61', '#fee08b', '#ffffbf', '#d9ef8b', '#a6d96a', '#66bd63', '#1a9850', '#006837'];

// Original COGs are read in bounded ranges and displayed on their verified
// sinusoidal grid. Display colors never apply a VI quality/reliability mask.
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
  return {
    hrefs:[asset.href],
    bounds: scene.bbox.slice(), index, itemId: scene.id,
  };
}

export function vegetationBrowsePreview(scene) {
  try { vegetationPreview(scene,'ndvi'); return null; }
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
