import {get as getProjection, getPointResolution, transform} from 'ol/proj.js';
import {register} from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import {rasterProjectionDefinition} from './workspace-map-geometry.js';

const positive = value => Number.isFinite(value) && value > 0;
const validBounds = bounds => bounds?.length === 4 && bounds.every(Number.isFinite)
  && bounds[0] < bounds[2] && bounds[1] < bounds[3];

// A source pixel may occupy up to four CSS pixels. Zoom changes the view only;
// it never changes the original grid or invents additional image detail.
export function rasterPixelResolution(raster, viewProjection) {
  if (!raster) return null;
  const code = typeof viewProjection === 'string' ? viewProjection : viewProjection.getCode();
  const grid = raster.grid || raster;
  const t = grid.transform, shape = grid.shape || [raster.height, raster.width];
  let bounds = raster.bounds, spacing = raster.pixelSize;
  if (t?.length === 6 && t.every(Number.isFinite) && t[1] === 0 && t[3] === 0
    && t[0] > 0 && t[4] < 0 && shape?.length === 2 && shape.every(positive)) {
    spacing = [t[0], -t[4]];
    bounds = [t[2], t[5] + shape[0] * t[4], t[2] + shape[1] * t[0], t[5]];
  }
  if (validBounds(bounds) && !spacing && shape?.length === 2 && shape.every(positive))
    spacing = [(bounds[2] - bounds[0]) / shape[1], (bounds[3] - bounds[1]) / shape[0]];
  if (spacing?.length === 2 && spacing.every(positive) && validBounds(bounds) && raster.crs) {
    try {
      if (raster.crs === code) return Math.min(...spacing);
      if (!getProjection(raster.crs)) {
        proj4.defs(raster.crs, rasterProjectionDefinition(raster.crs));
        register(proj4);
      }
      const center = [(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2];
      const origin = transform(center, raster.crs, code);
      const east = transform([center[0] + spacing[0], center[1]], raster.crs, code);
      const north = transform([center[0], center[1] + spacing[1]], raster.crs, code);
      const sizes = [Math.hypot(east[0] - origin[0], east[1] - origin[1]),
        Math.hypot(north[0] - origin[0], north[1] - origin[1])];
      if (sizes.every(positive)) return Math.min(...sizes);
    } catch { /* Unavailable source geometry may still have a declared GSD. */ }
  }
  if (positive(raster.gsd) && validBounds(raster.bbox)) {
    try {
      const center = transform([(raster.bbox[0] + raster.bbox[2]) / 2,
        (raster.bbox[1] + raster.bbox[3]) / 2], 'EPSG:4326', code);
      const metresPerUnit = getPointResolution(code, 1, center, 'm');
      if (positive(metresPerUnit)) return raster.gsd / metresPerUnit;
    } catch { /* Keep the existing safe limit when resolution is unknown. */ }
  }
  return null;
}

export function setRasterZoomLimit(view, rasters, fallback = 2.5) {
  if (!view) return;
  const sizes = rasters.map(raster => rasterPixelResolution(raster, view.getProjection())).filter(positive);
  const minimum = sizes.length ? Math.min(...sizes) / 4 : fallback;
  // OpenLayers snaps the maximum to whole zoom levels. Round down so the
  // oversampling bound is respected, including mixed-resolution layers.
  const maximum = Math.max(view.getMinZoom(), Math.floor(view.getZoomForResolution(minimum) + 1e-9));
  if (view.getMaxZoom() !== maximum) view.setMaxZoom(maximum);
}
