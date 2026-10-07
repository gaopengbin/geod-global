import React, { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { SettingsPage } from './settings-page.jsx';
import { I18nProvider } from './i18n.jsx';
import { desktopAvailable, runtimeRequest, syncDesktopLocale } from './runtime-client.js';

vi.mock('./runtime-client.js', () => ({ desktopAvailable: vi.fn(() => false), runtimeRequest: vi.fn(async () => []), syncDesktopLocale: vi.fn(async () => {}) }));
vi.mock('./proxy-ui.jsx', () => ({ ProxySettingsPanel: () => <section aria-label="Download connection">Connection settings</section> }));
vi.mock('./diagnostics-ui.jsx', () => ({ DiagnosticsPanel: () => <p>Detailed local diagnostics</p> }));

let storage;
beforeEach(() => {
  storage = new Map([['geod-global-locale', 'en']]);
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) } });
  desktopAvailable.mockReturnValue(false);
  syncDesktopLocale.mockClear();
  runtimeRequest.mockClear();
});

function Preferences() {
  const [theme, setTheme] = useState('light');
  return <I18nProvider><SettingsPage theme={theme} onThemeChange={setTheme}/></I18nProvider>;
}

describe('Settings preferences', () => {
  it('offers only current 2D source management', () => {
    render(<Preferences/>);
    expect(screen.queryByRole('heading',{name:'3D asset sources'})).toBeNull();
    expect(screen.queryByRole('button',{name:/3D/})).toBeNull();
    expect(screen.getByRole('button',{name:'Manage raster sources'})).toBeTruthy();
    expect(screen.getByRole('button',{name:'Manage coverage services'})).toBeTruthy();
  });
  it('changes language immediately, persists it and synchronizes desktop labels', async () => {
    const user = userEvent.setup();
    render(<Preferences/>);
    await user.click(screen.getByRole('combobox', { name: 'Interface language' }));
    await user.click(screen.getByRole('option', { name: '简体中文' }));
    expect(screen.getByRole('heading', { name: '设置', level: 1 })).toBeTruthy();
    expect(storage.get('geod-global-locale')).toBe('zh-CN');
    expect(document.documentElement.lang).toBe('zh-CN');
    expect(syncDesktopLocale).toHaveBeenLastCalledWith('zh-CN');
  });

  it('changes the controlled appearance preference without leaving the page', async () => {
    const user = userEvent.setup();
    render(<Preferences/>);
    await user.click(screen.getByRole('combobox', { name: 'Appearance' }));
    await user.click(screen.getByRole('option', { name: 'Dark', exact: true }));
    expect(screen.getByRole('combobox', { name: 'Appearance' }).textContent).toBe('Dark');
    expect(screen.getByRole('heading', { name: 'Settings', level: 1 })).toBeTruthy();
  });

  it('keeps detailed diagnostics collapsed and shows background behavior only in the desktop app', async () => {
    const user = userEvent.setup();
    desktopAvailable.mockReturnValue(true);
    render(<Preferences/>);
    expect(screen.getByText('Run in the background')).toBeTruthy();
    expect(screen.queryByText('Detailed local diagnostics')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Local diagnostics · advanced' }));
    expect(screen.getByText('Detailed local diagnostics')).toBeTruthy();
  });

  it('offers an explicit desktop exit in settings without exiting or hiding on entry', async () => {
    desktopAvailable.mockReturnValue(true);
    const { container } = render(<Preferences/>);
    const exit = screen.getByRole('button', { name: 'Exit app' });
    expect(container.querySelector('.settings-background').contains(exit)).toBe(true);
    expect(runtimeRequest).not.toHaveBeenCalled();
    await userEvent.click(exit);
    await waitFor(() => expect(within(screen.getByRole('dialog')).getByRole('status').textContent).toBe('Running or queued tasks: 0'));
    expect(screen.getByRole('button', { name: 'Exit and stop tasks' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Close', exact: true }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(runtimeRequest.mock.calls.map(([command]) => command)).toEqual(['list']);
  });
});
