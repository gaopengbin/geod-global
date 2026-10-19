import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { PublicDatasetExploreMap } from './public-dataset-explore-map.jsx';
import { deafricaCases } from './public-dataset-deafrica-fixtures.js';

const { capture, controls } = vi.hoisted(() => ({ capture: vi.fn(),
  controls: { zoomIn: vi.fn(), zoomOut: vi.fn(), fit: vi.fn(), fitArea: vi.fn() } }));
vi.mock('./explore-map.jsx', () => ({ ExploreMap: React.forwardRef((props, ref) => {
  capture(props); React.useImperativeHandle(ref, () => controls, []);
  return <div aria-label="Original footprint adapter"/>;
}) }));
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (value, values = {}) => value.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? '') }) }));
beforeEach(() => { capture.mockClear(); Object.values(controls).forEach(fn => fn.mockClear()); });
afterEach(cleanup);

const area = [115, 39, 117, 41];
const geometry = { type: 'Polygon', coordinates: [[[115, 39], [117, 39], [117, 41], [115, 41], [115, 39]]] };
const item = { id: 'published-1', geometry, bbox: [114, 38, 118, 42], datetime: null,
  startDatetime: '2025-12-01T00:00:00Z', endDatetime: '2025-12-31T23:59:59Z',
  assets: [{ href: 'https://publisher.example/raw.tif' }], thumbnail: 'https://publisher.example/browse.jpg' };

it('preserves declared geometry and actual periods while passing no raster or thumbnail loading to the map', () => {
  const original = structuredClone(item);
  render(<PublicDatasetExploreMap items={[item]} areaBounds={area} areaGeometry={geometry}
    selectedIds={['published-1', 'old-source']} focusedId="published-1"/>);
  const props = capture.mock.calls.at(-1)[0];
  expect(props.area).toBe(area); expect(props.areaGeometry).toBe(geometry); expect(props.showArea).toBe(true);
  expect(props.scenes).toEqual([{ id: item.id, geometry, bbox: item.bbox, datetime: null,
    startDatetime: item.startDatetime, endDatetime: item.endDatetime }]);
  expect(props.scenes[0].geometry).toBe(geometry); expect(props.loadedScenes).toEqual([]); expect(props.previewScenes).toEqual([]);
  expect(props.scene).toBeUndefined(); expect(props.previewChannel).toBeUndefined(); expect(props.activeDay).toBeUndefined();
  expect(props.selectedIds).toEqual(['published-1']); expect(props.focusedIds).toEqual(['published-1']);
  expect(item).toEqual(original);
  expect(screen.getByText('Catalog boundaries do not prove valid-pixel coverage.')).toBeTruthy();
});

it('uses a framing-only overview without turning an absent file extent or absent AOI into claimed coverage', () => {
  render(<PublicDatasetExploreMap items={[{ id: 'country-file', geometry: null, bbox: null, datetime: null }]}
    areaBounds={[0, 0, 0, 0]} areaGeometry={geometry}/>);
  const props = capture.mock.calls.at(-1)[0];
  expect(props.area).toEqual([-180, -80, 180, 80]); expect(props.showArea).toBe(false); expect(props.areaGeometry).toBeUndefined();
  expect(props.scenes[0]).toEqual({ id: 'country-file', geometry: null, bbox: null, datetime: null, startDatetime: null, endDatetime: null });
  expect(screen.getByRole('button', { name: 'Fit published footprints' }).disabled).toBe(true);
  expect(screen.getByRole('button', { name: 'Fit search area' }).disabled).toBe(true);
  expect(screen.getByText('Choose an area to search published products.')).toBeTruthy();
});

it('focuses only a current item and retains exact instantaneous publisher time without creating an optical date', () => {
  const onFocusItem = vi.fn(), instant = '2023-08-12 21:12:49Z';
  const view = render(<PublicDatasetExploreMap items={[{ ...item, datetime: instant, startDatetime: null, endDatetime: null }]}
    areaBounds={area} onFocusItem={onFocusItem}/>);
  let props = capture.mock.calls.at(-1)[0];
  expect(props.scenes[0].datetime).toBe(instant); expect(props.scenes[0].date).toBeUndefined();
  props.onFootprintsPick(['old-source', item.id]); expect(onFocusItem).toHaveBeenCalledWith(item.id);
  props.onFootprintsPick(['old-source']); expect(onFocusItem).toHaveBeenCalledTimes(1);
  view.rerender(<PublicDatasetExploreMap items={[{ id: 'new-source', bbox: null, geometry: null }]}
    areaBounds={area} selectedIds={[item.id]} focusedId={item.id} onFocusItem={onFocusItem}/>);
  props = capture.mock.calls.at(-1)[0]; expect(props.selectedIds).toEqual([]); expect(props.focusedIds).toEqual([]);
  props.onFootprintsPick([item.id]); expect(onFocusItem).toHaveBeenCalledTimes(1);
});

it('reuses map controls and exposes only the supplied area/list callbacks', () => {
  const onRequestArea = vi.fn(), onShowList = vi.fn();
  const view = render(<PublicDatasetExploreMap items={[item]} areaBounds={area}
    onRequestArea={onRequestArea} onShowList={onShowList} listHidden/>);
  for (const [label, method] of [['Zoom in', 'zoomIn'], ['Zoom out', 'zoomOut'],
    ['Fit published footprints', 'fit'], ['Fit search area', 'fitArea']]) {
    fireEvent.click(screen.getByRole('button', { name: label })); expect(controls[method]).toHaveBeenCalledOnce();
  }
  fireEvent.click(screen.getByRole('button', { name: 'Choose a search area' })); expect(onRequestArea).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole('button', { name: 'Show product list' })); expect(onShowList).toHaveBeenCalledOnce();
  view.rerender(<PublicDatasetExploreMap areaBounds={area} onShowList={onShowList}/>);
  expect(screen.queryByRole('button', { name: 'Show product list' })).toBeNull();
  expect(screen.queryByRole('button', { name: 'Choose a search area' })).toBeNull();
});

it('locks area changes during a write while leaving map viewing and list restoration available', () => {
  const onRequestArea = vi.fn(), onShowList = vi.fn();
  const view = render(<PublicDatasetExploreMap items={[item]} areaBounds={area} areaDisabled
    onRequestArea={onRequestArea} onShowList={onShowList} listHidden/>);
  const chooseArea = screen.getByRole('button', { name: 'Choose a search area' });
  expect(chooseArea.disabled).toBe(true); fireEvent.click(chooseArea); expect(onRequestArea).not.toHaveBeenCalled();
  for (const [label, method] of [['Zoom in', 'zoomIn'], ['Zoom out', 'zoomOut'],
    ['Fit published footprints', 'fit'], ['Fit search area', 'fitArea']]) {
    const button = screen.getByRole('button', { name: label }); expect(button.disabled).toBe(false);
    fireEvent.click(button); expect(controls[method]).toHaveBeenCalledOnce();
  }
  fireEvent.click(screen.getByRole('button', { name: 'Show product list' })); expect(onShowList).toHaveBeenCalledOnce();
  view.rerender(<PublicDatasetExploreMap areaBounds={area} onRequestArea={onRequestArea}/>);
  expect(screen.getByRole('button', { name: 'Choose a search area' }).disabled).toBe(false);
  fireEvent.click(screen.getByRole('button', { name: 'Choose a search area' })); expect(onRequestArea).toHaveBeenCalledOnce();
});

const fractional = deafricaCases.find(value => value.datasetId === 'deafrica-fractional-cover' && value.layer === 'pv_pc_50');
const rasterEntry = (assetKey, generation = 1) => ({
  id: JSON.stringify([fractional.snapshot.id, assetKey]), snapshotId: fractional.snapshot.id,
  assetKey, itemId: fractional.snapshot.itemId,
  title: fractional.plan.dataset.layers.find(layer => layer.id === assetKey).title,
  visible: true, opacity: 1, retry: 0, generation,
});
const lastMapProps = () => capture.mock.calls.at(-1)[0];
const previewRow = entry => screen.getByRole('checkbox', { name: `Show product preview ${entry.title}` }).closest('.public-dataset-map-layer');
async function report(entry, status, changes = {}) {
  await act(async () => lastMapProps().onProductRasterStatus(entry.id, {
    generation: entry.generation, retry: entry.retry, status, ...changes,
  }));
}

// These statuses are renderer-contract doubles. They test lifecycle/UI guards,
// not network transfer, TIFF decoding or real displayed-pixel acceptance.
it('forwards separate current asset pins to the product renderer while keeping the optical raster paths empty', () => {
  const entries = [rasterEntry('pv_pc_50', 1), rasterEntry('qa', 2), rasterEntry('count_valid', 3)];
  const original = structuredClone(fractional.snapshot);
  render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds}
    selectedIds={[fractional.snapshot.id]} rasterLayers={entries}/>);
  expect(lastMapProps().productRasterLayers).toEqual(entries);
  expect(lastMapProps().loadedScenes).toEqual([]); expect(lastMapProps().previewScenes).toEqual([]);
  expect(lastMapProps().scene).toBeUndefined(); expect(lastMapProps().previewChannel).toBeUndefined();
  expect(new Set(lastMapProps().productRasterLayers.map(entry => entry.id)).size).toBe(3);
  expect(lastMapProps().selectedIds).toEqual([fractional.snapshot.id]);
  expect(screen.getByLabelText('Product footprint map').getAttribute('data-footprint-only')).toBe('false');
  expect(screen.getByText('Map previews · 3')).toBeTruthy();
  expect(fractional.snapshot).toEqual(original);
});

it('excludes foreign snapshots, noncanonical identities, unknown keys and ineligible assets before forwarding previews', () => {
  const valid = rasterEntry('pv_pc_50');
  const unknownKey = { ...valid, assetKey: 'not-reviewed', id: JSON.stringify([valid.snapshotId, 'not-reviewed']) };
  const wrongIdentity = { ...valid, id: 'not-the-canonical-pin' };
  const otherSource = { ...valid, snapshotId: 'f'.repeat(64), id: JSON.stringify(['f'.repeat(64), valid.assetKey]) };
  const forbidden = rasterEntry('qa');
  const currentItem = { ...fractional.snapshot, assets: fractional.snapshot.assets.map(asset => asset.key === 'qa' ? { ...asset, eligible: false } : asset) };
  render(<PublicDatasetExploreMap items={[currentItem]} areaBounds={fractional.request.bounds}
    rasterLayers={[valid, unknownKey, wrongIdentity, otherSource, forbidden]}/>);
  expect(lastMapProps().productRasterLayers).toEqual([valid]);
  expect(screen.getByText('Map previews · 1')).toBeTruthy();
  expect(screen.queryByRole('checkbox', { name: `Show product preview ${forbidden.title}` })).toBeNull();
});

it('hide, opacity, raise, remove and clear controls invoke only the supplied preview callbacks', () => {
  const entry = rasterEntry('pv_pc_50'), onRasterChange = vi.fn(), onRemoveRaster = vi.fn(), onClearRasters = vi.fn(), onRaiseRaster = vi.fn(), onFocusItem = vi.fn();
  render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds}
    selectedIds={[fractional.snapshot.id]} rasterLayers={[entry]} onRasterChange={onRasterChange}
    onRemoveRaster={onRemoveRaster} onClearRasters={onClearRasters} onRaiseRaster={onRaiseRaster} onFocusItem={onFocusItem}/>);
  fireEvent.click(screen.getByRole('checkbox', { name: `Show product preview ${entry.title}` }));
  expect(onRasterChange).toHaveBeenNthCalledWith(1, entry.id, { visible: false });
  fireEvent.keyDown(screen.getByRole('slider', { name: `Product preview opacity ${entry.title}` }), { key: 'ArrowLeft' });
  expect(onRasterChange).toHaveBeenNthCalledWith(2, entry.id, { opacity: 0.9 });
  fireEvent.click(screen.getByRole('button', { name: `Bring product preview to front ${entry.title}` }));
  expect(onRaiseRaster).toHaveBeenCalledWith(entry.id);
  fireEvent.click(screen.getByRole('button', { name: `Remove product preview ${entry.title}` }));
  expect(onRemoveRaster).toHaveBeenCalledWith(entry.id);
  fireEvent.click(screen.getByRole('button', { name: 'Clear map previews' }));
  expect(onClearRasters).toHaveBeenCalledOnce();
  expect(onFocusItem).not.toHaveBeenCalled();
  expect(lastMapProps().selectedIds).toEqual([fractional.snapshot.id]);
});

it('one failed preview does not mark another layer failed or loaded and its retry targets only that failed pin', async () => {
  const loaded = rasterEntry('pv_pc_50', 1), failed = rasterEntry('qa', 2), pending = rasterEntry('count_valid', 3), onRasterChange = vi.fn();
  render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds}
    rasterLayers={[loaded, failed, pending]} onRasterChange={onRasterChange}/>);
  await report(loaded, 'ready');
  await report(failed, 'error', { error: 'Publisher blocked the QA preview.' });
  expect(previewRow(loaded).getAttribute('data-status')).toBe('ready');
  expect(within(previewRow(loaded)).getByText('Preview visible')).toBeTruthy();
  expect(previewRow(failed).getAttribute('data-status')).toBe('error');
  expect(within(previewRow(failed)).getByRole('alert').textContent).toBe('Publisher blocked the QA preview.');
  expect(previewRow(pending).getAttribute('data-status')).toBe('loading');
  expect(within(previewRow(pending)).queryByText('Preview visible')).toBeNull();
  fireEvent.click(within(previewRow(failed)).getByRole('button', { name: 'Retry preview' }));
  expect(onRasterChange).toHaveBeenCalledExactlyOnceWith(failed.id, { retry: true });
});

it('foreign, wrong-generation and old-retry status messages cannot label a current preview as ready', async () => {
  const entry = rasterEntry('pv_pc_50', 4);
  render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[entry]}/>);
  await report(entry, 'ready', { generation: 3 });
  await report(entry, 'ready', { retry: 1 });
  await act(async () => lastMapProps().onProductRasterStatus(JSON.stringify(['f'.repeat(64), 'pv_pc_50']), { status: 'ready', generation: 4, retry: 0 }));
  expect(previewRow(entry).getAttribute('data-status')).toBe('loading');
  expect(within(previewRow(entry)).queryByText('Preview visible')).toBeNull();
  await report(entry, 'ready');
  expect(previewRow(entry).getAttribute('data-status')).toBe('ready');
});

it('removing and reloading the same pin requires a new generation and ignores its old late completion', async () => {
  const old = rasterEntry('pv_pc_50', 1), current = { ...old, generation: 2 };
  const view = render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[old]}/>);
  const oldCallback = lastMapProps().onProductRasterStatus;
  view.rerender(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[]}/>);
  await act(async () => oldCallback(old.id, { status: 'ready', generation: old.generation, retry: old.retry }));
  expect(screen.queryByText('Preview visible')).toBeNull();
  view.rerender(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[current]}/>);
  await act(async () => oldCallback(old.id, { status: 'ready', generation: old.generation, retry: old.retry }));
  expect(previewRow(current).getAttribute('data-status')).toBe('loading');
  await report(current, 'ready');
  expect(previewRow(current).getAttribute('data-status')).toBe('ready');
});

it('retrying an error rejects the preceding retry status and preserves separate loaded layer visibility', async () => {
  const old = rasterEntry('qa', 3), updated = { ...old, retry: 1 }, hidden = { ...rasterEntry('pv_pc_50', 1), visible: false };
  const view = render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[hidden, old]}/>);
  await report(old, 'error', { error: 'Temporary transport failure.' });
  await report(hidden, 'ready');
  expect(within(previewRow(hidden)).getByText('Preview hidden')).toBeTruthy();
  view.rerender(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[hidden, updated]}/>);
  await report(old, 'ready');
  expect(previewRow(updated).getAttribute('data-status')).toBe('loading');
  expect(within(previewRow(updated)).queryByRole('alert')).toBeNull();
  expect(within(previewRow(hidden)).getByText('Preview hidden')).toBeTruthy();
  await report(updated, 'ready');
  expect(previewRow(updated).getAttribute('data-status')).toBe('ready');
});

it('a source change excludes old layers and old callbacks while preserving the new source footprint', async () => {
  const old = rasterEntry('pv_pc_50');
  const view = render(<PublicDatasetExploreMap items={[fractional.snapshot]} areaBounds={fractional.request.bounds} rasterLayers={[old]}/>);
  const callback = lastMapProps().onProductRasterStatus;
  view.rerender(<PublicDatasetExploreMap items={[item]} areaBounds={area} rasterLayers={[old]} selectedIds={[fractional.snapshot.id]}/>);
  await act(async () => callback(old.id, { status: 'ready', generation: old.generation, retry: old.retry }));
  expect(lastMapProps().productRasterLayers).toEqual([]);
  expect(lastMapProps().selectedIds).toEqual([]);
  expect(lastMapProps().scenes[0].id).toBe(item.id);
  expect(screen.queryByText('Preview visible')).toBeNull();
  expect(screen.queryByText('Map previews · 1')).toBeNull();
});
