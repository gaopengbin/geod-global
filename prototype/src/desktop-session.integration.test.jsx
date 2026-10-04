import React from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DesktopSessionControl } from './desktop-session.jsx';
import { hideDesktopWindow, quitDesktop } from './desktop-frame.js';

vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (value, variables) => value.replace('{count}', variables?.count ?? '{count}'), number: String }) }));
afterEach(() => { delete window.__TAURI__; });
const show = invoke => {
  if (invoke) window.__TAURI__ = { core: { invoke } };
  return render(<DesktopSessionControl />);
};
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const open = async () => userEvent.click(screen.getByRole('button', { name: 'Exit app' }));

test('browser previews expose no desktop exit control and cannot call native lifecycle commands', async () => {
  show();
  expect(screen.queryByRole('button')).toBeNull();
  await expect(hideDesktopWindow()).rejects.toThrow();
  await expect(quitDesktop()).rejects.toThrow();
});

test('exit waits for valid task status and counts running and queued work only', async () => {
  const request = deferred();
  const invoke = vi.fn(() => request.promise);
  show(invoke);
  await open();
  expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(true);
  expect(screen.getByRole('status').textContent).toBe('Checking active tasks…');
  await act(async () => request.resolve(['running', 'queued', 'succeeded', 'failed', 'interrupted', 'cancelled'].map(status => ({ status }))));
  expect(screen.getByRole('status').textContent).toBe('Running or queued tasks: 2');
  expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(false);
  expect(invoke).toHaveBeenCalledWith('list_jobs', {});
  await userEvent.click(screen.getByRole('button', { name: 'Close' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(invoke).toHaveBeenCalledTimes(1);
});

test('invalid task status blocks exit, retry recovers, and a dismissed status request cannot reopen the dialog', async () => {
  const later = deferred();
  const invoke = vi.fn().mockResolvedValueOnce([{ status: 'unknown' }]).mockReturnValueOnce(later.promise);
  show(invoke);
  await open();
  await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
  expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(true);
  await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
  await userEvent.click(screen.getByRole('button', { name: 'Close' }));
  await act(async () => later.resolve([{ status: 'running' }]));
  expect(screen.queryByRole('dialog')).toBeNull();
  // Allow the shared dialog's close animation and focus restoration to finish.
  await act(async () => new Promise(resolve => setTimeout(resolve, 200)));
  invoke.mockResolvedValueOnce([]);
  await open();
  await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Running or queued tasks: 0'));
  expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(false);
});

test('task read failure still allows hiding without stopping any work', async () => {
  const invoke = vi.fn(command => command === 'list_jobs' ? Promise.reject('task read failed') : Promise.resolve());
  show(invoke);
  await open();
  await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('task read failed'));
  await userEvent.click(screen.getByRole('button', { name: 'Keep running in tray' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(invoke).toHaveBeenLastCalledWith('hide_desktop_window');
  expect(invoke.mock.calls.some(([command]) => command === 'quit_desktop')).toBe(false);
});

test('graceful exit is called once and prevents dismissal while native cleanup is pending', async () => {
  const quit = deferred();
  const invoke = vi.fn(command => command === 'list_jobs' ? Promise.resolve([{ status: 'running' }]) : quit.promise);
  show(invoke);
  await open();
  await waitFor(() => expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(false));
  await userEvent.click(screen.getByRole('button', { name: 'Exit and stop tasks' }));
  expect(screen.getByRole('status').textContent).toBe('Saving task state and exiting…');
  for (const name of ['Close', 'Keep running in tray', 'Exit and stop tasks']) expect(screen.getByRole('button', { name }).disabled).toBe(true);
  await userEvent.keyboard('{Escape}');
  expect(screen.getByRole('dialog')).toBeTruthy();
  expect(invoke.mock.calls.filter(([command]) => command === 'quit_desktop')).toHaveLength(1);
  await act(async () => quit.resolve());
  expect(screen.getByRole('button', { name: 'Close' }).disabled).toBe(true);
});

test('native exit errors remain visible and allow retrying the exit', async () => {
  const invoke = vi.fn().mockResolvedValueOnce([]).mockRejectedValueOnce('shutdown unavailable').mockResolvedValueOnce();
  show(invoke);
  await open();
  await waitFor(() => expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(false));
  await userEvent.click(screen.getByRole('button', { name: 'Exit and stop tasks' }));
  await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('shutdown unavailable'));
  expect(screen.getByRole('button', { name: 'Exit and stop tasks' }).disabled).toBe(false);
  await userEvent.click(screen.getByRole('button', { name: 'Exit and stop tasks' }));
  expect(invoke.mock.calls.filter(([command]) => command === 'quit_desktop')).toHaveLength(2);
});
