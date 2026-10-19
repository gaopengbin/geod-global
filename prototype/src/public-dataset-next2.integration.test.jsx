import React from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PublicDatasetExplorer } from './public-dataset-ui.jsx';
import { StacRasterInspection } from './stac-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { publicDatasetRequest } from './public-dataset-client.js';
import { stacRequest } from './stac-client.js';
import { next2PublicDatasets, next2PublicCases, next2PublicFixtureScope } from './public-dataset-next2-fixtures.js';
import publicDatasetChinese from './locales/public-dataset.zh-CN.js';

const language = vi.hoisted(() => ({ value:'en' }));
vi.mock('./i18n.jsx', () => ({ useI18n:() => ({
  t:(text, values = {}) => (language.value === 'zh-CN' ? publicDatasetChinese[text] || text : text).replace(/\{(\w+)\}/g, (_, key) => values[key] ?? ''),
  number:String, date:value => value || '—', locale:language.value,
}) }));
vi.mock('./public-dataset-client.js', async original => ({ ...await original(), publicDatasetRequest:vi.fn() }));
vi.mock('./stac-client.js', async original => ({ ...await original(), stacRequest:vi.fn() }));
// API/UI replay only. The separate consumer map-model tests use actual native
// grid/proof responses; this double is not a map render or live browser pass.
vi.mock('./stac-map.jsx', () => ({ default:({ data }) => <div aria-label="Georeferenced original raster" data-original-bounds={data.bounds.join(',')}/> }));

const refresh = vi.fn(async () => {});
const byId = id => next2PublicCases.find(value => value.datasetId === id);
const operations = () => stacRequest.mock.calls.map(([operation]) => operation);
const mount = (value, props = {}) => render(<RuntimeContext.Provider value={{ projects:[], refresh }}><PublicDatasetExplorer datasetId={value.datasetId} areaBounds={value.request.bounds} {...props}/></RuntimeContext.Provider>);
const resolveLabel = value => value.plan.dataset.discovery === 'catalog-raster' ? 'Review catalog search' : value.plan.dataset.discovery === 'stac-items' ? 'Review published items' : 'Find published files';
const connectLabel = value => value.plan.dataset.discovery === 'catalog-raster' ? 'Search product catalog' : value.plan.dataset.discovery === 'stac-items' ? 'Read published item metadata' : 'Check source files';
function playback(value) {
  publicDatasetRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'list') return next2PublicDatasets;
    expect(payload).toEqual(value.request);
    if (operation === 'resolve') return value.resolvedPlan;
    if (operation === 'connect') return value.connection;
    throw new Error(`Unexpected dataset playback operation ${operation}`);
  });
  stacRequest.mockImplementation(async (operation, payload) => {
    if (operation === 'snapshot') return value.snapshot;
    if (operation === 'project') return value.project;
    if (operation === 'downloads') return { jobs:[value.job] };
    if (operation === 'inspect') return value.inspection;
    if (operation === 'pixel') return value.pixels.find(pixel => pixel.column === payload.column && pixel.row === payload.row);
    throw new Error(`Unexpected playback operation ${operation}`);
  });
}
async function selectActualLayer(value, user) {
  const dataset = value.plan.dataset;
  if (value.request.layer !== dataset.defaultLayer) {
    await user.click(await screen.findByRole('combobox', { name:'Product layer' }));
    await user.click(screen.getByRole('option', { name:dataset.layers.find(layer => layer.id === value.request.layer).title }));
  }
}
beforeEach(() => { vi.clearAllMocks(); language.value = 'en'; });

it.each(next2PublicCases)('$label: AOI is preserved through separate review, connect, select, save and original-download actions', async value => {
  expect(next2PublicFixtureScope.playbackIsLiveBrowserAcceptance).toBe(false);
  playback(value); const user = userEvent.setup(), changed = vi.fn(); mount(value, { onRequestChange:changed });
  const review = await screen.findByRole('button', { name:resolveLabel(value) });
  await waitFor(() => expect(review.disabled).toBe(false));
  await selectActualLayer(value, user);
  expect(screen.getByRole('textbox', { name:'Dataset search bounds' }).value).toBe(value.request.bounds.join(', '));
  expect(changed.mock.calls.at(-1)[0]).toEqual(value.request);
  expect(operations()).toEqual([]);
  if (value.plan.dataset.months?.length) {
    expect(screen.getByRole('combobox', { name:/month/i }).textContent).toBe(String(value.request.month));
  } else {
    expect(screen.queryByRole('combobox', { name:/month/i })).toBeNull();
  }
  await user.click(review);
  await screen.findByRole('button', { name:connectLabel(value) });
  expect(publicDatasetRequest.mock.calls.find(([operation]) => operation === 'resolve')[1]).toEqual(value.request);
  expect(operations()).toEqual([]);
  await waitFor(() => expect(screen.queryAllByRole('checkbox', { name:/Select asset/ })).toHaveLength(0));
  await user.click(screen.getByRole('button', { name:connectLabel(value) }));
  const selection = await screen.findByRole('checkbox', { name:`Select asset · ${value.snapshot.itemId} · ${value.assetKey}` });
  expect(operations()).toEqual(['snapshot']);
  expect(publicDatasetRequest.mock.calls.find(([operation]) => operation === 'connect')[1]).toEqual(value.request);
  expect(screen.getByRole('textbox', { name:'Dataset search bounds' }).value).toBe(value.request.bounds.join(', '));
  expect(screen.getByRole('button', { name:'Save asset selection' }).disabled).toBe(true);
  await user.click(selection);
  expect(selection.getAttribute('aria-checked')).toBe('true');
   const card = selection.closest('article');
  const layer = value.plan.dataset.layers.find(layer => layer.id === value.request.layer);
   await user.click(within(card).getByRole('button',{name:'Asset source details'}));
   const details=screen.getByRole('dialog',{name:'Asset source details'});
   expect(details.textContent).toContain(layer.unit);
   expect(details.textContent).toContain(layer.valueInterpretation);
   await user.keyboard('{Escape}');
  expect(operations()).toEqual(['snapshot']);
  await user.click(screen.getByRole('button', { name:'Save asset selection' }));
  await screen.findByRole('button', { name:'Download selected originals' });
  const save = stacRequest.mock.calls.find(([operation]) => operation === 'project')[1];
  expect(save.bounds).toEqual(value.request.bounds);
  expect(save.selections).toEqual([{ snapshotId:value.snapshot.id, assetKey:value.assetKey }]);
  expect(operations()).toEqual(['snapshot', 'project']);
  await user.click(screen.getByRole('button', { name:'Download selected originals' }));
  await screen.findByRole('button', { name:'Original files ready' });
  expect(stacRequest.mock.calls.find(([operation]) => operation === 'downloads')[1]).toEqual({ projectId:value.project.id, selections:save.selections });
  expect(screen.getByRole('link', { name:'Open project' }).getAttribute('href')).toBe(`#My%20Data?project=${value.project.id}`);
});

it('changing a GHSL exploration AOI clears its selected original and requires a new review without starting a download', async () => {
  const value = byId('ghsl-smod'), user = userEvent.setup(), changed = vi.fn(); playback(value);
  const view = mount(value, { onRequestChange:changed });
  await user.click(await screen.findByRole('button', { name:resolveLabel(value) }));
  await user.click(await screen.findByRole('button', { name:connectLabel(value) }));
  await user.click(await screen.findByRole('checkbox', { name:`Select asset · ${value.snapshot.itemId} · raster` }));
  const bounds = [116.35, 39.85, 116.45, 39.95];
  view.rerender(<RuntimeContext.Provider value={{ projects:[], refresh }}><PublicDatasetExplorer datasetId={value.datasetId} areaBounds={bounds} onRequestChange={changed}/></RuntimeContext.Provider>);
  await waitFor(() => expect(screen.getByRole('textbox', { name:'Dataset search bounds' }).value).toBe(bounds.join(', ')));
  expect(screen.queryByRole('checkbox', { name:/Select asset/ })).toBeNull();
  expect(screen.queryByRole('button', { name:'Save asset selection' })).toBeNull();
  expect(changed.mock.calls.at(-1)[0]).toEqual({ ...value.request, bounds });
  expect(operations()).toEqual(['snapshot']);
});

it.each(next2PublicCases)('$datasetId/$assetKey raw inspection retains original PNG, grid, scalar mask and numerical replies', async value => {
  playback(value); const user = userEvent.setup(); render(<StacRasterInspection job={value.job} onClose={vi.fn()}/>);
  await user.click(await screen.findByRole('button', { name:'File metadata and display' }));
  const rows = screen.getByRole('table').querySelectorAll('tbody tr');
  expect([...rows].map(row => within(row).getAllByRole('cell').map(cell => cell.textContent))).toEqual(value.inspection.bands.map(band => [String(band.index), band.dataType, band.nodata === null ? 'Not declared' : String(band.nodata)]));
  expect(screen.getByRole('img', { name:'First-band display preview' }).getAttribute('src')).toBe(value.inspection.previewDataUrl);
  expect(screen.getByLabelText('Georeferenced original raster').getAttribute('data-original-bounds')).toBe(value.inspection.bounds.join(','));
  const choices = [value.pixels.find(pixel => pixel.values.some(raw => raw === 0)), value.pixels.find(pixel => pixel.noData.some(Boolean)), value.pixels.find(pixel => pixel.values.some(raw => raw !== 0) && !pixel.noData.some(Boolean))].filter(Boolean);
  const seen = new Set();
  await user.keyboard('{Escape}');
  for (const pixel of choices) {
    const key = `${pixel.column}/${pixel.row}`; if (seen.has(key)) continue; seen.add(key);
    await user.clear(screen.getByRole('textbox', { name:'Pixel column' })); await user.type(screen.getByRole('textbox', { name:'Pixel column' }), String(pixel.column));
    await user.clear(screen.getByRole('textbox', { name:'Pixel row' })); await user.type(screen.getByRole('textbox', { name:'Pixel row' }), String(pixel.row));
    await user.click(screen.getByRole('button', { name:'Read raw pixel' }));
    await waitFor(() => expect([...document.querySelectorAll('.stac-pixel-result strong')].map(item => item.textContent)).toEqual(pixel.values.map((raw, index) => `${raw}${pixel.noData[index] ? ' · NoData' : ''}`)));
    expect(stacRequest.mock.calls.filter(([operation]) => operation === 'pixel').at(-1)[1]).toEqual({ id:value.job.id, column:pixel.column, row:pixel.row });
  }
  if (value.datasetId === 'ghsl-smod') {
    expect(screen.getByRole('note').textContent).toContain('All 43,202 original columns');
  }
  if (value.datasetId === 'maxar-open-legacy') {
    expect(screen.getByRole('note').textContent).toContain('separate validity mask');
    expect(screen.getByRole('note').textContent).toContain('a zero or unmasked sample does not establish valid imagery coverage');
  }
});

it('GHSL details preserve undocumented zero and Int16 -200 without relabeling the year as acquisition time', async () => {
  const value = byId('ghsl-smod'), user = userEvent.setup(); playback(value); mount(value);
  await user.click(await screen.findByRole('button', { name:'Product details' }));
  const details = document.querySelector('.public-dataset-definition');
  expect(details.textContent).toContain('undocumented raw code');
  expect(details.textContent).toContain('Int16'); expect(details.textContent).toContain('-200');
  expect(within(details).getByRole('heading', { name:'Product limitations' })).toBeTruthy();
  expect(details.textContent).toContain('modeled reference epoch');
  expect(details.textContent).toContain('not the acquisition date');
  expect(operations()).toEqual([]);
});

it('GLAD details retain the annual legend and valid code zero without inventing a scalar mask', async () => {
  const value = byId('glad-landcover-2020'); if (!value) return;
  const user = userEvent.setup(); playback(value); mount(value);
  await user.click(await screen.findByRole('button', { name:'Product details' }));
  const details = document.querySelector('.public-dataset-definition');
  expect(details.textContent).toContain('Code 0 is valid desert');
  expect(details.textContent).toContain('no scalar NoData tag');
  expect(details.textContent).toContain('not continuous reflectance');
  expect(operations()).toEqual([]);
});

it('changing Maxar analytic layers invalidates the old source selection before another layer can be saved or downloaded', async () => {
  const value = next2PublicCases.find(value => value.datasetId === 'maxar-open-legacy' && value.assetKey === 'ms_analytic');
  if (!value) return;
  const user = userEvent.setup(), changed = vi.fn(); playback(value); mount(value, { onRequestChange:changed });
  await screen.findByRole('button', { name:resolveLabel(value) });
  await selectActualLayer(value, user);
  expect(changed.mock.calls.at(-1)[0]).toEqual(value.request);
  await user.click(screen.getByRole('button', { name:resolveLabel(value) }));
  await user.click(await screen.findByRole('button', { name:connectLabel(value) }));
  await user.click(await screen.findByRole('checkbox', { name:`Select asset · ${value.snapshot.itemId} · ${value.assetKey}` }));
  expect(screen.getByRole('button', { name:'Save asset selection' }).disabled).toBe(false);
  await user.click(screen.getByRole('combobox', { name:'Product layer' }));
  await user.click(screen.getByRole('option', { name:value.plan.dataset.layers.find(layer => layer.id === 'pan_analytic').title }));
  await waitFor(() => expect(screen.queryAllByRole('checkbox', { name:/Select asset/ })).toHaveLength(0));
  expect(screen.queryByRole('button', { name:'Save asset selection' })).toBeNull();
  expect(screen.queryByRole('button', { name:'Download selected originals' })).toBeNull();
  expect(changed.mock.calls.at(-1)[0]).toEqual({ ...value.request, layer:'pan_analytic' });
  expect(operations()).toEqual(['snapshot']);
});

it.each(next2PublicCases.filter(value => value.datasetId === 'maxar-open-legacy'))('Maxar $assetKey shows its real point observation and excludes visual and unselected analytic assets before saving', async value => {
  playback(value); const user = userEvent.setup(); mount(value);
  await screen.findByRole('button', { name:resolveLabel(value) });
  await selectActualLayer(value, user);
  await user.click(screen.getByRole('button', { name:resolveLabel(value) }));
  await user.click(await screen.findByRole('button', { name:connectLabel(value) }));
  const choice = await screen.findByRole('checkbox', { name:`Select asset · ${value.snapshot.itemId} · ${value.assetKey}` });
  expect(screen.getAllByRole('checkbox', { name:/Select asset/ }).filter(item => !item.disabled)).toHaveLength(1);
   expect(choice.closest('.public-dataset-review').textContent).toContain(value.snapshot.properties.datetime.slice(0, 10));
  expect(screen.getByRole('checkbox', { name:/ · visual$/ }).disabled).toBe(true);
  for (const other of value.snapshot.assets.filter(asset => asset.key !== value.assetKey)) {
    const forbidden = screen.getByRole('checkbox', { name:`Select asset · ${value.snapshot.itemId} · ${other.key}` });
    expect(forbidden.disabled).toBe(true);
    await user.click(forbidden);
    expect(forbidden.getAttribute('aria-checked')).toBe('false');
  }
  expect(screen.getByRole('button', { name:'Save asset selection' }).disabled).toBe(true);
  expect(operations()).toEqual(['snapshot']);
});

it.each(next2PublicCases.filter(value => value.datasetId === 'openlandmap-nightlights'))('nighttime-light details disclose scale conflict and annual processed DN before any transfer', async value => {
  playback(value); const user = userEvent.setup(); mount(value);
  await user.click(await screen.findByRole('button', { name:'Product details' }));
  const details = document.querySelector('.public-dataset-definition');
  expect(details.textContent).toContain('Physical radiance calibration remains unresolved');
  expect(details.textContent).toContain('0–2000');
  expect(details.textContent).toContain('scale 10');
  expect(within(details).getByRole('heading', { name:'Product limitations' })).toBeTruthy();
  expect(details.textContent).toContain('2000–2011');
  expect(details.textContent).toContain('3,299');
  expect(operations()).toEqual([]);
});
