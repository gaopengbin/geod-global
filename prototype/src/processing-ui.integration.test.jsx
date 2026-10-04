import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RecipeEditorDialog } from './processing-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { processingRequest, recipeFingerprint } from './processing-client.js';
import { runtimeRequest } from './runtime-client.js';

vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: value => value, number: String }) }));
vi.mock('./runtime-client.js', () => ({ runtimeRequest: vi.fn() }));
vi.mock('./processing-client.js', async () => ({ ...await vi.importActual('./processing-client.js'), processingRequest: vi.fn() }));

const job = { id: '10000000-0000-0000-0000-000000000002', itemId: 'SCL_SOURCE', sha256: 'a'.repeat(64) };
const metadata = { sha256: job.sha256, bounds: [500000, 4100000, 501280, 4101280], width: 64, height: 64, crs: 'EPSG:32610', previewDataUrl: 'data:image/png;base64,a', previewWidth: 64, previewHeight: 64 };
const saved = {
  schemaVersion: 'geod-raster-recipe/v1', name: 'Saved central area', source: { jobId: job.id, sha256: job.sha256 },
  operation: { type: 'clip', crs: 'source', bounds: [500100, 4100100, 500500, 4100500] }, output: { format: 'GeoTIFF' },
};
const changedFile = { ...saved, name: 'Different file checksum', source: { ...saved.source, sha256: 'b'.repeat(64) } };

beforeEach(() => {
  vi.clearAllMocks();
  runtimeRequest.mockResolvedValue(metadata);
  processingRequest.mockImplementation(async (operation, recipe) => {
    if (operation === 'list') return [{ id: 'saved', recipe: saved }, { id: 'other', recipe: changedFile }];
    if (operation === 'plan') return {
      recipe, fingerprint: recipeFingerprint(recipe),
      plan: { width: 20, height: 20, window: [0, 0, 20, 20], pixelSize: [20, 20], bounds: recipe.operation.bounds, crs: metadata.crs, warnings: [] },
    };
    return {};
  });
});
const show = () => render(<RuntimeContext.Provider value={{ refresh: vi.fn() }}><RecipeEditorDialog sourceJob={job} onClose={vi.fn()}/></RuntimeContext.Provider>);
const openSaved = async user => {
  await user.click(await screen.findByRole('button', { name: 'Reuse clipping settings' }));
  await waitFor(() => expect(screen.getByRole('combobox', { name: 'Saved settings for this file' }).disabled).toBe(false));
};

describe('Contextual clipping settings', () => {
  it('loads saved settings only in the clipping flow, keeps the pinned source and invalidates a prior check', async () => {
    const user = userEvent.setup();
    show();
    await user.click(await screen.findByRole('button', { name: 'Check processing plan' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Run clip' }).disabled).toBe(false));
    expect(processingRequest.mock.calls.some(([operation]) => operation === 'list')).toBe(false);
    await openSaved(user);
    await user.click(screen.getByRole('combobox', { name: 'Saved settings for this file' }));
    expect(screen.queryByRole('option', { name: 'Different file checksum' })).toBeNull();
    await user.click(screen.getByRole('option', { name: saved.name }));
    expect(screen.getByRole('textbox', { name: 'Clip name' }).value).toBe(saved.name);
    expect(screen.getByRole('spinbutton', { name: 'Min X' }).value).toBe('500100');
    expect(screen.getByRole('button', { name: 'Run clip' }).disabled).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Check processing plan' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Run clip' }).disabled).toBe(false));
    expect(processingRequest.mock.calls.at(-1)[1].source).toEqual(saved.source);
    expect(processingRequest.mock.calls.some(([operation]) => operation === 'run')).toBe(false);
    await user.clear(screen.getByRole('spinbutton', { name: 'Min X' }));
    await user.type(screen.getByRole('spinbutton', { name: 'Min X' }), '500200');
    await user.click(screen.getByRole('combobox', { name: 'Saved settings for this file' }));
    await user.click(screen.getByRole('option', { name: saved.name }));
    expect(screen.getByRole('spinbutton', { name: 'Min X' }).value).toBe('500100');
  });

  it('rejects importing parameters for another checksum without changing the draft or starting work', async () => {
    const user = userEvent.setup();
    show();
    await openSaved(user);
    const originalName = screen.getByRole('textbox', { name: 'Clip name' }).value;
    await user.click(screen.getByRole('button', { name: 'Import clip plan JSON' }));
    const dialog = screen.getByRole('dialog', { name: 'Import clip plan JSON' });
    await user.click(within(dialog).getByRole('textbox', { name: 'Clip plan JSON' }));
    await user.paste(JSON.stringify(changedFile));
    await user.click(within(dialog).getByRole('button', { name: 'Validate JSON' }));
    expect(within(dialog).getByRole('alert').textContent).toContain('different source file');
    expect(within(dialog).getByRole('button', { name: 'Review processing plan' }).disabled).toBe(true);
    await user.click(within(dialog).getByRole('button', { name: 'Close clip plan import' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Import clip plan JSON' })).toBeNull());
    expect(screen.getByRole('textbox', { name: 'Clip name' }).value).toBe(originalName);
    expect(processingRequest.mock.calls.every(([operation]) => operation === 'list')).toBe(true);
  });

  it('saves checked settings locally and directs reuse to the clipping window', async () => {
    const user = userEvent.setup();
    show();
    await user.click(await screen.findByRole('button', { name: 'Check processing plan' }));
    await user.click(await screen.findByRole('button', { name: 'Save clipping settings' }));
    const section = screen.getByRole('button', { name: 'Save clipping settings', expanded: true }).closest('[data-slot=disclosure]');
    await user.click(within(section).getAllByRole('button', { name: 'Save clipping settings' })[1]);
    expect(await screen.findByText('Clip settings saved. Reuse them from this file’s clipping window.')).toBeTruthy();
    expect(processingRequest.mock.calls.some(([operation]) => operation === 'save')).toBe(true);
    expect(processingRequest.mock.calls.some(([operation]) => operation === 'run')).toBe(false);
  });
});
