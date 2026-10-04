import { providerById, providerForSearch, demProduct, srtmCell, srtmHref, isSupportedAsset, naipMatchesItem } from './providers.js';
import { MODIS_CRS, modisIdentity, modisAssetIdentity } from './modis.js';
import { vegetationIdentity, vegetationAssetIdentity, VEGETATION_KEYS } from './vegetation.js';
import { MODIS_SCIENCE } from './modis-science-layers.js';
import { viirsIdentity, viirsAssetIdentity } from './viirs.js';
import { imageryHrefs } from './explore-imagery.js';
import { RADAR_KEYS, radarAssetIdentity } from './radar.js';
import { LANDSAT_QUALITY_KEYS, landsatQualityIdentity } from './landsat-quality.js';

export const EARTH_SEARCH = "https://earth-search.aws.element84.com/v1/search";
export const SAMPLE_BBOX = [-122.55, 37.68, -122.32, 37.84];
export const INITIAL_SEARCH = { bbox: SAMPLE_BBOX.join(", "), start: "2025-06-01", end: "2025-06-30", cloudMin: 0, cloud: 60, limit: 20 };

export function defaultLiveSearch(now = new Date()) {
  const end = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
  const start = new Date(end);
  start.setUTCDate(start.getUTCDate() - 29);
  return { ...INITIAL_SEARCH, start: start.toISOString().slice(0, 10), end: end.toISOString().slice(0, 10) };
}

export function validateBounds(input) {
  const parts = Array.isArray(input) ? input : String(input).split(",").map((v) => v.trim());
  if (parts.length !== 4 || parts.some((v) => v === "" || !Number.isFinite(Number(v)))) throw new Error("Enter four coordinates: west, south, east, north.");
  const bbox = parts.map(Number), [west, south, east, north] = bbox;
  if (west < -180 || east > 180 || south < -90 || north > 90 || west >= east || south >= north) throw new Error("Use WGS 84 bounds with west < east and south < north. Split areas crossing the antimeridian into two searches.");
  return bbox;
}

export function validateSearch(input) {
  const bbox = validateBounds(input.bbox);
  const provider = providerById(input.provider), limit = Number(input.limit);
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error("Page size must be between 1 and 100 scenes.");
  if (provider.domain === 'elevation') return { bbox, limit, provider: provider.id };
  for (const key of ["start", "end"]) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(input[key]) || !Number.isFinite(Date.parse(input[key])) || new Date(input[key]).toISOString().slice(0, 10) !== input[key]) throw new Error("Choose valid start and end dates.");
  }
  if (input.start > input.end) throw new Error("The start date must be on or before the end date.");
  if (provider.domain === 'radar') {
    const orbit = input.orbit || 'all', polarization = input.polarization || 'all';
    if (!['all','ascending','descending'].includes(orbit) || !['all',...RADAR_KEYS].includes(polarization)) throw new Error('Choose a supported orbit and polarization.');
    return { bbox, start: input.start, end: input.end, limit, provider: provider.id, orbit, polarization };
  }
  if (['aerial', 'composite'].includes(provider.domain)) return { bbox, start: input.start, end: input.end, limit, provider: provider.id };
  const cloudMin = Number(input.cloudMin ?? 0), cloud = Number(input.cloud);
  if (!Number.isFinite(cloudMin) || !Number.isFinite(cloud) || cloudMin < 0 || cloud > 100 || cloudMin > cloud) throw new Error("Choose a cloud cover range from 0 to 100 percent, with minimum no greater than maximum.");
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error("Page size must be between 1 and 100 scenes.");
  if (input.provider) providerById(input.provider);
  return { bbox, start: input.start, end: input.end, cloudMin, cloud, limit, ...(input.provider ? { provider: input.provider } : {}) };
}

export function searchURL(input) {
  const q = validateSearch(input), provider = providerById(q.provider), url = new URL(provider.search);
  if (provider.domain === 'elevation') {
    url.search = new URLSearchParams({ collections: provider.collection, bbox: q.bbox.join(','), limit: String(q.limit) });
    return url.href;
  }
  url.search = new URLSearchParams({ collections: provider.collection || 'sentinel-2-l2a', bbox: q.bbox.join(","), datetime: `${q.start}T00:00:00Z/${q.end}T23:59:59.999Z`, query: JSON.stringify({ "eo:cloud_cover": { gte: q.cloudMin, lte: q.cloud } }), sortby: "-properties.datetime", limit: String(q.limit) });
  if (['aerial', 'composite', 'radar'].includes(provider.domain)) url.searchParams.delete('query');
  if (provider.domain === 'radar') {
    const query = { 'sar:instrument_mode': { eq: 'IW' } };
    if (q.orbit !== 'all') query['sat:orbit_state'] = { eq: q.orbit };
    // PC rejects array-contains queries. Keep the chosen polarization on our
    // cursor and filter every fetched page without stopping at an empty page.
    if (q.polarization !== 'all') url.searchParams.set('geod-polarization', q.polarization);
    url.searchParams.set('query', JSON.stringify(query));
  }
  if (['planetary-modis','planetary-vegetation'].includes(provider.id)) url.searchParams.set('sortby', '-properties.start_datetime');
  if (q.provider === 'planetary-landsat') {
    url.searchParams.set('query', JSON.stringify({ 'eo:cloud_cover': { gte: q.cloudMin, lte: q.cloud }, platform: { in: ['landsat-8', 'landsat-9'] } }));
  }
  if (q.provider === 'copernicus') {
    url.searchParams.delete('query');
    url.searchParams.set('filter-lang', 'cql2-json');
    url.searchParams.set('filter', JSON.stringify({ op: 'and', args: [{ op: '>=', args: [{ property: 'eo:cloud_cover' }, q.cloudMin] }, { op: '<=', args: [{ property: 'eo:cloud_cover' }, q.cloud] }] }));
  }
  return url.href;
}

function httpsURL(value) {
  try { const u = new URL(value); return u.protocol === "https:" && !u.username && !u.password ? u.href : null; } catch { return null; }
}

export function normalizeScene(item, providerId = 'earth-search') {
  const provider = providerById(providerId);
  const properties = item?.properties || {};
  const viirs = providerId.startsWith('nasa-viirs-') ? viirsIdentity(item?.id) : null;
  const vegetation = providerId === 'planetary-vegetation' ? vegetationIdentity(item?.id) : null;
  const period = providerId === 'planetary-vegetation' ? vegetation : providerId === 'planetary-modis' ? modisIdentity(item?.id) : viirs;
  const acquisition = period?.date || properties.datetime;
  if (typeof item?.id !== "string" || !item.id || !Number.isFinite(Date.parse(acquisition))) throw new Error("The catalog returned an item without a valid ID or acquisition date.");
  const assets = Object.fromEntries(Object.entries(item.assets || {}).filter(([, asset]) => asset && httpsURL(asset.href)).map(([key, asset]) => [key, { ...asset, href: httpsURL(asset.href) }]));
  if (providerId === 'planetary-radar') {
    if (item.collection !== provider.collection || properties['sar:instrument_mode'] !== 'IW' || !['ascending','descending','unknown'].includes(properties['sat:orbit_state'])) throw new Error('The catalog returned an unsupported Sentinel-1 RTC mode or orbit.');
    const keys = RADAR_KEYS.filter(key => assets[key]);
    if (!keys.length) throw new Error('The radar scene has no supported polarization.');
    for (const key of keys) {
      const identity = radarAssetIdentity(assets[key].href, key), band = assets[key]['raster:bands']?.[0];
      if (!identity?.ids.includes(item.id) || !properties['sar:polarizations']?.includes(key.toUpperCase()) || band?.data_type !== 'float32' || band.nodata !== -32768 || band.spatial_resolution !== 10) throw new Error('The radar COG does not match its polarization and RTC product.');
      assets[key] = { ...assets[key], type: 'image/tiff; application=geotiff' };
    }
    if (assets.rendered_preview) assets.thumbnail = assets.rendered_preview;
  }
  if (providerId.startsWith('nasa-viirs-')) {
    if (!viirs || viirs.provider !== providerId || item.collection !== viirs.collection
      || Date.parse(properties.start_datetime) !== Date.parse(viirs.date) || Date.parse(properties.end_datetime) !== Date.parse(viirs.endDate)) throw new Error('The catalog returned an unsupported VIIRS product or composite period.');
    const original = assets[viirs.production] || assets.viirs;
    if (viirsAssetIdentity(original?.href)?.id !== item.id) throw new Error('The VIIRS HDF5 does not match its product and composite period.');
    assets.viirs = { ...original, type: 'application/x-hdf5' };
    if (assets.browse?.href === viirs.browse) assets.thumbnail = assets.browse;
    else if (!assets.browse) assets.thumbnail = { href: viirs.browse, type: 'image/jpeg' };
    else throw new Error('The VIIRS browse image does not match its original product.');
  }
  if (providerId === 'planetary-modis') {
    if (!period || item.collection !== provider.collection || Date.parse(properties.start_datetime) !== Date.parse(period.date)
      || Date.parse(properties.end_datetime) !== Date.parse(period.endDate) || properties.platform !== period.platform.toLowerCase()
      || properties['modis:horizontal-tile'] !== period.h || properties['modis:vertical-tile'] !== period.v) throw new Error('The catalog returned an unsupported MODIS period or tile.');
    for (const [key, band] of [['red','01'],['green','04'],['blue','03']]) {
      const original = assets[`sur_refl_b${band}`] || assets[key];
      if (modisAssetIdentity(original?.href, key)?.id !== item.id) throw new Error('The MODIS COG does not match its composite period and band.');
      const info = original['raster:bands']?.[0];
      if (info && (info.data_type !== 'int16' || info.scale !== 0.0001 || info.spatial_resolution !== 500)) throw new Error('The MODIS COG calibration is unsupported.');
      assets[key] = { ...original, type: 'image/tiff; application=geotiff', rasterBand: { dataType: 'int16', scale: 0.0001, offset: 0, nodata: -28672, spatialResolution: 500 } };
    }
    for (const [key, name, dtype, nodata] of [['modis_qc','sur_refl_qc_500m','uint32',4294967295], ['modis_state','sur_refl_state_500m','uint16',65535]]) {
      const original = assets[name] || assets[key];
      if (!original) continue;
      const info = original['raster:bands']?.[0];
      if (modisAssetIdentity(original.href, key)?.id !== item.id || !info || info.data_type !== dtype
        || info.spatial_resolution !== 500 || info.scale !== undefined || info.offset !== undefined || original['eo:bands'] !== undefined)
        throw new Error('The MODIS quality layer does not match its product and unsigned bit-field type.');
      assets[key] = { ...original, type:'image/tiff; application=geotiff', rasterBand:{ dataType:dtype, nodata, spatialResolution:500 } };
    }
    if (assets.rendered_preview) assets.thumbnail = assets.rendered_preview;
  }
  if (providerId === 'planetary-vegetation') {
    if (!vegetation || item.collection !== provider.collection || Date.parse(properties.start_datetime) !== Date.parse(vegetation.date)
      || Date.parse(properties.end_datetime) !== Date.parse(vegetation.endDate)
      || properties.platform && properties.platform !== vegetation.platform.toLowerCase()
      || properties['modis:horizontal-tile'] !== vegetation.h || properties['modis:vertical-tile'] !== vegetation.v) throw new Error('The vegetation product, composite period or tile is unsupported.');
    for (const key of VEGETATION_KEYS) {
      const original = assets[`250m_16_days_${key.toUpperCase()}`] || assets[key], bands = original?.['raster:bands'], info = bands?.[0];
      if (vegetationAssetIdentity(original?.href,key)?.id !== item.id || bands?.length !== 1 || info.data_type !== 'int16'
        || info.scale !== 0.0001 || info.spatial_resolution !== 250 || info.unit !== key.toUpperCase()
        || info.offset !== undefined && info.offset !== 0 || info.nodata !== undefined && info.nodata !== -3000) throw new Error('The vegetation index COG does not match its product and calibration.');
      assets[key] = {...original,type:'image/tiff; application=geotiff',rasterBand:{dataType:'int16',scale:0.0001,offset:0,nodata:-3000,spatialResolution:250}};
    }
    for (const [key, layer] of Object.entries(MODIS_SCIENCE)) {
      const original = assets[layer.asset] || assets[key];
      if (!original) continue;
      const bands = original['raster:bands'], info = bands?.[0];
      if (vegetationAssetIdentity(original.href,key)?.id !== item.id || bands?.length !== 1 || info.data_type !== layer.dataType
        || (info.scale ?? 1) !== layer.scale || (info.offset ?? 0) !== 0 || (info.nodata ?? layer.nodata) !== layer.nodata
        || info.spatial_resolution !== 250 || (key === 'vi_doy' ? !['JulianDay','Julian Day'].includes(info.unit) : info.unit !== layer.catalogUnit)) throw new Error('The MODIS science layer does not match its original type, unit or calibration.');
      assets[key] = {...original,type:'image/tiff; application=geotiff',rasterBand:{dataType:layer.dataType,scale:layer.scale,offset:0,nodata:layer.nodata,spatialResolution:250}};
    }
    if (assets.rendered_preview) assets.thumbnail = assets.rendered_preview;
  }
  if (['copernicus-dem','copernicus-dem-90'].includes(provider.id)) {
    const product = demProduct(item.id), original = item.assets?.data;
    const expected = `s3://${product?.bucket}/${item.id}/${item.id}.tif`;
    const href = `https://${product?.host}/${item.id}/${item.id}.tif`;
    if (!product || product.provider !== provider.id || item.collection !== provider.collection || original?.href !== expected || !isSupportedAsset(href, 'elevation')) throw new Error('The catalog returned an unsupported elevation tile.');
    assets.elevation = { ...original, href, type: 'image/tiff; application=geotiff' };
    // Nominal product class is independent from the catalog's raster:bands
    // spatial_resolution (currently mislabeled as 30 for some GLO-90 items).
    // Actual angular grid and original values are verified from the local TIFF.
  }
  if (providerId === 'nasa-srtm') {
    if (!srtmCell(item.id) || item.collection !== provider.collection || assets.hgt?.href !== srtmHref(item.id)
      || !isSupportedAsset(assets.hgt?.href, 'srtm')) throw new Error('The catalog returned an unsupported SRTMGL1 tile.');
    assets.srtm = { ...assets.hgt, type: 'application/zip' };
    if (assets.browse) assets.thumbnail = assets.browse;
  }
  if (providerId === 'planetary-computer' && assets.SCL) assets.scl = assets.SCL;
  if (providerId === 'planetary-computer' && assets.rendered_preview) assets.thumbnail = assets.rendered_preview;
  if (providerId === 'planetary-landsat') {
    if (item.collection !== 'landsat-c2-l2' || !/^LC0[89]_/.test(item.id)) throw new Error('The catalog returned an unsupported Landsat product.');
    for (const key of LANDSAT_QUALITY_KEYS) {
      const asset = assets[key]; if (!asset) continue;
      const info = asset['raster:bands']?.[0];
      if (landsatQualityIdentity(asset.href,key)?.id !== item.id || asset['raster:bands']?.length !== 1
        || info.data_type !== 'uint16' || info.spatial_resolution !== 30 || info.scale !== undefined || info.offset !== undefined
        || asset['eo:bands'] !== undefined || (key === 'qa_pixel' && info.nodata !== 1)
        || (key === 'qa_radsat' && info.nodata !== undefined && info.nodata !== 0)) throw new Error('The Landsat quality file does not match its product and unsigned flags.');
      assets[key] = {...asset,rasterBand:{dataType:'uint16',spatialResolution:30,...(info.nodata !== undefined ? {nodata:info.nodata}:{})}};
    }
  }
  if (providerId === 'planetary-landsat' && assets.rendered_preview) assets.thumbnail = assets.rendered_preview;
  if (providerId === 'planetary-naip') {
    const original = assets.image;
    if (item.collection !== 'naip' || !isSupportedAsset(original?.href, 'aerial')
      || !naipMatchesItem(new URL(original.href).pathname, item.id)
      || original['eo:bands']?.map(band => band.common_name).join(',') !== 'red,green,blue,nir') throw new Error('The catalog returned an unsupported NAIP tile.');
    assets.aerial = original;
    // Azure .200.jpg previews also need SAS. Use the reviewed public PC render,
    // explicitly requesting RGB; neither preview is an original-file download.
    const preview = new URL('https://planetarycomputer.microsoft.com/api/data/v1/item/preview.png');
    preview.search = new URLSearchParams({ collection: 'naip', item: item.id, assets: 'image', asset_bidx: 'image|1,2,3', format: 'png' });
    assets.thumbnail = { href: preview.href, type: 'image/png' };
  }
  if (providerId === 'nasa-earthdata') {
    if (assets.browse) assets.thumbnail = assets.browse;
    // HLS L30 v2.0 uses signed Int16 reflectance values. The conversion comes
    // from the LP DAAC product specification, not Landsat Collection 2.
    for (const [key, band] of [['red', 'B04'], ['green', 'B03'], ['blue', 'B02']]) {
      if (assets[band]) assets[key] = { ...assets[band], type: 'image/tiff; application=geotiff', rasterBand: { dataType: 'int16', scale: 0.0001, offset: 0, nodata: -9999, spatialResolution: 30 } };
    }
  }
  const visual = assets.visual || assets.aerial || {}, thumbnail = assets.thumbnail?.href || null;
  const elevationProduct = assets.elevation ? demProduct(item.id) : null;
  const epsg = visual["proj:epsg"] ?? properties["proj:epsg"];
  const cloud = typeof properties["eo:cloud_cover"] === "number" && Number.isFinite(properties["eo:cloud_cover"]) ? properties["eo:cloud_cover"] : null;
  let transform = visual['proj:transform'] ?? properties['proj:transform'];
  if (providerId === 'planetary-naip' && transform?.length === 9 && transform.slice(6).join(',') === '0,0,1') transform = transform.slice(0,6);
  return { id: item.id, provider: providerId, dataset: provider.dataset || 'Sentinel-2 L2A', collection: item.collection || provider.collection || "sentinel-2-l2a", date: acquisition, ...(period ? { endDate: period.endDate } : {}), cloud: ['aerial','elevation','composite','radar'].includes(provider.domain) ? null : cloud, thumbnail, source: thumbnail, bbox: item.bbox, geometry: item.geometry, assets, properties, crs: viirs ? null : period ? MODIS_CRS : providerId === 'nasa-srtm' ? 'EPSG:4326' : visual["proj:code"] ?? properties["proj:code"] ?? (Number.isInteger(epsg) ? `EPSG:${epsg}` : null), grid: { shape: visual["proj:shape"] ?? properties["proj:shape"], transform }, gsd: elevationProduct ? elevationProduct.resolution : provider.domain === 'radar' ? 10 : viirs ? 1000 : vegetation ? 250 : period ? 500 : visual.gsd ?? properties.gsd ?? (providerId === 'nasa-earthdata' ? 30 : null), itemURL: `${provider.catalog}collections/${encodeURIComponent(item.collection || provider.collection || "sentinel-2-l2a")}/items/${encodeURIComponent(item.id)}` };
}

export function normalizeSample(catalog) {
  return { ...catalog, scenes: catalog.scenes.map((scene) => ({ ...normalizeScene({ ...scene, properties: { ...scene.properties, datetime: scene.date } }), ...scene, thumbnail: `./${scene.thumbnail}` })) };
}

export function compatibleScenes(a, b) {
  if (!a || !b || a.id === b.id || !imageryHrefs(a).length || !imageryHrefs(b).length || !a.crs || a.crs !== b.crs || !/^EPSG:(326|327)(0[1-9]|[1-5][0-9]|60)$/.test(a.crs)) return false;
  const supported = scene => Array.isArray(scene.grid?.shape) && scene.grid.shape.length === 2
    && scene.grid.shape.every(value => Number.isFinite(value) && value > 0)
    && Array.isArray(scene.grid?.transform) && scene.grid.transform.length === 6
    && scene.grid.transform.every(Number.isFinite)
    && scene.grid.transform[0] > 0 && scene.grid.transform[4] < 0
    && scene.grid.transform[1] === 0 && scene.grid.transform[3] === 0;
  return supported(a) && supported(b) && ["shape", "transform"].every(key => a.grid[key].every((value, index) => value === b.grid[key][index]));
}

export function nextPageURL(response, providerId = 'earth-search') {
  const next = response.links?.find((link) => link.rel === "next");
  if (!next) return null;
  const provider = providerById(providerId);
  const url = new URL(next.href, provider.search);
  if (providerForSearch(url.href).id !== providerId || (next.method && next.method !== "GET")) throw new Error("The catalog returned an unsupported pagination link.");
  return url.href;
}

export async function fetchCatalogPage(url, { signal, fetcher = fetch } = {}) {
  // A pagination link may never change the request to another host or operation.
  const provider = providerForSearch(url);
  const safeURL = nextPageURL({ links: [{ rel: "next", href: url }] }, provider.id);
  const deadline = AbortSignal.timeout(30_000);
  const requestSignal = signal ? AbortSignal.any([signal, deadline]) : deadline;
  const requestURL = new URL(safeURL), polarization = requestURL.searchParams.get('geod-polarization');
  if (polarization && (provider.domain !== 'radar' || !RADAR_KEYS.includes(polarization))) throw new Error('Choose a supported orbit and polarization.');
  requestURL.searchParams.delete('geod-polarization');
  const response = await fetcher(requestURL.href, { signal: requestSignal, credentials: "omit", headers: { Accept: "application/geo+json, application/json" } });
  if (!response.ok) throw new Error(`${provider.name} returned HTTP ${response.status}. Try again later.`);
  const data = await response.json();
  if (data.type !== "FeatureCollection" || !Array.isArray(data.features)) throw new Error('The source returned an invalid catalog response.');
  let scenes = data.features.map(item => normalizeScene(item, provider.id));
  let next = nextPageURL(data, provider.id);
  if (polarization) {
    scenes = scenes.filter(scene => scene.assets[polarization]);
    if (next) { const cursor = new URL(next); cursor.searchParams.set('geod-polarization',polarization); next = cursor.href; }
  }
  if (provider.id === 'nasa-earthdata') {
    // CMR-STAC currently ignores cloud query operators. Filter every returned
    // page and keep following the cursor even when this page has no matches.
    // Preserve the user's range if the server drops or changes query on next.
    const query = new URL(safeURL).searchParams.get('query');
    const range = JSON.parse(query || '{}')['eo:cloud_cover'] || { gte: 0, lte: 100 };
    if (!Number.isFinite(range.gte) || !Number.isFinite(range.lte) || range.gte < 0 || range.lte > 100 || range.gte > range.lte) throw new Error('The catalog returned an invalid cloud filter.');
    scenes = scenes.filter(scene => scene.cloud !== null && scene.cloud >= range.gte && scene.cloud <= range.lte);
    if (next && query) {
      const url = new URL(next);
      url.searchParams.set('query', query);
      next = url.href;
    }
  }
  return { provider: provider.id, scenes, next, retrievedAt: new Date().toISOString(), query: safeURL, attribution: `Catalog by ${provider.name}.`, registry: provider.terms };
}

// Abort the old HTTP request and also reject its result when a transport ignores abort.
export function createSearchRunner() {
  let generation = 0, controller;
  return {
    cancel() { generation += 1; controller?.abort(); },
    async run(url, options = {}) {
      controller?.abort();
      controller = new AbortController();
      const current = ++generation;
      try {
        const result = await fetchCatalogPage(url, { ...options, signal: controller.signal });
        return current === generation ? result : null;
      } catch (error) {
        if (current !== generation) return null;
        throw error;
      }
    },
    async runAll(url, { onPage, ...options } = {}) {
      controller?.abort();
      controller = new AbortController();
      const current = ++generation;
      const visited = new Set();
      const scenes = new Map();
      let next = url;
      let pages = 0;
      try {
        while (next) {
          if (visited.has(next)) throw new Error("The catalog repeated a pagination link.");
          visited.add(next);
          const page = await fetchCatalogPage(next, { ...options, signal: controller.signal });
          if (current !== generation) return null;
          for (const scene of page.scenes) scenes.set(scene.id, scene);
          pages += 1;
          next = page.next;
          onPage?.({ ...page, scenes: [...scenes.values()], next, pages, complete: !next });
        }
        return { scenes: [...scenes.values()], pages, complete: true };
      } catch (error) {
        if (current !== generation) return null;
        throw error;
      }
    },
  };
}
