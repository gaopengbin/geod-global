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
});

describe('File and task flows', () => {
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
    await user.click(screen.getAllByRole('button', { name: 'File details and provenance' })[0]);
    expect(screen.getByRole('button', { name: 'Inspect raster' })).toBeTruthy();
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

  it('separates live work, failed work and history, retaining the real retry action', async () => {
    const user = userEvent.setup();
    const act = vi.fn().mockResolvedValue({});
    wrap(<RuntimeTasks/>, { act, jobs: [rgb, { ...scl, status: 'failed' }, { ...rgb, id: 'live', status: 'running', title: 'Live source', totalBytes: 400, bytesDownloaded: 100 }] });
    expect(screen.getByText('Live source')).toBeTruthy();
    expect(screen.queryByText('SCL source')).toBeNull();
    await user.click(screen.getByRole('radio', { name: 'Needs attention · 1' }));
    expect(screen.getByText('SCL source')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Retry from start' }));
    expect(act).toHaveBeenCalledWith('retry', { id: scl.id });
    await user.click(screen.getByRole('radio', { name: 'History · 1' }));
    expect(screen.getByText('RGB source')).toBeTruthy();
    expect(screen.queryByText('Live source')).toBeNull();
  });
});
