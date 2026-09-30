import { normalizeScene } from './catalog.js';

export function projectExploreSearch(project, current) {
  const dates = project.scenes.map(scene => scene.date.slice(0, 10)).sort();
  const last = new Date(`${dates.at(-1).slice(0, 7)}-01T00:00:00Z`);
  last.setUTCMonth(last.getUTCMonth() + 1);
  last.setUTCDate(0);
  return { ...current, bbox: project.bounds.join(', '), start: `${dates[0].slice(0, 7)}-01`, end: last.toISOString().slice(0, 10), cloudMin: 0, cloud: 100 };
}

export function projectCatalogScenes(project) {
  return project.scenes.map(scene => normalizeScene({
    id: scene.itemId,
    bbox: scene.bbox,
    properties: { datetime: scene.date, 'eo:cloud_cover': scene.cloud, 'proj:code': scene.crs, 'grid:code': scene.gridCode, gsd: scene.assets.visual ? 10 : 20 },
    assets: Object.fromEntries(Object.entries(scene.assets).map(([key, asset]) => [key, { ...asset, type: asset.mediaType }])),
  }));
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
    const sameVisual = !saved.assets.visual || current.assets?.visual?.href === saved.assets.visual.href;
    scenes.set(saved.id, { ...current, assets, grid: sameVisual ? current.grid : saved.grid, crs: saved.crs || current.crs });
  }
  return { ...catalog, scenes: [...scenes.values()] };
}
