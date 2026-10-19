import React from 'react';
import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ExploreMap } from './explore-map.jsx';
import {usage} from './usage-client.js';

const { createPreview, maps, rasterLayers } = vi.hoisted(() => ({ createPreview: vi.fn(), maps: [], rasterLayers: [] }));
vi.mock('./public-dataset-preview-source.js', async importOriginal => ({ ...await importOriginal(), createProductRasterPreviewSource: createPreview }));
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (value, values = {}) => value.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? '') }) }));
vi.mock('ol/Map.js', () => ({ default: class {
  constructor(options) { this.view = options.view; this.layers = [...options.layers]; maps.push(this); }
  on() {} getSize() { return [1000, 600]; } getView() { return this.view; } updateSize() {} setTarget() {} dispose() {}
  addLayer(layer) { this.layers.push(layer); }
  removeLayer(layer) { this.layers = this.layers.filter(value => value !== layer); }
} }));
vi.mock('ol/layer/WebGLTile.js', () => ({ default: class {
  constructor(options) { Object.assign(this, options); rasterLayers.push(this); }
  setSource(value) { this.source = value; } setStyle(value) { this.style = value; } setExtent(value) { this.extent = value; }
  setVisible(value) { this.visible = value; } setOpacity(value) { this.opacity = value; } setZIndex(value) { this.zIndex = value; }
} }));

const pin = 'a'.repeat(64), area = [115, 39, 117, 41];
const entry = (assetKey, generation = 1) => ({ id: JSON.stringify([pin, assetKey]), snapshotId: pin, assetKey,
  title: assetKey, itemId: 'published', visible: true, opacity: 1, retry: 0, generation });
const metadata = { crs: 'EPSG:4326', bounds: [115, 39, 117, 41], width: 200, height: 200,
  displayBand: 1, displayMode: 'raw-first-band-grayscale', nodata: 0 };
function preview() {
  const events = new Map(); let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { source: { on: (name, fn) => { const entries = events.get(name) || []; entries.push(fn); events.set(name, entries); },
    getView: () => promise, getState: () => 'ready', getError: () => new Error('Preview failed.'), hasAlpha: true, bandCount: 2 },
  style: {}, getStyle: () => ({ color: 'actual gray' }), getMetadata: () => metadata, dispose: vi.fn(),
  resolve: () => resolve({}), reject, emit: (name, event) => events.get(name)?.forEach(fn => fn(event)) };
}
const previews = () => createPreview.mock.results.map(result => result.value);
const pixels = { tile: { getData: () => new Uint8Array([10, 255]), getSize: () => [1, 1] } };
const renderMap = (layers, callback, props = {}) => <ExploreMap scenes={[]} area={area} productRasterLayers={layers}
  onProductRasterStatus={callback} {...props}/>;
beforeEach(() => {
  // New installs record by default. Each isolated readiness case owns its
  // collector state; earlier map tests must not become this case's events.
  usage.setPreferences({enabled:false});
  createPreview.mockReset(); createPreview.mockImplementation(preview); maps.length = 0; rasterLayers.length = 0;
});
afterEach(() => { vi.restoreAllMocks(); });

// These replace only the canvas surface and source factory. Real React effects,
// descriptor reconciliation, projection transforms and readiness guards execute.
it('keeps metadata and empty tiles loading until a real nonempty DataTile arrives', async () => {
  const callback = vi.fn(), value = entry('raster'), view = render(renderMap([value], callback));
  const source = previews()[0];
  await act(async () => source.resolve());
  expect(callback.mock.calls.at(-1)[1]).toMatchObject({ status: 'loading', metadata, generation: 1, retry: 0 });
  expect(view.container.firstChild.getAttribute('data-product-raster-ready')).toBe('0');
  await act(async () => source.emit('tileloadend', { tile: { getData: () => new Uint8Array(0) } }));
  expect(callback.mock.calls.at(-1)[1].status).toBe('loading');
  await act(async () => source.emit('tileloadend', { tile: { getData: () => new Uint8Array([0, 0]), getSize: () => [1, 1] } }));
  expect(callback.mock.calls.at(-1)[1].status).toBe('loading');
  await act(async () => source.emit('tileloadend', pixels));
  expect(callback.mock.calls.at(-1)).toEqual([value.id, { status: 'ready', generation: 1, retry: 0, metadata }]);
  expect(view.container.firstChild.getAttribute('data-product-raster-ready')).toBe('1');
  expect(rasterLayers[0].extent.every(Number.isFinite)).toBe(true);
});

it('hiding, opacity and raising a product layer retain its original source and grid', async () => {
  const a = entry('pv'), b = entry('qa'), callback = vi.fn();
  const view = render(renderMap([a, b], callback));
  await act(async () => previews().forEach(source => source.resolve()));
  const sources = previews(), firstLayer = rasterLayers[0], secondLayer = rasterLayers[1];
  view.rerender(renderMap([{ ...b, opacity: 0.25 }, { ...a, visible: false }], callback));
  expect(createPreview).toHaveBeenCalledTimes(2); expect(sources.every(source => !source.dispose.mock.calls.length)).toBe(true);
  expect(firstLayer.visible).toBe(false); expect(secondLayer.opacity).toBe(0.25);
  expect(firstLayer.zIndex).toBeGreaterThan(secondLayer.zIndex);
  expect(view.container.firstChild.getAttribute('data-product-raster-layers')).toBe('2');
  expect(view.container.firstChild.getAttribute('data-product-raster-visible')).toBe('1');
});

it('adapts the zoom limit to original product metadata and keeps sources while changing visibility', async () => {
  createPreview.mockImplementation(()=>{
    const value=preview();
    value.getMetadata=()=>({...metadata,crs:'EPSG:3857',bounds:[0,0,300,300],width:1000,height:1000});
    return value;
  });
  const layer=entry('raster'), callback=vi.fn(), view=render(renderMap([layer],callback));
  const source=previews()[0]; await act(async()=>source.resolve());
  const mapView=maps[0].getView();
  expect(mapView.getMinResolution()).toBeGreaterThanOrEqual(0.075);
  expect(mapView.getMinResolution()).toBeLessThan(0.15);
  view.rerender(renderMap([{...layer,visible:false}],callback));
  expect(mapView.getMinResolution()).toBeGreaterThanOrEqual(2.5);
  view.rerender(renderMap([layer],callback));
  expect(mapView.getMinResolution()).toBeLessThan(0.15);
  expect(createPreview).toHaveBeenCalledOnce();
  expect(source.dispose).not.toHaveBeenCalled();
});

it('records real preview readiness only after decoded pixels and excludes item, area and asset identities',async()=>{
  usage.setPreferences({enabled:true});
  try{
    const layer={...entry('raster'),title:'private project title'},view=render(renderMap([layer],vi.fn()));
    const source=previews()[0];
    expect(usage.report().counts.preview_requested).toBe(1);
    expect(usage.report().counts.preview_ready).toBe(0);
    await act(async()=>source.resolve());
    expect(usage.report().counts.preview_ready).toBe(0);
    await act(async()=>source.emit('tileloadend',pixels));
    expect(usage.report().counts.preview_ready).toBe(1);
    view.rerender(renderMap([{...layer,opacity:0.5}],vi.fn()));
    expect(usage.report().counts.preview_requested).toBe(1);
    const report=JSON.stringify(usage.report().events);
    expect(report).not.toContain('private project title');expect(report).not.toContain(pin);expect(report).not.toContain('115,39,117,41');
  }finally{usage.setPreferences({enabled:false});}
});

it('one error and its targeted retry do not rebuild or mislabel a separate ready layer', async () => {
  const a = entry('pv'), b = entry('qa'), callback = vi.fn();
  const view = render(renderMap([a, b], callback)); const [good, bad] = previews();
  await act(async () => { good.resolve(); bad.resolve(); });
  await act(async () => { good.emit('tileloadend', pixels); bad.emit('tileloaderror'); });
  expect(bad.dispose).toHaveBeenCalledOnce(); expect(good.dispose).not.toHaveBeenCalled();
  expect(view.container.firstChild.getAttribute('data-product-raster-ready')).toBe('1');
  const count = callback.mock.calls.length;
  view.rerender(renderMap([a, { ...b, retry: 1 }], callback));
  expect(createPreview).toHaveBeenCalledTimes(3); expect(good.dispose).not.toHaveBeenCalled();
  await act(async () => bad.emit('tileloadend', pixels));
  expect(callback.mock.calls.slice(count).some(([_key, status]) => status.retry === 0 && status.status === 'ready')).toBe(false);
  const retried = previews()[2]; await act(async () => retried.resolve()); await act(async () => retried.emit('tileloadend', pixels));
  expect(callback.mock.calls.at(-1)[1]).toMatchObject({ status: 'ready', generation: 1, retry: 1 });
});

it('removal, new generation and area changes abort only their old sources and ignore late completions', async () => {
  const a = entry('pv'), callback = vi.fn(), view = render(renderMap([a], callback));
  const old = previews()[0]; view.rerender(renderMap([], callback));
  expect(old.dispose).toHaveBeenCalledOnce();
  const count = callback.mock.calls.length;
  await act(async () => { old.resolve(); old.emit('tileloadend', pixels); });
  expect(callback).toHaveBeenCalledTimes(count);
  view.rerender(renderMap([{ ...a, generation: 2 }], callback));
  const second = previews()[1]; view.rerender(renderMap([{ ...a, generation: 3 }], callback));
  expect(second.dispose).toHaveBeenCalledOnce(); expect(createPreview).toHaveBeenCalledTimes(3);
  const third = previews()[2]; view.rerender(renderMap([{ ...a, generation: 3 }], callback, { area: [10, 10, 11, 11] }));
  expect(third.dispose).toHaveBeenCalledOnce(); expect(createPreview).toHaveBeenCalledTimes(4);
  view.unmount(); expect(previews()[3].dispose).toHaveBeenCalledOnce();
  await act(async () => { third.resolve(); third.emit('tileloadend', pixels); });
  expect(callback.mock.calls.filter(([_key, value]) => value.status === 'ready')).toHaveLength(0);
});

it('fails a grid with no valid projection extent without using the catalog footprint', async () => {
  const callback = vi.fn(), value = entry('raster');
  const source = preview(); source.getMetadata = () => ({ ...metadata, bounds: [NaN, 0, 1, 1] });
  createPreview.mockReturnValue(source);
  render(renderMap([value], callback)); await act(async () => source.resolve());
  await waitFor(() => expect(callback.mock.calls.at(-1)[1].status).toBe('error'));
  expect(source.dispose).toHaveBeenCalledOnce();
});

it('reports overflow independently without silently starting more than four raster sources', () => {
  const callback = vi.fn(), entries = Array.from({ length: 5 }, (_, index) => entry(`band-${index}`));
  render(renderMap(entries, callback));
  expect(createPreview).toHaveBeenCalledTimes(4);
  expect(callback.mock.calls.at(-1)).toEqual([entries[4].id, { status: 'error', generation: 1, retry: 0,
    error: 'Load up to 4 original raster previews at a time.' }]);
  expect(rasterLayers.every(layer => layer.cacheSize === 32)).toBe(true);
});

it('delegates loading and error presentation to a supplied per-layer status owner without duplicate map overlays', async () => {
  const callback = vi.fn(), value = entry('raster');
  render(renderMap([value], callback));
  expect(callback.mock.calls.at(-1)[1].status).toBe('loading');
  expect(screen.queryByText('Loading original raster previews · 1')).toBeNull();
  const source = previews()[0];
  await act(async () => source.reject(new Error('This original has no supported north-up raster preview grid.')));
  expect(callback.mock.calls.at(-1)[1]).toMatchObject({ status: 'error', error: 'This original has no supported north-up raster preview grid.' });
  expect(screen.queryByRole('alert')).toBeNull();
});

it('retains loading and error summaries for standalone maps without a per-layer status owner', async () => {
  render(renderMap([entry('raster')]));
  expect(screen.getByText('Loading original raster previews · 1')).toBeTruthy();
  const source = previews()[0];
  await act(async () => source.reject(new Error('This original has no supported north-up raster preview grid.')));
  expect(screen.queryByText('Loading original raster previews · 1')).toBeNull();
  expect(screen.getByRole('alert').textContent).toContain('This original has no supported north-up raster preview grid.');
});

it('keeps a hidden pending source alive beyond the timeout and gives showing it a fresh grace period', async () => {
  vi.useFakeTimers();
  const value = entry('raster'), callback = vi.fn(), view = render(renderMap([value], callback));
  try {
    const source = previews()[0];
    view.rerender(renderMap([{ ...value, visible: false }], callback));
    await act(async () => vi.advanceTimersByTime(46000));
    expect(callback.mock.calls.some(([_key, status]) => status.status === 'error')).toBe(false);
    expect(source.dispose).not.toHaveBeenCalled();
    view.rerender(renderMap([value], callback));
    await act(async () => vi.advanceTimersByTime(44000));
    expect(callback.mock.calls.at(-1)[1].status).toBe('loading');
    expect(source.dispose).not.toHaveBeenCalled(); expect(createPreview).toHaveBeenCalledOnce();
    await act(async () => source.resolve());
    await act(async () => source.emit('tileloadend', pixels));
    expect(callback.mock.calls.at(-1)[1]).toMatchObject({ status: 'ready', generation: 1, retry: 0 });
    expect(view.container.firstChild.getAttribute('data-product-raster-ready')).toBe('1');
    await act(async () => vi.advanceTimersByTime(60000));
    expect(callback.mock.calls.some(([_key, status]) => status.status === 'error')).toBe(false);
  } finally { view.unmount(); vi.useRealTimers(); }
});

it('verified range progress keeps slow strip initialization alive without claiming pixels, and an actual stall still fails', async () => {
  vi.useFakeTimers();
  const callback=vi.fn(),view=render(renderMap([entry('BAND1')],callback));
  try {
    const source=previews()[0];
    await act(async()=>vi.advanceTimersByTime(40000));
    await act(async()=>source.emit('rasterreadprogress'));
    await act(async()=>vi.advanceTimersByTime(40000));
    expect(source.dispose).not.toHaveBeenCalled();
    expect(callback.mock.calls.at(-1)[1].status).toBe('loading');
    expect(view.container.firstChild.getAttribute('data-product-raster-ready')).toBe('0');
    await act(async()=>vi.advanceTimersByTime(46000));
    expect(callback.mock.calls.at(-1)[1]).toMatchObject({status:'error',error:'The original raster preview stopped responding. Retry this layer.'});
    expect(source.dispose).toHaveBeenCalledOnce();
  } finally {view.unmount();vi.useRealTimers();}
});
