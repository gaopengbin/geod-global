import { downloadableAssets, runtimeRequest } from './runtime-client.js';
import { SOURCE_ASSET_KEYS } from './providers.js';
import { verifiedViirsScience } from './viirs.js';

import { MODIS_SCIENCE_KEYS } from './modis-science-layers.js';

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
      .filter(asset => SOURCE_ASSET_KEYS.includes(asset.key))
      .map(asset => {
        const band = asset['raster:bands']?.[0];
        const rasterBand = asset.rasterBand || (band ? { dataType: band.data_type, scale: band.scale, offset: band.offset, nodata: band.nodata, spatialResolution: band.spatial_resolution } : null);
        return [asset.key, { href: asset.href, mediaType: asset.type, ...(rasterBand && ['red', 'green', 'blue', 'ndvi', 'evi', ...MODIS_SCIENCE_KEYS].includes(asset.key) ? { rasterBand } : {}) }];
      }));
    if (!Object.keys(assets).length) throw new Error(`Scene ${scene.id} has no supported source raster.`);
    return {
      itemId: scene.id,
      date: scene.date,
      cloud: scene.cloud,
      crs: scene.crs,
      gridCode: scene.properties?.['s2:mgrs_tile'] || scene.properties?.['grid:code'] || null,
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
  const outputs = new Set(jobs.filter(job => job.mosaic?.projectId === project.id || job.kind === 'raster_rgb' && job.rgbSpec?.projectId === project.id).map(job => job.id));
  const sources = new Set(jobs.filter(job => job.kind === 'download' && (project.scenes.some(scene =>
      scene.itemId === job.itemId && scene.assets?.[job.assetKey]?.href === job.href
    ) || job.assetKey === 'stac_asset' && project.stacItems?.some(item => item.snapshotId === job.stacSource?.snapshotId
      && item.assetKey === job.stacSource?.assetKey && item.href === job.href) || job.assetKey === 'wcs_coverage' && project.wcsItems?.some(item => item.planId === job.wcsSource?.planId && item.href === job.href))).map(job => job.id));
  let changed;
  do {
    changed = false;
    for (const job of jobs) {
      const parent = job.parentId || job.recipe?.source?.jobId;
      const safeParent = job.kind === 'raster_prepare' && sources.has(parent)
        && jobs.some(source => source.id === parent && source.assetKey === 'product'
          && source.itemId === job.itemId && source.href === job.href
          && job.safe?.sourceJobId === source.id && job.safe?.sourceSha256 === source.sha256);
      const viirsParent = job.kind === 'raster_prepare' && sources.has(parent)
        && jobs.some(source => source.id === parent && source.assetKey === 'viirs' && source.status === 'succeeded'
          && source.itemId === job.itemId && source.href === job.href && job.viirsPrepare?.sourceJobId === source.id
          && job.viirsPrepare?.sourceSha256 === source.sha256);
      if (!outputs.has(job.id) && parent && (outputs.has(parent) || safeParent || viirsParent)) {
        outputs.add(job.id); changed = true;
      }
    }
  } while (changed);
  return jobs.filter(job => sources.has(job.id) || outputs.has(job.id));
}

export function projectSourceJobs(project, jobs, assetKey) {
  return project.scenes.map(scene => {
    const matches = jobs.filter(job => job.itemId === scene.itemId && job.assetKey === assetKey
      && (job.kind === 'download' && job.href === scene.assets?.[assetKey]?.href
        || job.kind === 'raster_prepare' && scene.assets?.product?.href === job.href
          && jobs.some(parent => parent.id === job.parentId && parent.id === job.safe?.sourceJobId
            && parent.kind === 'download' && parent.status === 'succeeded' && parent.itemId === scene.itemId
            && parent.assetKey === 'product' && parent.href === job.href && parent.sha256 === job.safe?.sourceSha256)
        || job.kind === 'raster_prepare' && scene.assets?.viirs?.href === job.href
          && jobs.some(parent => parent.id === job.parentId && parent.id === job.viirsPrepare?.sourceJobId
            && parent.kind === 'download' && parent.status === 'succeeded' && parent.itemId === scene.itemId
            && parent.assetKey === 'viirs' && parent.href === job.href && parent.sha256 === job.viirsPrepare?.sourceSha256)))
      .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));
    return matches.find(job => job.status === 'succeeded' && (assetKey !== 'viirs' || verifiedViirsScience(job)))
      || matches.find(job => ['queued','running'].includes(job.status)) || matches[0];
  }).filter(Boolean);
}

// A checked original supersedes earlier unsuccessful downloads of that exact
// source pin. The complete records remain available in Tasks; processing jobs
// and downloads with different source identities are still actionable here.
export function pendingProjectJobs(related) {
  return related.filter(job => job.status !== 'succeeded' && !(job.kind === 'download'
    && ['failed','interrupted','cancelled'].includes(job.status)
    && related.some(done => done.kind === 'download' && done.status === 'succeeded' && done.sha256
      && done.itemId === job.itemId && done.assetKey === job.assetKey && done.href === job.href)));
}

function validateAssetKeys(assetKeys) {
  if (!Array.isArray(assetKeys) || !assetKeys.length || new Set(assetKeys).size !== assetKeys.length
    || assetKeys.some(key => !SOURCE_ASSET_KEYS.includes(key))) {
    throw new Error('Choose supported source files for the download.');
  }
}

export async function queueProjectDownloads(project, assetKeys, invoke = runtimeRequest, itemIds) {
  validateAssetKeys(assetKeys);
  const downloads = [];
  try {
    for (const assetKey of assetKeys) {
      downloads.push(await invoke('downloadProject', { id: project.id, assetKey, ...(itemIds ? { itemIds } : {}) }));
    }
  } catch (error) {
    error.project = project;
    error.downloads = downloads;
    throw error;
  }
  return { project, downloads };
}

export async function createProjectAndQueue(request, assetKeys, invoke = runtimeRequest) {
  validateAssetKeys(assetKeys);
  return queueProjectDownloads(await invoke('createProject', request), assetKeys, invoke);
}

export async function addProjectScenesAndQueue(id, request, assetKeys, invoke = runtimeRequest) {
  validateAssetKeys(assetKeys);
  const project = await invoke('addProjectScenes', { id, scenes: request.scenes });
  return queueProjectDownloads(project, assetKeys, invoke, request.scenes.map(scene => scene.itemId));
}
