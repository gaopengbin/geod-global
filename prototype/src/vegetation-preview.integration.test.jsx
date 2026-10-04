import React, { useState } from 'react';
import { afterEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { get as getProjection } from 'ol/proj.js';
import TileState from 'ol/TileState.js';
import fixture from '../public/samples/modis-vegetation-response.json';
import { normalizeScene } from './catalog.js';
import { I18nProvider } from './i18n.jsx';
import { createVegetationPreviewSource } from './vegetation-preview-source.js';
import { VegetationPreviewControls, VegetationPreviewLegend } from './vegetation-preview-ui.jsx';

const scene = normalizeScene(fixture.features[0], 'planetary-vegetation');
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

it('the shared index switch works by keyboard and keeps the legend tied to that index and composite period', async () => {
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
  function Preview() {
    const [index, setIndex] = useState('ndvi');
    return <I18nProvider><VegetationPreviewControls value={index} onValueChange={setIndex}/><VegetationPreviewLegend scene={scene} index={index}/></I18nProvider>;
  }
  const user = userEvent.setup();
  render(<Preview/>);
  expect(screen.getByRole('region', { name: '指数预览图例' }).textContent).toContain('未应用质量掩膜');
  expect(screen.getByRole('radio', { name: 'NDVI' }).getAttribute('aria-checked')).toBe('true');
  await user.tab(); await user.keyboard('{ArrowRight} ');
  expect(screen.getByRole('radio', { name: 'EVI' }).getAttribute('aria-checked')).toBe('true');
  expect(screen.getByRole('region', { name: '指数预览图例' }).textContent).toContain('EVI');
  expect(screen.getByRole('region', { name: '指数预览图例' }).textContent).toContain('06');
});

it('real OpenLayers XYZ coordinates retain the global Mercator grid, not a scene-local tile origin', () => {
  const preview = createVegetationPreviewSource(scene, 'evi');
  try {
    const grid = preview.source.getTileGrid();
    const extent = grid.getTileCoordExtent([9, 81, 197]);
    expect(extent[0]).toBeCloseTo(-13697515.4687, 3);
    expect(preview.source.getTileUrlFunction()([9, 81, 197], 1, getProjection('EPSG:3857'))).toContain('/9/81/197.png?');
    expect(preview.source.getTileUrlFunction()([9, 81, 197], 1, getProjection('EPSG:3857'))).toContain('assets=250m_16_days_EVI');
    expect(preview.source.getWrapX()).toBe(false);
  } finally { preview.dispose(); }
});

it('switching away aborts in-flight tiles and cannot commit a stale decoded image', async () => {
  let resolveRequest, signal;
  vi.stubGlobal('fetch', vi.fn((_url, options) => { signal = options.signal; return new Promise(resolve => resolveRequest = resolve); }));
  const preview = createVegetationPreviewSource(scene, 'ndvi');
  const tile = preview.source.getTile(9, 81, 197, 1, getProjection('EPSG:3857'));
  tile.load();
  expect(signal.aborted).toBe(false);
  const initial = tile.getImage().src;
  preview.dispose();
  expect(signal.aborted).toBe(true);
  resolveRequest(new Response('not a decoded tile', { headers: { 'content-type': 'image/png' } }));
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  expect(tile.getImage().src).toBe(initial);
});

it('network errors become tile errors instead of silently displaying an empty raster', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => new Response('temporarily unavailable', { status: 503 })));
  const preview = createVegetationPreviewSource(scene, 'ndvi');
  try {
    const tile = preview.source.getTile(9, 81, 197, 1, getProjection('EPSG:3857'));
    tile.load();
    await waitFor(() => expect(tile.getState()).toBe(TileState.ERROR),{timeout:3000});
    expect(fetch).toHaveBeenCalledTimes(3);
  } finally { preview.dispose(); }
});
