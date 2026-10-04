import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { verifyDesktopCspConfig } from '../../scripts/verify-desktop-csp.mjs';

const config = JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
test('desktop policy permits COG fetch and local decode without remote script execution', () => {
  assert.equal(verifyDesktopCspConfig(config).desktopCogPolicy, 'passed');
  for (const directive of ['csp', 'devCsp']) {
    const missingCog = structuredClone(config);
    missingCog.app.security[directive] = config.app.security[directive].replace(/connect-src[^;]+/, value => value.replace(' https://sentinel-cogs.s3.us-west-2.amazonaws.com', ''));
    assert.throws(() => verifyDesktopCspConfig(missingCog), /COG requests/);
    for (const origin of ['https://copernicus-dem-30m.s3.eu-central-1.amazonaws.com','https://copernicus-dem-90m.s3.eu-central-1.amazonaws.com','geod-elevation:','http://geod-elevation.localhost']) {
      const missingDem=structuredClone(config);
      missingDem.app.security[directive]=config.app.security[directive].replace(` ${origin}`,'');
      assert.throws(()=>verifyDesktopCspConfig(missingDem),/COG requests/);
    }
    const missingWorker = structuredClone(config);
    missingWorker.app.security[directive] = config.app.security[directive].replace("worker-src 'self' blob:", "worker-src 'self'");
    assert.throws(() => verifyDesktopCspConfig(missingWorker), /decoder workers/);
    const missingWasm = structuredClone(config);
    missingWasm.app.security[directive] = config.app.security[directive].replace(" 'wasm-unsafe-eval'", '');
    assert.throws(() => verifyDesktopCspConfig(missingWasm), /WebAssembly/);
  }
});
test('desktop policy rejects broad network and remote script access', () => {
  for (const [before, after] of [
    ["connect-src 'self'", "connect-src * 'self'"],
    ["script-src 'self'", "script-src 'self' https: 'unsafe-eval'"],
    ["worker-src 'self' blob:", "worker-src 'self' blob: https:"],
  ]) {
    const broadened = structuredClone(config);
    broadened.app.security.csp = config.app.security.csp.replace(before, after);
    assert.throws(() => verifyDesktopCspConfig(broadened));
  }
});
