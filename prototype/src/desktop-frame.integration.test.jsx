import React from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { AppHeader } from './app-header.jsx';
import { activateDesktopFrame, syncDesktopAppearance, handleDesktopTitlebarMouseDown } from './desktop-frame.js';

afterEach(() => { delete window.__TAURI__; document.querySelector('[data-tauri-plugin-decoration-root]')?.remove(); });
function header(props = {}) {
  return render(<I18nProvider><AppHeader theme="light" collapsed context={<span>Project</span>} actions={<span>Actions</span>} {...props} /></I18nProvider>);
}

test('browser header retains navigation and brand without exposing fake native controls or drag behavior', async () => {
  const toggle = vi.fn();
  const { container } = header({ onToggleNavigation: toggle });
  expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('web');
  expect(container.querySelector('[data-tauri-drag-region]')).toBeNull();
  expect(container.querySelector('.geod-brand img').getAttribute('src')).toBe('./brand/geod-symbol.png');
  await userEvent.click(screen.getByRole('button', { name: /Expand navigation|展开导航/ }));
  expect(toggle).toHaveBeenCalledTimes(1);
});

test('native frame activation fails visibly without leaving the custom layout enabled', async () => {
  window.__TAURI__ = { core: { invoke: vi.fn((command) => command === 'activate_desktop_frame' ? Promise.reject('native fallback shown') : Promise.resolve()) } };
  const { container } = header();
  await waitFor(() => expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('native'));
  expect(screen.getByRole('status').title).toBe('native fallback shown');
  expect(container.querySelector('[data-tauri-drag-region]')).toBeNull();
});

test('a temporary native geometry fallback recovers to one custom titlebar', async () => {
  const invoke = vi.fn(command => {
    if (command !== 'activate_desktop_frame') return Promise.resolve();
    return Promise.resolve(invoke.mock.calls.filter(([name]) => name === command).length === 1 ? 'native' : 'custom');
  });
  window.__TAURI__ = { core: { invoke } };
  const { container } = header();
  await waitFor(() => expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('custom'));
  expect(container.querySelectorAll('.app-header')).toHaveLength(1);
  expect(screen.queryByRole('button', { name: /Exit app|退出软件/ })).toBeNull();
  expect(container.querySelector('.lucide-power')).toBeNull();
  expect(container.querySelector('[data-tauri-drag-region]')).not.toBeNull();
  expect(invoke.mock.calls.filter(([command]) => command === 'activate_desktop_frame')).toHaveLength(2);
});

test('persistent native failure stays usable and stops retrying', async () => {
  const invoke = vi.fn(() => Promise.resolve('native'));
  window.__TAURI__ = { core: { invoke } };
  expect(await activateDesktopFrame()).toBe('native');
  expect(invoke).toHaveBeenCalledTimes(2);
});

test('minimized activation defers and restoration recovers without repeated native fallback', async () => {
  const invoke = vi.fn(command => {
    if (command !== 'activate_desktop_frame') return Promise.resolve();
    return Promise.resolve(invoke.mock.calls.filter(([name]) => name === command).length === 1 ? 'deferred' : 'custom');
  });
  window.__TAURI__ = { core: { invoke } };
  const { container } = header();
  await waitFor(() => expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(1));
  expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('preparing');
  expect(container.querySelector('[data-tauri-drag-region]')).toBeNull();
  fireEvent(window, new Event('resize'));
  fireEvent(window, new Event('focus'));
  await waitFor(() => expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('custom'));
  expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(2);
  fireEvent(window, new Event('resize'));
  fireEvent(window, new Event('focus'));
  await new Promise(resolve => setTimeout(resolve, 200));
  expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(2);
});

test('native fallback retries on window focus but does not loop on its own resize events', async () => {
  let recovered = false;
  const invoke = vi.fn(command => Promise.resolve(command === 'activate_desktop_frame' ? (recovered ? 'custom' : 'native') : undefined));
  window.__TAURI__ = { core: { invoke } };
  const { container } = header();
  await waitFor(() => expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('native'));
  fireEvent(window, new Event('resize'));
  await new Promise(resolve => setTimeout(resolve, 200));
  expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(2);
  recovered = true;
  fireEvent(window, new Event('focus'));
  await waitFor(() => expect(container.querySelector('header').getAttribute('data-titlebar-mode')).toBe('custom'));
  expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(3);
});

test('unmounting a deferred header cancels its scheduled restoration retry', async () => {
  const invoke = vi.fn(command => Promise.resolve(command === 'activate_desktop_frame' ? 'deferred' : undefined));
  window.__TAURI__ = { core: { invoke } };
  const { unmount } = header();
  await waitFor(() => expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(1));
  await new Promise(resolve => setTimeout(resolve, 20));
  fireEvent(window, new Event('focus'));
  unmount();
  await new Promise(resolve => setTimeout(resolve, 250));
  expect(invoke.mock.calls.filter(([name]) => name === 'activate_desktop_frame')).toHaveLength(1);
});

test('concurrent titlebar requests share one native activation', async () => {
  const invoke = vi.fn(() => Promise.resolve('custom'));
  window.__TAURI__ = { core: { invoke } };
  expect(await Promise.all([activateDesktopFrame(), activateDesktopFrame()])).toEqual(['custom', 'custom']);
  expect(invoke).toHaveBeenCalledTimes(1);
});

test('titlebar mouse handling uses only native drag and double-click actions and preserves right-click menu', async () => {
  const invoke = vi.fn(() => Promise.resolve());
  window.__TAURI__ = { core: { invoke } };
  const event = { button: 0, detail: 1, preventDefault: vi.fn(), stopPropagation: vi.fn() };
  await handleDesktopTitlebarMouseDown(event);
  expect(invoke).toHaveBeenLastCalledWith('plugin:window|start_dragging');
  expect(event.stopPropagation).toHaveBeenCalledTimes(1);
  await handleDesktopTitlebarMouseDown({ ...event, detail: 2 });
  expect(invoke).toHaveBeenLastCalledWith('plugin:window|internal_toggle_maximize');
  await handleDesktopTitlebarMouseDown({ ...event, button: 2 });
  expect(invoke).toHaveBeenCalledTimes(2);
  invoke.mockRejectedValueOnce(new Error('drag failed'));
  await expect(handleDesktopTitlebarMouseDown(event)).rejects.toThrow('drag failed');
});

test('caption localization retains the plugin restore state and accurately explains close-to-tray', async () => {
  window.__TAURI__ = { core: { invoke: vi.fn(() => Promise.resolve()) } };
  const root = document.createElement('div');
  root.setAttribute('data-tauri-plugin-decoration-root', '');
  root.innerHTML = '<button data-tauri-decoration-control="maximize" aria-label="Maximize window size">&#xE922;</button><button data-tauri-decoration-control="close">&#xE8BB;</button>';
  document.body.append(root);
  await syncDesktopAppearance('dark', 'zh-CN');
  expect(root.children[1].getAttribute('aria-label')).toBe('收起到托盘');
  root.children[0].textContent = '\uE923';
  root.children[0].setAttribute('aria-label', 'Restore window size');
  await waitFor(() => expect(root.children[0].getAttribute('aria-label')).toBe('还原窗口'));
  await syncDesktopAppearance('light', 'en');
  expect(root.children[0].getAttribute('aria-label')).toBe('Restore window');
  await expect(syncDesktopAppearance('pink', 'en')).rejects.toThrow('Invalid desktop appearance');
  window.__TAURI__.core.invoke.mockResolvedValue('unexpected');
  await expect(activateDesktopFrame()).rejects.toThrow('Invalid desktop frame state');
});
