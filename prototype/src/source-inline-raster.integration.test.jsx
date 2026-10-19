import React from 'react';
import { it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StacSourceExplorer } from './stac-ui.jsx';
import { WcsSourceExplorer } from './wcs-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { stacRequest } from './stac-client.js';
import { wcsRequest } from './wcs-client.js';
import { stacConnection, stacSnapshot, stacProject, stacJob } from './stac-fixtures.js';
import { wcsConnection, wcsDescription, wcsPlan, wcsProject, wcsJob } from './wcs-fixtures.js';

const map = vi.hoisted(() => ({ props: null }));
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (s, v={}) => s.replace(/\{(\w+)\}/g, (_, k) => v[k] ?? ''), date: String, number: String, locale: 'en' }) }));
vi.mock('./stac-client.js', async original => ({ ...await original(), stacRequest: vi.fn() }));
vi.mock('./wcs-client.js', async original => ({ ...await original(), wcsRequest: vi.fn() }));
vi.mock('./public-dataset-explore-map.jsx', () => ({ PublicDatasetExploreMap: props => {
  map.props = props;
  return <div aria-label="Shared source map"><output aria-label="Map preview count">{props.rasterLayers?.length || 0}</output><button onClick={props.onClearRasters}>Clear map previews</button></div>;
} }));
const context = { projects: [], refresh: vi.fn(async () => {}), jobs: [] };
const page = items => ({ items, complete: true, nextCursor: null, limitReached: false });
beforeEach(() => {
  vi.clearAllMocks(); map.props = null;
  stacRequest.mockImplementation(async op => ({ list: [stacConnection], search: page([stacSnapshot]), snapshot: stacSnapshot, project: stacProject,
    downloads: { jobs: [{ ...stacJob, status: 'queued' }] } })[op]);
  wcsRequest.mockImplementation(async op => ({ list: [wcsConnection], describe: wcsDescription, plan: wcsPlan, project: wcsProject,
    downloads: { jobs: [{ ...wcsJob, status: 'queued' }] } })[op]);
});
const wrap = node => <RuntimeContext.Provider value={context}>{node}</RuntimeContext.Provider>;
async function choose(user, label, name) {
  await waitFor(() => expect(screen.getByRole('combobox', { name: label }).disabled).toBe(false));
  await user.click(screen.getByRole('combobox', { name: label })); await user.click(screen.getByRole('option', { name }));
}
async function searchStac(user) {
  await choose(user, 'Saved raster source', stacConnection.name);
  await user.click(screen.getByRole('button', { name: 'Search raster items' })); await screen.findByText(stacSnapshot.title);
}

it('STAC keeps original file selection, map range previews and explicit downloads separate in the shared portal', async () => {
  const user = userEvent.setup(), sidebar = document.createElement('aside'); document.body.append(sidebar);
  const view = render(wrap(<StacSourceExplorer areaBounds={stacSnapshot.bbox} sidebarTarget={sidebar}/>));
  await searchStac(user);
  expect(screen.queryByRole('dialog')).toBeNull(); expect(screen.queryByRole('textbox', { name: 'Raster search bounds' })).toBeNull();
  expect(sidebar.querySelector('.stac-items')).toBeTruthy();
  await user.click(screen.getByRole('checkbox', { name: `Select asset · ${stacSnapshot.itemId} · surface_temp` }));
  await user.click(screen.getByRole('button', { name: 'Preview selected' }));
  expect(map.props.rasterLayers[0]).toMatchObject({ snapshotId: stacSnapshot.id, assetKey: 'surface_temp', visible: true, opacity: 1, retry: 0 });
  expect(map.props.items[0].bbox).toEqual(stacSnapshot.bbox);
  expect(stacRequest.mock.calls.some(([op]) => ['project','downloads'].includes(op))).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Clear map previews' }));
  expect(map.props.rasterLayers).toEqual([]); expect(screen.getByRole('checkbox', { name: `Select asset · ${stacSnapshot.itemId} · surface_temp` }).getAttribute('aria-checked')).toBe('true');
  await user.click(screen.getByRole('button', { name: 'Save and download' }));
  await user.type(screen.getByRole('textbox', { name: 'Raster project name' }), 'Temperature');
  expect(stacRequest.mock.calls.some(([op]) => ['project','downloads'].includes(op))).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Confirm and download' }));
  await screen.findByRole('link', { name: 'View download tasks' });
  expect(stacRequest.mock.calls.find(([op]) => op === 'project')[1]).toEqual({ name: 'Temperature', bounds: stacSnapshot.bbox, selections: [{ snapshotId: stacSnapshot.id, assetKey: 'surface_temp' }] });
  view.unmount(); sidebar.remove();
});

it('STAC can select more originals than the preview limit without starting an oversized map request', async () => {
  const items = Array.from({ length: 5 }, (_, index) => ({ ...stacSnapshot, id: String(index + 1).repeat(64), itemId: `item-${index}`, title: `Product ${index}` }));
  stacRequest.mockImplementation(async op => op === 'list' ? [stacConnection] : page(items));
  const user = userEvent.setup(); render(wrap(<StacSourceExplorer areaBounds={stacSnapshot.bbox}/>));
  await choose(user, 'Saved raster source', stacConnection.name); await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  await screen.findByText('Product 0');
  for (const item of items) await user.click(screen.getByRole('checkbox', { name: `Select asset · ${item.itemId} · surface_temp` }));
  expect(screen.getByRole('button', { name: 'Preview selected' }).disabled).toBe(true);
  expect(screen.getByRole('button', { name: 'Save and download' }).disabled).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Save and download' }));
  expect(screen.getByRole('button', { name: 'Confirm and download' }).disabled).toBe(true); // Project name is still required.
  expect(map.props.rasterLayers).toEqual([]); expect(stacRequest.mock.calls.some(([op]) => op === 'downloads')).toBe(false);
});

it('STAC AOI changes abort and discard a late native search and clear selected previews', async () => {
  let finish;
  const user = userEvent.setup(), view = render(wrap(<StacSourceExplorer areaBounds={stacSnapshot.bbox}/>));
  await searchStac(user);
  await user.click(screen.getByRole('checkbox', { name: `Select asset · ${stacSnapshot.itemId} · surface_temp` }));
  await user.click(screen.getByRole('button', { name: 'Preview selected' }));
  stacRequest.mockImplementation((op) => op === 'search' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve([stacConnection]));
  await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  const signal = stacRequest.mock.calls.filter(([op]) => op === 'search').at(-1)[2];
  view.rerender(wrap(<StacSourceExplorer areaBounds={[11,21,12,22]}/>));
  expect(signal.aborted).toBe(true); expect(map.props.rasterLayers).toEqual([]);
  await act(async () => finish(page([stacSnapshot])));
  expect(screen.queryByText(stacSnapshot.title)).toBeNull(); expect(map.props.items).toEqual([]);
});

it('STAC source changes clear the old item, selected files and map layers', async () => {
  const other = { ...stacConnection, id: 'another-source', name: 'Other catalog' };
  stacRequest.mockImplementation(async op => op === 'list' ? [stacConnection, other] : page([stacSnapshot]));
  const user = userEvent.setup(); render(wrap(<StacSourceExplorer areaBounds={stacSnapshot.bbox}/>));
  await searchStac(user); await user.click(screen.getByRole('checkbox', { name: `Select asset · ${stacSnapshot.itemId} · surface_temp` }));
  await user.click(screen.getByRole('button', { name: 'Preview selected' }));
  await choose(user, 'Saved raster source', other.name);
  expect(map.props.items).toEqual([]); expect(map.props.rasterLayers).toEqual([]); expect(map.props.selectedIds).toEqual([]);
});

it('a pinned raster source refreshes its metadata after AOI changes without restoring selections or downloading originals', async () => {
  const direct = { ...stacConnection, kind: 'raster', collections: [], snapshotIds: [stacSnapshot.id] };
  stacRequest.mockImplementation(async op => op === 'list' ? [direct] : stacSnapshot);
  const user = userEvent.setup(), view = render(wrap(<StacSourceExplorer areaBounds={stacSnapshot.bbox}/>));
  await choose(user, 'Saved raster source', direct.name); await screen.findByText(stacSnapshot.title);
  await user.click(screen.getByRole('checkbox', { name: `Select asset · ${stacSnapshot.itemId} · surface_temp` }));
  await user.click(screen.getByRole('button', { name: 'Preview selected' }));
  view.rerender(wrap(<StacSourceExplorer areaBounds={[11,21,12,22]}/>));
  await screen.findByText(stacSnapshot.title);
  await waitFor(() => expect(stacRequest.mock.calls.filter(([op]) => op === 'snapshot')).toHaveLength(2));
  expect(map.props.selectedIds).toEqual([]); expect(map.props.rasterLayers).toEqual([]);
  expect(stacRequest.mock.calls.some(([op]) => ['search','project','downloads'].includes(op))).toBe(false);
});

it('WCS shows verified coverage and plan boundaries using shared AOI, with explicit save before download', async () => {
  const user = userEvent.setup(); render(wrap(<WcsSourceExplorer areaBounds={wcsPlan.requestedBounds}/>));
  await choose(user, 'Saved coverage service', wcsConnection.name);
  expect(screen.queryByRole('dialog')).toBeNull(); expect(screen.queryByRole('textbox', { name: 'Coverage subset bounds' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Read coverage description' })); await screen.findByText('100 × 100 · EPSG:4326 · 1 range fields');
  expect(map.props.items).toEqual([{ id: wcsDescription.id, bbox: wcsDescription.bounds }]);
  await user.click(screen.getByRole('button', { name: 'Plan coverage subset' })); await screen.findByText('Planned coverage subset');
  expect(map.props.items.at(-1)).toEqual({ id: wcsPlan.id, bbox: wcsPlan.bounds });
  expect(wcsRequest.mock.calls.find(([op]) => op === 'plan')[1].bounds).toEqual(wcsPlan.requestedBounds);
  expect(wcsRequest.mock.calls.some(([op]) => op === 'downloads')).toBe(false);
  expect(screen.getByText('Coverage boundaries are shown before download. Pixel preview is available after downloading and inspecting the subset.')).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Save and download' }));
  await user.type(screen.getByRole('textbox', { name: 'Coverage project name' }), 'Temperature subset');
  expect(wcsRequest.mock.calls.some(([op]) => ['project','downloads'].includes(op))).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Confirm and download' }));
  await screen.findByRole('link', { name: 'View download tasks' });
  expect(wcsRequest.mock.calls.find(([op]) => op === 'downloads')[1]).toEqual({ projectId: wcsProject.id, selections: [{ planId: wcsPlan.id }] });
});

it('WCS AOI changes abort a late planned grid and prevent the old subset from reappearing', async () => {
  let finish;
  const user = userEvent.setup(), view = render(wrap(<WcsSourceExplorer areaBounds={wcsPlan.requestedBounds}/>));
  await choose(user, 'Saved coverage service', wcsConnection.name); await user.click(screen.getByRole('button', { name: 'Read coverage description' }));
  await screen.findByText('100 × 100 · EPSG:4326 · 1 range fields');
  wcsRequest.mockImplementation(op => op === 'plan' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(wcsDescription));
  await user.click(screen.getByRole('button', { name: 'Plan coverage subset' }));
  const signal = wcsRequest.mock.calls.find(([op]) => op === 'plan')[2];
  view.rerender(wrap(<WcsSourceExplorer areaBounds={[10.3,20.6,10.5,20.7]}/>));
  expect(signal.aborted).toBe(true);
  await act(async () => finish(wcsPlan));
  expect(screen.queryByText('Planned coverage subset')).toBeNull(); expect(map.props.items).toEqual([]);
  expect(screen.getByRole('button', { name: 'Plan coverage subset' }).disabled).toBe(true);
});
