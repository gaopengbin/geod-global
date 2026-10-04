import { VEGETATION_PRODUCT, vegetationAssetIdentity, vegetationIdentity } from './vegetation.js';
export const VI_SELECTION_SCHEMA = 'geod-modis-vi-selection/v1';
export const VI_SELECTION_RULE = 'newest-qualified-complete-ndvi-evi-observation';
export const VI_SELECTION_KEYS = ['ndvi','evi','vi_quality','vi_reliability'];
export const VI_POLICIES = {
  good: 'Good pixels only',
  usable: 'Good or marginal pixels',
};
const digest = v => typeof v === 'string' && /^[0-9a-f]{64}$/.test(v);
const id = v => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const count = v => Number.isSafeInteger(v) && v >= 0;

export function supportsViSelection(project) {
  return project?.scenes?.length > 0 && project.scenes.every(s=>vegetationIdentity(s.itemId || s.id) && VI_SELECTION_KEYS.every(key=>s.assets?.[key]));
}

export function validViSelectionJob(job) {
  const m = job?.mosaic, s = m?.viSelection, p = job?.mosaicOutput, r = p?.viQuality;
  if (!s) return r === undefined && p?.viIndex === undefined;
  if (job.kind !== 'raster_mosaic' || !['ndvi','evi'].includes(job.assetKey) || m.assetKey !== job.assetKey || !id(m.projectId)
    || job.itemId !== `project:${m.projectId}` || p?.viIndex !== job.assetKey || s.schemaVersion !== VI_SELECTION_SCHEMA
    || s.product !== VEGETATION_PRODUCT || s.selection !== VI_SELECTION_RULE || !VI_POLICIES[s.policy]
    || !Array.isArray(s.bounds) || s.bounds.length !== 4 || s.bounds.some(v=>!Number.isFinite(v)) || s.bounds[0] >= s.bounds[2] || s.bounds[1] >= s.bounds[3]
    || !s.scenes?.length || s.scenes.length > 32 || m.sources?.length !== s.scenes.length || p.sourceCount !== s.scenes.length
    || p.overlapPolicy !== VI_SELECTION_RULE || r?.schemaVersion !== VI_SELECTION_SCHEMA || r.policy !== s.policy || r.countsFullResolution !== true
    || !digest(r.specSha256) || !digest(r.selectionSha256) || r.indicesSha256?.length !== 2 || !r.indicesSha256.every(digest)
    || r.sceneValidPixels?.length !== s.scenes.length || !r.sceneValidPixels.every(count)
    || ![p.width,p.height,p.coveredPixels,p.maskedPixels,r.inputCommonValidPixels,r.removedValidPixels,r.rejectedPixels,r.fallbackPixels].every(count)
    || p.width === 0 || p.height === 0 || p.width > 20000 || p.height > 20000) return false;
  const total = p.width*p.height, index = job.assetKey === 'ndvi' ? 0 : 1, seen = new Set();
  if (p.maskedPixels+p.coveredPixels > total || r.inputCommonValidPixels > total-p.maskedPixels || r.inputCommonValidPixels < p.coveredPixels
    || r.removedValidPixels !== r.inputCommonValidPixels-p.coveredPixels || r.rejectedPixels !== total-p.coveredPixels
    || r.fallbackPixels > p.coveredPixels || r.sceneValidPixels.reduce((sum,n)=>sum+n,0) !== p.coveredPixels) return false;
  let previous;
  for (const [i,scene] of s.scenes.entries()) {
    const identity = vegetationIdentity(scene.itemId), order = `${scene.compositeStart}|${scene.itemId}`;
    if (!identity || new Date(scene.compositeStart).getTime() !== new Date(identity.date).getTime()
      || previous && previous >= order || scene.sources?.length !== 4) return false;
    previous = order;
    for (const [k,source] of scene.sources.entries()) {
      if (!id(source.jobId) || seen.has(source.jobId) || !digest(source.sha256) || !count(source.bytes) || !source.bytes
        || source.bytes > 512*1024*1024 || typeof source.attribution !== 'string' || source.attribution.length > 2048
        || vegetationAssetIdentity(source.href,VI_SELECTION_KEYS[k])?.id !== scene.itemId) return false;
      seen.add(source.jobId);
    }
    if (m.sources[i].jobId !== scene.sources[index].jobId || m.sources[i].sha256 !== scene.sources[index].sha256) return false;
  }
  return true;
}

export function viSelectionMatches(job, data) {
  if (!validViSelectionJob(job)) return false;
  const recorded = job.mosaicOutput?.viQuality, shown = data?.vegetation?.qualitySelection;
  if (!recorded) return shown === undefined;
  return Boolean(shown) && Object.keys(recorded).length === Object.keys(shown).length
    && Object.keys(recorded).every(k=>JSON.stringify(recorded[k]) === JSON.stringify(shown[k]));
}
