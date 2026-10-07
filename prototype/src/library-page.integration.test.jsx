import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { LibraryPage } from './library-page.jsx';
import { RuntimeContext } from './runtime-context.js';

vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: value => value, number: value => String(value) }) }));
vi.mock('./projects-ui.jsx', () => ({ ProjectsLibrary: ({ focusedProjectId }) => <section aria-label="Project content">{focusedProjectId || 'Project index'}</section> }));
vi.mock('./runtime-ui.jsx', () => ({ RuntimeLibrary: () => <section aria-label="File content">File index</section> }));

const show = props => render(<RuntimeContext.Provider value={{ jobs: [{ status: 'succeeded' }, { status: 'running' }] }}><LibraryPage {...props}/></RuntimeContext.Provider>);

describe('Library navigation', () => {
  it('keeps only the 2D collections and safely opens an old 3D bookmark', () => {
    const original=location.hash;
    try {
      location.hash='#My%20Data?view=3d';show();
      expect(screen.getByRole('region',{name:'Project content'})).toBeTruthy();
      expect(screen.queryByRole('radio',{name:'3D assets'})).toBeNull();
      expect(screen.getAllByRole('radio')).toHaveLength(5);
    } finally {location.hash=original;}
  });
  it('shows one collection at a time, restores direct file links and follows browser history', async () => {
    const user = userEvent.setup();
    const original = location.hash;
    try {
      location.hash = '#My%20Data?view=files';
      show();
      expect(screen.getByRole('region', { name: 'File content' })).toBeTruthy();
      expect(screen.queryByRole('button', { name: /Saved clip plans/ })).toBeNull();
      expect(screen.queryByRole('region', { name: 'Project content' })).toBeNull();
      expect(screen.getByRole('radio', { name: 'Raster files · 1' }).getAttribute('aria-checked')).toBe('true');
      await user.click(screen.getByRole('radio', { name: 'Projects' }));
      expect(location.hash).toBe('#My%20Data');
      expect(screen.getByRole('region', { name: 'Project content' })).toBeTruthy();
      expect(screen.queryByRole('region', { name: 'File content' })).toBeNull();
      act(() => { location.hash = '#My%20Data?view=files'; window.dispatchEvent(new Event('hashchange')); });
      expect(screen.getByRole('region', { name: 'File content' })).toBeTruthy();
    } finally { location.hash = original; }
  });

  it('keeps a direct project link focused on the existing project flow', () => {
    show({ focusedProjectId: 'saved-project' });
    expect(screen.getByRole('region', { name: 'Project content' }).textContent).toBe('saved-project');
    expect(screen.queryByRole('radio')).toBeNull();
    expect(screen.queryByRole('region', { name: 'File content' })).toBeNull();
  });
});
