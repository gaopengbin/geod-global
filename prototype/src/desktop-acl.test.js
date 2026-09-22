import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { verifyDesktopAclSources } from '../../scripts/verify-desktop-acl.mjs';

test('packaged desktop registers and grants every local runtime command without remote access', async () => {
  const [main, build, rawCapability, rawConfig] = await Promise.all([
    '../../src-tauri/src/main.rs', '../../src-tauri/build.rs', '../../src-tauri/capabilities/main-window.json', '../../src-tauri/tauri.conf.json',
  ].map(file => readFile(new URL(file, import.meta.url), 'utf8')));
  const capability = JSON.parse(rawCapability);
  const config = JSON.parse(rawConfig);
  assert.equal(verifyDesktopAclSources(main, build, capability, config).desktopCommandAcl, 'passed');
  // The exact regression: handler compilation succeeded while the packaged UI
  // was rejected by its incomplete app ACL before entering run_recipe.
  const missingPermission = { ...capability, permissions: capability.permissions.filter(value => value !== 'allow-run-recipe') };
  assert.throws(() => verifyDesktopAclSources(main, build, missingPermission, config), /permissions must cover/);
  assert.throws(() => verifyDesktopAclSources(main, build.replace('"run_recipe",', ''), capability, config), /manifest must cover/);
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, remote: { urls: ['https://example.invalid'] } }, config), /remote origins/);
});
