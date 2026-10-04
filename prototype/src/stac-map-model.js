import { utmDefinition } from './workspace-map-geometry.js';

export function genericProjectionDefinition(crs) {
  if (['EPSG:4326', 'EPSG:3857'].includes(crs)) return null;
  return utmDefinition(crs);
}

export function genericMapMetadata(data) {
  genericProjectionDefinition(data.crs);
  const transform = data.transform, bounds = data.bounds;
  if (!Array.isArray(transform) || transform.length !== 6 || !transform.every(Number.isFinite)
    || transform[0] <= 0 || transform[4] >= 0 || transform[1] !== 0 || transform[3] !== 0
    || !Array.isArray(bounds) || bounds.length !== 4 || !bounds.every(Number.isFinite) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3]
    || !Number.isSafeInteger(data.width) || !Number.isSafeInteger(data.height) || data.width < 1 || data.height < 1) throw new Error('This file has no supported north-up georeferenced grid.');
  const expected = [transform[2], transform[5] + data.height * transform[4], transform[2] + data.width * transform[0], transform[5]];
  if (bounds.some((value, index) => Math.abs(value - expected[index]) > Math.max(1, Math.abs(value)) * 1e-8)) throw new Error('The raster transform and file bounds disagree.');
  return data;
}

export function genericCoordinateToPixel(coordinate, data) {
  if (!coordinate?.every(Number.isFinite) || coordinate.length !== 2) return null;
  const [x, y] = coordinate, [west, south, east, north] = data.bounds;
  if (x < west || x >= east || y <= south || y > north) return null;
  return { column: Math.max(0, Math.min(data.width - 1, Math.floor((x - data.transform[2]) / data.transform[0]))), row: Math.max(0, Math.min(data.height - 1, Math.floor((y - data.transform[5]) / data.transform[4]))) };
}
