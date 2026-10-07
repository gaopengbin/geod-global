import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RuntimeJobRows, RuntimeLibrary, RuntimeTasks } from './runtime-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { I18nProvider } from './i18n.jsx';

vi.mock('./file-thumbnail.jsx', () => ({ FileThumbnail: () => <span>Local preview</span> }));
const id = '933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b';
const rgb = { id, kind: 'download', status: 'succeeded', assetKey: 'visual', itemId: 'RGB_SCENE', title: 'RGB source', mediaType: 'image/tiff', href: 'https://example.test/rgb.tif', bytesDownloaded: 1024, sha256: 'a'.repeat(64) };
const scl = { ...rgb, id: '733dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b', assetKey: 'scl', itemId: 'SCL_SCENE', title: 'SCL source', href: 'https://example.test/scl.tif' };
const wrap = (children, extra = {}) => render(<I18nProvider><RuntimeContext.Provider value={{ health: {}, jobs: [rgb, scl], projects: [{ id: 'project-1', scenes: [{ itemId: rgb.itemId, assets: { visual: { href: rgb.href } } }] }], act: vi.fn(), ...extra }}>{children}</RuntimeContext.Provider></I18nProvider>);
beforeEach(() => {
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'en', setItem: vi.fn() } });
  window.history.replaceState(null, '', '#Tasks');
});

describe('File and task flows', () => {
  it('a validated Agent job link opens its actual history tab and highlights the matching task', () => {
    window.history.replaceState(null, '', `#Tasks?job=${id}`);
    const { container } = wrap(<RuntimeTasks/>);
    expect(screen.getByText('RGB source')).toBeTruthy();
    expect(container.querySelector(`[data-item-id="${id}"]`).dataset.highlighted).toBe('true');
    expect(container.querySelector(`[data-item-id="${scl.id}"]`).hasAttribute('data-highlighted')).toBe(false);
    expect(screen.getByRole('radio', { name: 'History · 2' }).getAttribute('aria-checked')).toBe('true');
  });
  it('does not report an empty task list before the initial service connection finishes', () => {
    wrap(<RuntimeTasks/>, { jobs: [], health: null, checking: true });
    expect(screen.getByText('Loading local tasks…')).toBeTruthy();
    expect(screen.queryByText('No tasks running')).toBeNull();
  });

  it('shows unavailable local files instead of a false empty library while offline', () => {
    wrap(<RuntimeLibrary/>, { jobs: [], health: null, checking: false });
    expect(screen.getByText('Reconnect the task service to read your local files.')).toBeTruthy();
    expect(screen.queryByText('Completed downloads appear here with their local path, source and checksum.')).toBeNull();
  });

  it('keeps file opening and its owning project visible, with inspection grouped under details', async () => {
    const user = userEvent.setup();
    wrap(<RuntimeLibrary/>);
    expect(screen.getAllByRole('link', { name: 'Open in workspace' })[0].getAttribute('href')).toBe(`#Workspace?file=${id}&project=project-1`);
    expect(screen.getByRole('link', { name: 'Open project' }).getAttribute('href')).toBe('#My%20Data?project=project-1');
    expect(screen.queryByRole('button', { name: 'Inspect raster' })).toBeNull();
    const details = screen.getAllByRole('button', { name: 'File details and provenance' })[0];
    await user.click(details);
    const inspect = screen.getByRole('button', { name: 'Inspect raster' });
    expect(document.getElementById(details.getAttribute('aria-controls')).contains(inspect)).toBe(true);
    await user.tab();
    expect(document.activeElement).toBe(inspect);
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(details);
    await user.keyboard(' ');
    expect(screen.queryByRole('button', { name: 'Inspect raster' })).toBeNull();
  });

  it('keeps the current project when opening a source that another project also owns', () => {
    wrap(<RuntimeJobRows jobs={[rgb]} library projectName="Second project" projectId="project-2"/>);
    expect(screen.getByRole('link', { name: 'Open in workspace' }).getAttribute('href')).toBe(`#Workspace?file=${id}&project=project-2`);
  });

  it('filters actual files by source type without changing the collection', async () => {
    const user = userEvent.setup();
    wrap(<RuntimeLibrary/>);
    await user.click(screen.getByRole('combobox', { name: 'Data type' }));
    await user.click(screen.getByRole('option', { name: 'SCL classification' }));
    expect(screen.queryByText('RGB source')).toBeNull();
    expect(screen.getByText('SCL source')).toBeTruthy();
    expect(screen.getByRole('status').textContent).toBe('1 of 2 files');
  });

  it('groups prepared SAFE rasters with source files and keeps clips in derived results', async () => {
    const user = userEvent.setup();
    const prepared = { ...rgb, id: 'prepared', kind: 'raster_prepare', title: 'Prepared TCI' };
    const clipped = { ...scl, id: 'clip', kind: 'raster_crop', title: 'SCL area clip' };
    wrap(<RuntimeLibrary/>, { jobs: [rgb, prepared, clipped], projects: [] });
    await user.click(screen.getByRole('combobox', { name: 'File origin' }));
    await user.click(screen.getByRole('option', { name: 'Source files' }));
    expect(screen.getByText('Prepared TCI')).toBeTruthy();
    expect(screen.getByText('RGB source')).toBeTruthy();
    expect(screen.queryByText('SCL area clip')).toBeNull();
    expect(screen.getByRole('status').textContent).toBe('2 of 3 files');
    await user.click(screen.getByRole('combobox', { name: 'File origin' }));
    await user.click(screen.getByRole('option', { name: 'Derived outputs' }));
    expect(screen.getByText('SCL area clip')).toBeTruthy();
    expect(screen.queryByText('Prepared TCI')).toBeNull();
    expect(screen.getByRole('status').textContent).toBe('1 of 3 files');
  });

  it('a failed SAFE preparation describes retrying preparation rather than clipping', async () => {
    const user = userEvent.setup();
    wrap(<RuntimeJobRows jobs={[{ ...rgb, kind: 'raster_prepare', status: 'failed', error: 'ZIP CRC mismatch' }]}/>);
    await user.click(screen.getByRole('button', { name: 'Task details' }));
    expect(screen.getByText('SAFE preparation did not complete. Check the original product and retry.')).toBeTruthy();
    expect(screen.queryByText('Raster processing did not complete. Check the source file and retry the clip.')).toBeNull();
  });

  it('separates live work, failed work and history, retaining the real retry action', async () => {
    const user = userEvent.setup();
    const act = vi.fn().mockResolvedValue({});
    wrap(<RuntimeTasks/>, { act, jobs: [rgb, { ...scl, status: 'failed' }, { ...rgb, id: 'live', status: 'running', title: 'Live source', totalBytes: 400, bytesDownloaded: 100 }] });
    expect(screen.getByText('Live source')).toBeTruthy();
    expect(screen.queryByText('SCL source')).toBeNull();
    await user.click(screen.getByRole('radio', { name: 'Needs attention · 1' }));
    expect(screen.getByText('SCL source')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Retry download' }));
    expect(act).toHaveBeenCalledWith('retry', { id: scl.id });
    await user.click(screen.getByRole('radio', { name: 'History · 1' }));
    expect(screen.getByText('RGB source')).toBeTruthy();
    expect(screen.queryByText('Live source')).toBeNull();
  });

  it('keeps failed tasks compact and opens an unsuccessful retry explanation without losing the source identity', async () => {
    const user = userEvent.setup();
    const itemId = 'S2B_10SEG_20250622_0_L2A';
    const job = { ...scl, itemId, title: `${itemId} · SCL`, status: 'failed', error: 'Source unavailable', totalBytes: 1024 };
    const act = vi.fn().mockRejectedValue(new Error('Queue unavailable'));
    wrap(<RuntimeJobRows jobs={[job]}/>, { act });
    expect(screen.getByText('Jun 22, 2025 · 10SEG')).toBeTruthy();
    expect(screen.queryByText(itemId)).toBeNull();
    expect(screen.queryByText('Source unavailable')).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Task details' }));
    expect(screen.getByText(itemId)).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Task details' }));
    await user.click(screen.getByRole('button', { name: 'Retry download' }));
    expect(act).toHaveBeenCalledWith('retry', { id: job.id });
    expect(screen.getByRole('button', { name: 'Task details' }).getAttribute('aria-expanded')).toBe('true');
    expect(screen.getAllByRole('alert').some(alert => alert.textContent.includes('The task action failed.'))).toBe(true);
  });

  it('reports indeterminate transfers without inventing progress and cancels the correct task', async () => {
    const user = userEvent.setup();
    const act = vi.fn().mockResolvedValue({});
    wrap(<RuntimeTasks/>, { act, jobs: [{ ...scl, status: 'running', totalBytes: null }, { ...rgb, status: 'queued' }] });
    const progress = screen.getByRole('progressbar', { name: 'Download progress' });
    expect(progress.getAttribute('aria-valuenow')).toBeNull();
    expect(screen.getAllByRole('progressbar')).toHaveLength(1);
    await user.click(screen.getAllByRole('button', { name: 'Cancel download' })[0]);
    expect(act).toHaveBeenCalledWith('cancel', { id: scl.id });
  });
});
