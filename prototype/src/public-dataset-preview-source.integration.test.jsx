import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import WebGLTileLayer from 'ol/layer/WebGLTile.js';
import { createProductRasterLoader, createProductRasterPreviewSource, hasProductRasterTilePixels,
  productRasterPreviewURL, PRODUCT_PREVIEW_RANGE_LIMIT, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT, validateProductRasterImages } from './public-dataset-preview-source.js';

const transport = vi.hoisted(() => vi.fn());
vi.mock('geotiff', async importOriginal => ({ ...await importOriginal(), fromCustomClient: transport }));
const pin = 'a'.repeat(64);
beforeEach(() => { transport.mockReset(); delete window.__TAURI__; });
afterEach(() => { delete window.__TAURI__; vi.restoreAllMocks(); });

function image({ type = 1, crs = 4326, width = 512, height = 256, nodata = -9999, samples = 1,
  dx = 0.01, dy = -0.01, x = 115, y = 41, tags = {}, keys = {} } = {}) {
  return { getWidth: () => width, getHeight: () => height, getSamplesPerPixel: () => samples,
    getTileWidth: () => 256, getTileHeight: () => 256, getBytesPerPixel: () => samples * 2,
    getGDALNoData: () => nodata, getGDALMetadata: () => null,
    getGeoKeys: () => ({ GTRasterTypeGeoKey: type,
      ...([4326, 4269].includes(crs) ? { GTModelTypeGeoKey: 2, GeographicTypeGeoKey: crs, GeogAngularUnitsGeoKey: 9102 }
        : { GTModelTypeGeoKey: 1, ProjectedCSTypeGeoKey: crs, ProjLinearUnitsGeoKey: 9001 }), ...keys }),
    fileDirectory: { getValue: key => ({ BitsPerSample: Array(samples).fill(16), SampleFormat: [2],
      PhotometricInterpretation: 1, ...tags })[key] },
    getOrigin: () => [x, y, 0], getResolution: () => [dx, dy, 0],
    getBoundingBox: () => [x, y + height * dy, x + width * dx, y],
  };
}
function serve(images) {
  transport.mockResolvedValue({ getImageCount: async () => images.length, getImage: async index => images[index] });
}
function receipt(bytes, range = `bytes 0-${bytes.length - 1}/${bytes.length}`, status = 206, headers = {}) {
  return new Response(bytes, { status, headers: { 'Content-Range': range, ...headers } });
}
it('the reviewed SRTM15 display retains its original grid and reads only the verified finer levels',async()=>{
  const levels=[1,2,4,8,16].map(factor=>image({width:7200/factor,height:7200/factor,
    nodata:NaN,dx:factor/240,dy:-factor/240,x:89.9625,y:60.00416666666666,
    tags:{BitsPerSample:[32],SampleFormat:[3],NewSubfileType:factor===1?0:1}}));
  serve(levels);
  const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:'raster',displayRange:[-8000,4000],overviewPolicy:'srtm15plus-02-10-v255'});
  try {
    await preview.source.getView();
    expect(preview.getMetadata()).toMatchObject({width:7200,height:7200,crs:'EPSG:4326',overviewPolicy:'srtm15plus-02-10-v255'});
    expect(Number.isNaN(preview.getMetadata().nodata)).toBe(true);
    expect(preview.source.getTileGrid().getResolutions()).toHaveLength(2);
  } finally {preview.dispose();}
  serve(levels);
  const unscoped=createProductRasterPreviewSource({snapshotId:pin,assetKey:'raster'});
  try {await unscoped.source.getView();expect(unscoped.source.getTileGrid().getResolutions()).toHaveLength(5);}
  finally{unscoped.dispose();}
});

it('a reviewed overview policy fails closed on a different original grid or TIFF NoData',async()=>{
  expect(()=>createProductRasterPreviewSource({snapshotId:pin,assetKey:'raster',overviewPolicy:'anything'})).toThrow();
  for(const nodata of [-9999,null]) {
    serve([image({width:7200,height:7200,nodata,tags:{BitsPerSample:[32],SampleFormat:[3]}}),
      image({width:3600,height:3600,nodata,dx:0.02,dy:-0.02,tags:{BitsPerSample:[32],SampleFormat:[3]}})]);
    const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:'raster',overviewPolicy:'srtm15plus-02-10-v255'});
    try{await expect(preview.source.getView()).rejects.toThrow();}finally{preview.dispose();}
  }
});
function loader(fetcher) { return createProductRasterLoader('http://local/asset', new AbortController().signal, fetcher); }

it('pins web and desktop URLs without exposing the publisher URL', () => {
  expect(productRasterPreviewURL(pin, 'item/band 1')).toBe(`http://127.0.0.1:4318/stac/snapshots/${pin}/preview?assetKey=item%2Fband+1`);
  window.__TAURI__ = { core: { convertFileSrc: vi.fn((path, protocol) => `${protocol}://${path}`) } };
  expect(productRasterPreviewURL(pin, 'item/band 1')).toBe(`geod-stac-preview://${pin}/item%2Fband%201`);
  expect(window.__TAURI__.core.convertFileSrc).toHaveBeenCalledWith(`${pin}/item%2Fband%201`, 'geod-stac-preview');
  for (const [snapshot, key] of [['../bad', 'x'], [pin, ''], [pin, ' leading'], [pin, 'a\nb']]) {
    expect(() => productRasterPreviewURL(snapshot, key)).toThrow('Choose a saved original');
  }
});

// Only transport/image metadata is replaced. OpenLayers 10.10's grid, alpha,
// PixelIsPoint transform and asynchronous view configuration execute unchanged.
it.each([1, 2])('uses the actual TIFF grid and half-pixel edges for raster interpretation %s', async type => {
  serve([image({ type, samples: 3 })]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    const view = await preview.source.getView(), correction = type === 2 ? 0.005 : 0;
    expect(view.extent).toEqual([115 - correction, 41 - 2.56 + correction, 115 + 5.12 - correction, 41 + correction]);
    expect(preview.getMetadata()).toMatchObject({ crs: 'EPSG:4326', bands: 3, displayBand: 1,
      displayMode: 'raw-first-band-grayscale', nodata: -9999, pixelInterpretation: type === 2 ? 'point' : 'area' });
    expect(preview.source.bandCount).toBe(2); expect(preview.source.normalize_).toBe(true);
    expect(transport.mock.calls[0][1]).toMatchObject({ allowFullFile: false, blockSize: 65536, cacheSize: 8, maxRanges: 0 });
    expect(preview.source.sourceInfo_[0].bands).toEqual([1]);
  } finally { preview.dispose(); }
});

function shadeNormalizedTexture(preview, bytes, pixelIndex) {
  const layer = new WebGLTileLayer({ source: preview.source, style: preview.getStyle() });
  try {
    // Use the pinned OL layer's actual GLSL compiler, rather than interpreting
    // the style array ourselves. Uint8 texture samples are normalized by WebGL.
    const shader = layer.parseStyleForRender_().fragmentShader;
    const expression = /\n\s+color = ([^;]+);/.exec(shader)?.[1];
    expect(expression).toBeTruthy();
    const bandCount = preview.source.bandCount;
    const rgba = new Function('getBandValue', 'vec4', `return ${expression};`)(
      band => bytes[pixelIndex * bandCount + band - 1] / 255, (...components) => components);
    // The real fragment shader premultiplies RGB by alpha before blending.
    return [rgba[0] * rgba[3], rgba[1] * rgba[3], rgba[2] * rgba[3], rgba[3]];
  } finally { layer.setSource(null); layer.dispose(); }
}

it('renders normalized Uint8 black, middle gray and white at full opacity through the real OL GLSL compiler', async () => {
  serve([image({ nodata: null, tags: { BitsPerSample: [8], SampleFormat: [1] } })]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await preview.source.getView();
    const bytes = preview.source.composeTile_([3, 1], [[new Uint8Array([0, 128, 255])], null]);
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(shadeNormalizedTexture(preview, bytes, 0)).toEqual([0, 0, 0, 1]);
    expect(shadeNormalizedTexture(preview, bytes, 1)).toEqual([128 / 255, 128 / 255, 128 / 255, 1]);
    expect(shadeNormalizedTexture(preview, bytes, 2)).toEqual([1, 1, 1, 1]);
  } finally { preview.dispose(); }
});

it('renders the narrow FNF class stream across grayscale while keeping class-zero transparency and raw samples intact',async()=>{
  serve([image({nodata:0,tags:{BitsPerSample:[8],SampleFormat:[1]}})]);
  const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:'FNF',displayRange:[1,4]});
  try {
    await preview.source.getView();
    const raw=new Uint8Array([0,1,2,3,4]);
    const bytes=preview.source.composeTile_([5,1],[[raw],null]);
    expect([...raw]).toEqual([0,1,2,3,4]);
    expect(preview.getMetadata().displayRange).toEqual([1,4]);
    expect(shadeNormalizedTexture(preview,bytes,0)).toEqual([0,0,0,0]);
    expect(shadeNormalizedTexture(preview,bytes,3)).toEqual([170/255,170/255,170/255,1]);
    expect(shadeNormalizedTexture(preview,bytes,4)).toEqual([1,1,1,1]);
  } finally {preview.dispose();}
});

it.each(['NoData', 'TIFF mask'])('renders normalized grayscale visibly while preserving %s transparency in the real OL shader', async validity => {
  const tags = { BitsPerSample: [8], SampleFormat: [1] };
  const images = [image({ nodata: validity === 'NoData' ? 0 : null, tags })];
  if (validity === 'TIFF mask') images.push(image({ nodata: null, tags: { NewSubfileType: 4, BitsPerSample: [1], SampleFormat: [1] } }));
  serve(images);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await preview.source.getView();
    const bytes = preview.source.composeTile_([3, 1], [[new Uint8Array([0, 128, 255])],
      validity === 'TIFF mask' ? [new Uint8Array([0, 1, 1])] : null]);
    expect(preview.source.hasAlpha).toBe(true);
    expect(shadeNormalizedTexture(preview, bytes, 0)).toEqual([0, 0, 0, 0]);
    expect(shadeNormalizedTexture(preview, bytes, 1)).toEqual([128 / 255, 128 / 255, 128 / 255, 1]);
    expect(shadeNormalizedTexture(preview, bytes, 2)).toEqual([1, 1, 1, 1]);
  } finally { preview.dispose(); }
});

it.each([4269, 3857, 6933, 32650, 32755, 26910])('registers the declared local projection EPSG:%s without EPSG lookups', async crs => {
  serve([image({ crs, x: 500000, y: 4000000, dx: 30, dy: -30 })]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try { const view = await preview.source.getView(); expect(view.projection.getCode()).toBe(`EPSG:${crs}`); }
  finally { preview.dispose(); }
});

it('retains NoData and actual TIFF validity masks in normalized preview alpha', async () => {
  serve([image({ nodata: null }), image({ nodata: null, tags: { NewSubfileType: 4, BitsPerSample: [1], SampleFormat: [1] } })]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await preview.source.getView();
    expect(preview.getMetadata()).toMatchObject({ nodata: null, hasMask: true, maskApplied: true });
    expect(preview.source.hasAlpha).toBe(true);
    const values = preview.source.composeTile_([2, 1], [[new Int16Array([100, 200])], [new Uint8Array([1, 0])]]);
    expect(values[1]).toBe(255); expect(values[3]).toBe(0);
  } finally { preview.dispose(); }
});

it.each([-9999, NaN, -999.9])('keeps scalar NoData %s transparent after real OpenLayers tile composition', async nodata => {
  serve([image({ nodata, tags: { BitsPerSample: [32], SampleFormat: [3] } })]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await preview.source.getView();
    const values = preview.source.composeTile_([2, 1], [[new Float32Array([nodata, 0.5])], null]);
    expect(values[1]).toBe(0); expect(values[3]).toBe(255);
    expect(preview.getMetadata().nodata).toBe(nodata);
  } finally { preview.dispose(); }
});

it.each([32, 64])('compares Float%s NoData precisely without masking adjacent samples or valid zero', async bits => {
  const nodata=-999.9;
  const original=image({nodata,tags:{BitsPerSample:[bits],SampleFormat:[3]}});
  serve([original]);
  const preview=createProductRasterPreviewSource({snapshotId:pin,assetKey:'raster'});
  try {
    await preview.source.getView();
    const neighbour=new Float32Array([nodata]);
    new Uint32Array(neighbour.buffer)[0]-=1;
    const raw=bits===32 ? new Float32Array([nodata,neighbour[0],0]) : new Float64Array([nodata,Math.fround(nodata),0]);
    const saved=Array.from(raw);
    const bytes=preview.source.composeTile_([3,1],[[raw],null]);
    expect([bytes[1],bytes[3],bytes[5]]).toEqual([0,255,255]);
    expect(Array.from(raw)).toEqual(saved);
    expect(preview.getMetadata().nodata).toBe(nodata);
    expect(original.getGDALNoData()).toBe(nodata);
  } finally {preview.dispose();}
});

it('rejects OpenLayers strip coalescing above 16 MiB instead of allowing a full-file or unbounded logical read', async () => {
  const strip = image({ width: 40000 }); strip.getTileWidth = () => 40000; strip.getTileHeight = () => 1;
  transport.mockImplementation(async (client, options) => {
    if (options.blockSize > PRODUCT_PREVIEW_RANGE_LIMIT) await client.request({ headers: { Range: `bytes=0-${options.blockSize - 1}` } });
    return { getImageCount: async () => 1, getImage: async () => strip };
  });
  vi.spyOn(console, 'error').mockImplementation(() => {});
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await expect(preview.source.getView()).rejects.toThrow('larger read than the preview limit');
    expect(transport).toHaveBeenCalledTimes(2);
    expect(transport.mock.calls[1][1].allowFullFile).toBe(false);
  } finally { preview.dispose(); }
});

function segmentReceipt(range, total, fill = 1, headers = {}) {
  const [, first, end] = /^bytes=(\d+)-(\d+)$/.exec(range), start = Number(first), last = Math.min(Number(end), total - 1);
  const bytes = new Uint8Array(last - start + 1).fill(fill);
  return receipt(bytes, `bytes ${start}-${last}/${total}`, 206, { 'Content-Length': String(bytes.length), ...headers });
}

it('allows the real WPM strip block size through OL while keeping every network request below 4 MiB', async () => {
  const strip = image({ width: 14269, height: 14657, crs: 32721 }); strip.getTileWidth = () => 14269; strip.getTileHeight = () => 1;
  const fetcher = vi.spyOn(globalThis, 'fetch').mockImplementation(async (_url, options) => segmentReceipt(options.headers.Range, 174662589));
  transport.mockImplementation(async (client, options) => {
    if (options.blockSize > PRODUCT_PREVIEW_RANGE_LIMIT) {
      const response = await client.request({ headers: { Range: `bytes=0-${options.blockSize - 1}` } });
      expect((await response.getData()).byteLength).toBe(7305728);
    }
    return { getImageCount: async () => 1, getImage: async () => strip };
  });
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  const progress = vi.fn(); preview.source.on('rasterreadprogress', progress);
  try {
    await preview.source.getView();
    expect(transport).toHaveBeenCalledTimes(2);
    expect(transport.mock.calls[1][1]).toMatchObject({ blockSize: 7305728, allowFullFile: false });
    expect(fetcher.mock.calls.map(([, options]) => options.headers.Range)).toEqual(['bytes=0-4194303', 'bytes=4194304-7305727']);
    expect(preview.getMetadata()).toMatchObject({ crs: 'EPSG:32721', width: 14269, height: 14657 });
    expect(progress).toHaveBeenCalledTimes(2);
    expect(progress.mock.calls.map(([event]) => [event.start, event.end, event.bytes])).toEqual([[0, 4194303, 4194304], [4194304, 7305727, 3111424]]);
  } finally { preview.dispose(); }
});

it('retains a safe loader budget error when BlockedSource discards the underlying error', async () => {
  transport.mockImplementation(async client => {
    try { await client.request({ headers: { Range: `bytes=0-${PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT}` } }); }
    catch { throw new AggregateError([undefined], 'Request failed'); }
  });
  vi.spyOn(console, 'error').mockImplementation(() => {});
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try { await expect(preview.source.getView()).rejects.toThrow('larger read than the preview limit'); }
  finally { preview.dispose(); }
});

it.each([
  ['unsupported CRS', { crs: 3413 }], ['contradictory geographic units', { keys: { GeogAngularUnitsGeoKey: 9101 } }],
  ['sheared transform', { tags: { ModelTransformation: [1, 0.1, 0, 0] } }],
  ['westward grid', { dx: -1 }], ['south-up grid', { dy: 1 }],
  ['unsafe integer samples', { tags: { BitsPerSample: [64], SampleFormat: [1] } }],
  ['JPEG color interpretation', { tags: { PhotometricInterpretation: 6 } }], ['invalid orientation', { tags: { Orientation: 3 } }],
])('rejects %s instead of using catalog bounds as a raster grid', async (_label, options) => {
  serve([image(options)]);
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try { await expect(preview.source.getView()).rejects.toThrow('supported north-up'); expect(preview.source.getState()).toBe('error'); }
  finally { preview.dispose(); }
});

it('rejects mismatched masks and unsupported huge raster tiles', () => {
  expect(() => validateProductRasterImages([[image(), image({ width: 1, tags: { NewSubfileType: 4 } })]])).toThrow('validity masks');
  const huge = image(); huge.getTileWidth = () => 65536; huge.getTileHeight = () => 1024;
  expect(() => validateProductRasterImages([[huge]])).toThrow('decoding budget');
});

it('rejects initial transport failures promptly without returning upstream error details', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  transport.mockRejectedValue(new Error('upstream failed https://publisher/?sig=private'));
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await expect(preview.source.getView()).rejects.toThrow('could not be read');
    expect(preview.source.getError().message).not.toContain('private');
  } finally { preview.dispose(); }
});

it.each(['errors', 'cause'])('extracts only allowlisted explanations from nested %s without exposing upstream details', async kind => {
  const safe = new Error('The preview byte range is incomplete.');
  const nested = kind === 'errors' ? new AggregateError([new Error('https://publisher/?sig=private'), new AggregateError([safe], 'upstream')], 'Request failed')
    : new Error('https://publisher/?sig=private', { cause: new Error('Request failed', { cause: safe }) });
  transport.mockRejectedValue(nested); vi.spyOn(console, 'error').mockImplementation(() => {});
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try {
    await expect(preview.source.getView()).rejects.toThrow('The preview byte range is incomplete.');
    expect(preview.source.getError().message).not.toContain('private');
  } finally { preview.dispose(); }
});

it('bounds recursive error traversal for cycles and excessive nesting', async () => {
  const cycle = new Error('private'); cycle.cause = cycle;
  let deep = new Error('The preview byte range is incomplete.');
  for (let depth = 0; depth < 12; depth++) deep = new Error('private', { cause: deep });
  transport.mockRejectedValue(new AggregateError([cycle, deep], 'private')); vi.spyOn(console, 'error').mockImplementation(() => {});
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  try { await expect(preview.source.getView()).rejects.toThrow('could not be read'); }
  finally { preview.dispose(); }
});

it('accepts an exact bounded 206 range and a shorter final EOF block', async () => {
  const fetcher = vi.fn(async () => receipt(new Uint8Array([1, 2, 3]), 'bytes 2-4/5'));
  const response = await loader(fetcher)('http://local/asset', { Range: 'bytes=2-9' });
  expect([...new Uint8Array(await response.arrayBuffer())]).toEqual([1, 2, 3]);
  expect(fetcher.mock.calls[0][1]).toMatchObject({ credentials: 'omit', redirect: 'error', headers: { Range: 'bytes=2-9' } });
});

it.each([undefined, 'bytes=0-', 'bytes=0-2,4-5', `bytes=0-${PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT}`])('rejects unbounded or oversized logical range %s before fetching', async Range => {
  const fetcher = vi.fn();
  await expect(loader(fetcher)('http://local/asset', Range ? { Range } : {})).rejects.toThrow('larger read');
  expect(fetcher).not.toHaveBeenCalled();
});

it('keeps a 4 MiB request as one exact native range', async () => {
  const fetcher = vi.fn(async (_url, options) => segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_RANGE_LIMIT));
  const response = await loader(fetcher)('http://local/asset', { Range: `bytes=0-${PRODUCT_PREVIEW_RANGE_LIMIT - 1}` });
  expect(fetcher).toHaveBeenCalledOnce();
  expect(fetcher.mock.calls[0][1].headers.Range).toBe('bytes=0-4194303');
  expect((await response.arrayBuffer()).byteLength).toBe(PRODUCT_PREVIEW_RANGE_LIMIT);
});

it('assembles the 7,305,728 byte WPM logical block from two complete bounded ranges', async () => {
  const fetcher = vi.fn(async (_url, options) => segmentReceipt(options.headers.Range, 174662589, fetcher.mock.calls.length));
  const response = await loader(fetcher)('http://local/asset', { Range: 'bytes=0-7305727' });
  expect(fetcher.mock.calls.map(([, options]) => options.headers.Range)).toEqual(['bytes=0-4194303', 'bytes=4194304-7305727']);
  expect(response.headers.get('Content-Range')).toBe('bytes 0-7305727/174662589');
  expect(response.headers.get('Content-Length')).toBe('7305728');
  const bytes = new Uint8Array(await response.arrayBuffer());
  expect(bytes.length).toBe(7305728);
  expect([bytes[0], bytes[4194303], bytes[4194304], bytes[7305727]]).toEqual([1, 1, 2, 2]);
});

it('reports real progress only after the first and final segment have passed exact byte validation', async () => {
  const total = PRODUCT_PREVIEW_RANGE_LIMIT + 3, progress = vi.fn();
  const fetcher = vi.fn(async (_url, options) => {
    expect(progress).toHaveBeenCalledTimes(fetcher.mock.calls.length - 1);
    return segmentReceipt(options.headers.Range, total);
  });
  const read = createProductRasterLoader('http://local/asset', new AbortController().signal, fetcher, progress);
  await read('http://local/asset', { Range: 'bytes=0-7305727' });
  expect(progress.mock.calls.map(([value]) => value)).toEqual([
    { start: 0, end: 4194303, total, bytes: 4194304, loadedBytes: 4194304 },
    { start: 4194304, end: 4194306, total, bytes: 3, loadedBytes: total },
  ]);
});

it('never reports an unvalidated second segment as completed progress', async () => {
  const progress = vi.fn(), fetcher = vi.fn(async (_url, options) => fetcher.mock.calls.length === 1
    ? segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT)
    : receipt(new Uint8Array([1]), 'bytes 4194304-8388607/16777216'));
  const read = createProductRasterLoader('http://local/asset', new AbortController().signal, fetcher, progress);
  await expect(read('http://local/asset', { Range: 'bytes=0-12582911' })).rejects.toThrow('incomplete');
  expect(progress).toHaveBeenCalledOnce();
  expect(progress.mock.calls[0][0]).toMatchObject({ start: 0, end: 4194303, bytes: 4194304 });
});

it('does not publish source read progress after disposal while an original range is pending', async () => {
  let finish, request;
  vi.spyOn(globalThis, 'fetch').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  transport.mockImplementation(async client => {
    request = client.request({ headers: { Range: 'bytes=0-1' } });
    await request;
    return { getImageCount: async () => 1, getImage: async () => image() };
  });
  vi.spyOn(console, 'error').mockImplementation(() => {});
  const preview = createProductRasterPreviewSource({ snapshotId: pin, assetKey: 'raster' });
  const progress = vi.fn(); preview.source.on('rasterreadprogress', progress);
  await vi.waitFor(() => expect(finish).toBeTypeOf('function'));
  const rejected = expect(request).rejects.toMatchObject({ name: 'AbortError' });
  preview.dispose(); finish(receipt(new Uint8Array([1, 2]))); await rejected;
  expect(progress).not.toHaveBeenCalled();
});

it('stops at a known final EOF, returning exactly the shortened assembled logical span', async () => {
  const total = PRODUCT_PREVIEW_RANGE_LIMIT + 3;
  const fetcher = vi.fn(async (_url, options) => segmentReceipt(options.headers.Range, total));
  const response = await loader(fetcher)('http://local/asset', { Range: 'bytes=0-7305727' });
  expect(fetcher.mock.calls.map(([, options]) => options.headers.Range)).toEqual(['bytes=0-4194303', 'bytes=4194304-4194306']);
  expect(response.headers.get('Content-Range')).toBe(`bytes 0-4194306/${total}`);
  expect((await response.arrayBuffer()).byteLength).toBe(total);
});

it('accepts a 16 MiB logical span without any network request above 4 MiB', async () => {
  const fetcher = vi.fn(async (_url, options) => segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT));
  const response = await loader(fetcher)('http://local/asset', { Range: `bytes=0-${PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT - 1}` });
  expect(fetcher).toHaveBeenCalledTimes(4);
  for (const [, options] of fetcher.mock.calls) {
    const [, a, b] = /^bytes=(\d+)-(\d+)$/.exec(options.headers.Range);
    expect(Number(b) - Number(a) + 1).toBeLessThanOrEqual(PRODUCT_PREVIEW_RANGE_LIMIT);
  }
  expect((await response.arrayBuffer()).byteLength).toBe(PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT);
});

it.each(['total', 'truncated', 'full-file', 'encoding', 'length', 'shifted'])('rejects a bad second segment (%s) without delivering partial bytes or requesting later segments', async kind => {
  const fetcher = vi.fn(async (_url, options) => {
    if (fetcher.mock.calls.length === 1) return segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT);
    if (kind === 'total') return segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT + 1);
    if (kind === 'full-file') return receipt(new Uint8Array([1]), 'bytes 4194304-8388607/16777216', 200);
    if (kind === 'encoding') return segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT, 1, { 'Content-Encoding': 'gzip' });
    if (kind === 'length') return segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT, 1, { 'Content-Length': '3' });
    if (kind === 'shifted') return receipt(new Uint8Array([1]), 'bytes 4194305-8388608/16777216');
    return receipt(new Uint8Array([1]), 'bytes 4194304-8388607/16777216', 206, { 'Content-Length': '4194304' });
  });
  await expect(loader(fetcher)('http://local/asset', { Range: 'bytes=0-12582911' })).rejects.toThrow(/original byte range|incomplete/);
  expect(fetcher).toHaveBeenCalledTimes(2);
});

it('stops before the next segment when cancellation arrives after reading the first', async () => {
  const controller = new AbortController();
  const fetcher = vi.fn(async (_url, options) => {
    const response = segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT);
    const getReader = response.body.getReader.bind(response.body);
    response.body.getReader = () => {
      const reader = getReader();
      return { read: async () => { const result = await reader.read(); if (result.done) controller.abort(); return result; },
        cancel: (...args) => reader.cancel(...args), releaseLock: () => reader.releaseLock() };
    };
    return response;
  });
  await expect(loader(fetcher)('http://local/asset', { Range: 'bytes=0-7305727' }, controller.signal)).rejects.toMatchObject({ name: 'AbortError' });
  expect(fetcher).toHaveBeenCalledOnce();
});

it('aborts an active second segment and does not start a third', async () => {
  const controller = new AbortController();
  const fetcher = vi.fn(async (_url, options) => {
    if (fetcher.mock.calls.length === 1) return segmentReceipt(options.headers.Range, PRODUCT_PREVIEW_LOGICAL_RANGE_LIMIT);
    return new Promise((_resolve, reject) => options.signal.addEventListener('abort', () => reject(new DOMException('cancelled', 'AbortError')), { once: true }));
  });
  const promise = loader(fetcher)('http://local/asset', { Range: 'bytes=0-12582911' }, controller.signal);
  const rejected = expect(promise).rejects.toMatchObject({ name: 'AbortError' });
  await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2)); controller.abort(); await rejected;
  expect(fetcher).toHaveBeenCalledTimes(2);
  await loader(async () => receipt(new Uint8Array([2])))('http://local/asset', { Range: 'bytes=0-0' });
});

it.each([
  [new Uint8Array([1, 2]), 'bytes 0-1/2', 200, {}, 'requested original byte range'],
  [new Uint8Array([1, 2]), 'bytes 1-2/3', 206, {}, 'requested original byte range'],
  [new Uint8Array([1, 2]), 'bytes 0-1/2', 206, { 'Content-Encoding': 'gzip' }, 'requested original byte range'],
  [new Uint8Array([1]), 'bytes 0-1/2', 206, {}, 'incomplete'],
  [new Uint8Array([1, 2, 3]), 'bytes 0-1/2', 206, {}, 'exceeds'],
  [new Uint8Array([1, 2]), 'bytes 0-1/2', 206, { 'Content-Length': '3' }, 'incomplete'],
])('rejects full-file, wrong range, compression and incomplete byte receipts (%#)', async (bytes, range, status, headers, error) => {
  await expect(loader(async () => receipt(bytes, range, status, headers))('http://local/asset', { Range: 'bytes=0-1' })).rejects.toThrow(error);
});

it('bounds concurrent transfers and removes cancelled queued work without leaking a slot', async () => {
  const pending = [], fetcher = vi.fn(() => new Promise(resolve => pending.push(resolve)));
  const a = loader(fetcher)('http://local/asset', { Range: 'bytes=0-0' });
  const b = loader(fetcher)('http://local/asset', { Range: 'bytes=0-0' });
  const controller = new AbortController();
  const c = loader(fetcher)('http://local/asset', { Range: 'bytes=0-0' }, controller.signal);
  const rejected = expect(c).rejects.toMatchObject({ name: 'AbortError' });
  await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2)); controller.abort(); await rejected;
  pending.forEach(resolve => resolve(receipt(new Uint8Array([1])))); await Promise.all([a, b]);
  await loader(async () => receipt(new Uint8Array([2])))('http://local/asset', { Range: 'bytes=0-0' });
});

it('source lifetime cancellation aborts an active request', async () => {
  const controller = new AbortController(), fetcher = vi.fn((_url, options) => new Promise((_resolve, reject) => {
    options.signal.addEventListener('abort', () => reject(new DOMException('cancelled', 'AbortError')), { once: true });
  }));
  const promise = createProductRasterLoader('http://local/asset', controller.signal, fetcher)('http://local/asset', { Range: 'bytes=0-0' });
  const rejected = expect(promise).rejects.toMatchObject({ name: 'AbortError' });
  await vi.waitFor(() => expect(fetcher).toHaveBeenCalledOnce()); controller.abort(); await rejected;
});

it('metadata and empty/outside tiles never count as pixels read', () => {
  expect(hasProductRasterTilePixels({})).toBe(false);
  expect(hasProductRasterTilePixels({ tile: { getData: () => new Uint8Array(0) } })).toBe(false);
  expect(hasProductRasterTilePixels({ tile: { getData: () => new Uint8Array(4), getSize: () => [0, 1] } })).toBe(false);
  expect(hasProductRasterTilePixels({ tile: { getData: () => new Uint8Array(4), getSize: () => [2, 1] } })).toBe(true);
});

it('requires a real visible sample when TIFF NoData or a mask supplies alpha', () => {
  const tile = data => ({ tile: { getData: () => new Uint8Array(data), getSize: () => [2, 1] } });
  const alpha = { hasAlpha: true, bandCount: 2 };
  expect(hasProductRasterTilePixels(tile([0, 0, 0, 0]), alpha)).toBe(false);
  expect(hasProductRasterTilePixels(tile([0, 255, 0, 0]), alpha)).toBe(true);
  expect(hasProductRasterTilePixels(tile([1, 255]), alpha)).toBe(false);
  expect(hasProductRasterTilePixels(tile([0, 0]), { hasAlpha: false, bandCount: 1 })).toBe(true);
});
