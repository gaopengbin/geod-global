import { isSupportedAsset, SOURCE_ASSET_KEYS } from './providers.js';
import { validRadarMetadata } from './radar.js';
import { validQualityMetadata } from './quality.js';
import { validReflectanceMetadata } from './reflectance.js';
import { validVegetationMetadata } from './vegetation.js';
import { validScienceMetadata } from './modis-science.js';
import { validElevationMetadata } from './elevation.js';
import { AERIAL_VIEWS, aerialView, validAerialMetadata } from './aerial.js';
import { validateCompositeInspection } from './local-rgb.js';

const SERVICE = 'http://127.0.0.1:4318';

export function desktopAvailable() {
  return Boolean(globalThis.window?.__TAURI__?.core?.invoke);
}

export async function syncDesktopLocale(locale) {
  if (!['en', 'zh-CN'].includes(locale)) throw new Error('Unsupported desktop language.');
  if (!desktopAvailable()) return;
  await window.__TAURI__.core.invoke('set_desktop_locale', { locale });
}

export function validateFileThumbnail(data, id) {
  if (!data || data.jobId !== id || !/^[a-f0-9]{64}$/i.test(data.sha256 || '')
    || !Number.isSafeInteger(data.width) || data.width < 1 || data.width > 160
    || !Number.isSafeInteger(data.height) || data.height < 1 || data.height > 160
    || typeof data.dataUrl !== 'string' || data.dataUrl.length > 200000
    || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(data.dataUrl)) {
    throw new Error('The file preview returned invalid data.');
  }
  return data;
}

function waitForNative(promise, signal) {
  return new Promise((resolve, reject) => {
    const abort = () => reject(signal.reason || new DOMException('Request cancelled', 'AbortError'));
    if (signal.aborted) { abort(); return; }
    signal.addEventListener('abort', abort, { once: true });
    promise.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort));
  });
}

// WebView versions without AbortSignal.timeout/any still enforce the same
// deadline and dispose listeners and timers on every completion path.
function requestAbort(external, ms) {
  const controller = new AbortController();
  const abort = () => controller.abort(external.reason || new DOMException('Request cancelled', 'AbortError'));
  if (external?.aborted) abort();
  else external?.addEventListener('abort', abort, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException('Request timed out', 'TimeoutError')), ms);
  return { signal: controller.signal, dispose() { clearTimeout(timer); external?.removeEventListener('abort', abort); } };
}

export function validateAccountStatus(value) {
  if (!value || !['nasa-earthdata', 'copernicus'].includes(value.provider)
    || !['not-connected', 'saved', 'connected', 'expired', 'storage-error', 'unsupported'].includes(value.status)
    || Object.keys(value).some(key => !['provider', 'status', 'expiresAt', 'verifiedAt'].includes(key))
    || [value.expiresAt, value.verifiedAt].some(time => time !== null && (typeof time !== 'string' || !Number.isFinite(Date.parse(time))))
    || (value.status === 'connected' && (!value.expiresAt || !value.verifiedAt))) {
    throw new Error('The account service returned invalid status data.');
  }
  return value;
}
function accountResult(operation, value) {
  if (operation === 'accounts') {
    if (!Array.isArray(value) || value.length !== 2 || new Set(value.map(account => account.provider)).size !== 2) throw new Error('The account service returned invalid status data.');
    return value.map(validateAccountStatus);
  }
  return ['connectAccount', 'verifyAccount', 'disconnectAccount'].includes(operation) ? validateAccountStatus(value) : value;
}

export function validateRasterInspection(data) {
  const quality = data?.quality !== undefined;
  const radar = data?.radar !== undefined;
  const reflectance = data?.reflectance !== undefined;
  const vegetation = data?.vegetation !== undefined;
  const science = data?.science !== undefined;
  const elevation = data?.elevation !== undefined;
  const aerial = data?.aerial !== undefined;
  const positiveInteger = value => Number.isSafeInteger(value) && value > 0;
  const finiteArray = (value, length) => Array.isArray(value) && value.length === length && value.every(Number.isFinite);
  if (!data || !positiveInteger(data.width) || !positiveInteger(data.height)
    || !positiveInteger(data.bandCount) || typeof data.dataType !== 'string' || typeof data.crs !== 'string'
    || !finiteArray(data.bounds, 4) || !finiteArray(data.pixelSize, 2) || data.pixelSize.some(value => value <= 0)
    || !positiveInteger(data.previewWidth) || !positiveInteger(data.previewHeight)
    || data.previewWidth > 768 || data.previewHeight > 768
    || typeof data.previewDataUrl !== 'string' || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(data.previewDataUrl)
    || typeof data.sha256 !== 'string' || !/^[a-f0-9]{64}$/i.test(data.sha256)
    || ![1, 3, 4].includes(data.bandCount) || (data.bandCount === 4 && !aerial)
    || Number(quality) + Number(radar) + Number(reflectance) + Number(vegetation) + Number(science) + Number(elevation) + Number(aerial) > 1 || (science ? !validScienceMetadata(data) : quality ? !validQualityMetadata(data) : radar ? !validRadarMetadata(data) : aerial ? !validAerialMetadata(data) : elevation ? !validElevationMetadata(data) : reflectance ? !validReflectanceMetadata(data) : vegetation ? !validVegetationMetadata(data) : data.dataType !== 'UInt8')
    || !Array.isArray(data.classes) || !science && (data.bandCount === 1 && !reflectance && !vegetation && !elevation && !radar ? !data.classes.length : data.classes.length !== 0)
    || data.classes.some(item => !Number.isSafeInteger(item.value) || item.value < 0 || item.value > 255
      || typeof item.label !== 'string' || !/^#[a-f0-9]{6}$/i.test(item.color)
      || !Number.isSafeInteger(item.count) || item.count < 0)
    || new Set(data.classes.map(item => item.value)).size !== data.classes.length
    || (data.bandCount === 1 && !quality && !science && !reflectance && !vegetation && !elevation && !radar && data.classes.reduce((sum, item) => sum + item.count, 0) !== data.width * data.height)
    || (data.nodata !== null && !Number.isFinite(data.nodata))) {
    throw new Error('The raster service returned incomplete or invalid inspection data.');
  }
  return data;
}

function rasterResponse(data, payload) {
  validateRasterInspection(data);
  if ((data.aerial || payload?.aerialView !== undefined) && aerialView(data) !== (payload?.aerialView || 'rgb')) {
    throw new Error('The aerial display does not match the requested channels or source grid.');
  }
  return data;
}

export async function runtimeRequest(operation, payload, signal) {
  if (operation === 'raster' && payload?.aerialView !== undefined && !AERIAL_VIEWS.includes(payload.aerialView)) throw new Error('Unsupported NAIP display selection.');
  const commands = { resolveProducts: 'resolve_copernicus_products', accounts: 'list_provider_accounts', connectAccount: 'connect_provider_account', verifyAccount: 'verify_provider_account', disconnectAccount: 'disconnect_provider_account', health: 'health', diagnostics: 'diagnostics', proxy: 'get_proxy_settings', saveProxy: 'save_proxy_settings', testProxy: 'test_proxy_settings', list: 'list_jobs', create: 'create_job', cancel: 'cancel_job', retry: 'retry_job', reveal: 'reveal_job', planRgb: 'plan_scientific_rgb', runRgb: 'run_scientific_rgb', rgb: 'inspect_scientific_rgb', rgbPixel: 'sample_scientific_rgb', composite: 'inspect_composite', compositePixel: 'sample_composite', raster: 'inspect_raster', thumbnail: 'file_thumbnail', pixel: 'sample_raster', package: 'prepare_artifact', revealPackage: 'reveal_artifact', recipes: 'list_recipes', planRecipe: 'plan_recipe', saveRecipe: 'save_recipe', runRecipe: 'run_recipe', projects: 'list_projects', createProject: 'create_project', renameProject: 'rename_project', addProjectScenes: 'add_project_scenes', downloadProject: 'download_project', mosaicProject: 'mosaic_project', prepareProject: 'prepare_project' };
  if (!commands[operation]) throw new Error('Unknown task service operation.');
  const accountMutation = ['connectAccount', 'verifyAccount', 'disconnectAccount'].includes(operation);
  if (accountMutation && !desktopAvailable()) throw new Error('Open the desktop app to manage data source authorization.');
  const recipeOperation = ['planRecipe', 'saveRecipe', 'runRecipe'].includes(operation);
  const abort = requestAbort(signal, ['planRgb', 'runRgb', 'rgb', 'rgbPixel', 'composite', 'compositePixel', 'raster', 'thumbnail', 'pixel', 'package', 'revealPackage', 'downloadProject', 'mosaicProject', 'prepareProject', 'resolveProducts'].includes(operation) || recipeOperation || accountMutation ? 60000 : operation === 'testProxy' ? 30000 : 10000);
  const requestSignal = abort.signal;
  try {
  if (requestSignal.aborted) throw requestSignal.reason;
  if (desktopAvailable()) {
    try {
      const result = await waitForNative(window.__TAURI__.core.invoke(commands[operation], operation === 'addProjectScenes' ? { id: payload.id, request: { scenes: payload.scenes } } : ['planRgb', 'runRgb', 'composite', 'compositePixel', 'create', 'createProject', 'connectAccount', 'resolveProducts'].includes(operation) ? { request: payload } : ['saveProxy', 'testProxy'].includes(operation) ? { settings: payload } : recipeOperation ? { recipe: payload } : payload || {}), requestSignal);
      return operation === 'thumbnail' ? validateFileThumbnail(result, payload.id) : operation === 'raster' ? rasterResponse(result, payload) : ['composite','rgb'].includes(operation) ? validateCompositeInspection(result) : accountResult(operation, result);
    } catch (error) {
      if (error?.name === 'AbortError' || error?.name === 'TimeoutError') throw error;
      throw new Error(typeof error === 'string' ? error : error?.message || 'The desktop task command failed.');
    }
  }
  const routes = { planRgb: '/rasters/rgb/plan', runRgb: '/rasters/rgb', rgb: `/jobs/${encodeURIComponent(payload?.id)}/rgb`, rgbPixel: `/jobs/${encodeURIComponent(payload?.id)}/rgb/pixel?${new URLSearchParams({x: String(payload?.x), y: String(payload?.y)})}`, composite: '/rasters/composite', compositePixel: '/rasters/composite/pixel', resolveProducts: '/providers/copernicus/products', accounts: '/accounts', thumbnail: `/jobs/${encodeURIComponent(payload?.id)}/thumbnail`, health: '/health', diagnostics: '/diagnostics', proxy: '/proxy', saveProxy: '/proxy', testProxy: '/proxy/test', list: '/jobs', create: '/jobs', cancel: `/jobs/${encodeURIComponent(payload?.id)}/cancel`, retry: `/jobs/${encodeURIComponent(payload?.id)}/retry`, raster: `/jobs/${encodeURIComponent(payload?.id)}/raster`, pixel: `/jobs/${encodeURIComponent(payload?.id)}/pixel?${new URLSearchParams({x: String(payload?.x), y: String(payload?.y)})}`, package: `/jobs/${encodeURIComponent(payload?.id)}/package`, recipes: '/recipes', planRecipe: '/recipes/plan', saveRecipe: '/recipes', runRecipe: '/recipes/run', projects: '/projects', createProject: '/projects', renameProject: `/projects/${encodeURIComponent(payload?.id)}/rename`, addProjectScenes: `/projects/${encodeURIComponent(payload?.id)}/scenes`, downloadProject: `/projects/${encodeURIComponent(payload?.id)}/downloads`, mosaicProject: `/projects/${encodeURIComponent(payload?.id)}/mosaics`, prepareProject: `/projects/${encodeURIComponent(payload?.id)}/rasters` };
  if (!routes[operation]) throw new Error('Open the desktop app to reveal local files.');
  if (operation === 'raster' && payload?.aerialView !== undefined) routes.raster += '?' + new URLSearchParams({aerialView:payload.aerialView});
  const mutation = ['planRgb', 'runRgb', 'composite', 'compositePixel', 'create', 'cancel', 'retry', 'package', 'createProject', 'renameProject', 'addProjectScenes', 'downloadProject', 'mosaicProject', 'prepareProject', 'saveProxy', 'testProxy', 'resolveProducts'].includes(operation) || recipeOperation;
  const response = await fetch(SERVICE + routes[operation], {
    method: mutation ? 'POST' : 'GET',
    headers: mutation ? { 'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global' } : {},
    body: ['planRgb', 'runRgb', 'composite', 'compositePixel', 'create', 'createProject', 'saveProxy', 'testProxy', 'resolveProducts'].includes(operation) || recipeOperation ? JSON.stringify(payload) : operation === 'renameProject' ? JSON.stringify({ name: payload.name }) : operation === 'addProjectScenes' ? JSON.stringify({ scenes: payload.scenes }) : ['downloadProject', 'mosaicProject', 'prepareProject'].includes(operation) ? JSON.stringify({ assetKey: payload.assetKey, ...(operation === 'downloadProject' && payload.itemIds ? { itemIds: payload.itemIds } : {}), ...(operation === 'mosaicProject' && payload.viQuality ? { viQuality: payload.viQuality } : {}) }) : mutation ? '{}' : undefined,
    signal: requestSignal,
  });
  const body = await response.json();
  if (!response.ok) throw new Error(typeof body.error === 'string' ? body.error : body.message || `Task service returned ${response.status}`);
  return operation === 'thumbnail' ? validateFileThumbnail(body, payload.id) : operation === 'raster' ? rasterResponse(body, payload) : ['composite','rgb'].includes(operation) ? validateCompositeInspection(body) : accountResult(operation, body);
  } finally { abort.dispose(); }
}

export function downloadableAssets(scene) {
  return ['scl', 'visual', ...SOURCE_ASSET_KEYS.filter(key => !['scl', 'visual'].includes(key)), 'thumbnail'].flatMap(key => {
    const asset = scene?.assets?.[key];
    if (!asset?.href) return [];
    if (!isSupportedAsset(asset.href, key)) return [];
    return [{ key, ...asset }];
  });
}

export function formatBytes(value, locale = 'en') {
  if (!Number.isFinite(value) || value < 0) return 'Unknown size';
  if (value < 1024) return `${new Intl.NumberFormat(locale).format(value)} B`;
  const unit = value < 1048576 ? 'KiB' : value < 1073741824 ? 'MiB' : 'GiB';
  const scale = unit === 'KiB' ? 1024 : unit === 'MiB' ? 1048576 : 1073741824;
  return `${new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value / scale)} ${unit}`;
}

export function formatClassShare(count, total, locale = 'en') {
  if (!Number.isFinite(count) || !Number.isFinite(total) || total <= 0 || count < 0) return '—';
  const formatter = new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 2 });
  const share = count / total;
  return share > 0 && share < 0.0001 ? `<${formatter.format(0.0001)}` : formatter.format(share);
}
