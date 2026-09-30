import { downloadableAssets, runtimeRequest } from './runtime-client.js';

export const MAX_PROJECT_SCENES = 32;

export function scenesForDownload({ scenes, selectedIds = [], loadedIds = [], currentScene }) {
  const byId = new Map(scenes.map(scene => [scene.id, scene]));
  const ids = selectedIds.length ? selectedIds : loadedIds.length ? loadedIds : currentScene ? [currentScene.id] : [];
  return [...new Set(ids)].map(id => byId.get(id)).filter(Boolean);
}

export function projectRequest({ scenes, bounds, geometry, name }) {
  if (!Array.isArray(scenes) || !scenes.length || scenes.length > MAX_PROJECT_SCENES) {
    throw new Error(`Select 1 to ${MAX_PROJECT_SCENES} scenes for one local project.`);
  }
  const normalized = scenes.map(scene => {
    const assets = Object.fromEntries(downloadableAssets(scene)
      .filter(asset => asset.key === 'scl' || asset.key === 'visual')
      .map(asset => [asset.key, { href: asset.href, mediaType: asset.type }]));
    if (!Object.keys(assets).length) throw new Error(`Scene ${scene.id} has no supported source raster.`);
    return {
      itemId: scene.id,
      date: scene.date,
      cloud: scene.cloud,
      crs: scene.crs,
      gridCode: scene.properties?.['s2:mgrs_tile'] || null,
      bbox: scene.bbox,
      assets,
    };
  });
  return { name: name.trim(), bounds, geometry: geometry || null, scenes: normalized };
}

export async function createProject(request) {
  return runtimeRequest('createProject', request);
}

export function jobsForProject(project, jobs) {
  const outputs = new Set(jobs.filter(job => job.mosaic?.projectId === project.id).map(job => job.id));
  const sources = new Set(jobs.filter(job => job.kind === 'download' && project.scenes.some(scene =>
      scene.itemId === job.itemId && scene.assets?.[job.assetKey]?.href === job.href
    )).map(job => job.id));
  let changed;
  do {
    changed = false;
    for (const job of jobs) {
      const parent = job.parentId || job.recipe?.source?.jobId;
      if (!outputs.has(job.id) && parent && outputs.has(parent)) {
        outputs.add(job.id); changed = true;
      }
    }
  } while (changed);
  return jobs.filter(job => sources.has(job.id) || outputs.has(job.id));
}

export async function createProjectAndQueue(request, assetKeys, invoke = runtimeRequest) {
  if (!Array.isArray(assetKeys) || !assetKeys.length || new Set(assetKeys).size !== assetKeys.length
    || assetKeys.some(key => key !== 'scl' && key !== 'visual')) {
    throw new Error('Choose SCL, true-color imagery, or both for the download.');
  }
  const project = await invoke('createProject', request);
  const downloads = [];
  try {
    for (const assetKey of assetKeys) {
      downloads.push(await invoke('downloadProject', { id: project.id, assetKey }));
    }
  } catch (error) {
    error.project = project;
    error.downloads = downloads;
    throw error;
  }
  return { project, downloads };
}
