import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PublicDatasetExplorer } from './public-dataset-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { stacRequest } from './stac-client.js';
import { nextPublicCases, nextPublicDatasets } from './public-dataset-next-fixtures.js';
import { next2PublicCases, next2PublicDatasets } from './public-dataset-next2-fixtures.js';
import { deafricaCases, deafricaDatasets } from './public-dataset-deafrica-fixtures.js';
import nativeDirectReceipt from '../../docs/public-dataset-integration-20261012.json';

const mapRendered = vi.hoisted(() => vi.fn());
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({
  t: (text, values = {}) => text.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? ''),
  number: String, date: value => value || '—', locale: 'en',
}) }));
vi.mock('./public-dataset-client.js', async original => ({ ...await original(), publicDatasetRequest: vi.fn() }));
vi.mock('./stac-client.js', async original => ({ ...await original(), stacRequest: vi.fn() }));
// Transport/UI regression playback is not a live network or raster acceptance.
// The real footprint renderer has its own geometry and no-raster-load tests.
vi.mock('./public-dataset-explore-map.jsx', () => {
  const Map = props => {
    mapRendered(props);
    return <section aria-label="Product footprint map" data-testid="exploration-map">
      {props.onRequestArea && <button onClick={props.onRequestArea}>Choose a search area</button>}
      {props.listHidden && props.onShowList && <button onClick={props.onShowList}>Show product list</button>}
    </section>;
  };
  return { default: Map, PublicDatasetExploreMap: Map };
});

const ghsl = next2PublicCases.find(value => value.datasetId === 'ghsl-smod');
const pan = next2PublicCases.find(value => value.datasetId === 'maxar-open-legacy' && value.assetKey === 'pan_analytic');
const ms = next2PublicCases.find(value => value.datasetId === 'maxar-open-legacy' && value.assetKey === 'ms_analytic');
const jaxa = nextPublicCases.find(value => value.datasetId === 'jaxa-sgli-lst');
const worldpop = nativeDirectReceipt.cases.find(value => value.request.datasetId === 'worldpop').connectionResult.plan.dataset;
const fractional = { ...deafricaCases.find(value => value.datasetId === 'deafrica-fractional-cover' && value.layer === 'pv_pc_50'), assetKey: 'pv_pc_50' };
const settlementCase = deafricaCases.find(value => value.datasetId === 'wsf-settlements' && value.request.year === 2019);
const settlements = { ...settlementCase, assetKey: 'wsf2019' };
const datasets = [...nextPublicDatasets, ...next2PublicDatasets, ...deafricaDatasets, worldpop];
const refresh = vi.fn(async () => {});
const operations = () => stacRequest.mock.calls.map(([operation]) => operation);
const connections = () => publicDatasetRequest.mock.calls.filter(([operation]) => operation === 'connect');
const choiceLabel = value => `Select asset · ${value.snapshot.itemId} · ${value.assetKey}`;
const deliveryAction = () => within(within(sidebar).getByRole('region', { name: 'Selected product actions' })).getByRole('button', { name: 'Save and download' });
const clone = value => JSON.parse(JSON.stringify(value));
let sidebar;

function defer() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

// Replaying a second legitimate request must bind both direct-raster proof
// copies to it. Original publisher declarations, geometry, layers and units
// remain unchanged; only consumer request/transport snapshot identity vary.
function withArea(value, bounds) {
  const next = clone(value);
  next.request.bounds = bounds;
  next.plan.request.bounds = bounds;
  next.resolvedPlan.request.bounds = bounds;
  next.connection.plan.request.bounds = bounds;
  next.snapshot.id = 'f'.repeat(64);
  next.connection.stacConnections[0].snapshotIds = [next.snapshot.id];
  for (const proof of [next.snapshot.properties.datasetPreset, next.snapshot.provenance.reviewedDataset,
    ...next.snapshot.assets.map(asset => asset.metadata.datasetPreset)].filter(Boolean)) proof.request.bounds = bounds;
  return next;
}

function component(value, props = {}) {
  return <RuntimeContext.Provider value={{ projects: [], refresh }}>
    <PublicDatasetExplorer key={value.datasetId} datasetId={value.datasetId} areaBounds={value.request.bounds}
      autoSearch sidebarTarget={sidebar} {...props}/>
  </RuntimeContext.Provider>;
}
const mount = (value, props) => render(component(value, props));

function playback(value) {
  publicDatasetRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'list') return datasets;
    expect(operation).toBe('connect');
    expect(payload).toEqual(value.request);
    return value.connection;
  });
  stacRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'snapshot') { expect(payload).toEqual({ id: value.snapshot.id }); return value.snapshot; }
    if (operation === 'project') return value.project;
    if (operation === 'downloads') return { jobs: [value.job] };
    throw new Error(`Unexpected exploration operation ${operation}`);
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  sidebar = document.createElement('aside');
  sidebar.setAttribute('aria-label', 'Product sidebar');
  document.body.appendChild(sidebar);
});
afterEach(() => sidebar.remove());

it.each([jaxa, ghsl, pan])('$datasetId selecting a source searches metadata once, leaves the map visible and portals controls/results into the sidebar', async value => {
  playback(value);
  const [west, south, east, north] = value.request.bounds;
  const areaGeometry = { type: 'Polygon', coordinates: [[[west, south], [east, south], [east, north], [west, north], [west, south]]] };
  const view = mount(value, { areaGeometry });
  const selection = await within(sidebar).findByRole('checkbox', { name: choiceLabel(value) });
  expect(selection.getAttribute('aria-checked')).toBe('false');
  expect(publicDatasetRequest.mock.calls.map(([operation]) => operation)).toEqual(['list', 'connect']);
  expect(operations()).toEqual(['snapshot']);
  expect(view.container.querySelector('[data-testid="exploration-map"]')).toBeTruthy();
  expect(within(view.container).queryByRole('combobox', { name: 'Product layer' })).toBeNull();
  expect(within(sidebar).getByRole('combobox', { name: 'Product layer' })).toBeTruthy();
  expect(screen.queryByRole('textbox', { name: 'Dataset search bounds' })).toBeNull();
  expect(screen.queryByRole('button', { name: /Review published items|Find published files|Check source files|Read published item metadata/ })).toBeNull();
  expect(mapRendered.mock.calls.at(-1)[0]).toMatchObject({ items: [value.snapshot], areaBounds: value.request.bounds, areaGeometry });
  expect(deliveryAction().disabled).toBe(true);
  expect(within(sidebar).queryByRole('button', { name: 'Save asset selection' })).toBeNull();
});

it('metadata search and selection stay read-only until the user confirms the download review', async () => {
  playback(ghsl); const user = userEvent.setup(); mount(ghsl);
  await user.click(await within(sidebar).findByRole('checkbox', { name: choiceLabel(ghsl) }));
  expect(operations()).toEqual(['snapshot']);
  expect(screen.queryByRole('button', { name: 'Download selected originals' })).toBeNull();
  await user.click(deliveryAction());
  expect(operations()).toEqual(['snapshot']);
  await user.click(within(screen.getByRole('dialog', { name: 'Confirm download' })).getByRole('button', { name: 'Confirm and download' }));
  await within(screen.getByRole('dialog', { name: 'Confirm download' })).findByRole('region', { name: 'Download and results' });
  expect(operations()).toEqual(['snapshot', 'project', 'downloads']);
  const save = stacRequest.mock.calls.find(([operation]) => operation === 'project')[1];
  expect(save.bounds).toEqual(ghsl.request.bounds);
  expect(save.selections).toEqual([{ snapshotId: ghsl.snapshot.id, assetKey: ghsl.assetKey }]);
  expect(within(screen.getByRole('dialog', { name: 'Confirm download' })).getByRole('link', { name: 'View download tasks' })).toBeTruthy();
  expect(stacRequest.mock.calls.find(([operation]) => operation === 'downloads')[1]).toEqual({ projectId: ghsl.project.id, selections: save.selections });
});

it('a changed map area searches the new request, clears selected originals and never creates a download', async () => {
  const next = withArea(ghsl, [116.35, 39.85, 116.45, 39.95]);
  const user = userEvent.setup(); playback(ghsl); const view = mount(ghsl);
  await user.click(await screen.findByRole('checkbox', { name: choiceLabel(ghsl) }));
  playback(next); view.rerender(component(next));
  await waitFor(() => expect(connections()).toHaveLength(2));
  await waitFor(() => expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([next.snapshot]));
  expect(connections().at(-1)[1]).toEqual(next.request);
  expect(within(sidebar).getByRole('checkbox', { name: choiceLabel(next) }).getAttribute('aria-checked')).toBe('false');
  expect(deliveryAction().disabled).toBe(true);
  expect(operations()).toEqual(['snapshot', 'snapshot']);
});

it('a new search revision searches the same source and area with its edited analytical layer and retained year/month', async () => {
  const changed = vi.fn(), user = userEvent.setup();
  playback(pan); const view = mount(pan, { searchRevision: 0, onRequestChange: changed });
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(pan) });
  await user.click(within(sidebar).getByRole('combobox', { name: 'Product layer' }));
  await user.click(screen.getByRole('option', { name: ms.plan.dataset.layers.find(layer => layer.id === ms.request.layer).title }));
  playback(ms);
  await user.click(within(sidebar).getByRole('button', { name: 'Search products' }));
  await user.click(await within(sidebar).findByRole('checkbox', { name: choiceLabel(ms) }));
  expect(connections()).toHaveLength(2);
  expect(deliveryAction().disabled).toBe(false);
  const contextCount = changed.mock.calls.length;
  view.rerender(component(ms, { searchRevision: 1, onRequestChange: changed }));
  await waitFor(() => expect(connections()).toHaveLength(3));
  await waitFor(() => expect(within(sidebar).getByRole('checkbox', { name: choiceLabel(ms) }).getAttribute('aria-checked')).toBe('false'));
  expect(connections().at(-1)[1]).toEqual(ms.request);
  expect(within(sidebar).getByRole('combobox', { name: 'Product layer' }).textContent).toBe(ms.plan.dataset.layers.find(layer => layer.id === ms.request.layer).title);
  expect(within(sidebar).getByRole('combobox', { name: 'Observation year' }).textContent).toBe(String(ms.request.year));
  expect(within(sidebar).getByRole('combobox', { name: 'Observation month' }).textContent).toBe(String(ms.request.month));
  expect(changed.mock.calls.length).toBeGreaterThan(contextCount);
  expect(changed.mock.calls.at(-1)[0]).toEqual(ms.request);
  expect(deliveryAction().disabled).toBe(true);
  expect(operations()).toEqual(['snapshot', 'snapshot', 'snapshot']);
});

it('an area changed during directory loading keeps that load alive and first searches the newer area', async () => {
  const directory = defer(), next = withArea(ghsl, [116.35, 39.85, 116.45, 39.95]), changed = vi.fn();
  publicDatasetRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'list') return directory.promise;
    expect(operation).toBe('connect'); expect(payload).toEqual(next.request);
    return next.connection;
  });
  stacRequest.mockImplementation(async (operation, payload) => {
    expect(operation).toBe('snapshot'); expect(payload).toEqual({ id: next.snapshot.id });
    return next.snapshot;
  });
  const view = mount(ghsl, { onRequestChange: changed });
  await waitFor(() => expect(publicDatasetRequest).toHaveBeenCalledTimes(1));
  const directorySignal = publicDatasetRequest.mock.calls[0][2];
  view.rerender(component(next, { onRequestChange: changed }));
  expect(directorySignal.aborted).toBe(false);
  expect(connections()).toHaveLength(0); expect(operations()).toEqual([]);
  await act(async () => directory.resolve(datasets));
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(next) });
  expect(publicDatasetRequest.mock.calls.map(([operation]) => operation)).toEqual(['list', 'connect']);
  expect(connections()[0][1]).toEqual(next.request);
  expect(changed.mock.calls.at(-1)[0]).toEqual(next.request);
  expect(mapRendered.mock.calls.at(-1)[0]).toMatchObject({ areaBounds: next.request.bounds, items: [next.snapshot] });
  expect(operations()).toEqual(['snapshot']);
});

it('clearing a valid map area removes its results and map region and prevents any old-area search', async () => {
  const user = userEvent.setup(), changed = vi.fn(); playback(ghsl);
  const view = mount(ghsl, { onRequestChange: changed });
  await user.click(await within(sidebar).findByRole('checkbox', { name: choiceLabel(ghsl) }));
  view.rerender(component(ghsl, { areaBounds: undefined, onRequestChange: changed }));
  const search = within(sidebar).getByRole('button', { name: 'Search products' });
  await waitFor(() => expect(search.disabled).toBe(true));
  expect(within(sidebar).queryByRole('checkbox', { name: /Select asset/ })).toBeNull();
  expect(within(sidebar).queryByRole('button', { name: 'Save asset selection' })).toBeNull();
  expect(mapRendered.mock.calls.at(-1)[0]).toMatchObject({ items: [], areaBounds: undefined, selectedIds: [] });
  expect(changed.mock.calls.at(-1)[0]).toBeNull();
  await user.click(search);
  expect(connections()).toHaveLength(1); expect(operations()).toEqual(['snapshot']);
});

it.each(['connect', 'snapshot'])('a late old %s response cannot replace the new area results', async delayedStage => {
  const late = defer(), next = withArea(ghsl, [116.35, 39.85, 116.45, 39.95]);
  publicDatasetRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'list') return datasets;
    expect(operation).toBe('connect');
    return payload.bounds[0] === ghsl.request.bounds[0]
      ? delayedStage === 'connect' ? late.promise : ghsl.connection : next.connection;
  });
  stacRequest.mockImplementation(async (operation, payload) => {
    expect(operation).toBe('snapshot');
    return payload.id === next.snapshot.id ? next.snapshot : late.promise;
  });
  const view = mount(ghsl);
  await waitFor(() => expect(delayedStage === 'snapshot' ? operations() : connections()).toHaveLength(1));
  const oldSignal = connections()[0][2];
  view.rerender(component(next));
  await waitFor(() => expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([next.snapshot]));
  expect(oldSignal.aborted).toBe(true);
  await act(async () => late.resolve(delayedStage === 'connect' ? ghsl.connection : ghsl.snapshot));
  expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([next.snapshot]);
  expect(operations()).toEqual(delayedStage === 'connect' ? ['snapshot'] : ['snapshot', 'snapshot']);
  expect(screen.queryByRole('alert')).toBeNull();
});

it('switching the selected source key cancels its pending metadata and leaves only the new source', async () => {
  const late = defer();
  publicDatasetRequest.mockImplementation(async (operation, payload) => operation === 'list' ? datasets
    : payload.datasetId === ghsl.datasetId ? late.promise : jaxa.connection);
  stacRequest.mockResolvedValue(jaxa.snapshot);
  const view = mount(ghsl);
  await waitFor(() => expect(connections()).toHaveLength(1));
  const oldSignal = connections()[0][2];
  view.rerender(component(jaxa));
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(jaxa) });
  expect(oldSignal.aborted).toBe(true);
  await act(async () => late.resolve(ghsl.connection));
  expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([jaxa.snapshot]);
  expect(within(sidebar).queryByRole('checkbox', { name: choiceLabel(ghsl) })).toBeNull();
  expect(operations()).toEqual(['snapshot']);
});

it.each([{ bounds: [] }, { bounds: [116, 40, 115, 39] }, { bounds: [116, NaN, 117, 40] }])('invalid or missing map bounds $bounds cannot automatically start a request', async ({ bounds }) => {
  playback(ghsl); mount(ghsl, { areaBounds: bounds });
  const search = await within(sidebar).findByRole('button', { name: 'Search products' });
  await waitFor(() => expect(search.disabled).toBe(true));
  expect(connections()).toHaveLength(0); expect(operations()).toEqual([]);
  expect(screen.getByLabelText('Product footprint map')).toBeTruthy();
});

it('WorldPop never guesses the map country and starts its first search only after an explicit valid code', async () => {
  const pending = defer();
  publicDatasetRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'list') return datasets;
    expect(operation).toBe('connect');
    expect(payload).toEqual(nativeDirectReceipt.cases.find(value => value.request.datasetId === 'worldpop').connectionResult.plan.request);
    return pending.promise;
  });
  const user = userEvent.setup(); mount({ datasetId: worldpop.id, request: { bounds: [-169.95, -19.2, -169.75, -18.95] } });
  const country = await within(sidebar).findByRole('textbox', { name: 'Dataset country code' });
  expect(country.value).toBe('');
  expect(within(sidebar).getByRole('button', { name: 'Search products' }).disabled).toBe(true);
  await user.type(country, 'NI');
  expect(connections()).toHaveLength(0); expect(operations()).toEqual([]);
  await user.type(country, 'U');
  await waitFor(() => expect(connections()).toHaveLength(1));
  expect(operations()).toEqual([]);
});

it('changing Maui analytic layers clears the old list until Search products and only permits the reviewed layer', async () => {
  playback(pan); const user = userEvent.setup(); mount(pan);
  await user.click(await within(sidebar).findByRole('checkbox', { name: choiceLabel(pan) }));
  await user.click(within(sidebar).getByRole('combobox', { name: 'Product layer' }));
  await user.click(screen.getByRole('option', { name: ms.plan.dataset.layers.find(layer => layer.id === ms.request.layer).title }));
  expect(within(sidebar).queryByRole('checkbox', { name: /Select asset/ })).toBeNull();
  expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([]);
  expect(connections()).toHaveLength(1); expect(operations()).toEqual(['snapshot']);
  playback(ms);
  await user.click(within(sidebar).getByRole('button', { name: 'Search products' }));
  const selection = await within(sidebar).findByRole('checkbox', { name: choiceLabel(ms) });
  expect(connections().at(-1)[1]).toEqual(ms.request);
  expect(selection.disabled).toBe(false);
  expect(within(sidebar).queryByRole('checkbox', { name: choiceLabel(pan) })).toBeNull();
  expect(within(sidebar).queryByRole('checkbox', { name: / · visual$/ })).toBeNull();
  expect(within(sidebar).getAllByRole('checkbox', { name: /Select asset/ })).toHaveLength(1);
  expect(deliveryAction().disabled).toBe(true);
});

it('JAXA retains observation month, original DN and actual period when moved into the exploration sidebar', async () => {
  playback(jaxa); const user = userEvent.setup(); mount(jaxa);
  const selection = await within(sidebar).findByRole('checkbox', { name: choiceLabel(jaxa) });
  expect(within(sidebar).getByRole('combobox', { name: 'Observation year' }).textContent).toBe('2025');
  expect(within(sidebar).getByRole('combobox', { name: 'Observation month' }).textContent).toBe('12');
  expect(selection.closest('.public-dataset-review').querySelector('.stac-item-id').textContent).toContain(`${jaxa.snapshot.startDatetime.slice(0, 10)} – ${jaxa.snapshot.endDatetime.slice(0, 10)}`);
  await user.click(within(selection.closest('.explore-result-row')).getByRole('button', { name: 'Asset source details' }));
  expect(screen.getByRole('dialog',{name:'Asset source details'}).textContent).toContain(jaxa.plan.dataset.layers[0].unit);
  expect(screen.getByRole('dialog',{name:'Asset source details'}).textContent).toContain(jaxa.plan.dataset.layers[0].valueInterpretation);
  expect(mapRendered.mock.calls.at(-1)[0].items[0]).toEqual(jaxa.snapshot);
});

it('map actions use parent area/list callbacks while its sidebar is collapsed', async () => {
  playback(ghsl); const user = userEvent.setup(), onRequestArea = vi.fn(), onShowList = vi.fn();
  mount(ghsl, { listHidden: true, onRequestArea, onShowList });
  await waitFor(() => expect(mapRendered.mock.calls.at(-1)[0].items).toEqual([ghsl.snapshot]));
  expect(within(sidebar).queryByRole('checkbox')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Choose a search area' }));
  await user.click(screen.getByRole('button', { name: 'Show product list' }));
  expect(onRequestArea).toHaveBeenCalledTimes(1); expect(onShowList).toHaveBeenCalledTimes(1);
  expect(operations()).toEqual(['snapshot']);
});

it('a selected file reaches the map only after explicit loading and repeated loading does not duplicate its pin or create jobs', async () => {
  playback(ghsl); const user = userEvent.setup(); mount(ghsl);
  const selection = await within(sidebar).findByRole('checkbox', { name: choiceLabel(ghsl) });
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  await user.click(selection);
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  expect(operations()).toEqual(['snapshot']);
  const load = within(sidebar).getByRole('button', { name: 'Load selected files to map' });
  await user.click(load);
  const pin = [ghsl.snapshot.id, ghsl.assetKey];
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([expect.objectContaining({
    id: JSON.stringify(pin), snapshotId: pin[0], assetKey: pin[1], visible: true,
  })]);
  await user.click(load);
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toHaveLength(1);
  expect(selection.getAttribute('aria-checked')).toBe('true');
  expect(operations()).toEqual(['snapshot']);
  expect(publicDatasetRequest.mock.calls.map(([operation]) => operation)).toEqual(['list', 'connect']);
});

it('three reviewed components of one annual item remain separate map pins and keep the original download choices', async () => {
  playback(fractional); const user = userEvent.setup(); mount(fractional);
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(fractional) });
  const keys = ['pv_pc_50', 'qa', 'count_valid'];
  for (const assetKey of keys) await user.click(within(sidebar).getByRole('checkbox', { name: `Select asset · ${fractional.snapshot.itemId} · ${assetKey}` }));
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  await user.click(within(sidebar).getByRole('button', { name: 'Load selected files to map' }));
  const entries = mapRendered.mock.calls.at(-1)[0].rasterLayers;
  expect(entries.map(entry => [entry.snapshotId, entry.assetKey])).toEqual(keys.map(assetKey => [fractional.snapshot.id, assetKey]));
  expect(new Set(entries.map(entry => entry.id)).size).toBe(3);
  expect(entries.every(entry => entry.id === JSON.stringify([fractional.snapshot.id, entry.assetKey]))).toBe(true);
  expect(mapRendered.mock.calls.at(-1)[0].selectedIds).toEqual([fractional.snapshot.id]);
  expect(within(sidebar).getByText('3 selected')).toBeTruthy();
  expect(operations()).toEqual(['snapshot']);
});

it('bulk selection chooses only the current reviewed product layer and clearing selection keeps already loaded previews', async () => {
  playback(fractional); const user = userEvent.setup(); mount(fractional);
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(fractional) });
  await user.click(within(sidebar).getByRole('button', { name: 'Select reviewed files' }));
  const choices = within(sidebar).getAllByRole('checkbox', { name: /Select asset/ });
  expect(choices.filter(choice => choice.getAttribute('aria-checked') === 'true').map(choice => choice.getAttribute('aria-label'))).toEqual([choiceLabel(fractional)]);
  expect(within(sidebar).getByText('1 selected')).toBeTruthy();
  await user.click(within(sidebar).getByRole('button', { name: 'Load selected files to map' }));
  const entries = mapRendered.mock.calls.at(-1)[0].rasterLayers;
  await user.click(within(sidebar).getByRole('button', { name: 'Clear selection' }));
  expect(choices.every(choice => choice.getAttribute('aria-checked') === 'false')).toBe(true);
  expect(deliveryAction().disabled).toBe(true);
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual(entries);
  expect(operations()).toEqual(['snapshot']);
});

it('a five-file download selection exceeds the four-preview limit without silently dropping any original file', async () => {
  playback(fractional); const user = userEvent.setup(); mount(fractional);
  await within(sidebar).findByRole('checkbox', { name: choiceLabel(fractional) });
  const keys = ['pv_pc_50', 'npv_pc_50', 'bs_pc_50', 'count_valid', 'qa'];
  for (const assetKey of keys) await user.click(within(sidebar).getByRole('checkbox', { name: `Select asset · ${fractional.snapshot.itemId} · ${assetKey}` }));
  expect(within(sidebar).getByText('5 selected')).toBeTruthy();
  const load = within(sidebar).getByRole('button', { name: 'Load selected files to map' });
  expect(load.disabled).toBe(true);
  await user.click(load);
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  expect(within(sidebar).getAllByRole('checkbox', { name: /Select asset/ }).filter(choice => choice.getAttribute('aria-checked') === 'true')).toHaveLength(5);
  expect(deliveryAction().disabled).toBe(false);
  expect(operations()).toEqual(['snapshot']);
});

it('hiding, changing opacity, unloading and clearing previews leave the saved file selection available for its explicit download', async () => {
  playback(ghsl); const user = userEvent.setup(); mount(ghsl);
  const choice = await within(sidebar).findByRole('checkbox', { name: choiceLabel(ghsl) });
  await user.click(choice);
  await user.click(within(sidebar).getByRole('button', { name: 'Load selected files to map' }));
  const original = mapRendered.mock.calls.at(-1)[0].rasterLayers[0];
  await user.click(deliveryAction());
  await user.click(within(screen.getByRole('dialog', { name: 'Confirm download' })).getByRole('button', { name: 'Confirm and download' }));
  await within(screen.getByRole('dialog', { name: 'Confirm download' })).findByRole('region', { name: 'Download and results' });
  await act(async () => mapRendered.mock.calls.at(-1)[0].onRasterChange(original.id, { visible: false, opacity: 0.35 }));
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers[0]).toMatchObject({ id: original.id, visible: false, opacity: 0.35 });
  expect(choice.getAttribute('aria-checked')).toBe('true');
  expect(within(screen.getByRole('dialog', {name:'Confirm download'})).getByRole('link', { name: 'View download tasks' })).toBeTruthy();
  await act(async () => mapRendered.mock.calls.at(-1)[0].onRemoveRaster(original.id));
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  expect(choice.getAttribute('aria-checked')).toBe('true');
  await user.click(screen.getByRole('button', {name:'Close'}));
  await waitFor(()=>expect(screen.queryByRole('dialog', {name:'Confirm download'})).toBeNull());
  await user.click(within(sidebar).getByRole('button', { name: 'Load selected files to map' }));
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers[0].generation).toBeGreaterThan(original.generation);
  await act(async () => mapRendered.mock.calls.at(-1)[0].onClearRasters());
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]);
  expect(choice.getAttribute('aria-checked')).toBe('true');
  expect(operations()).toEqual(['snapshot', 'project', 'downloads']);
  await user.click(deliveryAction());
  await screen.findByRole('dialog', {name:'Confirm download'});
  expect(within(screen.getByRole('dialog', { name: 'Confirm download' })).getByRole('link', { name: 'View download tasks' })).toBeTruthy();
  expect(stacRequest.mock.calls.find(([operation]) => operation === 'downloads')[1]).toEqual({
    projectId: ghsl.project.id, selections: [{ snapshotId: ghsl.snapshot.id, assetKey: ghsl.assetKey }],
  });
});

it.each(['area', 'source', 'layer', 'year', 'revision'])('changing %s removes old map preview entries independently of any original download', async change => {
  const value = change === 'year' ? settlements : change === 'layer' ? fractional : ghsl;
  playback(value); const user = userEvent.setup(), view = mount(value, { searchRevision: 0 });
  await user.click(await within(sidebar).findByRole('checkbox', { name: choiceLabel(value) }));
  await user.click(within(sidebar).getByRole('button', { name: 'Load selected files to map' }));
  expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toHaveLength(1);
  if (change === 'area') {
    const next = withArea(ghsl, [116.35, 39.85, 116.45, 39.95]);
    playback(next); view.rerender(component(next));
    await within(sidebar).findByRole('checkbox', { name: choiceLabel(next) });
  } else if (change === 'source') {
    playback(jaxa); view.rerender(component(jaxa));
    await within(sidebar).findByRole('checkbox', { name: choiceLabel(jaxa) });
  } else if (change === 'revision') {
    view.rerender(component(value, { searchRevision: 1 }));
    await waitFor(() => expect(connections()).toHaveLength(2));
  } else {
    await user.click(within(sidebar).getByRole('combobox', { name: change === 'layer' ? 'Product layer' : 'Reference year' }));
    await user.click(screen.getByRole('option', { name: change === 'layer'
      ? fractional.plan.dataset.layers.find(layer => layer.id === 'qa').title : '2015', exact: true }));
  }
  await waitFor(() => expect(mapRendered.mock.calls.at(-1)[0].rasterLayers).toEqual([]));
  expect(stacRequest.mock.calls.some(([operation]) => ['project', 'downloads'].includes(operation))).toBe(false);
});
