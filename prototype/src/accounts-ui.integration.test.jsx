import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ProviderAccountsPanel } from './accounts-ui.jsx';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { desktopAvailable, runtimeRequest } from './runtime-client.js';

vi.mock('./runtime-client.js', () => ({ desktopAvailable: vi.fn(() => true), runtimeRequest: vi.fn(), syncDesktopLocale: vi.fn(async () => {}) }));
const state = (provider, status = 'not-connected') => ({ provider, status, expiresAt: status === 'connected' ? '2099-01-01T00:00:00Z' : null, verifiedAt: status === 'connected' ? '2026-10-01T00:00:00Z' : null });
let accounts, storage;
beforeEach(() => {
  history.replaceState(null, '', '#Settings');
  accounts = [state('nasa-earthdata'), state('copernicus')];
  storage = new Map([['geod-global-locale', 'en']]);
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) } });
  desktopAvailable.mockReturnValue(true);
  runtimeRequest.mockReset();
  runtimeRequest.mockImplementation(async operation => operation === 'accounts' ? accounts : null);
});
function renderAccounts() { return render(<I18nProvider><RuntimeContext.Provider value={{ health: { status: 'ok' } }}><ProviderAccountsPanel/></RuntimeContext.Provider></I18nProvider>); }
async function open(provider) {
  await waitFor(() => expect(screen.getAllByRole('button', { name: 'Connect account' })[0].disabled).toBe(false));
  const card = screen.getByRole('heading', { name: provider }).closest('article');
  await userEvent.click(within(card).getByRole('button', { name: 'Connect account' }));
  return screen.getByRole('dialog');
}
describe('Native provider account entry', () => {
  it('shows public provider entry but blocks browser credential submission', async () => {
    desktopAvailable.mockReturnValue(false);
    renderAccounts();
    await screen.findByText('Open the desktop app to manage data source authorization.');
    expect(screen.getAllByRole('button', { name: 'Connect account' }).every(button => button.disabled)).toBe(true);
    expect(screen.getByRole('link', { name: 'Open NASA Earthdata website' }).href).toBe('https://urs.earthdata.nasa.gov/profile');
    expect(runtimeRequest.mock.calls.map(([operation]) => operation)).toEqual(['accounts']);
  });

  it('masks the token, submits once, and accepts only returned server status', async () => {
    let resolve; const requests = [];
    runtimeRequest.mockImplementation(async (operation, payload) => {
      if (operation === 'accounts') return accounts;
      if (operation === 'connectAccount') { requests.push({ ...payload }); return new Promise(done => { resolve = done; }); }
    });
    renderAccounts();
    const dialog = await open('NASA Earthdata');
    const token = within(dialog).getByLabelText('Earthdata user token');
    expect(token.type).toBe('password');
    await userEvent.type(token, 'PRIVATE-USER-TOKEN');
    await userEvent.dblClick(within(dialog).getByRole('button', { name: 'Verify and save' }));
    expect(requests).toEqual([{ provider: 'nasa-earthdata', token: 'PRIVATE-USER-TOKEN' }]);
    expect(token.value).toBe('');
    expect(within(dialog).getByRole('button', { name: 'Close', exact: true }).disabled).toBe(true);
    expect(screen.queryByText('Authorization verified')).toBeNull();
    resolve(state('nasa-earthdata', 'connected'));
    await screen.findByText('Authorization verified');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(JSON.stringify([...storage])).not.toContain('PRIVATE');
  });

  it('clears secrets after rejection, preserves username, and shows a safe retry reference', async () => {
    let payload;
    runtimeRequest.mockImplementation(async (operation, data) => {
      if (operation === 'accounts') return accounts;
      payload = { ...data };
      throw new Error('backend body PASSWORD-DO-NOT-LEAK Bearer SECRET');
    });
    renderAccounts();
    const dialog = await open('Copernicus Data Space');
    await userEvent.type(within(dialog).getByLabelText('Username or email'), 'tester@example.test');
    await userEvent.type(within(dialog).getByLabelText('Password', { exact: true }), 'PRIVATE-PASSWORD');
    await userEvent.type(within(dialog).getByLabelText('Two-step code · optional'), '123456');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verify and save' }));
    expect(payload).toEqual({ provider: 'copernicus', username: 'tester@example.test', password: 'PRIVATE-PASSWORD', totp: '123456' });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Could not complete authorization. Retry or reconnect this account.');
    expect(alert.textContent).toContain('AUTH-');
    expect(alert.textContent).not.toContain('SECRET');
    expect(within(dialog).getByLabelText('Username or email').value).toBe('tester@example.test');
    expect(within(dialog).getByLabelText('Password', { exact: true }).value).toBe('');
    expect(within(dialog).getByLabelText('Two-step code · optional').value).toBe('');
    expect(JSON.stringify([...storage])).not.toContain('PRIVATE');
  });

  it('rejects malformed two-step codes before sending credentials', async () => {
    renderAccounts();
    const dialog = await open('Copernicus Data Space');
    await userEvent.type(within(dialog).getByLabelText('Username or email'), 'tester');
    await userEvent.type(within(dialog).getByLabelText('Password', { exact: true }), 'pass');
    await userEvent.type(within(dialog).getByLabelText('Two-step code · optional'), 'bad');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verify and save' }));
    expect((await screen.findByRole('alert')).textContent).toContain('six digits');
    expect(runtimeRequest.mock.calls.filter(([operation]) => operation === 'connectAccount')).toHaveLength(0);
  });

  it('verifies restored credentials and forgets local access without returning any token', async () => {
    accounts[0] = { ...state('nasa-earthdata', 'saved'), expiresAt: '2099-01-01T00:00:00Z', verifiedAt: '2026-10-01T00:00:00Z' };
    runtimeRequest.mockImplementation(async (operation, payload) => {
      if (operation === 'accounts') return accounts;
      return state(payload.provider, operation === 'verifyAccount' ? 'connected' : 'not-connected');
    });
    renderAccounts();
    await screen.findByText('Saved · check access');
    await userEvent.click(screen.getByRole('button', { name: 'Check NASA Earthdata access' }));
    await screen.findByText('Authorization verified');
    await userEvent.click(screen.getByRole('button', { name: 'Forget NASA Earthdata authorization' }));
    await waitFor(() => expect(screen.queryByText('Authorization verified')).toBeNull());
    expect(runtimeRequest.mock.calls.filter(([operation]) => operation !== 'accounts')).toEqual([['verifyAccount', { provider: 'nasa-earthdata' }], ['disconnectAccount', { provider: 'nasa-earthdata' }]]);
  });

  it('opens the selected provider from an exploration authorization link', async () => {
    history.replaceState(null, '', '#Settings?account=copernicus');
    renderAccounts();
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByRole('heading', { name: 'Authorize Copernicus Data Space' })).toBeTruthy();
    expect(within(dialog).getByRole('link', { name: 'Register or manage Copernicus account' })).toBeTruthy();
  });
});
