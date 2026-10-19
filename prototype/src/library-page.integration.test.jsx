import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { LibraryPage } from './library-page.jsx';
import { RuntimeContext } from './runtime-context.js';

vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: value => value, number: value => String(value) }) }));
vi.mock('./projects-ui.jsx', () => ({ ProjectsLibrary: ({ focusedProjectId }) => <section aria-label="Project content">{focusedProjectId || 'Project index'}</section> }));
vi.mock('./runtime-ui.jsx', () => ({ RuntimeLibrary: () => <section aria-label="File content">File index</section> }));
vi.mock('./gebco-ui.jsx', () => ({ GebcoExplorer: ({libraryOnly,areaBounds}) => <section aria-label="Saved regional content" data-library-only={libraryOnly} data-exploration-area={areaBounds}>Saved numerical regions</section> }));

const show = props => render(<RuntimeContext.Provider value={{ jobs: [{ status: 'succeeded' }, { status: 'running' }] }}><LibraryPage {...props}/></RuntimeContext.Provider>);

describe('Library navigation', () => {
  it('keeps only the 2D collections and safely opens an old 3D bookmark', () => {
    const original=location.hash;
    try {
      location.hash='#My%20Data?view=3d';show();
      expect(screen.getByRole('region',{name:'Project content'})).toBeTruthy();
      expect(screen.queryByRole('radio',{name:'3D assets'})).toBeNull();
      expect(screen.getAllByRole('radio')).toHaveLength(3);
      expect(screen.queryByRole('radio',{name:'Local raster originals'})).toBeNull();
      expect(screen.getByRole('button',{name:'More collections'})).toBeTruthy();
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

  it('reveals secondary collections on demand and keeps a directly opened collection selected',async()=>{
    const original=location.hash,user=userEvent.setup();
    try {
      location.hash='#My%20Data';show();
      await user.click(screen.getByRole('button',{name:'More collections'}));
      await user.click(screen.getByRole('button',{name:'GEBCO regions'}));
      await screen.findByRole('region',{name:'Saved regional content'});
      expect(location.hash).toBe('#My%20Data?view=gebco-regions');
      expect(screen.getByRole('radio',{name:'GEBCO regions'}).getAttribute('aria-checked')).toBe('true');
      await user.click(screen.getByRole('radio',{name:'Projects'}));
      expect(screen.queryByRole('radio',{name:'GEBCO regions'})).toBeNull();
      expect(screen.queryByRole('region',{name:'Saved regional content'})).toBeNull();
    } finally {location.hash=original;}
  });

  it('restores saved numerical regions in library mode without inheriting an exploration area',async()=>{
    const original=location.hash,user=userEvent.setup();
    try {
      location.hash='#My%20Data?view=gebco-regions';
      show({areaBounds:[116.,39.,117.,40.]});
      const region=screen.getByRole('region',{name:'Saved regional content'});
      expect(region.getAttribute('data-library-only')).toBe('true');
      expect(region.getAttribute('data-exploration-area')).toBeNull();
      await user.click(screen.getByRole('radio',{name:'Projects'}));
      expect(screen.queryByRole('region',{name:'Saved regional content'})).toBeNull();
      act(()=>{location.hash='#My%20Data?view=gebco-regions';window.dispatchEvent(new Event('hashchange'));});
      expect(screen.getByRole('region',{name:'Saved regional content'})).toBeTruthy();
    } finally {location.hash=original;}
  });
});
