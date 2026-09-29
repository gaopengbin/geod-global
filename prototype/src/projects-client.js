import { downloadableAssets, runtimeRequest } from './runtime-client.js';

export const MAX_PROJECT_SCENES = 32;

export function projectRequest({ scenes, bounds, geometry, name }) {
  if (!Array.isArray(scenes) || !scenes.length || scenes.length > MAX_PROJECT_SCENES) {
    throw new Error(`Select 1 to ${MAX_PROJECT_SCENES} scenes for one local project.`);
  }
  const normalized = scenes.map(scene => {
    const assets = Object.fromEntries(downloadableAssets(scene)
      .filter(asset => asset.key === 'scl' || asset.key === 'visual')
      .map(asset => [asset.key, { href: asset.href, mediaType: asset.type }]));
    if (!assets.scl || !assets.visual) throw new Error(`Scene ${scene.id} does not provide both SCL and true-color source files.`);
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
