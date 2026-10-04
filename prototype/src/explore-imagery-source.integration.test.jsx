import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import fixture from '../public/samples/landsat-response.json';
import Projection from 'ol/proj/Projection.js';
import { addProjection } from 'ol/proj.js';
import { normalizeScene } from './catalog.js';
import { prepareAssetAccess } from './providers.js';
import { imageryHrefs } from './explore-imagery.js';
import { createImagerySource } from './explore-imagery-source.js';

const loader = vi.hoisted(() => vi.fn());
vi.mock('geotiff', async importOriginal => ({ ...await importOriginal(), fromUrl: loader }));
let scene;
beforeEach(async () => {
  loader.mockReset();
  scene = normalizeScene(fixture.features[0], 'planetary-landsat');
  addProjection(new Projection({ code: scene.crs, units: 'm' }));
  await prepareAssetAccess(imageryHrefs(scene), { force: true, fetcher: async () => ({ ok: true,
    json: async () => ({ token: 'sp=r&sr=c&sig=test-only', 'msft:expiry': new Date(Date.now() + 3_600_000).toISOString() }) }) });
});
afterEach(() => vi.restoreAllMocks());

// Mock only COG transport/image metadata. OpenLayers' real configure_, tile
// grid, transform direction and view promise run from the pinned dependency.
function image(type = 2, width = scene.grid.shape[1]) {
  const height = scene.grid.shape[0], x = scene.grid.transform[2] + (type === 2 ? 15 : 0), y = scene.grid.transform[5] - (type === 2 ? 15 : 0);
  return {
    getWidth: () => width, getHeight: () => height, getSamplesPerPixel: () => 1,
    getTileWidth: () => 256, getTileHeight: () => 256, getGDALNoData: () => 0, getGDALMetadata: () => null,
    getGeoKeys: () => ({ GTRasterTypeGeoKey: type, ProjectedCSTypeGeoKey: 32610, ProjLinearUnitsGeoKey: 9001 }),
    fileDirectory: { getValue: key => ({ BitsPerSample: [16], SampleFormat: [1] })[key] },
    getOrigin: () => [x, y, 0], getResolution: () => [30, -30, 0],
    getBoundingBox: () => [x, y - height * 30, x + width * 30, y],
  };
}
function serve(images) {
  let index = 0;
  loader.mockImplementation(async () => ({ getImageCount: async () => 1, getImage: async () => images[index++] }));
}
it.each([1, 2])('OpenLayers keeps raw RGB channels and returns outer pixel edges for raster interpretation %s', async type => {
  serve([image(type), image(type), image(type)]);
  const { source } = createImagerySource(scene);
  try {
    const view = await source.getView(), [height, width] = scene.grid.shape, t = scene.grid.transform;
    expect(view.extent).toEqual([t[2], t[5] - height * 30, t[2] + width * 30, t[5]]);
    expect(source.bandCount).toBe(4); expect(source.normalize_).toBe(false); expect(source.getState()).toBe('ready');
    expect(loader).toHaveBeenCalledTimes(3);
  } finally { source.dispose(); }
});
it('an async metadata mismatch rejects the view instead of leaving map loading unresolved', async () => {
  serve([image(), image(2, 1), image()]);
  const { source } = createImagerySource(scene);
  try { await expect(source.getView()).rejects.toThrow('Landsat COG metadata does not match'); expect(source.getState()).toBe('error'); expect(source.getError().message).not.toContain('sig='); }
  finally { source.dispose(); }
});
it('an initial COG transport failure rejects the view with a message free of signed URLs', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  loader.mockRejectedValue(new Error('HTTP failure ?sig=test-only'));
  const { source } = createImagerySource(scene);
  try { await expect(source.getView()).rejects.toThrow('Landsat COG metadata does not match'); expect(source.getState()).toBe('error'); expect(source.getError().message).not.toContain('sig='); }
  finally { source.dispose(); }
});
