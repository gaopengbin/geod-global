export const EARTH_SEARCH = "https://earth-search.aws.element84.com/v1/search";
export const SAMPLE_BBOX = [-122.55, 37.68, -122.32, 37.84];
export const INITIAL_SEARCH = { bbox: SAMPLE_BBOX.join(", "), start: "2025-06-01", end: "2025-06-30", cloud: 60, limit: 20 };

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
  for (const key of ["start", "end"]) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(input[key]) || !Number.isFinite(Date.parse(input[key])) || new Date(input[key]).toISOString().slice(0, 10) !== input[key]) throw new Error("Choose valid start and end dates.");
  }
  if (input.start > input.end) throw new Error("The start date must be on or before the end date.");
  const cloud = Number(input.cloud), limit = Number(input.limit);
  if (!Number.isFinite(cloud) || cloud < 0 || cloud > 100) throw new Error("Cloud cover must be between 0 and 100 percent.");
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error("Page size must be between 1 and 100 scenes.");
  return { bbox, start: input.start, end: input.end, cloud, limit };
}

export function searchURL(input) {
  const q = validateSearch(input), url = new URL(EARTH_SEARCH);
  url.search = new URLSearchParams({ collections: "sentinel-2-l2a", bbox: q.bbox.join(","), datetime: `${q.start}T00:00:00Z/${q.end}T23:59:59.999Z`, query: JSON.stringify({ "eo:cloud_cover": { lte: q.cloud } }), sortby: "-properties.datetime", limit: String(q.limit) });
  return url.href;
}

function httpsURL(value) {
  try { const u = new URL(value); return u.protocol === "https:" && !u.username && !u.password ? u.href : null; } catch { return null; }
}

export function normalizeScene(item) {
  const properties = item?.properties || {};
  if (typeof item?.id !== "string" || !item.id || !Number.isFinite(Date.parse(properties.datetime))) throw new Error("The catalog returned an item without a valid ID or acquisition date.");
  const assets = Object.fromEntries(Object.entries(item.assets || {}).filter(([, asset]) => asset && httpsURL(asset.href)).map(([key, asset]) => [key, { ...asset, href: httpsURL(asset.href) }]));
  const visual = assets.visual || {}, thumbnail = assets.thumbnail?.href || null;
  const epsg = visual["proj:epsg"] ?? properties["proj:epsg"];
  const cloud = typeof properties["eo:cloud_cover"] === "number" && Number.isFinite(properties["eo:cloud_cover"]) ? properties["eo:cloud_cover"] : null;
  return { id: item.id, collection: item.collection || "sentinel-2-l2a", date: properties.datetime, cloud, thumbnail, source: thumbnail, bbox: item.bbox, geometry: item.geometry, assets, properties, crs: visual["proj:code"] ?? properties["proj:code"] ?? (Number.isInteger(epsg) ? `EPSG:${epsg}` : null), grid: { shape: visual["proj:shape"] ?? properties["proj:shape"], transform: visual["proj:transform"] ?? properties["proj:transform"] }, gsd: visual.gsd ?? properties.gsd ?? null, itemURL: `https://earth-search.aws.element84.com/v1/collections/${encodeURIComponent(item.collection || "sentinel-2-l2a")}/items/${encodeURIComponent(item.id)}` };
}

export function normalizeSample(catalog) {
  return { ...catalog, scenes: catalog.scenes.map((scene) => ({ ...normalizeScene({ ...scene, properties: { ...scene.properties, datetime: scene.date } }), ...scene, thumbnail: `./${scene.thumbnail}` })) };
}

export function compatibleScenes(a, b) {
  if (!a || !b || a.id === b.id || !a.assets?.visual?.href || !b.assets?.visual?.href || !a.crs || a.crs !== b.crs || !/^EPSG:(326|327)(0[1-9]|[1-5][0-9]|60)$/.test(a.crs)) return false;
  const supported = scene => Array.isArray(scene.grid?.shape) && scene.grid.shape.length === 2
    && scene.grid.shape.every(value => Number.isFinite(value) && value > 0)
    && Array.isArray(scene.grid?.transform) && scene.grid.transform.length === 6
    && scene.grid.transform.every(Number.isFinite)
    && scene.grid.transform[0] > 0 && scene.grid.transform[4] < 0
    && scene.grid.transform[1] === 0 && scene.grid.transform[3] === 0;
  return supported(a) && supported(b) && ["shape", "transform"].every(key => a.grid[key].every((value, index) => value === b.grid[key][index]));
}

export function nextPageURL(response) {
  const next = response.links?.find((link) => link.rel === "next");
  if (!next) return null;
  const url = new URL(next.href, EARTH_SEARCH);
  if (url.origin !== new URL(EARTH_SEARCH).origin || url.pathname !== "/v1/search" || url.username || url.password || (next.method && next.method !== "GET")) throw new Error("The catalog returned an unsupported pagination link.");
  return url.href;
}

export async function fetchCatalogPage(url, { signal, fetcher = fetch } = {}) {
  // A pagination link may never change the request to another host or operation.
  const safeURL = nextPageURL({ links: [{ rel: "next", href: url }] });
  const deadline = AbortSignal.timeout(30_000);
  const requestSignal = signal ? AbortSignal.any([signal, deadline]) : deadline;
  const response = await fetcher(safeURL, { signal: requestSignal, credentials: "omit", headers: { Accept: "application/geo+json, application/json" } });
  if (!response.ok) throw new Error(`Earth Search returned HTTP ${response.status}. Try again later.`);
  const data = await response.json();
  if (data.type !== "FeatureCollection" || !Array.isArray(data.features)) throw new Error("Earth Search returned an invalid catalog response.");
  return { scenes: data.features.map(normalizeScene), next: nextPageURL(data), retrievedAt: new Date().toISOString(), query: safeURL, attribution: "Contains Copernicus Sentinel data. Catalog by Earth Search / Element 84.", registry: "https://registry.opendata.aws/sentinel-2-l2a-cogs/" };
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
  };
}
