import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './main.jsx';
import { I18nProvider } from './i18n.jsx';
import { RuntimeProvider } from './runtime-ui.jsx';
import { agentRequest } from './agent-client.js';
import { createSearchRunner, normalizeScene } from './catalog.js';
import { signedOutIdentity } from './identity-client.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { publicDataset } from './public-dataset-fixtures.js';
import { runtimeRequest } from './runtime-client.js';
import { scenesForDownload } from './projects-client.js';
import sentinelFixture from '../public/samples/earth-search-response.json';
import vegetationFixture from '../public/samples/modis-vegetation-response.json';
import demFixture from '../public/samples/cop-dem-response.json';

const map = vi.hoisted(() => ({ current: null }));
vi.mock('./agent-client.js', () => ({ agentRequest: vi.fn(), subscribeAgentUpdates: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('./runtime-client.js', async original => ({ ...await original(), desktopAvailable: () => true, syncDesktopLocale: vi.fn().mockResolvedValue(undefined), runtimeRequest: vi.fn() }));
vi.mock('./catalog.js', async original => ({ ...await original(), createSearchRunner: vi.fn() }));
vi.mock('./public-dataset-client.js', async original => ({ ...await original(), publicDatasetRequest: vi.fn() }));
vi.mock('./projects-client.js', async original => {
  const real = await original();
  // Observe the real download-selection computation; do not replace its result.
  return { ...real, scenesForDownload: vi.fn(real.scenesForDownload) };
});
vi.mock('./explore-map.jsx', async () => {
  const { forwardRef, useImperativeHandle } = await import('react');
  return { ExploreMap: forwardRef(function CatalogMapContract(props, ref) {
    // This records the OpenLayers boundary, not network loading or real pixels.
    // The empty arrays match the real ExploreMap prop defaults.
    map.current = { ...props, loadedScenes: props.loadedScenes || [], previewScenes: props.previewScenes || [] };
    useImperativeHandle(ref, () => ({ zoomIn() {}, zoomOut() {}, fit() {}, fitArea() {} }), []);
    return <div aria-label="Catalog map contract test" />;
  }) };
});

// Historical publisher metadata for the neighbouring W122 tile, captured with
// the W123 sample in the two-tile seam check. Both originals were subsequently
// inspected in prototype/qa/dem-native-verification.json. Freeze the metadata
// here because the tracked single-tile cop-dem-response sample has only W123.
// This is catalog playback, not a fresh download or a pixel-success claim.
const neighbouringDem = {
  type: 'Feature', id: 'Copernicus_DSM_COG_10_N37_00_W122_00_DEM', collection: 'cop-dem-glo-30',
  bbox: [-122.00013888888888, 37.00013888888889, -121.00013888888888, 38.00013888888889],
  geometry: { type: 'Polygon', coordinates: [[
    [-121.00013888888888, 37.00013888888889], [-121.00013888888888, 38.00013888888889],
    [-122.00013888888888, 38.00013888888889], [-122.00013888888888, 37.00013888888889],
    [-121.00013888888888, 37.00013888888889],
  ]] },
  properties: {
    platform: 'tandem-x', gsd: 30, 'proj:epsg': 4326,
    'proj:transform': [0.0002777777777777778, 0, -122.00013888888888, 0, -0.0002777777777777778, 38.00013888888889],
    'proj:shape': [3600, 3600], 'proj:centroid': { lat: 37.5, lon: -121.5 }, 'grid:code': 'CDEM-N37W122',
    'storage:platform': 'AWS', 'storage:region': 'eu-central-1', 'storage:requester_pays': false,
    'earthsearch:payload_id': 'cop-dem-glo/workflow-cop-dem-glo-to-stac/210ee48775cac08e17df220a39532e07',
    datetime: '2021-04-22T00:00:00Z', 'processing:software': { 'cop-dem-glo-to-stac': '2023.03.28' },
    created: '2023-03-28T15:21:07.101Z', updated: '2023-04-20T18:11:38.192Z',
  },
  assets: { data: {
    href: 's3://copernicus-dem-30m/Copernicus_DSM_COG_10_N37_00_W122_00_DEM/Copernicus_DSM_COG_10_N37_00_W122_00_DEM.tif',
    type: 'image/tiff; application=geotiff; profile=cloud-optimized', title: 'Data', roles: ['data'],
    'raster:bands': [{ sampling: 'point', data_type: 'float32', spatial_resolution: 30, unit: 'meter' }],
  } },
};
const rgbScenes = sentinelFixture.features.slice(0, 2).map(item => normalizeScene(item, 'earth-search'));
const themeCases = [
  { name: 'NDVI', provider: 'planetary-vegetation', channel: 'ndvi', scenes: vegetationFixture.features.slice(0, 2).map(item => normalizeScene(item, 'planetary-vegetation')) },
  { name: 'DEM', provider: 'copernicus-dem', channel: 'height', scenes: [demFixture.features[0], neighbouringDem].map(item => normalizeScene(item, 'copernicus-dem')) },
];
const originalScrollTo = Element.prototype.scrollTo;
let catalogScenes, start;
beforeEach(() => {
  Element.prototype.scrollTo = vi.fn();
  window.history.replaceState(null, '', '/');
  const storage = new Map([['geod-global-locale', 'en']]);
  const local = { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) };
  Object.defineProperty(window, 'localStorage', { configurable: true, value: local });
  vi.stubGlobal('localStorage', local);
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 720 });
  vi.clearAllMocks();
  map.current = null;
  runtimeRequest.mockImplementation(async operation => operation === 'health' ? {} : []);
  window.__TAURI__ = { core: { invoke: vi.fn(async command => command === 'activate_desktop_frame' ? 'native' : command === 'identity_snapshot' ? signedOutIdentity() : []) } };
  const backend = { version: 1, revision: 1, runtimeAvailable: true, configured: true, busy: false, mode: 'review-first', model: { label: 'Owned QA', model: 'test-model', protocol: 'openai-compatible' }, selected: null, sessions: [], plans: [] };
  agentRequest.mockImplementation(async () => structuredClone(backend));
  publicDatasetRequest.mockImplementation(async () => [publicDataset]);
  catalogScenes = [];
  start = vi.fn(async (url, { onPage }) => {
    const result = { scenes: structuredClone(catalogScenes), complete: true, pages: 1, retrievedAt: '2026-10-13T00:00:00Z' };
    onPage(result);
    return result;
  });
  createSearchRunner.mockReturnValue({ cancel: vi.fn(), start });
});
afterEach(() => {
  cleanup();
  delete window.__TAURI__;
  vi.unstubAllGlobals();
  if (originalScrollTo) Element.prototype.scrollTo = originalScrollTo;
  else delete Element.prototype.scrollTo;
});

const ids = scenes => scenes.map(scene => scene.id).sort();
const escape = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const selectButton = scene => screen.getByRole('checkbox', { name: new RegExp(`^Select scene .*${escape(scene.id)}$`) });
const layerCheckbox = scene => screen.getByRole('checkbox', { name: new RegExp(`^Show imagery layer .*${escape(scene.id)}$`) });
const loadButton = () => screen.getAllByRole('button', { name: 'Preview selected' })[0];
const selectedDownloadIds = () => ids(scenesForDownload.mock.results.at(-1).value);
function expectNoJobsOrModelTurn() {
  expect(agentRequest.mock.calls.some(([operation]) => ['send', 'approvePlan', 'continueGoal'].includes(operation))).toBe(false);
  expect(runtimeRequest.mock.calls.some(([operation]) => !['health', 'list', 'projects'].includes(operation))).toBe(false);
}
async function explore(user, provider, scenes) {
  catalogScenes = scenes;
  render(<I18nProvider><RuntimeProvider><App /></RuntimeProvider></I18nProvider>);
  await user.click(await screen.findByRole('button', { name:/^Explore data/ }));
  const picker=await screen.findByRole('dialog',{name:'Choose a data source'});
  await user.click(picker.querySelector(`[data-source-id="${provider}"] .source-picker-select`));
  await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Choose a data source'})).toBeNull());
  await user.click(screen.getByRole('button', { name: 'Filters', exact: true }));
  fireEvent.change(screen.getByRole('textbox', { name: 'Search bounding box' }), { target: { value: '-123.5,37,-121,39' } });
  await user.click(screen.getByRole('button', { name: 'Search catalog' }));
  await screen.findByText('Catalog complete · 2 scenes');
  await waitFor(() => expect(ids(map.current.scenes)).toEqual(ids(scenes)));
}
async function selectBoth(user, scenes) {
  for (const scene of scenes) await user.click(selectButton(scene));
  expect(map.current.selectedIds.slice().sort()).toEqual(ids(scenes));
  expect(selectedDownloadIds()).toEqual(ids(scenes));
}
async function loadedLayerControls(user) {
  const show = screen.queryByRole('button', { name: 'Show inspector' });
  if (show) await user.click(show);
  await user.click(await screen.findByRole('button', { name: 'Loaded layers · 2' }));
}
async function switchToRgb(user, oldScenes) {
  catalogScenes = rgbScenes;
  await user.click(screen.getByRole('button', { name: 'Choose a data source' }));
  const dialog = screen.getByRole('dialog', { name: 'Choose a data source' });
  await user.click(within(dialog).getByRole('button', { name: 'Choose Sentinel-2 L2A · COG' }));
  await waitFor(() => expect(ids(map.current.scenes)).toEqual(ids(rgbScenes)));
  expect(map.current.previewScenes).toEqual([]);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewChannel).toBeUndefined();
  expect(map.current.selectedIds).toEqual([]);
  for (const scene of oldScenes) expect(screen.queryByRole('button', { name: new RegExp(`^Select scene .*${escape(scene.id)}$`) })).toBeNull();
}

// This full journey includes both source pickers, selection, preview visibility,
// clearing and switching providers. Hosted Windows needs more than the default
// five seconds for the combined DOM work; keep each awaited assertion bounded.
it.each(themeCases)('$name multi-selection uses the thematic preview path and keeps downloads independent of visibility and clearing', async ({ provider, channel, scenes }) => {
  const user = userEvent.setup();
  await explore(user, provider, scenes);
  // Preserve the existing automatic single-item preview, before batch loading.
  expect(map.current.previewChannel).toBe(channel);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewScenes).toEqual([]);
  await selectBoth(user, scenes);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewScenes).toEqual([]);

  await user.click(loadButton());
  await waitFor(() => expect(ids(map.current.previewScenes)).toEqual(ids(scenes)));
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewChannel).toBe(channel);
  expectNoJobsOrModelTurn();

  await loadedLayerControls(user);
  await user.click(layerCheckbox(scenes[0]));
  expect(layerCheckbox(scenes[0]).getAttribute('aria-checked')).toBe('false');
  expect(ids(map.current.previewScenes)).toEqual([scenes[1].id]);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.selectedIds.slice().sort()).toEqual(ids(scenes));
  expect(selectedDownloadIds()).toEqual(ids(scenes));

  await user.click(screen.getAllByRole('button', { name: 'Clear map previews' })[0]);
  expect(map.current.previewScenes).toEqual([]);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewChannel).toBeUndefined();
  expect(map.current.selectedIds.slice().sort()).toEqual(ids(scenes));
  expect(selectedDownloadIds()).toEqual(ids(scenes));
  for (const scene of scenes) expect(selectButton(scene).getAttribute('aria-checked')).toBe('true');

  // Re-load the preserved choices before switching, so source isolation cannot
  // pass merely because the preceding clear already removed the old previews.
  await user.click(loadButton());
  await waitFor(() => expect(ids(map.current.previewScenes)).toEqual(ids(scenes)));
  await switchToRgb(user, scenes);
  expect(start).toHaveBeenCalledTimes(2);
  expectNoJobsOrModelTurn();
}, 15_000);

it('opens a two-scene download review directly from selection, without creating a project or starting a transfer before confirmation',async()=>{
  const user=userEvent.setup();await explore(user,'earth-search',rgbScenes);
  await selectBoth(user,rgbScenes);
  await user.click(screen.getByRole('button',{name:'Save and download'}));
  const dialog=await screen.findByRole('dialog',{name:'Confirm download'});
  expect(within(dialog).getByRole('button',{name:'Confirm and download'})).toBeTruthy();
  expect(within(dialog).getByRole('combobox',{name:'Download content'})).toBeTruthy();
  expectNoJobsOrModelTurn();
  await user.click(within(dialog).getByRole('button',{name:'Cancel'}));
  await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
  expect(map.current.selectedIds.slice().sort()).toEqual(ids(rgbScenes));
  expectNoJobsOrModelTurn();
});

it('RGB multi-selection still uses the true-color COG path and clearing previews preserves both download choices', async () => {
  const user = userEvent.setup();
  await explore(user, 'earth-search', rgbScenes);
  await selectBoth(user, rgbScenes);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewScenes).toEqual([]);
  await user.click(loadButton());
  await waitFor(() => expect(ids(map.current.loadedScenes)).toEqual(ids(rgbScenes)));
  expect(map.current.previewScenes).toEqual([]);
  expect(map.current.previewChannel).toBeUndefined();

  await loadedLayerControls(user);
  await user.click(layerCheckbox(rgbScenes[0]));
  expect(ids(map.current.loadedScenes)).toEqual([rgbScenes[1].id]);
  expect(map.current.previewScenes).toEqual([]);
  expect(selectedDownloadIds()).toEqual(ids(rgbScenes));
  await user.click(screen.getAllByRole('button', { name: 'Clear map previews' })[0]);
  expect(map.current.loadedScenes).toEqual([]);
  expect(map.current.previewScenes).toEqual([]);
  expect(map.current.previewChannel).toBeUndefined();
  expect(map.current.selectedIds.slice().sort()).toEqual(ids(rgbScenes));
  expect(selectedDownloadIds()).toEqual(ids(rgbScenes));
  for (const scene of rgbScenes) expect(selectButton(scene).getAttribute('aria-checked')).toBe('true');
  expectNoJobsOrModelTurn();
});

it('row title, thumbnail and metadata locate their grid repeatedly while checkboxes and preview actions stay separate',async()=>{
  const user=userEvent.setup();await explore(user,'earth-search',rgbScenes);
  const locate=screen.getByRole('button',{name:`Locate scene ${rgbScenes[1].id}`}),row=locate.closest('.explore-result-row');
  await user.click(locate);
  expect(map.current.focusRequest.id).toBe(rgbScenes[1].id);
  expect(map.current.focusedIds).toEqual([rgbScenes[1].id]);
  expect(map.current.selectedIds).toEqual([]);expect(map.current.loadedScenes).toEqual([]);
  const revision=map.current.focusRequest.revision;
  await user.click(row.querySelector('.explore-result-thumbnail'));
  await user.click(row.querySelector('.explore-result-meta'));
  expect(map.current.focusRequest.revision).toBe(revision+2);
  const request=map.current.focusRequest;
  await user.click(selectButton(rgbScenes[0]));
  expect(map.current.focusRequest).toBe(request);expect(map.current.selectedIds).toEqual([rgbScenes[0].id]);
  await user.click(within(row).getByRole('button',{name:`Preview ${rgbScenes[1].id}`}));
  await waitFor(()=>expect(ids(map.current.loadedScenes)).toEqual([rgbScenes[1].id]));
  expect(map.current.focusRequest).toBe(request);expect(map.current.selectedIds).toEqual([rgbScenes[0].id]);
  expectNoJobsOrModelTurn();
});
