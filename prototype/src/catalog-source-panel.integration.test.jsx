import React, { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { providerById } from './providers.js';
import { CatalogSourcePanel } from './catalog-source-panel.jsx';

describe('Catalog source availability and connection actions', () => {
  it('keeps catalog-only sources selectable without presenting their originals as ready', async () => {
    Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'en', setItem: vi.fn() } });
    const user = userEvent.setup(), changed = vi.fn();
    function Panel() {
      const [value, setValue] = useState('planetary-landsat');
      return <I18nProvider><CatalogSourcePanel provider={providerById(value)} onChange={next => { changed(next); setValue(next); }} onOpenStac={vi.fn()} onOpenWcs={vi.fn()}/></I18nProvider>;
    }
    render(<Panel/>);
    const picker = screen.getByRole('combobox', { name: 'Data source' });
    expect(picker.textContent).toContain('Landsat 8 / 9');
    expect(screen.getByText('Downloadable', { exact: true })).toBeTruthy();
    await user.click(picker);
    const available = screen.getByRole('group', { name: 'Downloads available · no account needed · 9' });
    const deferred = screen.getByRole('group', { name: 'Catalog only · original downloads pending verification · 6' });
    expect(within(available).getAllByRole('option')).toHaveLength(9);
    expect(within(deferred).getAllByRole('option')).toHaveLength(6);
    await user.click(within(deferred).getByRole('option', { name: 'NASA Earthdata · HLS' }));
    expect(changed).toHaveBeenCalledWith('nasa-earthdata');
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(screen.getByText('Catalog only', { exact: true })).toBeTruthy();
    expect(screen.getByText('Catalog search is available. Original downloads await real-account verification.')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Manage authorization' }).getAttribute('href')).toBe('#Settings?account=nasa-earthdata');
    expect(document.activeElement).toBe(picker);
  });

  it('groups custom services behind the add action and returns keyboard focus before opening a service', async () => {
    Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
    const user = userEvent.setup(), stac = vi.fn(), wcs = vi.fn();
    render(<I18nProvider><CatalogSourcePanel provider={providerById('earth-search')} onChange={vi.fn()} onOpenStac={stac} onOpenWcs={wcs}/></I18nProvider>);
    const add = screen.getByRole('button', { name: '添加数据源' });
    expect(screen.queryByRole('button', { name: /栅格目录/ })).toBeNull();
    await user.click(add);
    expect(screen.getByRole('dialog', { name: '添加数据源' })).toBeTruthy();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.activeElement).toBe(add);
    await user.click(add);
    await user.click(screen.getByRole('button', { name: /覆盖数据服务（WCS）/ }));
    await waitFor(() => expect(wcs).toHaveBeenCalledOnce());
    expect(stac).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(add);
  });
});
