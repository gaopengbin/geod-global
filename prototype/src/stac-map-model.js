import { utmDefinition } from './workspace-map-geometry.js';
import ghslProfiles from '../../crates/geod-runtime/src/datasets/next-ghsl-profiles.json' with { type: 'json' };
import { nextDirectRange } from './next-public-products.js';
import clcdProfile from '../../crates/geod-runtime/src/datasets/clcd-reviewed-index.json' with {type:'json'};
import {reviewedClcdInspection} from './original-backlog-client.js';

export function genericProjectionDefinition(crs) {
  if (['EPSG:4326', 'EPSG:3857'].includes(crs)) return null;
  if (crs === 'EPSG:4269') return '+proj=longlat +datum=NAD83 +no_defs';
  if (crs === 'EPSG:6933') return '+proj=cea +lat_ts=30 +lon_0=0 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs';
  if (crs === 'MODIS:Sinusoidal') return '+proj=sinu +R=6371007.181 +lon_0=0 +x_0=0 +y_0=0 +units=m +no_defs';
  if (crs === 'CLCD:AlbersWGS84') return clcdProfile.profile.proj4;
  return utmDefinition(crs);
}

// Only the display footprint is intersected with the physical globe. An
// original geographic edge cell may straddle a pole/antimeridian if its centre
// is valid. Keep its image extent, transform and samples exactly unchanged.
export function genericMapDisplayBounds(data, geographicBounds, source) {
  if (!Array.isArray(geographicBounds) || geographicBounds.length !== 4 || !geographicBounds.every(Number.isFinite)
    || geographicBounds[0] >= geographicBounds[2] || geographicBounds[1] >= geographicBounds[3]) throw new Error('This file cannot be placed in the supported map projections.');
  const [west, south, east, north] = geographicBounds;
  if (west >= -180 && east <= 180 && south >= -90 && north <= 90) return geographicBounds;
  genericMapMetadata(data);
  // The measured GHS-SMOD original contains two published buffer columns
  // across the antimeridian. Only this immutable original and its matching
  // native source receipt permit a physical display fit; the raster image
  // extent, coordinates, values and masks still use the complete file grid.
  if (reviewedSmodDisplay(data, geographicBounds, source)) return [-180, south, 180, north];
  if (reviewedGhslDisplay(data, geographicBounds, source)) return [-180, south, 180, north];
  const halfX = data.transform[0] / 2, halfY = -data.transform[4] / 2;
  if (!['EPSG:4326', 'EPSG:4269'].includes(data.crs) || !['area', 'point'].includes(data.pixelInterpretation)
    || west < -180 - halfX || east > 180 + halfX || south < -90 - halfY || north > 90 + halfY
    || west + halfX < -180 || east - halfX > 180 || south + halfY < -90 || north - halfY > 90
    || west >= 180 || east <= -180 || south >= 90 || north <= -90) throw new Error('This file cannot be placed in the supported map projections.');
  return [Math.max(-180, west), Math.max(-90, south), Math.min(180, east), Math.min(90, north)];
}

// ImageStatic must keep the original full-file extent. The legal intersection
// controls only fitting and the layer's display clip, never PNG stretching or
// the transform used to map a click back to its original row and column.
export function genericMapPlacement(data, geographicBounds, source) {
  genericMapMetadata(data);
  const fitBounds = genericMapDisplayBounds(data, geographicBounds, source);
  return { fitBounds, imageBounds: [...data.bounds],
    clipBounds: fitBounds.some((value,index) => value !== geographicBounds[index]) ? [...fitBounds] : null };
}

export function reviewedGhslDisplay(data, bounds, source) {
  const profile = ghslProfiles.profiles.find(item => item.sha256 === data?.sha256);
  const proof = source?.properties?.datasetPreset, dataset = proof?.dataset, asset = source?.assets?.[0];
  if (!profile || !proof || !dataset || !asset) return false;
  return data.crs === 'EPSG:4326' && (data.horizontalCrs == null || data.horizontalCrs === 'EPSG:4326')
    && data.verticalCrs == null && data.pixelInterpretation === 'area'
    && data.width === profile.width && data.height === profile.height && data.storageLayout === 'tile'
    && data.bands?.length === 1 && data.bands[0].index === 1
    && data.bands[0].dataType === profile.dataType && data.bands[0].nodata === null
    && sameCoordinates(data.transform,profile.transform) && sameCoordinates(data.bounds,profile.sourceBounds)
    && sameCoordinates(bounds,profile.sourceBounds)
    && source.provenance?.documentUrl === profile.url && source.provenance.search == null
    && source.collectionId == null && source.datetime == null && source.startDatetime == null && source.endDatetime == null
    && source.assets.length === 1 && asset.eligible === true && asset.key === 'raster'
    && asset.href === profile.url && (asset.originalHref == null || asset.originalHref === profile.url)
    && asset.transportAdapter == null && asset.metadata?.kind === 'direct-raster'
    && JSON.stringify(asset.metadata.datasetPreset) === JSON.stringify(proof)
    && dataset.id === profile.datasetId && dataset.title === profile.title
    && dataset.version === `${profile.code} R2023A V1.0 · EPSG:4326 30ss · reference epoch 2020`
    && JSON.stringify(dataset.years) === '[2020]' && dataset.defaultYear === 2020
    && (!dataset.months || dataset.months.length === 0) && dataset.defaultMonth == null
    && dataset.layers?.length === 1 && dataset.defaultLayer === profile.layer
    && proof.sourceTitle === `JRC ${profile.code} V1.0 · total · reference epoch 2020`
    && sameCoordinates(proof.sourceBounds,profile.queryBounds)
    && Boolean(nextDirectRange(dataset,asset));
}

const smodUrl = 'https://jrc-ghsl.s3.eu-central-1.amazonaws.com/ghs-smod/r2023a/4326/30asec/2020/GHS_SMOD_E2020_GLOBE_R2023A_4326_30ss_V2_0.tif';
const smodTransform = [0.008333333300326923,0,-180.00791593130032,0,-0.00833333329979504,89.0995831776456];
const smodBounds = [-180.00791593130032,-89.10041610517152,180.00874930942342,89.0995831776456];
const sameCoordinates = (a,b) => Array.isArray(a) && a.length === b.length && a.every((value,index)=>Math.abs(value-b[index])<1e-11);
function reviewedSmodDisplay(data, bounds, source) {
  const proof = source?.properties?.datasetPreset;
  return data.sha256 === '20661ac0f98166bd2253ce1155e131d589b0100fc2d5eed9dc7c632228071416'
    && data.crs === 'EPSG:4326' && data.pixelInterpretation === 'area' && data.width === 43202 && data.height === 21384
    && sameCoordinates(data.transform,smodTransform) && sameCoordinates(data.bounds,smodBounds) && sameCoordinates(bounds,smodBounds)
    && source?.provenance?.documentUrl === smodUrl && source.provenance.search == null && source.collectionId == null
    && source.assets?.length === 1 && source.assets[0].key === 'raster' && source.assets[0].href === smodUrl
    && source.assets[0].metadata?.kind === 'direct-raster'
    && JSON.stringify(source.assets[0].metadata.datasetPreset) === JSON.stringify(proof)
    && proof?.schemaVersion === 2 && proof.dataset?.id === 'ghsl-smod' && proof.request?.datasetId === 'ghsl-smod'
    && proof.sourceId === 'GHS_SMOD_E2020_GLOBE_R2023A_4326_30ss_V2_0'
    && proof.sourceTitle === 'JRC GHS-SMOD V2.0 · level 2 · reference epoch 2020'
    && proof.declarationKind === 'reviewed-client-preset; publisher file bytes and embedded metadata verified separately'
    && proof.dataset.version === 'GHS-SMOD R2023A V2.0 · EPSG:4326 30ss · reference epoch 2020 · reviewed 2026-10-12'
    && JSON.stringify(proof.dataset.years) === '[2020]' && proof.dataset.defaultYear === 2020
    && proof.dataset.layers?.length === 1 && proof.dataset.layers[0].id === 'settlement-class'
    && proof.dataset.layers[0].nodata === -200 && proof.dataset.layers[0].scale === 1 && proof.dataset.layers[0].offset === 0
    && JSON.stringify(proof.layer) === JSON.stringify(proof.dataset.layers[0])
    && proof.request.year === 2020 && proof.request.layer === 'settlement-class' && proof.layer?.id === 'settlement-class'
    && sameCoordinates(proof.sourceBounds,[-180,smodBounds[1],180,smodBounds[3]]);
}

export function genericMapMetadata(data) {
  genericProjectionDefinition(data.crs);
  if(data.crs==='CLCD:AlbersWGS84'&&!reviewedClcdInspection(data)) throw new Error('The CLCD map grid differs from the exact complete original.');
  if(data.crs==='MODIS:Sinusoidal') {
    const dtype=data.sha256==='6537d61e333a8e7e478affd9de6fc7752a4ac60cbf513340cde33b043a04d073'?'Int16'
      :data.sha256==='5f2f9800fcfad9aabc6bbe5beade4ccd5922fd589868212adb012b2bbe6b1820'?'UInt8':null;
    if(!dtype||data.width!==2400||data.height!==2400||data.storageLayout!=='tile'||data.horizontalCrs!==data.crs
      ||data.verticalCrs!=null||data.pixelInterpretation!=='area'||data.bands?.length!==1||data.bands[0].index!==1
      ||data.bands[0].dataType!==dtype||data.bands[0].nodata!==(dtype==='Int16'?-1:null)
      ||!sameCoordinates(data.transform,[463.3127165283333,0,10007554.677014,0,-463.3127165279167,4447802.078664]))throw new Error('The MODIS map grid differs from its reviewed original sphere and tile.');
  }
  const transform = data.transform, bounds = data.bounds;
  if (!Array.isArray(transform) || transform.length !== 6 || !transform.every(Number.isFinite)
    || transform[0] <= 0 || transform[4] >= 0 || transform[1] !== 0 || transform[3] !== 0
    || !Array.isArray(bounds) || bounds.length !== 4 || !bounds.every(Number.isFinite) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3]
    || !Number.isSafeInteger(data.width) || !Number.isSafeInteger(data.height) || data.width < 1 || data.height < 1) throw new Error('This file has no supported north-up georeferenced grid.');
  const expected = [transform[2], transform[5] + data.height * transform[4], transform[2] + data.width * transform[0], transform[5]];
  if (bounds.some((value, index) => Math.abs(value - expected[index]) > Math.max(1, Math.abs(value)) * 1e-8)) throw new Error('The raster transform and file bounds disagree.');
  return data;
}

// A nonlinear projection can have extrema inside a rectangle's edge. Fit in
// geographic coordinates while the image and click sampling keep native CRS.
export function genericGeographicBounds(data,transformBounds) {
  genericMapMetadata(data);
  if(data.crs==='CLCD:AlbersWGS84') return [...clcdProfile.sourceBounds];
  return transformBounds(data.bounds,data.crs,'EPSG:4326',64);
}

export function genericCoordinateToPixel(coordinate, data) {
  if (!coordinate?.every(Number.isFinite) || coordinate.length !== 2) return null;
  const [x, y] = coordinate, [west, south, east, north] = data.bounds;
  if (x < west || x >= east || y <= south || y > north) return null;
  return { column: Math.max(0, Math.min(data.width - 1, Math.floor((x - data.transform[2]) / data.transform[0]))), row: Math.max(0, Math.min(data.height - 1, Math.floor((y - data.transform[5]) / data.transform[4]))) };
}
