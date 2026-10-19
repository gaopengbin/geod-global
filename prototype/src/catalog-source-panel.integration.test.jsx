import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { providerById } from './providers.js';
import { CatalogSourcePanel } from './catalog-source-panel.jsx';

const locale = value => Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => value, setItem: vi.fn() } });

describe('Catalog source availability and selection entry', () => {
  it('opens the classified picker from a dialog button and blocks selection during a product mutation', async () => {
    locale('en'); const user = userEvent.setup(), open = vi.fn();
    const content = disabled => <I18nProvider><CatalogSourcePanel provider={providerById('planetary-landsat')} disabled={disabled} onOpenPicker={open}/></I18nProvider>;
    const { rerender } = render(content(false));
    const trigger = screen.getByRole('button', { name: 'Choose a data source' });
    expect(trigger.textContent).toContain('Landsat 8 / 9');
    expect(trigger.getAttribute('aria-haspopup')).toBe('dialog');
    expect(screen.queryByRole('combobox', { name: 'Data source' })).toBeNull();
    await user.click(trigger); expect(open).toHaveBeenCalledOnce();
    rerender(content(true)); await user.click(trigger); expect(open).toHaveBeenCalledOnce();
  });

  it('retains honest catalog-only availability and authorization actions beside the picker', () => {
    locale('en');
    render(<I18nProvider><CatalogSourcePanel provider={providerById('copernicus')} onOpenPicker={vi.fn()}/></I18nProvider>);
    expect(screen.getByText('Catalog only', { exact: true })).toBeTruthy();
    expect(screen.getByText('Catalog search is available. Original downloads await real-account verification.')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Manage authorization' }).getAttribute('href')).toBe('#Settings?account=copernicus');
  });

  it('keeps custom services behind the add action and restores focus before handing them off', async () => {
    locale('zh-CN'); const user = userEvent.setup(), stac = vi.fn(), wcs = vi.fn();
    render(<I18nProvider><CatalogSourcePanel provider={providerById('earth-search')} onOpenPicker={vi.fn()} onOpenStac={stac} onOpenWcs={wcs}/></I18nProvider>);
    const add = screen.getByRole('button', { name: '添加数据源' });
    await user.click(add); expect(screen.getByRole('dialog', { name: '添加数据源' })).toBeTruthy();
    await user.keyboard('{Escape}'); await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.activeElement).toBe(add);
    await user.click(add); await user.click(screen.getByRole('button', { name: /覆盖数据服务（WCS）/ }));
    await waitFor(() => expect(wcs).toHaveBeenCalledOnce()); expect(stac).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(add);
  });

  it('uses a visible source selector and keeps long translated details behind disclosure', async () => {
    locale('zh-CN');
    render(<I18nProvider><CatalogSourcePanel datasetId="worldpop" dataset={{id:'worldpop',title:'WorldPop Global 1 population',nominalResolution:'100 m',description:'Historical Global 1 unconstrained estimated population counts, one original national GeoTIFF for the selected ISO3 country and year.'}}
      pickerOpen onOpenPicker={vi.fn()}/></I18nProvider>);
    const trigger = screen.getByRole('button', { name: '选择数据源' });
    expect(trigger.textContent).toContain('WorldPop Global 1 历史人口');
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(trigger.getAttribute('data-variant')).toBe('secondary');
    expect(screen.queryByText('原始文件可下载', { exact: true })).toBeNull();
    await userEvent.setup().click(screen.getByRole('button', {name:'来源详情'}));
    expect(screen.getByText('原始文件可下载', { exact: true })).toBeTruthy();
    expect(screen.getByRole('dialog', {name:'来源详情'}).textContent).toContain('100 m');
    expect(screen.queryByText(/按国家三位代码和参考年份获取历史 Global 1 非约束人口估计/)).toBeNull();
    expect(screen.getByRole('region', { name: '数据源' }).dataset.preview).toBe('manual-file');
  });
});
