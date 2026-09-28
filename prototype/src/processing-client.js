import { runtimeRequest } from './runtime-client.js';

export const RECIPE_SCHEMA = 'geod-raster-recipe/v1';
export const POLYGON_RECIPE_SCHEMA = 'geod-raster-recipe/v2';
const object = value => value && typeof value === 'object' && !Array.isArray(value);
function exactKeys(value, keys) {
  return object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

function geometryBounds(geometry) {
  if (!exactKeys(geometry, ['type', 'coordinates']) || !['Polygon', 'MultiPolygon'].includes(geometry.type))
    throw new Error('Use a GeoJSON Polygon or MultiPolygon geometry.');
  const polygons = geometry.type === 'Polygon' ? [geometry.coordinates] : geometry.coordinates;
  if (!Array.isArray(polygons) || !polygons.length || polygons.length > 500) throw new Error('A polygon needs 1 to 500 parts.');
  const extent = [Infinity, Infinity, -Infinity, -Infinity];
  let positions = 0;
  for (const polygon of polygons) {
    if (!Array.isArray(polygon) || !polygon.length || polygon.length > 1000) throw new Error('Each polygon part needs 1 to 1000 rings.');
    for (const ring of polygon) {
      if (!Array.isArray(ring) || ring.length < 4 || !ring.every(point => Array.isArray(point) && point.length === 2 && point.every(Number.isFinite))) throw new Error('Polygon rings need at least four finite longitude/latitude positions.');
      positions += ring.length;
      if (positions > 30000) throw new Error('Polygon has more than 30000 positions.');
      if (ring[0][0] !== ring.at(-1)[0] || ring[0][1] !== ring.at(-1)[1]) throw new Error('Polygon rings must be closed.');
      let area = 0;
      for (let i = 0; i < ring.length - 1; i++) {
        const [x, y] = ring[i], [nextX, nextY] = ring[i + 1];
        if (x < -180 || x > 180 || y < -80 || y > 84 || Math.abs(nextX - x) > 180) throw new Error('Polygon must stay within WGS84 UTM coverage and must not cross the date line.');
        extent[0] = Math.min(extent[0], x); extent[1] = Math.min(extent[1], y);
        extent[2] = Math.max(extent[2], x); extent[3] = Math.max(extent[3], y);
        area += x * nextY - nextX * y;
      }
      if (Math.abs(area) < 1e-12) throw new Error('Polygon rings must have nonzero area.');
    }
  }
  if (extent[2] - extent[0] > 180) throw new Error('Polygon longitude span cannot exceed 180 degrees.');
  return extent;
}

export function validateRecipe(value) {
  if (!exactKeys(value, ['schemaVersion', 'name', 'source', 'operation', 'output']) || ![RECIPE_SCHEMA, POLYGON_RECIPE_SCHEMA].includes(value.schemaVersion))
    throw new Error('Use a supported geod-raster-recipe/v1 or v2 document with only the supported fields.');
  if (typeof value.name !== 'string' || !value.name.trim() || [...value.name.trim()].length > 120 || /\p{Cc}/u.test(value.name))
    throw new Error('Enter a recipe name between 1 and 120 characters, without control characters.');
  if (!exactKeys(value.source, ['jobId', 'sha256']) || typeof value.source.jobId !== 'string'
    || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value.source.jobId)
    || typeof value.source.sha256 !== 'string' || !/^[0-9a-f]{64}$/i.test(value.source.sha256))
    throw new Error('The source must identify a local job and its SHA-256 checksum.');
  const polygon = value.schemaVersion === POLYGON_RECIPE_SCHEMA;
  if (!exactKeys(value.operation, polygon ? ['type', 'crs', 'bounds', 'geometry'] : ['type', 'crs', 'bounds']) || value.operation.type !== 'clip'
    || !['source', 'EPSG:4326'].includes(value.operation.crs))
    throw new Error('Only rectangular clipping in source coordinates or EPSG:4326 is supported.');
  const bounds = value.operation.bounds;
  if (!Array.isArray(bounds) || bounds.length !== 4 || !bounds.every(Number.isFinite)
    || bounds[0] >= bounds[2] || bounds[1] >= bounds[3])
    throw new Error('Enter four coordinates with west less than east and south less than north.');
  if (value.operation.crs === 'EPSG:4326' && (bounds[0] < -180 || bounds[2] > 180 || bounds[1] < -80 || bounds[3] > 84 || bounds[2] - bounds[0] > 180))
    throw new Error('Use longitudes from -180 to 180, latitudes from -80 to 84, and a longitude span of at most 180 degrees.');
  if (polygon) {
    if (value.operation.crs !== 'EPSG:4326') throw new Error('Polygon clips require WGS84 coordinates.');
    const extent = geometryBounds(value.operation.geometry);
    if (extent[0] >= bounds[2] || extent[2] <= bounds[0] || extent[1] >= bounds[3] || extent[3] <= bounds[1]) throw new Error('Polygon mask and output window must overlap.');
  }
  if (!exactKeys(value.output, ['format']) || value.output.format !== 'GeoTIFF')
    throw new Error('The output format must be GeoTIFF.');
  return {
    schemaVersion: value.schemaVersion, name: value.name.trim(),
    source: { jobId: value.source.jobId.toLowerCase(), sha256: value.source.sha256.toLowerCase() },
    operation: { type: 'clip', crs: value.operation.crs, bounds: [...bounds], ...(polygon ? { geometry: structuredClone(value.operation.geometry) } : {}) }, output: { format: 'GeoTIFF' },
  };
}

export function parseRecipeJSON(text) {
  if (typeof text !== 'string' || new TextEncoder().encode(text).length > 512000) throw new Error('Recipe JSON must be smaller than 512 KB.');
  let value;
  try { value = JSON.parse(text); } catch { throw new Error('The recipe is not valid JSON. Check the syntax and try again.'); }
  return validateRecipe(value);
}

export function recipeFingerprint(recipe) { return JSON.stringify(validateRecipe(recipe)); }
export function planMatches(recipe, review) {
  try { return Boolean(review && review.fingerprint === recipeFingerprint(recipe) && recipeFingerprint(review.recipe) === review.fingerprint); }
  catch { return false; }
}

export function validatePlanResponse(response, requestedRecipe) {
  const recipe = validateRecipe(response?.recipe);
  const fingerprint = recipeFingerprint(requestedRecipe);
  const plan = response?.plan;
  const finiteArray = (value, length) => Array.isArray(value) && value.length === length && value.every(Number.isFinite);
  if (recipeFingerprint(recipe) !== fingerprint || !plan || !Number.isSafeInteger(plan.width) || plan.width <= 0
    || !Number.isSafeInteger(plan.height) || plan.height <= 0 || typeof plan.crs !== 'string'
    || !finiteArray(plan.bounds, 4) || !finiteArray(plan.pixelSize, 2) || !finiteArray(plan.window, 4)
    || plan.bounds[0] >= plan.bounds[2] || plan.bounds[1] >= plan.bounds[3] || plan.pixelSize.some(value => value <= 0)
    || plan.window.some(value => !Number.isSafeInteger(value) || value < 0)
    || plan.window[2] !== plan.width || plan.window[3] !== plan.height
    || !Array.isArray(plan.warnings) || plan.warnings.some(value => typeof value !== 'string')
    || (recipe.operation.geometry && (!Number.isSafeInteger(plan.maskedPixels) || plan.maskedPixels < 0 || plan.maskedPixels >= plan.width * plan.height || !Number.isSafeInteger(plan.nodata)))
    || plan.sourceSha256?.toLowerCase() !== recipe.source.sha256)
    throw new Error('The service returned an incomplete plan or a plan for a different recipe.');
  return { recipe, plan, fingerprint };
}

export async function processingRequest(operation, recipe, signal) {
  if (operation === 'list') return runtimeRequest('recipes', undefined, signal);
  const value = validateRecipe(recipe);
  const operations = { plan: 'planRecipe', save: 'saveRecipe', run: 'runRecipe' };
  if (!operations[operation]) throw new Error('Unknown recipe operation.');
  const response = await runtimeRequest(operations[operation], value, signal);
  return operation === 'plan' ? validatePlanResponse(response, value) : response;
}
