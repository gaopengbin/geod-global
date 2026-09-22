import { runtimeRequest } from './runtime-client.js';

export const RECIPE_SCHEMA = 'geod-raster-recipe/v1';
const object = value => value && typeof value === 'object' && !Array.isArray(value);
function exactKeys(value, keys) {
  return object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

export function validateRecipe(value) {
  if (!exactKeys(value, ['schemaVersion', 'name', 'source', 'operation', 'output']) || value.schemaVersion !== RECIPE_SCHEMA)
    throw new Error('Use a geod-raster-recipe/v1 recipe with only the supported fields.');
  if (typeof value.name !== 'string' || !value.name.trim() || [...value.name.trim()].length > 120 || /\p{Cc}/u.test(value.name))
    throw new Error('Enter a recipe name between 1 and 120 characters, without control characters.');
  if (!exactKeys(value.source, ['jobId', 'sha256']) || typeof value.source.jobId !== 'string'
    || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value.source.jobId)
    || typeof value.source.sha256 !== 'string' || !/^[0-9a-f]{64}$/i.test(value.source.sha256))
    throw new Error('The source must identify a local job and its SHA-256 checksum.');
  if (!exactKeys(value.operation, ['type', 'crs', 'bounds']) || value.operation.type !== 'clip'
    || !['source', 'EPSG:4326'].includes(value.operation.crs))
    throw new Error('Only rectangular clipping in source coordinates or EPSG:4326 is supported.');
  const bounds = value.operation.bounds;
  if (!Array.isArray(bounds) || bounds.length !== 4 || !bounds.every(Number.isFinite)
    || bounds[0] >= bounds[2] || bounds[1] >= bounds[3])
    throw new Error('Enter four coordinates with west less than east and south less than north.');
  if (value.operation.crs === 'EPSG:4326' && (bounds[0] < -180 || bounds[2] > 180 || bounds[1] < -80 || bounds[3] > 84 || bounds[2] - bounds[0] > 180))
    throw new Error('Use longitudes from -180 to 180, latitudes from -80 to 84, and a longitude span of at most 180 degrees.');
  if (!exactKeys(value.output, ['format']) || value.output.format !== 'GeoTIFF')
    throw new Error('The output format must be GeoTIFF.');
  return {
    schemaVersion: RECIPE_SCHEMA, name: value.name.trim(),
    source: { jobId: value.source.jobId.toLowerCase(), sha256: value.source.sha256.toLowerCase() },
    operation: { type: 'clip', crs: value.operation.crs, bounds: [...bounds] }, output: { format: 'GeoTIFF' },
  };
}

export function parseRecipeJSON(text) {
  if (typeof text !== 'string' || new TextEncoder().encode(text).length > 100000) throw new Error('Recipe JSON must be smaller than 100 KB.');
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
