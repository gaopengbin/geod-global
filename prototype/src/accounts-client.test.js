import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { runtimeRequest, validateAccountStatus } from './runtime-client.js';

afterEach(() => { delete globalThis.window; });
const status = { provider: 'nasa-earthdata', status: 'connected', expiresAt: '2099-01-01T00:00:00Z', verifiedAt: '2026-10-01T00:00:00Z' };
test('credential mutations cannot fall back to loopback HTTP', async () => {
  for (const operation of ['connectAccount', 'verifyAccount', 'disconnectAccount']) {
    await assert.rejects(runtimeRequest(operation, { provider: 'nasa-earthdata', token: 'DO-NOT-SEND' }), /desktop app/);
  }
});
test('account responses reject secrets, unknown providers and unverified success', () => {
  assert.equal(validateAccountStatus(status), status);
  for (const value of [{ ...status, token: 'LEAK' }, { ...status, expiresAt: null }, { ...status, verifiedAt: 'not a date' }, { ...status, provider: 'unreviewed' }]) {
    assert.throws(() => validateAccountStatus(value), /invalid status/);
  }
});
test('native credentials use a request envelope and work without modern AbortSignal helpers', async () => {
  const timeout = AbortSignal.timeout, any = AbortSignal.any;
  try {
    AbortSignal.timeout = undefined; AbortSignal.any = undefined;
    let call;
    globalThis.window = { __TAURI__: { core: { invoke: async (command, args) => { call = { command, args }; return status; } } } };
    const result = await runtimeRequest('connectAccount', { provider: 'nasa-earthdata', token: 'PRIVATE' });
    assert.deepEqual(call, { command: 'connect_provider_account', args: { request: { provider: 'nasa-earthdata', token: 'PRIVATE' } } });
    assert.equal(result, status);
    const controller = new AbortController(); controller.abort();
    await assert.rejects(runtimeRequest('accounts', null, controller.signal), { name: 'AbortError' });
  } finally { AbortSignal.timeout = timeout; AbortSignal.any = any; }
});
