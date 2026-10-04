import { RECIPE_SCHEMA, validateRecipe } from './processing-client.js';
import { localRasterKeys, reflectanceMatchesJob, validReflectancePixel } from './reflectance.js';
import { LANDSAT_BANDS } from './providers.js';
import { elevationMatchesJob, validElevationPixel, isElevationKey } from './elevation.js';
import { RADAR_KEYS, radarMatchesJob, validRadarPixel } from './radar.js';
import { aerialMatchesJob, validAerialPixel } from './aerial.js';
import { qualityMatchesJob, validQualityPixel, QUALITY_KEYS } from './quality.js';
import { vegetationMatchesJob, validVegetationPixel, VEGETATION_KEYS } from './vegetation.js';

import { MODIS_SCIENCE } from './modis-science-layers.js';
import { scienceMatchesJob, validSciencePixel } from './modis-science.js';

import { MODIS_CRS, MODIS_PROJECTION } from './modis.js';
import { VIIRS_CRS } from './viirs.js';

export const MAX_MAP_LAYERS = 4;
export const validBounds = bounds => Array.isArray(bounds) && bounds.length === 4 && bounds.every(Number.isFinite)
  && bounds[0] < bounds[2] && bounds[1] < bounds[3];

export function utmDefinition(crs) {
  const nad83 = /^EPSG:269(0[1-9]|1[0-9]|2[0-3])$/.exec(crs);
  if (nad83) return `+proj=utm +zone=${Number(nad83[1])} +datum=NAD83 +units=m +no_defs`;
  const match = /^EPSG:(326|327)(\d{2})$/.exec(crs);
  const zone = Number(match?.[2]);
  if (!match || zone < 1 || zone > 60) throw new Error('The map supports WGS84 UTM raster projections only.');
  return `+proj=utm +zone=${zone}${match[1] === '327' ? ' +south' : ''} +datum=WGS84 +units=m +no_defs`;
}

export function verifiedMapMetadata(job, metadata) {
  rasterProjectionDefinition(metadata?.crs);
  const reflectance = LANDSAT_BANDS.includes(job?.assetKey);
  const elevation = isElevationKey(job?.assetKey);
  const aerial = job?.assetKey === 'aerial';
  if (job?.status !== 'succeeded' || !localRasterKeys.includes(job?.assetKey) || !job.sha256
    || metadata?.sha256?.toLowerCase() !== job.sha256.toLowerCase()
    || metadata?.bandCount !== (aerial ? 4 : job.assetKey === 'visual' ? 3 : 1)
    || (MODIS_SCIENCE[job.assetKey] ? !scienceMatchesJob(job,metadata) : VEGETATION_KEYS.includes(job.assetKey) ? !vegetationMatchesJob(job,metadata) : QUALITY_KEYS.includes(job.assetKey) ? !qualityMatchesJob(job,metadata) : RADAR_KEYS.includes(job.assetKey) ? !radarMatchesJob(job, metadata) : aerial ? !aerialMatchesJob(job, metadata) : elevation ? !elevationMatchesJob(job, metadata) : reflectance ? !reflectanceMatchesJob(job, metadata) : metadata?.dataType !== 'UInt8' || metadata.science !== undefined || metadata.vegetation !== undefined || metadata.quality !== undefined || metadata.reflectance !== undefined || metadata.elevation !== undefined || metadata.aerial !== undefined)
    || !validBounds(metadata.bounds) || !Number.isSafeInteger(metadata.width) || metadata.width < 1
    || !Number.isSafeInteger(metadata.height) || metadata.height < 1
    || !Array.isArray(metadata.pixelSize) || metadata.pixelSize.length !== 2
    || metadata.pixelSize.some(value => !Number.isFinite(value) || value <= 0))
    throw new Error('The raster geometry or checksum does not match this completed source.');
  const expected = [(metadata.bounds[2] - metadata.bounds[0]) / metadata.width, (metadata.bounds[3] - metadata.bounds[1]) / metadata.height];
  if (expected.some((value, index) => Math.abs(value - metadata.pixelSize[index]) > Math.max(1, value) * 1e-8))
    throw new Error('The raster bounds and pixel grid are inconsistent.');
  return metadata;
}
export function rasterProjectionDefinition(crs) {
  if ([MODIS_CRS, VIIRS_CRS].includes(crs)) return MODIS_PROJECTION;
  return crs === 'EPSG:4326' ? '+proj=longlat +datum=WGS84 +no_defs' : utmDefinition(crs);
}

// North-up rasters have their origin at the top-left. The east and south
// edges are outside the pixel domain, unlike inclusive display extents.
export function coordinateToPixel(coordinate, metadata) {
  if (!Array.isArray(coordinate) || coordinate.length !== 2 || !coordinate.every(Number.isFinite)) return null;
  const [x, y] = coordinate;
  const [west, south, east, north] = metadata.bounds;
  if (x < west || x >= east || y <= south || y > north) return null;
  return [Math.min(metadata.width - 1, Math.floor((x - west) / metadata.pixelSize[0])),
    Math.min(metadata.height - 1, Math.floor((north - y) / metadata.pixelSize[1]))];
}

export function intersectBounds(bounds, coverage) {
  if (!validBounds(bounds) || !validBounds(coverage)) return null;
  const result = [Math.max(bounds[0], coverage[0]), Math.max(bounds[1], coverage[1]), Math.min(bounds[2], coverage[2]), Math.min(bounds[3], coverage[3])];
  return validBounds(result) ? result : null;
}

export function focusRasterExtent(sceneExtent, areaExtent) {
  return intersectBounds(sceneExtent, areaExtent) || sceneExtent;
}

// This is a UI preview only. A native, hash-verified plan is still required
// before Save or Run; source values are never read from the display PNG.
export function previewPixelWindow(bounds, metadata) {
  const clipped = intersectBounds(bounds, metadata.bounds);
  if (!clipped) return null;
  const [west, , , north] = metadata.bounds;
  const [dx, dy] = metadata.pixelSize;
  const left = Math.max(0, Math.floor((clipped[0] - west) / dx));
  const top = Math.max(0, Math.floor((north - clipped[3]) / dy));
  const right = Math.min(metadata.width, Math.ceil((clipped[2] - west) / dx));
  const bottom = Math.min(metadata.height, Math.ceil((north - clipped[1]) / dy));
  return [left, top, right - left, bottom - top];
}

export function mapClipRecipe(job, metadata, bounds, name) {
  if (job.assetKey !== 'scl') throw new Error('Use the project to clip or mosaic true-color imagery.');
  verifiedMapMetadata(job, metadata);
  if (!previewPixelWindow(bounds, metadata)) throw new Error('The rectangle must overlap the active raster.');
  return validateRecipe({ schemaVersion: RECIPE_SCHEMA, name,
    source: { jobId: job.id, sha256: metadata.sha256 },
    operation: { type: 'clip', crs: 'source', bounds }, output: { format: 'GeoTIFF' } });
}

export function verifyPixelResult(result, job, metadata, coordinate) {
  const pixel = coordinateToPixel(coordinate, metadata);
  if (!result || !pixel || result.jobId !== job.id || result.sha256 !== metadata.sha256
    || result.crs !== metadata.crs || !Array.isArray(result.pixel) || result.pixel.length !== 2
    || result.pixel.some((value, index) => value !== pixel[index])
    || !Array.isArray(result.coordinate) || result.coordinate.length !== 2
    || result.coordinate.some((value, index) => !Number.isFinite(value) || Math.abs(value - coordinate[index]) > 1e-7)
    || (MODIS_SCIENCE[job.assetKey] ? !validSciencePixel(result,metadata) : VEGETATION_KEYS.includes(job.assetKey) ? !validVegetationPixel(result,metadata) : QUALITY_KEYS.includes(job.assetKey) ? !validQualityPixel(result,metadata) : RADAR_KEYS.includes(job.assetKey) ? !validRadarPixel(result,metadata) : job.assetKey === 'aerial' ? !validAerialPixel(result, metadata) : isElevationKey(job.assetKey) ? !validElevationPixel(result, metadata) : LANDSAT_BANDS.includes(job.assetKey) ? !validReflectancePixel(result, metadata) : job.assetKey === 'visual' ? !Array.isArray(result.values) || result.values.length !== 3
      || result.values.some(value => !Number.isInteger(value) || value < 0 || value > 255)
      || result.color?.toLowerCase() !== '#' + result.values.map(value => value.toString(16).padStart(2, '0')).join('')
      : !Number.isInteger(result.value) || result.value < 0 || result.value > 11)
    || typeof result.label !== 'string' || !/^#[a-f\d]{6}$/i.test(result.color) || typeof result.isNoData !== 'boolean')
    throw new Error('The pixel response does not match the active raster and coordinate.');
  return result;
}
