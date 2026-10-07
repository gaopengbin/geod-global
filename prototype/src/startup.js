import { validateBounds } from './catalog.js';

export function selectedSearchBounds(input, appliedSearch = null) {
  if (appliedSearch?.bbox) return appliedSearch.bbox;
  try { return validateBounds(input.bbox); } catch { return null; }
}

export function agentMapContext({ page, input, appliedSearch, areaPolygon, projectId }) {
  const bounds = selectedSearchBounds(input, appliedSearch);
  // The conversation homepage has no selected map. Native tools supply current
  // UTC search defaults and resolve the place described by the human.
  if (page === 'Home' || !bounds) return null;
  const search = appliedSearch || input;
  return { page, provider:search.provider || 'earth-search', bounds, start:search.start, end:search.end,
    cloudMax:search.cloud ?? 100, projectId:projectId || null, geometry:areaPolygon?.geometry || null };
}
