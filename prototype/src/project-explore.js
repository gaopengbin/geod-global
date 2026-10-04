import { modisIdentity, MODIS_QUALITY_KEYS } from './modis.js';
import { vegetationIdentity, VEGETATION_KEYS } from './vegetation.js';
import { MODIS_SCIENCE } from './modis-science-layers.js';
import { landsatQualityIdentity } from './landsat-quality.js';
import { LANDSAT_QUALITY_KEYS } from './landsat-quality.js';
import { RADAR_KEYS, radarAssetIdentity } from './radar.js';
import { viirsIdentity } from './viirs.js';
import { normalizeScene } from './catalog.js';
import { providerForAssets, providerById, isSupportedAsset, demProduct, naipPixelSize } from './providers.js';

export function projectExploreSearch(project, current) {
  const dates = project.scenes.map(scene => scene.date.slice(0, 10)).sort();
  const last = new Date(`${dates.at(-1).slice(0, 7)}-01T00:00:00Z`);
  last.setUTCMonth(last.getUTCMonth() + 1);
  last.setUTCDate(0);
  return { ...current, provider: providerForAssets(project.scenes[0]?.assets), bbox: project.bounds.join(', '), start: `${dates[0].slice(0, 7)}-01`, end: last.toISOString().slice(0, 10), cloudMin: 0, cloud: 100 };
}

export function projectCatalogScenes(project) {
  return project.scenes.map(scene => normalizeScene({
    id: scene.itemId,
    bbox: scene.bbox,
    collection: providerById(providerForAssets(scene.assets)).collection,
    properties: { ...(RADAR_KEYS.some(key => scene.assets[key]) ? { 'sar:instrument_mode':'IW', 'sar:polarizations':RADAR_KEYS.filter(key => scene.assets[key]).map(key => key.toUpperCase()), 'sat:orbit_state':'unknown' } : {}), ...(viirsIdentity(scene.itemId) ? (() => { const p=viirsIdentity(scene.itemId); return { start_datetime:p.date, end_datetime:p.endDate }; })() : {}), ...((modisIdentity(scene.itemId) || vegetationIdentity(scene.itemId)) ? (() => { const p = modisIdentity(scene.itemId) || vegetationIdentity(scene.itemId); return { start_datetime: p.date, end_datetime: p.endDate, platform: p.platform.toLowerCase(), 'modis:horizontal-tile': p.h, 'modis:vertical-tile': p.v }; })() : {}), datetime: scene.date, 'eo:cloud_cover': scene.cloud, 'proj:code': scene.crs, 'grid:code': scene.gridCode, gsd: demProduct(scene.itemId)?.resolution || (scene.assets.aerial ? naipPixelSize(scene.itemId) : scene.assets.product || scene.assets.visual ? 10 : scene.assets.scl ? 20 : scene.assets.red?.rasterBand?.spatialResolution || 30) },
    assets: { ...Object.fromEntries(Object.entries(scene.assets).map(([key, asset]) => [key, { ...asset, type: asset.mediaType, ...(MODIS_SCIENCE[key] && asset.rasterBand ? { 'raster:bands': [{data_type:asset.rasterBand.dataType,scale:asset.rasterBand.scale,offset:asset.rasterBand.offset,nodata:asset.rasterBand.nodata,spatial_resolution:asset.rasterBand.spatialResolution,...(MODIS_SCIENCE[key].catalogUnit ? {unit:MODIS_SCIENCE[key].catalogUnit}:{})}] } : {}), ...(radarAssetIdentity(asset.href,key) ? { 'raster:bands': [{ data_type:'float32', nodata:-32768, spatial_resolution:10 }] } : {}), ...(landsatQualityIdentity(asset.href,key)?.id === scene.itemId ? { 'raster:bands':[{data_type:'uint16',spatial_resolution:30,...(key === 'qa_pixel' ? {nodata:1}:{})}] } : {}), ...(VEGETATION_KEYS.includes(key) ? { 'raster:bands': [{data_type:'int16',scale:0.0001,offset:0,nodata:-3000,spatial_resolution:250,unit:key.toUpperCase()}] } : {}), ...(MODIS_QUALITY_KEYS.includes(key) && isSupportedAsset(asset.href,key) ? { 'raster:bands': [{ data_type:key === 'modis_qc' ? 'uint32' : 'uint16', spatial_resolution:500 }] } : {}) }])),
      ...(isSupportedAsset(scene.assets.srtm?.href, 'srtm') ? { hgt: { ...scene.assets.srtm, type: 'application/zip' } } : {}),
      ...(isSupportedAsset(scene.assets.elevation?.href, 'elevation') ? { data: { ...scene.assets.elevation, href: `s3://${new URL(scene.assets.elevation.href).hostname.split('.')[0]}/${scene.itemId}/${scene.itemId}.tif` } } : {}),
      ...(isSupportedAsset(scene.assets.aerial?.href, 'aerial') ? { image: { ...scene.assets.aerial, type: scene.assets.aerial.mediaType, 'eo:bands': ['red','green','blue','nir'].map(common_name => ({common_name})) } } : {}) },
  }, providerForAssets(scene.assets)));
}

export function mergeProjectCatalog(catalog, savedScenes) {
  const scenes = new Map((catalog?.scenes || []).map(scene => [scene.id, scene]));
  for (const saved of savedScenes) {
    const current = scenes.get(saved.id);
    if (!current) { scenes.set(saved.id, saved); continue; }
    const assets = { ...current.assets };
    for (const [key, asset] of Object.entries(saved.assets)) {
      assets[key] = current.assets?.[key]?.href === asset.href ? { ...current.assets[key], ...asset } : asset;
    }
    const pinnedBand = ['red','green','blue'].map(key=>saved.assets[key]).find(Boolean);
    if (pinnedBand) for (const key of LANDSAT_QUALITY_KEYS) {
      if (!saved.assets[key] && assets[key] && assets[key].href.slice(0,assets[key].href.lastIndexOf('/')) !== pinnedBand.href.slice(0,pinnedBand.href.lastIndexOf('/'))) delete assets[key];
    }
    const sameImagery = ['visual', 'red', 'green', 'blue', ...VEGETATION_KEYS, ...Object.keys(MODIS_SCIENCE), 'elevation', 'aerial', 'srtm', ...RADAR_KEYS].every(key => !saved.assets[key] || current.assets?.[key]?.href === saved.assets[key].href);
    scenes.set(saved.id, { ...current, assets, grid: sameImagery ? current.grid : saved.grid, crs: saved.crs || current.crs });
  }
  return { ...catalog, scenes: [...scenes.values()] };
}
