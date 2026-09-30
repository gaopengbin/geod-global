import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { FileThumbnail } from './file-thumbnail.jsx';
import { RuntimeContext } from './runtime-context.js';
import { loadFileThumbnail } from './file-thumbnail.js';
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: value => value }) }));
vi.mock('./file-thumbnail.js', () => ({ cachedFileThumbnail: () => null, fileThumbnailKey: job => job.id, loadFileThumbnail: vi.fn() }));

describe('Preview availability', () => {
  it('stops showing a spinner while offline and loads on reconnection', async () => {
    loadFileThumbnail.mockResolvedValue({ dataUrl: 'data:image/png;base64,AAAA', width: 1, height: 1 });
    const job = { id: 'preview-1', assetKey: 'visual', title: 'Local image' };
    const component = health => <RuntimeContext.Provider value={{ health, checking: false }}><FileThumbnail job={job}/></RuntimeContext.Provider>;
    const view = render(component(null));
    expect(loadFileThumbnail).not.toHaveBeenCalled();
    expect(screen.queryByRole('status')).toBeNull();
    expect(screen.getByLabelText('Preview paused while the task service is offline')).toBeTruthy();
    view.rerender(component({ status: 'ok' }));
    await screen.findByRole('img', { name: 'Local true-color file preview · Local image' });
    expect(loadFileThumbnail).toHaveBeenCalledTimes(1);
  });
});
