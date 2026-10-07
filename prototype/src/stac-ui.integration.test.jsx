import React from 'react';
import { it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StacSourceDialog, StacRasterInspection, StacProjectAssets } from './stac-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';
import { RuntimeJobRows } from './runtime-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { stacRequest } from './stac-client.js';
import { runtimeRequest } from './runtime-client.js';
import { stacConnection, stacSnapshot, stacInspection, stacJob, stacProject, snapshotId, sourceHash } from './stac-fixtures.js';
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (s, v={}) => s.replace(/\{(\w+)\}/g, (_, k) => v[k] ?? ''), date: s => s || '—', number: String, locale: 'en' }) }));
vi.mock('./stac-client.js', async original => ({ ...await original(), stacRequest: vi.fn() }));
vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn() }));
vi.mock('./file-thumbnail.jsx', () => ({ FileThumbnail: () => <span>Local thumbnail</span> }));
vi.mock('./stac-map.jsx', () => ({ default: ({ onPixel }) => <div aria-label="Georeferenced original raster"><button onClick={() => onPixel({ column: 1, row: 1 })}>Sample map pixel</button></div> }));
const refresh = vi.fn(async () => {});
const runtime = { jobs: [stacJob], projects: [stacProject], health: {}, refresh, act: vi.fn() };
const renderDialog = props => render(<RuntimeContext.Provider value={runtime}><StacSourceDialog areaBounds={[10,20,11,21]} onClose={() => {}} {...props}/></RuntimeContext.Provider>);
beforeEach(() => { vi.clearAllMocks(); stacRequest.mockImplementation(async op => op === 'list' ? [stacConnection] : op === 'snapshot' ? stacSnapshot : op === 'connect' ? stacConnection : op === 'project' ? stacProject : op === 'inspect' ? stacInspection : { items: [stacSnapshot], nextCursor: null, complete: true, limitReached: false }); runtimeRequest.mockResolvedValue([stacProject]); });
async function selectSource(user) { await waitFor(() => expect(screen.getByRole('combobox', { name: 'Saved raster source' }).disabled).toBe(false)); await user.click(screen.getByRole('combobox', { name: 'Saved raster source' })); await user.click(screen.getByRole('option', { name: 'Local test catalog' })); }
it('static catalog scans explicit directories, continues an empty page, and uses native branch keys without claiming API search', async () => {
  const key = 'e'.repeat(64), child = 'f'.repeat(64);
  const connection = { ...stacConnection, kind: 'catalog', collections: [], searchUrl: null, capabilities: { searchGet: false, searchPost: false },
    catalogNodes: [{ key, id: 'root', title: 'Published directory', kind: 'Catalog', description: 'Static catalog', license: null, url: stacConnection.url, parentKey: null },
      { key: child, id: 'actual-collection', title: 'Actual collection', kind: 'Collection', description: 'Actual declaration', license: 'CC0-1.0', url: `${stacConnection.url}/collection.json`, parentKey: key }] };
  let searches = 0;
  stacRequest.mockImplementation(async (op, payload) => op === 'list' ? [connection] : op === 'search' ? (++searches === 1
    ? { items: [], scannedItems: 32, nextCursor: 'opaque-static-cursor', complete: false, limitReached: false }
    : { items: [stacSnapshot], scannedItems: 33, nextCursor: null, complete: true, limitReached: false }) : stacProject);
  const user = userEvent.setup(); renderDialog(); await selectSource(user);
  expect(screen.getByText('Static STAC catalog')).toBeTruthy();
  expect(stacRequest.mock.calls.some(([op]) => op === 'snapshot')).toBe(false);
  await user.click(screen.getByRole('combobox', { name: 'Raster collection' })); await user.click(screen.getByRole('option', { name: 'Actual collection · Collection' }));
  await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  await screen.findByText(/32 item documents scanned/);
  expect(stacRequest.mock.calls.find(([op]) => op === 'search')[1].collectionId).toBe(child);
  expect(screen.queryByRole('textbox', { name: 'Raster project bounds' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Continue scanning' })); await screen.findByText('Temperature field');
  expect(stacRequest.mock.calls.filter(([op]) => op === 'search')[1][1].cursor).toBe('opaque-static-cursor');
  expect(screen.queryByRole('button', { name: 'Continue scanning' })).toBeNull();
  await user.clear(screen.getByRole('textbox', { name: 'Raster search bounds' }));
  expect(screen.queryByText('Temperature field')).toBeNull(); expect(screen.queryByText(/33 item documents scanned/)).toBeNull();
});
it('uses an explicit source and collection, omits date/cloud defaults, selects original assets and saves before enqueue', async () => {
  const user = userEvent.setup(); renderDialog();
  expect(screen.getByRole('textbox', { name: 'Source URL' }).value).toBe('');
  await selectSource(user); await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  await screen.findByText('Temperature field');
  const request = stacRequest.mock.calls.find(([op]) => op === 'search')[1];
  expect(request).toEqual({ connectionId: stacConnection.id, collectionId: 'temperature', bounds: [10,20,11,21], limit: 20 });
  expect(screen.getByRole('checkbox', { name: 'Select asset · model-42 · readme' }).disabled).toBe(true);
  expect(document.querySelector('img')).toBeNull();
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · surface_temp' }));
  await user.type(screen.getByRole('textbox', { name: 'Raster project name' }), 'Temperature');
  expect(stacRequest.mock.calls.some(([op]) => op === 'downloads')).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  await screen.findByRole('button', { name: 'Download selected originals' });
  expect(stacRequest.mock.calls.find(([op]) => op === 'project')[1]).toEqual({ name: 'Temperature', bounds: [10,20,11,21], selections: [{ snapshotId, assetKey: 'surface_temp' }] });
  expect(stacRequest.mock.calls.some(([op]) => op === 'downloads')).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Download selected originals' }));
  await screen.findByRole('button', { name: 'Downloads queued' });
  expect(stacRequest.mock.calls.find(([op]) => op === 'downloads')[1]).toEqual({ projectId: stacProject.id, selections: [{ snapshotId, assetKey: 'surface_temp' }] });
});
it('does not automatically request another page and discards old results when region changes', async () => {
  stacRequest.mockImplementation(async op => op === 'list' ? [stacConnection] : { items: [stacSnapshot], nextCursor: 'opaque-filter-bound-cursor', complete: false, limitReached: false });
  const user = userEvent.setup(); renderDialog(); await selectSource(user); await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  await screen.findByRole('button', { name: 'Load more items' });
  expect(stacRequest.mock.calls.filter(([op]) => op === 'search')).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: 'Load more items' }));
  await waitFor(() => expect(stacRequest.mock.calls.filter(([op]) => op === 'search')).toHaveLength(2));
  expect(stacRequest.mock.calls.filter(([op]) => op === 'search')[1][1].cursor).toBe('opaque-filter-bound-cursor');
  await user.clear(screen.getByRole('textbox', { name: 'Raster search bounds' }));
  expect(screen.queryByText('Temperature field')).toBeNull(); expect(screen.getByRole('button', { name: 'Search raster items' }).disabled).toBe(true);
});
it('direct raster uses saved snapshots without searching or inventing date or collection', async () => {
  stacRequest.mockImplementation(async op => op === 'list' ? [{ ...stacConnection, kind: 'raster', collections: [], snapshotIds: [snapshotId] }] : { ...stacSnapshot, collectionId: null, datetime: null, startDatetime: null, endDatetime: null });
  const user = userEvent.setup(); renderDialog(); await selectSource(user); await screen.findByText('Temperature field');
  expect(stacRequest).toHaveBeenCalledWith('snapshot', { id: snapshotId }, expect.any(AbortSignal));
  expect(screen.queryByRole('button', { name: 'Search raster items' })).toBeNull();
  expect(screen.queryByText('Cloud cover')).toBeNull(); expect(screen.getByRole('textbox', { name: 'Raster project bounds' }).value).toBe('10, 20, 11, 21');
});
it('custom-only projects count assets and show original download actions without SAFE/SCL/RGB processing', async () => {
  render(<RuntimeContext.Provider value={runtime}><ProjectsLibrary focusedProjectId={stacProject.id} onContinueExploring={() => {}}/></RuntimeContext.Provider>);
  await screen.findByText('1 raster assets');
  expect(screen.getByRole('button', { name: 'Download missing originals' }).disabled).toBe(true);
  expect(screen.queryByText('Prepare SCL')).toBeNull(); expect(screen.queryByText('Download true-color')).toBeNull();
  expect(screen.queryByRole('button', { name: 'Explore and add scenes to this project' })).toBeNull();
  expect(screen.getByText('Custom raster · original file')).toBeTruthy();
});
it('custom task descriptions avoid built-in sensor interpretation even when item ID resembles Sentinel', () => {
  render(<RuntimeContext.Provider value={runtime}><RuntimeJobRows jobs={[{ ...stacJob, status: 'running', itemId: 'S2A_10SEG_20250101_0_L2A' }]}/></RuntimeContext.Provider>);
  expect(screen.getByText('Original raster download')).toBeTruthy(); expect(screen.getByText('Temperature field')).toBeTruthy();
  expect(screen.queryByText('Preview download')).toBeNull();
});
it('keeps refreshed checkboxes selected while downloading the verified canonical pins from the existing project', async () => {
  const refreshedId = 'e'.repeat(64), canonical = [{ snapshotId, assetKey: 'surface_temp' }];
  stacRequest.mockImplementation(async op => op === 'list' ? [stacConnection] : op === 'project' ? { ...stacProject, canonicalSelections: canonical }
    : op === 'downloads' ? { jobs: [stacJob] } : { items: [{ ...stacSnapshot, id: refreshedId }], nextCursor: null, complete: true, limitReached: false });
  const user = userEvent.setup(); renderDialog({ currentProject: stacProject }); await selectSource(user);
  await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  const checkbox = await screen.findByRole('checkbox', { name: 'Select asset · model-42 · surface_temp' }); await user.click(checkbox);
  await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  const download = await screen.findByRole('button', { name: 'Download selected originals' });
  expect(checkbox.getAttribute('aria-checked')).toBe('true');
  expect(stacRequest.mock.calls.find(([op]) => op === 'project')[1].selections).toEqual([{ snapshotId: refreshedId, assetKey: 'surface_temp' }]);
  await user.click(download); await screen.findByRole('button', { name: 'Original files ready' });
  expect(stacRequest.mock.calls.find(([op]) => op === 'downloads')[1]).toEqual({ projectId: stacProject.id, selections: canonical });
});
it('allows a POST-only source through search, explicit pagination and project selection', async () => {
  const connection = { ...stacConnection, searchMethod: 'POST', capabilities: { searchGet: false, searchPost: true } };
  stacRequest.mockImplementation(async op => op === 'list' ? [connection] : op === 'project' ? stacProject : { items: [stacSnapshot], nextCursor: 'native-post-body-cursor', complete: false, limitReached: false });
  const user = userEvent.setup(); renderDialog(); await selectSource(user);
  expect(screen.getByRole('button', { name: 'Search raster items' }).disabled).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Search raster items' }));
  await user.click(await screen.findByRole('button', { name: 'Load more items' }));
  await waitFor(() => expect(stacRequest.mock.calls.filter(([op]) => op === 'search')).toHaveLength(2));
  expect(stacRequest.mock.calls.filter(([op]) => op === 'search')[1][1].cursor).toBe('native-post-body-cursor');
  await user.click(screen.getByRole('checkbox', { name: 'Select asset · model-42 · surface_temp' }));
  await user.type(screen.getByRole('textbox', { name: 'Raster project name' }), 'POST originals');
  await user.click(screen.getByRole('button', { name: 'Save asset selection' }));
  await screen.findByRole('button', { name: 'Download selected originals' });
  expect(stacRequest.mock.calls.find(([op]) => op === 'project')[1].selections).toEqual([{ snapshotId, assetKey: 'surface_temp' }]);
});
it.each([
  ['intact', 'succeeded', '1 verified · 0 repair downloads queued'],
  ['missing or corrupt', 'queued', '0 verified · 1 repair downloads queued'],
  ['already being repaired', 'running', '0 verified · 0 repair downloads queued · 1 repairs already running'],
])('verifies all-completed project originals and accurately reports %s files', async (_condition, status, message) => {
  stacRequest.mockResolvedValue({ projectId: stacProject.id, assetKey: 'stac_asset', jobs: [{ ...stacJob, status }] });
  const user = userEvent.setup();
  render(<RuntimeContext.Provider value={runtime}><StacProjectAssets project={stacProject} jobs={[stacJob]}/></RuntimeContext.Provider>);
  expect(screen.getByRole('button', { name: 'Download missing originals' }).disabled).toBe(true);
  const action = screen.getByRole('button', { name: 'Verify downloaded originals' });
  expect(action.disabled).toBe(false);
  await user.click(action);
  await screen.findByText(message);
  expect(stacRequest).toHaveBeenCalledWith('downloads', { projectId: stacProject.id, selections: [{ snapshotId, assetKey: 'surface_temp' }] });
  expect(refresh).toHaveBeenCalledOnce();
});
it('file inspection separates source metadata and file geometry, supports exact raw values and map picks', async () => {
  stacRequest.mockImplementation(async (op, payload) => op === 'inspect' ? stacInspection : op === 'snapshot' ? stacSnapshot : { jobId: stacJob.id, sha256: sourceHash, column: payload.column, row: payload.row, values: ['9007199254740993'], noData: [false] });
  const user = userEvent.setup(); render(<StacRasterInspection job={stacJob} onClose={() => {}}/>);
  await user.click(await screen.findByRole('button', { name: 'File metadata and display' }));
  await screen.findByText('Float32'); expect(screen.getByText('NaN')).toBeTruthy();
  await user.click(await screen.findByRole('button', { name: 'Sample map pixel' }));
  await screen.findByText('9007199254740993');
  expect(stacRequest).toHaveBeenCalledWith('pixel', { id: stacJob.id, column: 1, row: 1 });
  expect(screen.getByRole('textbox', { name: 'Pixel column' }).value).toBe('1');
  await user.click(screen.getByRole('button', { name: 'Original item metadata' }));
  expect(screen.getByText(/<img src=x/)).toBeTruthy(); expect(document.querySelector('img[src=x]')).toBeNull();
});
