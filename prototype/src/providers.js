import { MODIS_HOST, modisAssetIdentity, modisIdentity } from './modis.js';
import { vegetationAssetIdentity, vegetationIdentity } from './vegetation.js';
import { MODIS_SCIENCE, MODIS_SCIENCE_KEYS } from './modis-science-layers.js';
import { VIIRS_PRODUCTS, viirsIdentity, viirsAssetIdentity } from './viirs.js';
import { RADAR_HOST, radarAssetIdentity } from './radar.js';
import { retryPreviewRequest } from './preview-network.js';
// Source-specific capabilities; a public catalogue does not imply asset access.
export const PROVIDERS = Object.freeze([
  { id: 'planetary-vegetation', name: 'MODIS NDVI / EVI · Planetary Computer', collection: 'modis-13Q1-061', dataset: 'MODIS 16-day Vegetation Indices v6.1', domain: 'composite', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://planetarycomputer.microsoft.com/dataset/modis-13Q1-061', download: true, map: false },
  { id: 'planetary-radar', name: 'Sentinel-1 RTC · Planetary Computer', collection: 'sentinel-1-rtc', dataset: 'Sentinel-1 IW RTC', domain: 'radar', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://planetarycomputer.microsoft.com/dataset/sentinel-1-rtc', download: true, map: false },
  ...Object.entries(VIIRS_PRODUCTS).map(([product, info]) => ({ id: info.provider, name: `NASA VIIRS · ${info.platform}`, collection: `${product}_002`, dataset: `VIIRS ${product} v002`, domain: 'composite', search: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/search', catalog: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/', terms: `https://doi.org/10.5067/VIIRS/${product}.002`, download: true, map: false, account: 'nasa-earthdata' })),
  { id: 'planetary-modis', name: 'MODIS · Planetary Computer', collection: 'modis-09A1-061', dataset: 'MODIS 8-day Surface Reflectance v6.1', domain: 'composite', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://planetarycomputer.microsoft.com/dataset/modis-09A1-061', download: true, map: false },
  { id: 'nasa-srtm', name: 'NASA Earthdata · SRTM', collection: 'SRTMGL1_003', dataset: 'NASA SRTMGL1 v003', domain: 'elevation', search: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/search', catalog: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/', terms: 'https://doi.org/10.5067/MEASURES/SRTM/SRTMGL1.003', download: true, map: false, account: 'nasa-earthdata' },
  { id: 'planetary-naip', name: 'NAIP · Planetary Computer', collection: 'naip', dataset: 'USDA NAIP RGB + NIR', domain: 'aerial', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://registry.opendata.aws/naip/', download: true },
  { id: 'copernicus-dem', name: 'Copernicus DEM · GLO-30 Public', collection: 'cop-dem-glo-30', dataset: 'Copernicus DEM GLO-30 Public', domain: 'elevation', search: 'https://earth-search.aws.element84.com/v1/search', catalog: 'https://earth-search.aws.element84.com/v1/', terms: 'https://registry.opendata.aws/copernicus-dem/', download: true, map: false },
  { id: 'copernicus-dem-90', name: 'Copernicus DEM · GLO-90', collection: 'cop-dem-glo-90', dataset: 'Copernicus DEM GLO-90', domain: 'elevation', search: 'https://earth-search.aws.element84.com/v1/search', catalog: 'https://earth-search.aws.element84.com/v1/', terms: 'https://registry.opendata.aws/copernicus-dem/', download: true, map: false },
  { id: 'earth-search', name: 'Earth Search', search: 'https://earth-search.aws.element84.com/v1/search', catalog: 'https://earth-search.aws.element84.com/v1/', terms: 'https://registry.opendata.aws/sentinel-2-l2a-cogs/', download: true },
  { id: 'planetary-computer', name: 'Planetary Computer', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://planetarycomputer.microsoft.com/dataset/sentinel-2-l2a', download: true },
  { id: 'copernicus', name: 'Copernicus Data Space', search: 'https://stac.dataspace.copernicus.eu/v1/search', catalog: 'https://stac.dataspace.copernicus.eu/v1/', terms: 'https://dataspace.copernicus.eu/explore-data/data-collections/sentinel-data/sentinel-2', download: true, map: false, account: 'copernicus' },
  { id: 'planetary-landsat', name: 'Landsat · Planetary Computer', collection: 'landsat-c2-l2', dataset: 'Landsat Collection 2 L2', search: 'https://planetarycomputer.microsoft.com/api/stac/v1/search', catalog: 'https://planetarycomputer.microsoft.com/api/stac/v1/', terms: 'https://planetarycomputer.microsoft.com/dataset/landsat-c2-l2', download: true },
  { id: 'nasa-earthdata', name: 'NASA Earthdata · HLS', collection: 'HLSL30_2.0', dataset: 'NASA HLS Landsat L30 v2.0', search: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/search', catalog: 'https://cmr.earthdata.nasa.gov/stac/LPCLOUD/', terms: 'https://www.earthdata.nasa.gov/data/catalog/lpcloud-hlsl30-2.0', download: true, map: false, account: 'nasa-earthdata' },
]);
export const PC_HOST = 'sentinel2l2a01.blob.core.windows.net';
export const LANDSAT_HOST = 'landsateuwest.blob.core.windows.net';
export const NAIP_HOST = 'naipeuwest.blob.core.windows.net';
export const NASA_HOST = 'data.lpdaac.earthdatacloud.nasa.gov';
export const CDSE_HOST = 'download.dataspace.copernicus.eu';
export const DEM_HOST = 'copernicus-dem-30m.s3.eu-central-1.amazonaws.com';
export const DEM90_HOST = 'copernicus-dem-90m.s3.eu-central-1.amazonaws.com';
export const DEM_PRODUCTS = Object.freeze({
  '10': Object.freeze({ product:'cop-dem-glo-30-public', label:'GLO-30 Public', host:DEM_HOST, bucket:'copernicus-dem-30m', height:3600, widths:[3600,2400,1800,1200,720,360], provider:'copernicus-dem', resolution:30 }),
  '30': Object.freeze({ product:'cop-dem-glo-90', label:'GLO-90', host:DEM90_HOST, bucket:'copernicus-dem-90m', height:1200, widths:[1200,800,600,400,240,120], provider:'copernicus-dem-90', resolution:90 }),
});
export const demProduct = id => demCell(id) ? DEM_PRODUCTS[id.split('_')[3]] : null;
export const copDemLabel = items => [...new Set(items.map(item => demProduct(item.itemId || item.id)?.label).filter(Boolean))].join(' / ') || 'Copernicus DEM';
export const SOURCE_ASSET_KEYS = ['visual', 'scl', 'red', 'green', 'blue', 'ndvi', 'evi', ...MODIS_SCIENCE_KEYS, 'modis_qc', 'modis_state', 'qa_pixel', 'qa_radsat', 'product', 'elevation', 'aerial', 'srtm', 'viirs', 'vv', 'vh', 'hh', 'hv'];
export const LANDSAT_BANDS = ['red', 'green', 'blue'];
export function assetLabel(key) {
  if (MODIS_SCIENCE[key]) return MODIS_SCIENCE[key].label;
  if (key === 'ndvi') return 'NDVI vegetation index';
  if (key === 'evi') return 'EVI vegetation index';
  if (key === 'qa_pixel') return 'Landsat pixel quality';
  if (key === 'qa_radsat') return 'Landsat saturation quality';
  if (key === 'modis_qc') return 'MODIS band quality';
  if (key === 'modis_state') return 'MODIS pixel state';
  if (['vv','vh','hh','hv'].includes(key)) return `Radar backscatter · ${key.toUpperCase()}`;
  return ({ reflectance_rgb: 'Scientific RGB', visual: 'True-color imagery', scl: 'SCL classification', red: 'Red reflectance band', green: 'Green reflectance band', blue: 'Blue reflectance band', product: 'Sentinel-2 SAFE product', elevation: 'Surface elevation', aerial: 'Aerial imagery · RGB + NIR', srtm: 'SRTM elevation', viirs: 'VIIRS original product' })[key] || 'Preview image';
}
export function srtmCell(id) {
  const m = /^([NS])(\d{2})([EW])(\d{3})\.SRTMGL1\.hgt$/.exec(id);
  if (!m || m[1] === 'S' && Number(m[2]) === 0 || m[3] === 'W' && Number(m[4]) === 0) return null;
  const lat = Number(m[2]) * (m[1] === 'S' ? -1 : 1), lon = Number(m[4]) * (m[3] === 'W' ? -1 : 1);
  return lat >= -56 && lat < 60 && lon >= -180 && lon < 180 ? [lon, lat] : null;
}
export const srtmHref = id => `https://${NASA_HOST}/lp-prod-protected/SRTMGL1.003/${id}/${id}.zip`;
function validNaipDate(value) {
  if (!/^\d{8}$/.test(value)) return false;
  const date = `${value.slice(0,4)}-${value.slice(4,6)}-${value.slice(6,8)}`;
  return Number.isFinite(Date.parse(date)) && new Date(date).toISOString().slice(0,10).replaceAll('-', '') === value;
}
function naipFilename(stem) {
  const m = /^m_(\d{7})_(nw|ne|sw|se)_(\d{2})_(030|060|100|1|h|\.6)_(\d{8})(?:_(\d{8}))?$/.exec(stem);
  if (!m || Number(m[3]) < 1 || Number(m[3]) > 23 || !validNaipDate(m[5]) || m[6] && !validNaipDate(m[6])) return null;
  return { grid:m[1], cm:({1:'100',h:'060','.6':'060'})[m[4]] || m[4], date:m[5] };
}
export function naipIdentity(path) {
  if (typeof path !== 'string') return null;
  const p = path.split('/');
  if (p.length !== 8 || p[0] !== '' || p[1] !== 'naip' || p[2] !== 'v002' || !/^[a-z]{2}$/.test(p[3]) || !p[7].endsWith('.tif')) return null;
  const stem = p[7].slice(0,-4), info = naipFilename(stem);
  return info && p[4] === info.date.slice(0,4) && p[5] === `${p[3]}_${info.cm}cm_${p[4]}` && p[6] === info.grid.slice(0,5) ? `${p[3]}_${stem}` : null;
}
export function naipMatchesItem(path, itemId) {
  const identity = naipIdentity(path);
  // Some catalogue IDs retain a second date absent from the original filename.
  // Preserve the full ID; the native downloader checks its official item too.
  // Reviewed 2016 files use `h` for half-metre-class imagery, while their
  // official catalogue IDs use `.6`. The directory and TIFF still specify 0.6 m.
  const catalogueIdentity = identity?.split('_').map((part,index) => index === 5 && part === 'h' ? '.6' : part).join('_');
  return Boolean(identity && [identity,catalogueIdentity].some(value => value === itemId || value.split('_').length === 7
    && typeof itemId === 'string' && itemId.startsWith(`${value}_`) && validNaipDate(itemId.slice(value.length + 1))));
}
export function naipLegacyNir(path, itemId) {
  if (!naipMatchesItem(path,itemId)) return false;
  const fileResolution = naipIdentity(path).split('_')[5], itemResolution = itemId.split('_')[5];
  return fileResolution === '1' && itemResolution === '1' || fileResolution === 'h' && itemResolution === '.6';
}
export function naipPixelSize(itemId) {
  const m = /^([a-z]{2})_(m_.+)$/.exec(itemId), info = m && naipFilename(m[2]);
  return info ? Number(info.cm) / 100 : null;
}
export function demCell(id) {
  const match = /^Copernicus_DSM_COG_(?:10|30)_([NS])(\d{2})_00_([EW])(\d{3})_00_DEM$/.exec(id);
  if (!match || (match[1] === 'S' && match[2] === '00') || (match[3] === 'W' && match[4] === '000')) return null;
  const lat = Number(match[2]) * (match[1] === 'S' ? -1 : 1), lon = Number(match[4]) * (match[3] === 'W' ? -1 : 1);
  return lat >= -90 && lat < 90 && lon >= -180 && lon < 180 ? [lon, lat] : null;
}
export function demTileLabel(id) {
  if (srtmCell(id)) return id.slice(0, 7);
  const match = /^Copernicus_DSM_COG_(?:10|30)_([NS]\d{2})_00_([EW]\d{3})_00_DEM$/.exec(id);
  return demCell(id) ? `${match[1]}${match[2]}` : id;
}
export function providerById(id = 'earth-search') {
  const provider = PROVIDERS.find(entry => entry.id === id);
  if (!provider) throw new Error('Unsupported data source.');
  return provider;
}
export function canDisplayImagery(provider) {
  return provider.download && provider.map !== false;
}
export function scenePlatformLabel(scene) {
  if (scene.provider === 'planetary-radar') return `Sentinel-1${scene.id.slice(2,3)}`;
  if (viirsIdentity(scene.id)) return `VIIRS ${viirsIdentity(scene.id).platform}`;
  if (scene.provider === 'planetary-modis') return `MODIS ${modisIdentity(scene.id)?.platform || ''}`;
  if (scene.provider === 'planetary-vegetation') return `MODIS ${vegetationIdentity(scene.id)?.platform || ''}`;
  if (scene.provider === 'nasa-srtm') return 'NASA SRTM';
  if (scene.provider === 'planetary-naip') return 'USDA NAIP';
  if (['copernicus-dem','copernicus-dem-90'].includes(scene.provider)) return 'Copernicus DEM';
  if (scene.provider === 'nasa-earthdata') return 'HLS Landsat L30';
  if (scene.provider === 'planetary-landsat') return scene.id.startsWith('LC09') ? 'Landsat 9' : 'Landsat 8';
  return scene.id.startsWith('S2C') ? 'Sentinel-2C' : scene.id.startsWith('S2B') ? 'Sentinel-2B' : 'Sentinel-2A';
}
export function providerForSearch(href) {
  const url = new URL(href);
  const collections = [...url.searchParams.getAll('collections'), ...url.searchParams.getAll('collections[0]')];
  if (collections.length > 1 || [...url.searchParams.keys()].some(key => /^collections\[/.test(key) && key !== 'collections[0]')) throw new Error('The catalog returned an unsupported pagination link.');
  const provider = PROVIDERS.find(entry => {
    const target = new URL(entry.search);
    return url.origin === target.origin && url.pathname === target.pathname && (collections[0] || 'sentinel-2-l2a') === (entry.collection || 'sentinel-2-l2a');
  });
  if (!provider || url.username || url.password || url.hash) throw new Error('The catalog returned an unsupported pagination link.');
  return provider;
}
export function isSupportedAsset(href, assetKey) {
  try {
    const url = new URL(href);
    if (url.hostname === RADAR_HOST) return Boolean(radarAssetIdentity(href, assetKey));
    if (viirsAssetIdentity(href)) return !assetKey || assetKey === 'viirs';
    if (url.hostname === MODIS_HOST) return Boolean(modisAssetIdentity(href, assetKey) || vegetationAssetIdentity(href,assetKey));
    if (assetKey) {
      if (url.hostname === NAIP_HOST) {
        if (assetKey !== 'aerial') return false;
      } else if ([DEM_HOST,DEM90_HOST].includes(url.hostname)) {
        if (assetKey !== 'elevation') return false;
      } else if (url.hostname === CDSE_HOST) {
        if (assetKey !== 'product') return false;
      } else if (url.hostname === NASA_HOST) {
        if (assetKey === 'srtm') return srtmCell(url.pathname.split('/')[3]) && href === srtmHref(url.pathname.split('/')[3]) ? true : false;
        const band = { red: 'B04', green: 'B03', blue: 'B02' }[assetKey];
        if (!band || !url.pathname.endsWith(`.${band}.tif`)) return false;
      } else if (url.hostname === LANDSAT_HOST) {
        const suffix = { red: '_SR_B4.TIF', green: '_SR_B3.TIF', blue: '_SR_B2.TIF', qa_pixel:'_QA_PIXEL.TIF', qa_radsat:'_QA_RADSAT.TIF' }[assetKey];
        if (!suffix || !url.pathname.endsWith(suffix)) return false;
      } else if (url.hostname === PC_HOST) {
        if (!['scl', 'visual'].includes(assetKey) || !url.pathname.endsWith(assetKey === 'scl' ? '_SCL_20m.tif' : '_TCI_10m.tif')) return false;
      } else if (!['scl', 'visual', 'thumbnail'].includes(assetKey)) return false;
    }
    return url.protocol === 'https:' && (!url.port || url.port === '443') && !url.username && !url.password && !url.search && !url.hash && !url.pathname.includes('%')
      && ((url.hostname === NAIP_HOST && Boolean(naipIdentity(url.pathname)))
        || ([DEM_HOST,DEM90_HOST].includes(url.hostname) && (() => { const parts = url.pathname.split('/'); return parts.length === 3 && demProduct(parts[1])?.host === url.hostname && parts[2] === `${parts[1]}.tif`; })())
        || (url.hostname === 'sentinel-cogs.s3.us-west-2.amazonaws.com' && url.pathname.startsWith('/sentinel-s2-l2a-cogs/'))
        || (url.hostname === PC_HOST && url.pathname.startsWith('/sentinel2-l2/') && /_(TCI_10m|SCL_20m)\.tif$/.test(url.pathname))
        || (url.hostname === LANDSAT_HOST && url.pathname.startsWith('/landsat-c2/level-2/standard/oli-tirs/') && /_(SR_B[234]|QA_PIXEL|QA_RADSAT)\.TIF$/.test(url.pathname))
        || (url.hostname === NASA_HOST && /^\/lp-prod-protected\/HLSL30\.020\/(HLS\.L30\.T\d{2}[A-Z]{3}\.\d{7}T\d{6}\.v2\.0)\/\1\.B0[234]\.tif$/.test(url.pathname))
        || (url.hostname === NASA_HOST && srtmCell(url.pathname.split('/')[3]) && href === srtmHref(url.pathname.split('/')[3]))
        || (url.hostname === CDSE_HOST && /^\/odata\/v1\/Products\([a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}\)\/\$value$/.test(url.pathname)));
  } catch { return false; }
}
export function providerForAssets(assets) {
  if (['ndvi','evi'].some(key => vegetationAssetIdentity(assets?.[key]?.href,key))) return 'planetary-vegetation';
  if (Object.values(assets || {}).some(asset => radarAssetIdentity(asset?.href))) return 'planetary-radar';
  const viirs = viirsAssetIdentity(assets?.viirs?.href);
  if (viirs) return viirs.provider;
  if (Object.values(assets || {}).some(asset => isSupportedAsset(asset?.href, 'srtm'))) return 'nasa-srtm';
  const hosts = Object.values(assets || {}).flatMap(asset => { try { return [new URL(asset.href).hostname]; } catch { return []; } });
  return hosts.includes(MODIS_HOST) ? 'planetary-modis' : hosts.includes(NAIP_HOST) ? 'planetary-naip' : hosts.includes(DEM90_HOST) ? 'copernicus-dem-90' : hosts.includes(DEM_HOST) ? 'copernicus-dem' : hosts.includes(CDSE_HOST) ? 'copernicus' : hosts.includes(NASA_HOST) ? 'nasa-earthdata' : hosts.includes(LANDSAT_HOST) ? 'planetary-landsat' : hosts.includes(PC_HOST) ? 'planetary-computer' : 'earth-search';
}

// SAS tokens stay in memory and never enter scene assets, projects or job JSON.
let access, pendingAccess;
let landsatAccess, pendingLandsatAccess;
let naipAccess, pendingNaipAccess;
async function requestPublicAccess(url,fetcher) {
  const signal=AbortSignal.timeout(30_000);
  try {
    return await retryPreviewRequest(async()=>{
      const response=await fetcher(url,{credentials:'omit',signal});
      if(response.status>=500)throw new Error('Planetary Computer data access is unavailable. Retry loading imagery.');
      return response;
    },signal);
  } catch(error) {
    if(['AbortError','TimeoutError'].includes(error.name))throw error;
    throw new Error('Planetary Computer data access is unavailable. Retry loading imagery.');
  }
}
async function prepareNaipAccess(hrefs, { fetcher, force, now }) {
  if (!hrefs.every(href => isSupportedAsset(href, 'aerial'))) throw new Error('Unsupported NAIP data access.');
  if (!force && naipAccess?.expires > now + 60_000) return;
  if (!pendingNaipAccess) pendingNaipAccess = (async () => {
    const response = await requestPublicAccess('https://planetarycomputer.microsoft.com/api/sas/v1/token/naipeuwest/naip',fetcher);
    if (!response.ok) throw new Error('Planetary Computer data access is unavailable. Retry loading imagery.');
    const data = await response.json(), expires = Date.parse(data?.['msft:expiry']), token = new URLSearchParams(data?.token);
    const end = Date.parse(token.get('se'));
    if (!Number.isFinite(expires) || !Number.isFinite(end) || Math.min(expires, end) <= now + 60_000
      || typeof data?.token !== 'string' || data.token.length > 8192
      || ['sig', 'sp', 'sr', 'se'].some(key => token.getAll(key).length !== 1)
      || !token.get('sig') || !['r', 'rl'].includes(token.get('sp')) || token.get('sr') !== 'c') throw new Error('Planetary Computer returned invalid data access.');
    naipAccess = { token: token.toString(), expires: Math.min(expires, end) };
  })().finally(() => { pendingNaipAccess = undefined; });
  await pendingNaipAccess;
}
async function prepareLandsatAccess(hrefs, { fetcher, force, now }) {
  if (!hrefs.every(href => isSupportedAsset(href))) throw new Error('Unsupported Landsat data access.');
  if (!force && landsatAccess?.expires > now + 60_000) return;
  // Like the official SDK, share one container token across all selected
  // scenes/bands. Per-file signing would make 48 requests for 16 RGB scenes.
  if (!pendingLandsatAccess) pendingLandsatAccess = (async () => {
    const response = await requestPublicAccess('https://planetarycomputer.microsoft.com/api/sas/v1/token/landsateuwest/landsat-c2',fetcher);
    if (!response.ok) throw new Error(response.status === 429 ? 'Planetary Computer is rate limiting data access. Wait a moment before retrying.' : 'Planetary Computer data access is unavailable. Retry loading imagery.');
    const data = await response.json(), expires = Date.parse(data?.['msft:expiry']), token = new URLSearchParams(data?.token);
    if (!Number.isFinite(expires) || expires <= now + 60_000 || typeof data?.token !== 'string' || data.token.length > 8192
      || ['sig', 'sp', 'sr'].some(key => token.getAll(key).length !== 1)
      || !token.get('sig') || !['r', 'rl'].includes(token.get('sp')) || token.get('sr') !== 'c') throw new Error('Planetary Computer returned invalid data access.');
    landsatAccess = { token: data.token, expires };
  })().finally(() => { pendingLandsatAccess = undefined; });
  await pendingLandsatAccess;
}
export async function prepareAssetAccess(hrefs, { fetcher = fetch, signal, force = false, now = Date.now() } = {}) {
  if (signal?.aborted) throw signal.reason || new DOMException('Request cancelled', 'AbortError');
  const naipHrefs = [...new Set(hrefs)].filter(href => { try { return new URL(href).hostname === NAIP_HOST; } catch { return false; } });
  if (naipHrefs.length) await prepareNaipAccess(naipHrefs, { fetcher, force, now });
  const landsatHrefs = [...new Set(hrefs)].filter(href => { try { return new URL(href).hostname === LANDSAT_HOST; } catch { return false; } });
  if (landsatHrefs.length) await prepareLandsatAccess(landsatHrefs, { fetcher, force, now });
  if (signal?.aborted) throw signal.reason || new DOMException('Request cancelled', 'AbortError');
  if (!hrefs.some(href => { try { return new URL(href).hostname === PC_HOST; } catch { return false; } })) return;
  if (!force && access?.expires > now + 60_000) return;
  if (!pendingAccess) pendingAccess = (async () => {
    const response = await requestPublicAccess('https://planetarycomputer.microsoft.com/api/sas/v1/token/sentinel2l2a01/sentinel2-l2',fetcher);
    if (!response.ok) throw new Error(response.status === 429 ? 'Planetary Computer is rate limiting data access. Wait a moment before retrying.' : 'Planetary Computer data access is unavailable. Retry loading imagery.');
    const data = await response.json();
    const expires = Date.parse(data['msft:expiry']);
    const token = new URLSearchParams(data.token);
    if (!Number.isFinite(expires) || expires <= now + 60_000 || typeof data.token !== 'string' || data.token.length > 8192 || !token.get('sig') || !['r', 'rl'].includes(token.get('sp'))) throw new Error('Planetary Computer returned invalid data access.');
    access = { token: data.token, expires };
  })().finally(() => { pendingAccess = undefined; });
  await pendingAccess;
  if (signal?.aborted) throw signal.reason || new DOMException('Request cancelled', 'AbortError');
}
export function assetReadURL(href, now = Date.now()) {
  const url = new URL(href);
  if (url.hostname === NAIP_HOST) {
    if (!isSupportedAsset(href, 'aerial') || !naipAccess || naipAccess.expires <= now + 15_000) throw new Error('Data access expired. Retry loading imagery.');
    url.search = naipAccess.token;
    return url.href;
  }
  if (url.hostname === LANDSAT_HOST) {
    if (!isSupportedAsset(href) || !landsatAccess || landsatAccess.expires <= now + 15_000) throw new Error('Data access expired. Retry loading imagery.');
    url.search = landsatAccess.token;
    return url.href;
  }
  if (url.hostname !== PC_HOST) return href;
  if (!isSupportedAsset(href) || !access || access.expires <= now + 15_000) throw new Error('Data access expired. Retry loading imagery.');
  url.search = access.token;
  return url.href;
}
