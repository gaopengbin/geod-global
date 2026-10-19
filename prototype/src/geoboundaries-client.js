// gbOpen supplies complete immutable country files, rather than bbox-filtered
// OGC pages. Keep its metadata, release identity and receipts together.
export const GEOBOUNDARIES_LICENSE = 'https://creativecommons.org/licenses/by/4.0/';
export const GEOBOUNDARIES_ATTRIBUTION = 'geoBoundaries · William & Mary geoLab · CC BY 4.0';
export const GEOBOUNDARIES_SELECTION = 'geoboundaries-full-administrative-level';
const maxBytes = 20 * 1024 * 1024;
const digest = /^[a-f0-9]{64}$/;
const gitRef = /^[a-f0-9]{7,40}$/;
const plain = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const utf8 = value => new TextEncoder().encode(value);
const clean = (value, max) => typeof value === 'string' && value.length > 0 && value === value.trim() && Array.from(value).length <= max && !/[\u0000-\u001f\u007f-\u009f]/.test(value);
const absent = value => value === undefined || value === null;
const fail = message => { throw new Error(message); };
const only = (value, keys) => plain(value) && Object.keys(value).every(key => keys.includes(key));
export const validGeoBoundariesCountry = value => typeof value === 'string' && /^[A-Z]{3}$/.test(value) && value !== 'ALL';
const validAdmin = value => typeof value === 'string' && /^ADM[0-5]$/.test(value);

function publicCanonical(raw) {
  if (typeof raw !== 'string' || utf8(raw).length > 2048) fail('Invalid geoBoundaries URL.');
  let url;
  try { url = new URL(raw); } catch { fail('Invalid geoBoundaries URL.'); }
  if (url.protocol !== 'https:' || url.port || url.username || url.password || raw.includes('?') || raw.includes('#') || url.href !== raw) fail('Invalid geoBoundaries URL.');
  return url;
}
export function geoBoundariesServiceRoot(raw) {
  const url = publicCanonical(raw);
  const match = /^\/api\/current\/gbOpen\/([A-Z]{3})\/ALL\/$/.exec(url.pathname);
  if (url.hostname !== 'www.geoboundaries.org' || !match || !validGeoBoundariesCountry(match[1])) fail('Invalid geoBoundaries gbOpen country endpoint.');
  return match[1];
}
export function geoBoundariesLayerApi(country, level) {
  if (!validGeoBoundariesCountry(country) || !validAdmin(level)) fail('Invalid geoBoundaries administrative identity.');
  return `https://www.geoboundaries.org/api/current/gbOpen/${country}/${level}/`;
}
function releaseUrl(raw, country, level) {
  const url = publicCanonical(raw);
  const prefixes = {
    'github.com': '/wmgeolab/geoBoundaries/raw/',
    'raw.githubusercontent.com': '/wmgeolab/geoBoundaries/',
    'media.githubusercontent.com': '/media/wmgeolab/geoBoundaries/',
  };
  const prefix = prefixes[url.hostname];
  if (!prefix || !url.pathname.startsWith(prefix)) fail('geoBoundaries download is outside the publisher repository.');
  const parts = url.pathname.slice(prefix.length).split('/');
  if (parts.length !== 6 || !gitRef.test(parts[0]) || parts.slice(1, 5).join('/') !== `releaseData/gbOpen/${country}/${level}`
    || parts[5] !== `geoBoundaries-${country}-${level}.geojson`) fail('geoBoundaries requires the complete original GeoJSON at a fixed Git ref.');
  return parts[0];
}

// Metadata is bounded to 64 KiB. A synchronous SHA-256 lets saved-registry and
// vector validators reject a changed optional publisher field before rendering.
const roundConstants = [0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2];
const rotate = (word, count) => word >>> count | word << (32 - count);
function sha256(bytes) {
  const buffer = new Uint8Array(Math.ceil((bytes.length + 9) / 64) * 64);
  buffer.set(bytes); buffer[bytes.length] = 0x80;
  const view = new DataView(buffer.buffer);
  view.setUint32(buffer.length - 8, Math.floor(bytes.length / 0x20000000));
  view.setUint32(buffer.length - 4, bytes.length * 8);
  const hash = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
  const words = new Uint32Array(64);
  for (let offset = 0; offset < buffer.length; offset += 64) {
    for (let n = 0; n < 16; n++) words[n] = view.getUint32(offset + n * 4);
    for (let n = 16; n < 64; n++) {
      const a = words[n - 15], b = words[n - 2];
      words[n] = (words[n - 16] + (rotate(a, 7) ^ rotate(a, 18) ^ a >>> 3) + words[n - 7] + (rotate(b, 17) ^ rotate(b, 19) ^ b >>> 10)) >>> 0;
    }
    let [a,b,c,d,e,f,g,h] = hash;
    for (let n = 0; n < 64; n++) {
      const first = (h + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + (e & f ^ ~e & g) + roundConstants[n] + words[n]) >>> 0;
      const second = ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + (a & b ^ a & c ^ b & c)) >>> 0;
      h=g; g=f; f=e; e=(d+first)>>>0; d=c; c=b; b=a; a=(first+second)>>>0;
    }
    for (const [n, word] of [a,b,c,d,e,f,g,h].entries()) hash[n] = (hash[n] + word) >>> 0;
  }
  return hash.map(word => word.toString(16).padStart(8, '0')).join('');
}
function canonicalJson(value, depth = 0) {
  if (depth > 32) fail('geoBoundaries metadata nesting exceeds its bound.');
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return JSON.stringify(value);
  if (typeof value === 'number' && Number.isFinite(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(entry => canonicalJson(entry, depth + 1)).join(',')}]`;
  if (plain(value)) return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key], depth + 1)}`).join(',')}}`;
  fail('Invalid geoBoundaries metadata value.');
}
function metadataDigest(value) {
  const bytes = utf8(canonicalJson(value));
  if (bytes.length > 65536) fail('geoBoundaries metadata exceeds its bound.');
  return sha256(bytes);
}
const layerFields = ['boundaryId','countryIso3','countryName','adminLevel','yearRepresented','originalSource','originalLicense','licenseSource','sourceDataUpdateDate','buildDate','unitCount','releaseRef','downloadUrl','metadata','metadataSha256'];
const metadataFields = [['boundaryId','boundaryID',120],['countryIso3','boundaryISO',3],['countryName','boundaryName',160],['adminLevel','boundaryType',4],['yearRepresented','boundaryYearRepresented',120],['originalSource','boundarySource',8192],['originalLicense','boundaryLicense',2048],['licenseSource','licenseSource',4096],['sourceDataUpdateDate','sourceDataUpdateDate',160],['buildDate','buildDate',160],['downloadUrl','gjDownloadURL',2048]];
export function validateGeoBoundariesLayer(layer) {
  if (!only(layer, layerFields) || Object.keys(layer).length !== layerFields.length || !plain(layer.metadata) || !digest.test(layer.metadataSha256)
    || metadataDigest(layer.metadata) !== layer.metadataSha256) fail('Invalid geoBoundaries layer metadata fingerprint.');
  for (const [field, key, max] of metadataFields) if (!clean(layer.metadata[key], max) || layer[field] !== layer.metadata[key]) fail('geoBoundaries metadata identity changed.');
  if (!validGeoBoundariesCountry(layer.countryIso3) || !validAdmin(layer.adminLevel) || !layer.boundaryId.startsWith(`${layer.countryIso3}-${layer.adminLevel}-`)
    || !clean(layer.metadata.admUnitCount, 12) || !/^\+?\d+$/.test(layer.metadata.admUnitCount) || !Number.isSafeInteger(layer.unitCount) || layer.unitCount < 1 || layer.unitCount > 50000
    || Number(layer.metadata.admUnitCount) !== layer.unitCount || releaseUrl(layer.downloadUrl, layer.countryIso3, layer.adminLevel) !== layer.releaseRef) fail('Invalid geoBoundaries country or release identity.');
  return layer;
}
function discovery(receipt, root, count) {
  if (!only(receipt, ['url','sha256','bytes','returned','parameters']) || receipt.url !== root || !digest.test(receipt.sha256) || !Number.isSafeInteger(receipt.bytes) || receipt.bytes < 1 || receipt.bytes > 1024 * 1024
    || !Number.isSafeInteger(receipt.returned) || receipt.returned < 1 || receipt.returned > 6 || !absent(receipt.parameters) || count !== undefined && receipt.returned !== count) fail('Invalid geoBoundaries discovery receipt.');
}
export function validateGeoBoundariesService(service) {
  const country = geoBoundariesServiceRoot(service.url), metadata = service.geoboundaries;
  if (!only(service, ['id','name','url','title','collections','connectedAt','arcgis','overpass','wfs','geoboundaries']) || !only(metadata, ['releaseType','countryIso3','discoveryReceipt']) || metadata.releaseType !== 'gbOpen' || metadata.countryIso3 !== country
    || ['arcgis','overpass','wfs'].some(key => !absent(service[key])) || !Array.isArray(service.collections) || service.collections.length < 1 || service.collections.length > 6) fail('Invalid saved geoBoundaries service.');
  discovery(metadata.discoveryReceipt, service.url, service.collections.length);
  const ids = new Set();
  for (const collection of service.collections) {
    const layer = validateGeoBoundariesLayer(collection.geoboundaries);
    if (!only(collection, ['id','title','description','itemsUrl','licenseLinks','arcgis','wfs','geoboundaries']) || layer.countryIso3 !== country || collection.id !== layer.adminLevel || ids.has(collection.id) || collection.itemsUrl !== geoBoundariesLayerApi(country, layer.adminLevel)
      || !Array.isArray(collection.licenseLinks) || collection.licenseLinks.length !== 1 || collection.licenseLinks[0] !== GEOBOUNDARIES_LICENSE || !absent(collection.arcgis) || !absent(collection.wfs)) fail('Invalid saved geoBoundaries collection.');
    ids.add(collection.id);
  }
  return service;
}
function validBounds(bounds) {
  return Array.isArray(bounds) && bounds.length === 4 && bounds.every(Number.isFinite) && bounds[0] >= -180 && bounds[2] <= 180 && bounds[1] >= -90 && bounds[3] <= 90 && bounds[0] < bounds[2] && bounds[1] < bounds[3];
}
function validArea(area, bounds) {
  if (absent(area)) return true;
  if (!only(area, ['type','coordinates']) || Object.keys(area).length !== 2 || !['Polygon','MultiPolygon'].includes(area.type)) return false;
  const polygons = area.type === 'Polygon' ? [area.coordinates] : area.coordinates;
  let points = 0;
  return Array.isArray(polygons) && polygons.length > 0 && polygons.length <= 500 && polygons.every(polygon => Array.isArray(polygon) && polygon.length > 0 && polygon.length <= 1000 && polygon.every(ring => {
    if (!Array.isArray(ring) || ring.length < 4 || (points += ring.length) > 30000 || ring.some(point => !Array.isArray(point) || point.length !== 2 || !point.every(Number.isFinite) || point[0] < bounds[0] || point[0] > bounds[2] || point[1] < bounds[1] || point[1] > bounds[3]) || ring[0].some((value, index) => value !== ring.at(-1)[index])) return false;
    return Math.abs(ring.slice(1).reduce((sum, point, index) => sum + ring[index][0] * point[1] - point[0] * ring[index][1], 0)) >= 1e-12;
  }));
}
export function validateGeoBoundariesProvenance(source, count) {
  if (!only(source, ['serviceUrl','serviceName','collectionId','collectionTitle','licenseLinks','requestedBounds','areaGeometry','requestedAt','pages','numberMatched','featureCount','selection','arcgis','wfs','geoboundaries'])) fail('Invalid geoBoundaries country-file provenance.');
  const country = geoBoundariesServiceRoot(source.serviceUrl), snapshot = source.geoboundaries;
  if (!only(snapshot, ['releaseType','layer','discoveryReceipt','resolvedReleaseCommit','attribution','geometrySelection','sourceCrs'])) fail('Invalid geoBoundaries snapshot.');
  if (!absent(snapshot.sourceCrs) && (!only(snapshot.sourceCrs, ['type','properties']) || Object.keys(snapshot.sourceCrs).length !== 2 || snapshot.sourceCrs.type !== 'name' || !only(snapshot.sourceCrs.properties, ['name']) || Object.keys(snapshot.sourceCrs.properties).length !== 1 || snapshot.sourceCrs.properties.name !== 'urn:ogc:def:crs:OGC:1.3:CRS84')) fail('Invalid original geoBoundaries CRS declaration.');
  const layer = validateGeoBoundariesLayer(snapshot.layer);
  discovery(snapshot.discoveryReceipt, source.serviceUrl);
  if (!clean(source.serviceName, 80) || !clean(source.collectionTitle, 240) || snapshot.releaseType !== 'gbOpen' || layer.countryIso3 !== country || source.collectionId !== layer.adminLevel
    || source.selection !== GEOBOUNDARIES_SELECTION || snapshot.geometrySelection !== source.selection || snapshot.attribution !== GEOBOUNDARIES_ATTRIBUTION
    || !Number.isSafeInteger(count) || count < 1 || count > 50000 || source.featureCount !== count || count !== layer.unitCount || source.numberMatched !== count
    || !validBounds(source.requestedBounds) || !validArea(source.areaGeometry, source.requestedBounds) || typeof source.requestedAt !== 'string' || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(source.requestedAt) || !Number.isFinite(Date.parse(source.requestedAt))
    || ['arcgis','wfs','overpass'].some(key => !absent(source[key])) || !Array.isArray(source.licenseLinks) || source.licenseLinks.length !== 1 || source.licenseLinks[0] !== GEOBOUNDARIES_LICENSE || !Array.isArray(source.pages) || source.pages.length !== 1) fail('Invalid geoBoundaries country-file provenance.');
  const receipt = source.pages[0];
  if (!only(receipt, ['url','sha256','bytes','returned','parameters']) || !digest.test(receipt.sha256) || !Number.isSafeInteger(receipt.bytes) || receipt.bytes < 1 || receipt.bytes > maxBytes || receipt.returned !== count || !absent(receipt.parameters)) fail('Invalid geoBoundaries original-file receipt.');
  const resolvedRef = releaseUrl(receipt.url, country, layer.adminLevel);
  if (!resolvedRef.startsWith(layer.releaseRef) || snapshot.resolvedReleaseCommit !== resolvedRef) fail('geoBoundaries original-file release changed.');
  return source;
}
export function validateGeoBoundariesFeatures(features, source) {
  const layer = source.geoboundaries.layer, ids = new Set();
  if (!Array.isArray(features) || features.length !== layer.unitCount) fail('geoBoundaries original unit count changed.');
  for (const feature of features) {
    const properties = feature?.properties;
    if (feature?.type !== 'Feature' || !['Polygon','MultiPolygon'].includes(feature?.geometry?.type) || !plain(properties) || !clean(properties.shapeID, 256) || ids.has(properties.shapeID)
      || properties.shapeGroup !== layer.countryIso3 || properties.shapeType !== layer.adminLevel || typeof properties.shapeName !== 'string') fail('geoBoundaries original feature identity changed.');
    ids.add(properties.shapeID);
  }
  return features;
}
