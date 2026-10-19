import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './main.jsx';
import { I18nProvider } from './i18n.jsx';
import { RuntimeProvider } from './runtime-ui.jsx';
import { signedOutIdentity } from './identity-client.js';
import { agentRequest } from './agent-client.js';
import { runtimeRequest } from './runtime-client.js';
import { createSearchRunner, normalizeScene } from './catalog.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { publicDataset } from './public-dataset-fixtures.js';
import { SOURCE_DIRECTORY } from './source-directory.js';
import sentinelFixture from '../public/samples/earth-search-response.json';

// This suite tests the real App routing, picker, AOI ownership and panel state.
// The source adapters and OpenLayers canvas are boundary probes; it makes no
// claim about fresh network discovery, downloaded files or rendered pixels.
const probe = vi.hoisted(() => ({ sources: {}, catalog: null }));
vi.mock('./agent-client.js', () => ({ agentRequest: vi.fn(), subscribeAgentUpdates: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('./runtime-client.js', async original => ({ ...await original(), desktopAvailable: () => true, syncDesktopLocale: vi.fn().mockResolvedValue(undefined), runtimeRequest: vi.fn() }));
vi.mock('./catalog.js', async original => ({ ...await original(), createSearchRunner: vi.fn() }));
vi.mock('./public-dataset-client.js', async original => ({ ...await original(), publicDatasetRequest: vi.fn() }));
vi.mock('./area-picker.jsx', () => ({ AreaPicker: ({ onApply }) => <button onClick={() => onApply({
  bounds: [116.25, 39.8, 116.5, 40],
  geometry: { type: 'Polygon', coordinates: [[[116.25, 39.8], [116.5, 39.8], [116.4, 40], [116.25, 39.8]]] },
  place: { kind: 'custom', name: 'Chosen polygon' },
})}>Apply test polygon</button> }));
vi.mock('./explore-map.jsx', async () => {
  const { forwardRef, useImperativeHandle } = await import('react');
  return { ExploreMap: forwardRef(function CatalogBoundary(props, ref) {
    probe.catalog = { ...props, loadedScenes: props.loadedScenes || [], previewScenes: props.previewScenes || [] };
    useImperativeHandle(ref, () => ({ zoomIn() {}, zoomOut() {}, fit() {}, fitArea() {} }), []);
    return <div aria-label="Catalog canvas boundary" />;
  }) };
});

async function sourceProbe(kind) {
  const { ExploreSourceFrame } = await import('./explore-source-workspace.jsx');
  const { useEffect } = await import('react');
  return function InlineSourceBoundary(props) {
    probe.sources[kind] = props;
    useEffect(() => { if (kind === 'dataset') props.onCatalogChange?.([publicDataset]); }, [props.onCatalogChange]);
    return <ExploreSourceFrame {...props} title={kind} map={<section aria-label={`${kind} canvas boundary`}>
      <button onClick={props.onRequestArea}>Change source area</button>
      {props.listHidden && <button onClick={props.onShowList}>Show source results</button>}
    </section>}><span>{kind} search controls boundary</span></ExploreSourceFrame>;
  };
}
vi.mock('./public-dataset-ui.jsx', async original => ({ ...await original(), PublicDatasetExplorer: await sourceProbe('dataset') }));
vi.mock('./stac-ui.jsx', async original => ({ ...await original(), StacSourceExplorer: await sourceProbe('stac') }));
vi.mock('./wcs-ui.jsx', async original => ({ ...await original(), WcsSourceExplorer: await sourceProbe('wcs') }));
vi.mock('./wms-ui.jsx', async original => ({ ...await original(), MapServiceExplorer: await sourceProbe('map') }));
vi.mock('./features-ui.jsx', async original => ({ ...await original(), FeatureServiceExplorer: await sourceProbe('vector') }));
vi.mock('./tiles-ui.jsx', async original => ({ ...await original(), TileSourceExplorer: await sourceProbe('tiles') }));
vi.mock('./earth-engine-ui.jsx', async original => ({ ...await original(), EarthEngineExplorer: await sourceProbe('earth-engine') }));

const scenes = sentinelFixture.features.slice(0, 2).map(item => normalizeScene(item, 'earth-search'));
const polygon = { type: 'Polygon', coordinates: [[[116.25, 39.8], [116.5, 39.8], [116.4, 40], [116.25, 39.8]]] };
const bounds = [116.25, 39.8, 116.5, 40];
const originalScrollTo = Element.prototype.scrollTo;
let runAll;
beforeEach(() => {
  Element.prototype.scrollTo = vi.fn();
  window.history.replaceState(null, '', '/');
  const storage = new Map([['geod-global-locale', 'en']]);
  const local = { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) };
  Object.defineProperty(window, 'localStorage', { configurable: true, value: local });
  vi.stubGlobal('localStorage', local);
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 900 });
  vi.clearAllMocks();
  probe.sources = {}; probe.catalog = null;
  runtimeRequest.mockImplementation(async operation => operation === 'health' ? {} : []);
  window.__TAURI__ = { core: { invoke: vi.fn(async command => command === 'activate_desktop_frame' ? 'native' : command === 'identity_snapshot' ? signedOutIdentity() : []) } };
  const backend = { version: 1, revision: 1, runtimeAvailable: true, configured: true, busy: false, mode: 'review-first', model: { label: 'Owned QA', model: 'test-model', protocol: 'openai-compatible' }, selected: null, sessions: [], plans: [] };
  agentRequest.mockImplementation(async () => structuredClone(backend));
  publicDatasetRequest.mockImplementation(async () => [publicDataset]);
  runAll = vi.fn(async (url, { onPage }) => {
    const result = { scenes: structuredClone(scenes), complete: true, pages: 1, retrievedAt: '2026-10-13T00:00:00Z' };
    onPage(result); return result;
  });
  createSearchRunner.mockReturnValue({ cancel: vi.fn(), runAll });
});
afterEach(() => {
  cleanup(); delete window.__TAURI__; vi.unstubAllGlobals();
  if (originalScrollTo) Element.prototype.scrollTo = originalScrollTo;
  else delete Element.prototype.scrollTo;
});

const mount = () => render(<I18nProvider><RuntimeProvider><App /></RuntimeProvider></I18nProvider>);

it('opens Explore with the Agent closed after leaving Home and respects an explicit sidebar open while switching sources', async () => {
  Object.defineProperty(window,'innerWidth',{configurable:true,value:1280});
  const user=userEvent.setup();mount();
  await chooseHome(user,'earth-search');
  expect(screen.getByRole('button',{name:'Open Agent'}).getAttribute('aria-expanded')).toBe('false');
  expect(document.querySelector('#geod-agent-panel')).toBeNull();
  await user.click(screen.getByRole('button',{name:'Open Agent'}));
  expect(within(document.querySelector('.app-header')).getByRole('button',{name:'Close Agent'}).getAttribute('aria-expanded')).toBe('true');
  await choose(user,'jaxa-modis-ndvi');
  expect(within(document.querySelector('.app-header')).getByRole('button',{name:'Close Agent'}).getAttribute('aria-expanded')).toBe('true');
  await user.click(screen.getByRole('link',{name:'Home',exact:true}));
  await user.click(screen.getByRole('link',{name:'Explore',exact:true}));
  expect(screen.getByRole('button',{name:'Open Agent'}).getAttribute('aria-expanded')).toBe('false');
});
const byId = id => SOURCE_DIRECTORY.find(source => source.id === id);
async function choose(user, id) {
  await user.click(screen.getByRole('button', { name: 'Choose a data source' }));
  const dialog = screen.getByRole('dialog', { name: 'Choose a data source' });
  const card = dialog.querySelector(`[data-source-id="${id}"]`);
  expect(card).not.toBeNull();
  await user.click(card.querySelector('.source-picker-select'));
  await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Choose a data source' })).toBeNull());
}
async function chooseHome(user,id) {
  await user.click(await screen.findByRole('button',{name:/^Explore data/}));
  const dialog=await screen.findByRole('dialog',{name:'Choose a data source'});
  await user.click(dialog.querySelector(`[data-source-id="${id}"] .source-picker-select`));
  await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Choose a data source'})).toBeNull());
}
async function applyArea(user) {
  await user.click(document.querySelector('.area-picker'));
  await user.click(await screen.findByRole('button', { name: 'Apply test polygon' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
}
function expectNoMutation() {
  expect(agentRequest.mock.calls.some(([op]) => ['send', 'approvePlan', 'continueGoal'].includes(op))).toBe(false);
  expect(runtimeRequest.mock.calls.some(([op]) => !['health', 'list', 'projects'].includes(op))).toBe(false);
  expect(window.__TAURI__.core.invoke.mock.calls.every(([command]) => command.startsWith('list_') || ['identity_snapshot', 'distribution_snapshot', 'activate_desktop_frame', 'set_desktop_appearance'].includes(command))).toBe(true);
}
async function expectInline(kind) {
  await screen.findByRole('main', { name: 'Data source exploration' });
  const controls = screen.getByRole('region', { name: 'Source search and results' });
  const discovery = document.querySelector('#explore-discovery');
  expect(discovery.contains(controls)).toBe(true);
  expect(screen.getByRole('region', { name: `${kind} canvas boundary` })).toBeTruthy();
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(location.hash).toBe('#Explore');
}

it('all connected source families share the Explore sidebar and canvas, retaining the same AOI and polygon through switches', async () => {
  const user = userEvent.setup(); mount();
  await chooseHome(user,'earth-search');
  await applyArea(user);
  await waitFor(() => expect(probe.catalog?.area).toEqual(bounds));
  expect(probe.catalog.areaGeometry).toEqual(polygon);
  expect(runAll).toHaveBeenCalledOnce();

  for (const [id, kind] of [
    ['cgls-landcover', 'dataset'], ['jaxa-modis-ndvi', 'dataset'], ['custom-stac-api', 'stac'], ['wcs-service', 'wcs'],
    ['wms-service', 'map'], ['ogc-features', 'vector'], ['geoboundaries', 'vector'], ['protomaps', 'tiles'], ['earth-engine-export', 'earth-engine'],
  ]) {
    await choose(user, id); await expectInline(kind);
    expect(probe.sources[kind].areaBounds).toEqual(bounds);
    expect(probe.sources[kind].areaGeometry || probe.sources[kind].areaPolygon?.geometry).toEqual(polygon);
    expect(screen.getByRole('button', { name: 'Choose a data source' }).textContent).toContain(byId(id).name);
    expect(screen.queryByRole('textbox', { name: 'Search scenes' })).toBeNull();
    expect(runAll).toHaveBeenCalledOnce();
    expectNoMutation();
  }

  await choose(user, 'earth-search');
  await screen.findByLabelText('Catalog canvas boundary');
  await waitFor(() => expect(probe.catalog?.area).toEqual(bounds));
  expect(probe.catalog.areaGeometry).toEqual(polygon);
  expect(probe.catalog.selectedIds).toEqual([]);
  expect(probe.catalog.loadedScenes).toEqual([]);
  expect(probe.catalog.previewScenes).toEqual([]);
  expect(screen.queryByRole('main', { name: 'Data source exploration' })).toBeNull();
  expect(runAll).toHaveBeenCalledTimes(2);
  expectNoMutation();
}, 20000);

it.each([
  ['custom-stac-api', 'stac'], ['wcs-service', 'wcs'], ['wms-service', 'map'],
  ['ogc-features', 'vector'], ['protomaps', 'tiles'], ['earth-engine-export', 'earth-engine'],
])('%s restores the same source results after collapsing the shared Explore list', async (id, kind) => {
  const user = userEvent.setup(); mount();
  await chooseHome(user,'earth-search');
  await applyArea(user); await choose(user, id); await expectInline(kind);
  const sourceGeneration = probe.sources[kind].initialKind || probe.sources[kind].initialProtocol;
  await user.click(screen.getByRole('button', { name: 'Hide scene list' }));
  await waitFor(() => expect(screen.queryByRole('region', { name: 'Source search and results' })).toBeNull());
  expect(screen.getByRole('region', { name: `${kind} canvas boundary` })).toBeTruthy();
  expect(probe.sources[kind].listHidden).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Show source results' }));
  await expectInline(kind);
  expect(probe.sources[kind].listHidden).toBe(false);
  expect(probe.sources[kind].initialKind || probe.sources[kind].initialProtocol).toBe(sourceGeneration);
  expect(probe.sources[kind].areaBounds).toEqual(bounds);
  expect(runAll).toHaveBeenCalledOnce(); expectNoMutation();
});

it.each([
  ['custom-stac-api', 'stac', { initialKind: 'api' }], ['static-stac', 'stac', { initialKind: 'catalog' }],
  ['stac-item', 'stac', { initialKind: 'item' }], ['direct-cog', 'stac', { initialKind: 'raster' }],
  ['wcs-service', 'wcs', {}], ['nasa-gibs', 'map', { initialProtocol: 'WMS', initialPreset: 'gibs' }],
  ['wmts-service', 'map', { initialProtocol: 'WMTS' }], ['xyz-service', 'map', { initialProtocol: 'XYZ' }],
  ['tms-service', 'map', { initialProtocol: 'TMS' }], ['arcgis-map', 'map', { initialProtocol: 'ArcGIS' }],
  ['osm-overpass', 'vector', { initialProtocol: 'Overpass' }], ['ogc-features', 'vector', { initialProtocol: 'OGC' }],
  ['wfs-service', 'vector', { initialProtocol: 'WFS2' }], ['arcgis-features', 'vector', { initialProtocol: 'ArcGIS' }],
  ['geoboundaries', 'vector', { initialProtocol: 'geoBoundaries' }], ['jaxa-modis-ndvi', 'dataset', { datasetId: 'jaxa-modis-ndvi' }],
  ['protomaps', 'tiles', {}], ['earth-engine-export', 'earth-engine', {}],
])('the home entry for %s opens its correct inline adapter in Explore without starting a search or download', async (id, kind, properties) => {
  const user = userEvent.setup(); mount();
  await chooseHome(user,id);
  await expectInline(kind);
  expect(probe.sources[kind]).toMatchObject(properties);
  expect(runAll).not.toHaveBeenCalled();
  expectNoMutation();
});

it('the new monthly vegetation and complete administrative sources retain the shared AOI through picker switches in Explore',async()=>{
  const user=userEvent.setup();mount();
  await chooseHome(user,'earth-search');await applyArea(user);
  await waitFor(()=>expect(probe.catalog?.area).toEqual(bounds));
  await choose(user,'jaxa-modis-ndvi');await expectInline('dataset');
  expect(probe.sources.dataset.datasetId).toBe('jaxa-modis-ndvi');expect(probe.sources.dataset.autoSearch).toBe(true);
  expect(probe.sources.dataset.areaBounds).toEqual(bounds);expect(probe.sources.dataset.areaGeometry).toEqual(polygon);
  await choose(user,'geoboundaries');await expectInline('vector');
  expect(probe.sources.vector.initialProtocol).toBe('geoBoundaries');expect(probe.sources.vector.areaBounds).toEqual(bounds);expect(probe.sources.vector.areaGeometry).toEqual(polygon);
  await choose(user,'jaxa-modis-ndvi');await expectInline('dataset');
  expect(probe.sources.dataset.datasetId).toBe('jaxa-modis-ndvi');expect(probe.sources.dataset.areaBounds).toEqual(bounds);expect(probe.sources.dataset.areaGeometry).toEqual(polygon);
  expect(runAll).toHaveBeenCalledOnce();expectNoMutation();
});

it('a late satellite catalog page cannot reappear after changing to a service source', async () => {
  const user = userEvent.setup(); let page, resolve;
  runAll.mockImplementation((url, options) => { page = options.onPage; return new Promise(done => { resolve = done; }); });
  mount(); await chooseHome(user,'earth-search');
  await applyArea(user); expect(runAll).toHaveBeenCalledOnce();
  await choose(user, 'custom-stac-api'); await expectInline('stac');
  const late = { scenes: [{ ...scenes[0], id: 'old-satellite-response' }], complete: true, pages: 1 };
  await act(async () => { page(late); resolve(late); });
  expect(screen.queryByText('old-satellite-response')).toBeNull();
  expect(screen.queryByLabelText('Catalog canvas boundary')).toBeNull();
  expect(probe.sources.stac.areaBounds).toEqual(bounds);
  expectNoMutation();
});

it('restoring an imagery project replaces the previously selected service adapter with its imagery results and project AOI', async () => {
  const project={id:'a1234567-1234-1234-1234-123456789abc',name:'Saved imagery project',bounds:[10,20,11,21],geometry:{type:'Polygon',coordinates:[[[10,20],[11,20],[11,21],[10,20]]]},scenes:[{
    itemId:'saved-imagery-item',date:'2024-02-28T00:00:00Z',bbox:[10,20,11,21],cloud:0,crs:'EPSG:4326',assets:{visual:{href:'https://example.com/pinned.tif',mediaType:'image/tiff'}},
  }]};
  runtimeRequest.mockImplementation(async operation=>operation==='projects'?[project]:operation==='health'?{}:[]);
  const user=userEvent.setup();mount();
  await chooseHome(user,'custom-stac-api');await expectInline('stac');
  await act(async()=>{location.hash=`Explore?project=${project.id}`;window.dispatchEvent(new HashChangeEvent('hashchange'));});
  await screen.findByLabelText('Catalog canvas boundary');
  await waitFor(()=>expect(probe.catalog?.area).toEqual(project.bounds));
  expect(probe.catalog.areaGeometry).toEqual(project.geometry);
  expect(screen.queryByRole('main',{name:'Data source exploration'})).toBeNull();
  expect(screen.queryByRole('region',{name:'stac canvas boundary'})).toBeNull();
  expect(screen.getByRole('button',{name:'Choose a data source'}).textContent).toContain('Sentinel-2');
  expect(runAll).toHaveBeenCalledOnce();expectNoMutation();
});
