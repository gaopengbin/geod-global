import GeoTIFF from 'ol/source/GeoTIFF.js';
import { register } from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import { genericProjectionDefinition } from './stac-map-model.js';
import { isMcdOverviewPolicy } from './next-public-products.js';

export const PRODUCT_PREVIEW_RANGE_LIMIT = 4 * 1024 * 1024;
export const PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT = 16 * 1024 * 1024;
const SOURCE_OPTIONS = Object.freeze({ allowFullFile: false, blockSize: 65536, cacheSize: 8, maxRanges: 0 });
const PREVIEW_ERROR = 'The original raster preview could not be read. Retry this layer or download the original.';
const GRID_ERROR = 'This original has no supported north-up raster preview grid.';
const SAFE_ERRORS = new Set([PREVIEW_ERROR, GRID_ERROR,
  'This raster requires a larger read than the preview limit. Download the original to inspect it.',
  'The preview source did not return the requested original byte range.',
  'The preview byte range is incomplete.', 'The preview byte range exceeds its declared length.',
  'The TIFF validity masks do not match its image levels.',
  'The original TIFF tiles exceed the bounded preview decoding budget.']);
const safeError = (...errors) => {
  const seen = new Set(); let inspected = 0;
  const find = (error, depth = 0) => {
    if (!error || typeof error !== 'object' || depth > 6 || inspected >= 32 || seen.has(error)) return;
    seen.add(error); inspected++;
    if (SAFE_ERRORS.has(error.message) && error.message !== PREVIEW_ERROR) return error.message;
    const cause = find(error.cause, depth + 1); if (cause) return cause;
    if (Array.isArray(error.errors)) for (const child of error.errors.slice(0, 8)) {
      const message = find(child, depth + 1); if (message) return message;
    }
  };
  for (const error of errors) { const message = find(error); if (message) return new Error(message); }
  return new Error(PREVIEW_ERROR);
};
let activeRequests = 0;
const waiting = [];
const cancelled = () => new DOMException('Raster preview cancelled', 'AbortError');

function validatePin(snapshotId, assetKey) {
  if (!/^[a-f0-9]{64}$/.test(snapshotId || '') || typeof assetKey !== 'string' || !assetKey.trim()
    || assetKey !== assetKey.trim() || assetKey.length > 512 || /[\u0000-\u001f\u007f]/.test(assetKey)) {
    throw new Error('Choose a saved original raster asset to preview.');
  }
}

export function productRasterPreviewURL(snapshotId, assetKey) {
  validatePin(snapshotId, assetKey);
  const core = globalThis.window?.__TAURI__?.core;
  return typeof core?.convertFileSrc === 'function'
    ? core.convertFileSrc(`${snapshotId}/${encodeURIComponent(assetKey)}`, 'geod-stac-preview')
    : `http://127.0.0.1:4318/stac/snapshots/${snapshotId}/preview?${new URLSearchParams({ assetKey })}`;
}

function drain() {
  while (activeRequests < 2 && waiting.length) {
    const task = waiting.shift();
    task.signal.removeEventListener('abort', task.abort);
    if (task.signal.aborted) { task.reject(cancelled()); continue; }
    activeRequests++;
    task.resolve(() => { activeRequests--; drain(); });
  }
}
function acquire(signal) {
  if (signal.aborted) return Promise.reject(cancelled());
  return new Promise((resolve, reject) => {
    const task = { signal, resolve, reject, abort: () => {
      const index = waiting.indexOf(task); if (index >= 0) waiting.splice(index, 1);
      reject(cancelled());
    } };
    signal.addEventListener('abort', task.abort, { once: true }); waiting.push(task); drain();
  });
}

// Requests stay behind the native immutable-asset proxy. Never ask the browser
// to follow a publisher URL or quietly fetch a whole file for a range preview.
export function createProductRasterLoader(url, lifetime, fetcher = (...args) => fetch(...args), onProgress, expectedTotal) {
  return async (requestedUrl, headers, signal) => {
    if (requestedUrl !== url || lifetime.aborted || signal?.aborted) throw cancelled();
    const range = new Headers(headers).get('Range');
    const match = /^bytes=(\d+)-(\d+)$/.exec(range || '');
    const start = match && Number(match[1]), end = match && Number(match[2]);
    if (!match || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start
      || end - start + 1 > PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT) throw new Error('This raster requires a larger read than the preview limit. Download the original to inspect it.');
    const controller = new AbortController(), abort = () => controller.abort();
    lifetime.addEventListener('abort', abort, { once: true }); signal?.addEventListener('abort', abort, { once: true });
    if (lifetime.aborted || signal?.aborted) controller.abort();
    try {
      let cursor = start, total = null, bytes = null;
      while (cursor <= end && (total === null || cursor < total)) {
        if (controller.signal.aborted) throw cancelled();
        const segmentEnd = Math.min(end, cursor + Math.min(PRODUCT_PREVIEW_RANGE_LIMIT, end - cursor + 1) - 1,
          total === null ? end : total - 1);
        let release;
        try {
          release = await acquire(controller.signal);
          if (controller.signal.aborted) throw cancelled();
          const response = await fetcher(url, { headers: { Range: `bytes=${cursor}-${segmentEnd}` },
            signal: controller.signal, credentials: 'omit', redirect: 'error' });
          const receipt = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(response.headers.get('Content-Range') || '');
          const first = receipt && Number(receipt[1]), last = receipt && Number(receipt[2]), declaredTotal = receipt && Number(receipt[3]);
          if (response.status !== 206 || !receipt || ![first, last, declaredTotal].every(Number.isSafeInteger)
            || first !== cursor || declaredTotal <= cursor || last !== Math.min(segmentEnd, declaredTotal - 1)
            || total !== null && declaredTotal !== total || expectedTotal !== undefined && declaredTotal !== expectedTotal
            || response.headers.get('Content-Encoding') && response.headers.get('Content-Encoding') !== 'identity') {
            await response.body?.cancel(); throw new Error('The preview source did not return the requested original byte range.');
          }
          const length = last - first + 1;
          if (response.headers.has('Content-Length') && Number(response.headers.get('Content-Length')) !== length) {
            await response.body?.cancel(); throw new Error('The preview byte range is incomplete.');
          }
          const reader = response.body?.getReader();
          if (!reader) throw new Error('The preview byte range is incomplete.');
          total = declaredTotal;
          bytes ??= new Uint8Array(Math.min(end, total - 1) - start + 1);
          let offset = 0;
          try {
            while (true) {
              if (controller.signal.aborted) throw cancelled();
              const { value, done } = await reader.read(); if (done) break;
              if (offset + value.length > length) throw new Error('The preview byte range exceeds its declared length.');
              bytes.set(value, cursor - start + offset); offset += value.length;
            }
          } catch (error) {
            await reader.cancel().catch(() => {});
            if (controller.signal.aborted) throw cancelled();
            throw error;
          } finally { reader.releaseLock(); }
          if (controller.signal.aborted) throw cancelled();
          if (offset !== length) throw new Error('The preview byte range is incomplete.');
          onProgress?.({ start: first, end: last, total, bytes: length, loadedBytes: last - start + 1 });
          cursor = last + 1;
        } finally { release?.(); }
      }
      if (controller.signal.aborted) throw cancelled();
      return new Response(bytes, { status: 206, headers: { 'Content-Type': 'image/tiff',
        'Content-Range': `bytes ${start}-${cursor - 1}/${total}`, 'Content-Length': String(bytes.length) } });
    } finally {
      lifetime.removeEventListener('abort', abort); signal?.removeEventListener('abort', abort);
    }
  };
}

const tag = (image, name) => image.fileDirectory.getValue(name);
const mask = image => ((tag(image, 'NewSubfileType') || 0) & 4) === 4;
const MCD_KEYS = Object.freeze([1,1,0,18,1024,0,1,1,1025,0,1,1,1026,34737,8,0,2048,0,1,32767,
  2049,34737,153,8,2050,0,1,32767,2054,0,1,9102,2056,0,1,32767,2057,34736,1,3,
  2058,34736,1,4,2061,34736,1,5,3072,0,1,32767,3074,0,1,32767,3075,0,1,24,
  3076,0,1,9001,3082,34736,1,1,3083,34736,1,2,3088,34736,1,0]);
const MCD_DOUBLES = Object.freeze([0,0,0,6371007.181,6371007.181,0]);
const MCD_ASCII = 'unnamed|GCS Name = Unknown datum based upon the custom spheroid|Datum = Not specified (based on custom spheroid)|Ellipsoid = Custom spheroid|Primem = Greenwich||\0';
const MCD_SCALE = Object.freeze([463.3127165283333,463.3127165279167,0]);
const MCD_TIEPOINT = Object.freeze([0,0,0,10007554.677014,4447802.078664,0]);
const exactArray = (value,expected) => (Array.isArray(value) || ArrayBuffer.isView(value))
  && value.length===expected.length && expected.every((n,index)=>value[index]===n);

function validateMcdCrs(image) {
  const keys=image.getGeoKeys();
  if (!exactArray(tag(image,'GeoKeyDirectory'),MCD_KEYS)
    || !exactArray(tag(image,'GeoDoubleParams'),MCD_DOUBLES) || tag(image,'GeoAsciiParams')!==MCD_ASCII
    || keys?.GTModelTypeGeoKey!==1 || keys.GTRasterTypeGeoKey!==1
    || keys.ProjectedCSTypeGeoKey!==32767 || keys.GeographicTypeGeoKey!==32767
    || keys.ProjCoordTransGeoKey!==24 || keys.ProjLinearUnitsGeoKey!==9001
    || keys.GeogSemiMajorAxisGeoKey!==6371007.181 || keys.GeogSemiMinorAxisGeoKey!==6371007.181
    || keys.ProjCenterLongGeoKey!==0 || keys.ProjFalseEastingGeoKey!==0 || keys.ProjFalseNorthingGeoKey!==0
    || !exactArray(tag(image,'ModelPixelScale'),MCD_SCALE) || !exactArray(tag(image,'ModelTiepoint'),MCD_TIEPOINT)
    || !exactArray(image.getOrigin(),MCD_TIEPOINT.slice(3))
    || !exactArray(image.getResolution(),[MCD_SCALE[0],-MCD_SCALE[1],0])) throw new Error(GRID_ERROR);
}

function validateMcdLevels(images,masks,policy) {
  if (masks.length || images.length!==4) throw new Error(GRID_ERROR);
  const bits=policy.assetKey==='Burn_Date'?16:8, format=policy.assetKey==='Burn_Date'?2:1;
  const nodata=policy.assetKey==='Burn_Date'?-1:null;
  for (const [index,image] of images.entries()) {
    const edge=2400/(2**index);
    if (image.getWidth()!==edge || image.getHeight()!==edge || image.getSamplesPerPixel()!==1
      || image.getTileWidth()!==512 || image.getTileHeight()!==512
      || !exactArray(tag(image,'BitsPerSample'),[bits]) || !exactArray(tag(image,'SampleFormat'),[format])
      || image.getGDALNoData()!==nodata || (tag(image,'NewSubfileType')??0)!==(index===0?0:1)
      || tag(image,'Compression')!==8 || tag(image,'PhotometricInterpretation')!==1
      || tag(image,'PlanarConfiguration')!==1 || tag(image,'Predictor')!==1
      || (tag(image,'Orientation')??1)!==1 || tag(image,'ExtraSamples')!=null
      || tag(image,'ColorMap')!=null || tag(image,'ModelTransformation')!=null
      || (policy.assetKey==='Burn_Date'?tag(image,'GDAL_NODATA')!=='-1\0':tag(image,'GDAL_NODATA')!=null)) throw new Error(GRID_ERROR);
    // These exact published overviews inherit the main grid. An unreviewed
    // overview grid or mask cannot be inserted into this selected-band policy.
    if (index>0 && (image.getGeoKeys()!=null || tag(image,'GeoKeyDirectory')!=null
      || tag(image,'GeoDoubleParams')!=null || tag(image,'GeoAsciiParams')!=null
      || tag(image,'ModelPixelScale')!=null || tag(image,'ModelTiepoint')!=null)) throw new Error(GRID_ERROR);
  }
}

function imageCrs(image,overviewPolicy) {
  if (isMcdOverviewPolicy(overviewPolicy)) { validateMcdCrs(image); return overviewPolicy.crs; }
  const keys = image.getGeoKeys();
  const code = keys?.GTModelTypeGeoKey === 2 ? keys.GeographicTypeGeoKey : keys?.GTModelTypeGeoKey === 1 ? keys.ProjectedCSTypeGeoKey : null;
  const crs = `EPSG:${code}`;
  if (![4326, 4269, 3857, 6933].includes(code) && !/^EPSG:(326|327|269)(0[1-9]|[1-5][0-9]|60)$/.test(crs)) throw new Error(GRID_ERROR);
  if ([4326, 4269].includes(code) !== (keys.GTModelTypeGeoKey === 2)) throw new Error(GRID_ERROR);
  if (keys.GTModelTypeGeoKey === 2 && keys.GeogAngularUnitsGeoKey != null && keys.GeogAngularUnitsGeoKey !== 9102
    || keys.GTModelTypeGeoKey === 1 && keys.ProjLinearUnitsGeoKey != null && keys.ProjLinearUnitsGeoKey !== 9001) throw new Error(GRID_ERROR);
  try { genericProjectionDefinition(crs); } catch { throw new Error(GRID_ERROR); }
  return crs;
}

export function validateProductRasterImages(sources,overviewPolicy) {
  if (sources.length !== 1 || !sources[0]?.length || sources[0].length > 64) throw new Error(GRID_ERROR);
  const images = sources[0].filter(image => !mask(image)).sort((a, b) => b.getWidth() - a.getWidth());
  const masks = sources[0].filter(mask).sort((a, b) => b.getWidth() - a.getWidth());
  const image = images[0]; if (!image) throw new Error(GRID_ERROR);
  const mcdPolicy=isMcdOverviewPolicy(overviewPolicy)?overviewPolicy:undefined;
  if (overviewPolicy!==undefined && overviewPolicy!=='srtm15plus-02-10-v255' && !mcdPolicy) throw new Error(GRID_ERROR);
  if (mcdPolicy) validateMcdLevels(images,masks,mcdPolicy);
  const crs = imageCrs(image,mcdPolicy), width = image.getWidth(), height = image.getHeight(), samples = image.getSamplesPerPixel();
  const type = image.getGeoKeys().GTRasterTypeGeoKey ?? 1;
  if (![1, 2].includes(type) || !Number.isSafeInteger(width) || !Number.isSafeInteger(height)
    || width < 1 || height < 1 || width > 131072 || height > 131072 || width * height > 8_000_000_000
    || !Number.isInteger(samples) || samples < 1 || samples > 16 || tag(image, 'ModelTransformation')) throw new Error(GRID_ERROR);
  const origin = image.getOrigin(), resolution = image.getResolution();
  if (![origin?.[0], origin?.[1], resolution?.[0], resolution?.[1]].every(Number.isFinite)
    || resolution[0] <= 0 || resolution[1] >= 0) throw new Error(GRID_ERROR);
  const west = origin[0] - (type === 2 ? resolution[0] / 2 : 0), north = origin[1] - (type === 2 ? resolution[1] / 2 : 0);
  const bounds = [west, north + height * resolution[1], west + width * resolution[0], north];
  if (!bounds.every(Number.isFinite) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3]) throw new Error(GRID_ERROR);
  if (masks.length && masks.length !== images.length) throw new Error('The TIFF validity masks do not match its image levels.');
  for (const [index, level] of images.entries()) {
    const bits = Array.from(tag(level, 'BitsPerSample') || []), formats = Array.from(tag(level, 'SampleFormat') || [1]);
    const nodata = level.getGDALNoData();
    if (!Number.isSafeInteger(level.getWidth()) || !Number.isSafeInteger(level.getHeight())
      || level.getWidth() < 1 || level.getHeight() < 1 || level.getWidth() > width || level.getHeight() > height) throw new Error(GRID_ERROR);
    if (level.getSamplesPerPixel() !== samples || tag(level, 'ModelTransformation')
      || ![0, 1, 2, 3].includes(tag(level, 'PhotometricInterpretation') ?? 1)
      || (tag(level, 'Orientation') ?? 1) !== 1 || (tag(level, 'PlanarConfiguration') ?? 1) !== 1
      || ![1, samples].includes(bits.length) || ![1, samples].includes(formats.length)
      || bits.some((bit, band) => !([1, 2].includes(formats[Math.min(band, formats.length - 1)]) ? [8, 16, 32] : formats[Math.min(band, formats.length - 1)] === 3 ? [32, 64] : []).includes(bit))
      || nodata !== null && typeof nodata !== 'number') throw new Error(GRID_ERROR);
    const tileBytes = level.getTileWidth() * level.getTileHeight() * samples * Math.max(...bits) / 8;
    if (!Number.isFinite(tileBytes) || tileBytes <= 0 || tileBytes > 64 * 1024 * 1024) throw new Error('The original TIFF tiles exceed the bounded preview decoding budget.');
    const keys = level.getGeoKeys();
    if ((keys?.ProjectedCSTypeGeoKey || keys?.GeographicTypeGeoKey) && imageCrs(level,mcdPolicy) !== crs
      || keys?.GTRasterTypeGeoKey != null && keys.GTRasterTypeGeoKey !== type) throw new Error(GRID_ERROR);
    // Embedded overviews may omit georeferencing and inherit the main grid.
    // If they declare their own grid, it must cover those same outer edges.
    if (index > 0 && (tag(level, 'ModelPixelScale') || tag(level, 'ModelTiepoint'))) {
      const point = level.getOrigin(), scale = level.getResolution(image);
      const edgeX = point[0] - (type === 2 ? scale[0] / 2 : 0), edgeY = point[1] - (type === 2 ? scale[1] / 2 : 0);
      const actual = [edgeX, edgeY + level.getHeight() * scale[1], edgeX + level.getWidth() * scale[0], edgeY];
      if (!actual.every(Number.isFinite) || scale[0] <= 0 || scale[1] >= 0
        || actual.some((value, side) => Math.abs(value - bounds[side]) > Math.max(Math.abs(scale[0]), Math.abs(scale[1])))) throw new Error(GRID_ERROR);
    }
    if (masks.length && (masks[index].getWidth() !== level.getWidth() || masks[index].getHeight() !== level.getHeight()
      || masks[index].getSamplesPerPixel() !== 1)) throw new Error('The TIFF validity masks do not match its image levels.');
  }
  // Register only known local projections; never fetch a guessed EPSG definition.
  const definition = genericProjectionDefinition(crs);
  if (definition) { proj4.defs(crs, definition); register(proj4); }
  return { crs, width, height, bands: samples, bounds,
    transform: [resolution[0], 0, west, 0, resolution[1], north], pixelInterpretation: type === 2 ? 'point' : 'area',
    displayBand: 1, displayMode: 'raw-first-band-grayscale', nodata: image.getGDALNoData(),
    hasMask: masks.length > 0, maskApplied: masks.length > 0, hasPalette: (tag(image, 'PhotometricInterpretation') ?? 1) === 3,
    pointTransform: type === 2 ? [1, 0, 0, 1, resolution[0] / 2, resolution[1] / 2] : undefined,
    ...(mcdPolicy?{overviewPolicy:mcdPolicy.kind,previewVerification:'selected-source-ranges-and-physical-grid',wholeFileVerified:false}:{} ) };
}

export function productRasterGrayStyle(hasAlpha = false) {
  // normalize:true creates Uint8 tiles; WebGL samples those textures at 0..1.
  // OL's color operator takes RGB at 0..255 but alpha already at 0..1.
  const gray = ['*', ['band', 1], 255];
  return { color: ['color', gray, gray, gray, hasAlpha ? ['band', 2] : 1] };
}

class ProductRasterSource extends GeoTIFF {
  constructor(url, displayRange, overviewPolicy) {
    const controller = new AbortController();
    const mcdPolicy=isMcdOverviewPolicy(overviewPolicy)?overviewPolicy:undefined;
    if (mcdPolicy) { proj4.defs(mcdPolicy.crs,genericProjectionDefinition(mcdPolicy.crs)); register(proj4); }
    let currentSource, readError;
    const read = createProductRasterLoader(url, controller.signal, undefined, progress => {
      if (currentSource && !currentSource.previewDisposed && !controller.signal.aborted)
        currentSource.dispatchEvent({ type: 'rasterreadprogress', ...progress });
    },mcdPolicy?.bytes);
    const loader = async (...args) => {
      try { return await read(...args); }
      catch (error) {
        // BlockedSource can replace the real loader error with [undefined].
        // Keep only a known safe explanation for this source's first failure.
        const safe = safeError(error); if (!readError && safe.message !== PREVIEW_ERROR) readError = safe;
        throw error;
      }
    };
    super({ sources: [{ url, bands: [1], ...(displayRange ? {min:displayRange[0],max:displayRange[1]} : {}), loader }],
      sourceOptions: SOURCE_OPTIONS, normalize: true, convertToRGB: false, interpolate: false, wrapX: false,
      ...(mcdPolicy?{projection:mcdPolicy.crs}:{} ) });
    this.controller = controller; this.rawMetadata = null; this.previewDisposed = false; this.displayRange = displayRange; this.overviewPolicy = overviewPolicy;
    currentSource = this;
    this.on('change', () => {
      if (this.getState() === 'error') { this.error_ = safeError(this.error_, readError); this.viewRejector(this.error_); }
    });
  }
  async configure_(sources) {
    try {
      if (this.previewDisposed) throw cancelled();
      this.rawMetadata = validateProductRasterImages(sources,this.overviewPolicy);
      if (this.displayRange) this.rawMetadata.displayRange = this.displayRange.slice();
      // OL compares raw samples in JS before producing its alpha channel.
      // Decimal TIFF NoData must be compared at the declared sample precision.
      const levels = sources[0].filter(image => !mask(image));
      const nodata = this.rawMetadata.nodata;
      const storedNoData = nodata === null ? null : Math.fround(nodata);
      if (storedNoData !== null && (!Number.isFinite(nodata) || Number.isFinite(storedNoData))
        && levels.every(image => tag(image, 'BitsPerSample')?.[0] === 32
          && tag(image, 'SampleFormat')?.[0] === 3 && image.getGDALNoData() !== null
          && Object.is(Math.fround(image.getGDALNoData()), storedNoData))) {
        this.sourceInfo_[0].nodata = storedNoData;
      }
      let displaySources=sources;
      if(this.overviewPolicy==='srtm15plus-02-10-v255') {
        const original=levels.find(image=>image.getWidth()===7200 && image.getHeight()===7200);
        if(!original || this.rawMetadata.crs!=='EPSG:4326' || this.rawMetadata.bands!==1
          || !Number.isNaN(nodata) || tag(original,'BitsPerSample')?.[0]!==32
          || tag(original,'SampleFormat')?.[0]!==3 || this.rawMetadata.hasMask) throw new Error(GRID_ERROR);
        const finer=sources[0].filter(image=>image.getWidth()>=3600 && image.getHeight()>=3600);
        if(!finer.some(image=>image.getWidth()===3600 && image.getHeight()===3600)) throw new Error(GRID_ERROR);
        displaySources=[finer];
        this.rawMetadata.overviewPolicy=this.overviewPolicy;
      }
      await super.configure_(displaySources);
      if (this.previewDisposed) throw cancelled();
    } catch (error) {
      this.error_ = safeError(error);
      this.setState('error'); this.viewRejector(this.error_); this.dispatchEvent('error');
    }
  }
  determineTransformMatrix(sources) { super.determineTransformMatrix(sources); this.transformMatrix = this.rawMetadata?.pointTransform; }
  disposeInternal() { this.previewDisposed = true; this.controller.abort(); super.disposeInternal(); }
}

export function createProductRasterPreviewSource({ snapshotId, assetKey, displayRange, overviewPolicy }) {
  if (displayRange !== undefined && (!Array.isArray(displayRange) || displayRange.length!==2
    || !displayRange.every(Number.isFinite) || displayRange[0]>=displayRange[1])) throw new Error(GRID_ERROR);
  if(overviewPolicy!==undefined && overviewPolicy!=='srtm15plus-02-10-v255'
    && !isMcdOverviewPolicy(overviewPolicy,{snapshotId,assetKey})) throw new Error(GRID_ERROR);
  const source = new ProductRasterSource(productRasterPreviewURL(snapshotId, assetKey),displayRange,overviewPolicy);
  return { source, style: productRasterGrayStyle(), getStyle: () => productRasterGrayStyle(source.hasAlpha),
    getMetadata: () => source.rawMetadata, dispose: () => source.dispose() };
}

export function hasProductRasterTilePixels(event, { hasAlpha = false, bandCount } = {}) {
  const data = event?.tile?.getData?.(), size = event?.tile?.getSize?.();
  if (!ArrayBuffer.isView(data) || !Number.isInteger(data.length) || data.length === 0
    || size && !(size.length === 2 && size.every(value => Number.isInteger(value) && value > 0))) return false;
  if (Number.isInteger(bandCount) && size && data.length !== size[0] * size[1] * bandCount) return false;
  if (!hasAlpha) return true;
  if (!Number.isInteger(bandCount) || bandCount < 2 || data.length % bandCount) return false;
  // Filled outside-window tiles and completely masked/NoData tiles can contain
  // decoded array bytes. They are not evidence of any visible original pixel.
  for (let index = bandCount - 1; index < data.length; index += bandCount) if (data[index] > 0) return true;
  return false;
}
