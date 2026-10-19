import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { readDesktopAclSources, verifyDesktopAcl, verifyDesktopAclSources } from '../../scripts/verify-desktop-acl.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const managedSourceCommands = [
  'list_local_rasters', 'open_local_raster', 'inspect_local_raster', 'sample_local_raster', 'export_local_raster',
  'list_official_vector_packages', 'search_official_vector_packages', 'download_official_vector_package',
  'list_official_parquet', 'search_official_parquet', 'download_official_parquet', 'parquet_original_metadata', 'parquet_original_rows',
  'gebco_grids', 'gebco_list', 'gebco_get', 'gebco_create_subset',
  'gebco_inspect', 'gebco_pixel', 'gebco_export_source', 'gebco_export_archive',
];

test('packaged desktop registers and grants every local runtime command without remote access', async () => {
  const { main, build, capability, config } = await readDesktopAclSources(root);
  assert.equal(verifyDesktopAclSources(main, build, capability, config).desktopCommandAcl, 'passed');
  assert.equal((await verifyDesktopAcl(root)).desktopCommandAcl, 'passed');
  // The exact regression: handler compilation succeeded while the packaged UI
  // was rejected by its incomplete app ACL before entering run_recipe.
  const missingPermission = { ...capability, permissions: capability.permissions.filter(value => value !== 'allow-run-recipe') };
  assert.throws(() => verifyDesktopAclSources(main, build, missingPermission, config), /permissions must cover/);
  assert.throws(() => verifyDesktopAclSources(main, build.replace('"run_recipe",', ''), capability, config), /manifest must cover/);
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, remote: { urls: ['https://example.invalid'] } }, config), /remote origins/);
});

test('each managed-source desktop command requires its declaration, handler, manifest and exact permission', async (t) => {
  const { main, build, capability, config } = await readDesktopAclSources(root);
  for (const command of managedSourceCommands) {
    await t.test(command, () => {
      const declaration = new RegExp('#\\[tauri::command\\]\\s*(?:pub\\s+)?(?:async\\s+)?fn\\s+' + command + '\\s*\\(');
      assert.match(main, declaration, 'The verifier must read the real command module');
      assert.throws(() => verifyDesktopAclSources(main.replace(declaration, 'fn ' + command + '('), build, capability, config), /Every declared Tauri command must be registered/);
      // rustfmt can put the same identifier on its own line in a `use` block.
      // Mutate the registration itself, never an unrelated import.
      const handler = main.match(/tauri::generate_handler!\[([\s\S]*?)\]/)[0];
      const missingHandler = main.replace(handler, handler.replace(new RegExp('(^[ \\t]*)' + command + ',[ \\t]*$', 'm'), ''));
      assert.notEqual(missingHandler, main, 'The command must have a real handler registration');
      assert.throws(() => verifyDesktopAclSources(missingHandler, build, capability, config), /Every declared Tauri command must be registered/);
      assert.throws(() => verifyDesktopAclSources(main, build.replace('"' + command + '",', ''), capability, config), /manifest must cover/);
      const permission = 'allow-' + command.replaceAll('_', '-');
      assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, permissions: capability.permissions.filter(value => value !== permission) }, config), /permissions must cover/);
    });
  }
});

test('desktop command ACL rejects extra commands, duplicate permissions and widened scope', async () => {
  const { main, build, capability, config } = await readDesktopAclSources(root);
  assert.throws(() => verifyDesktopAclSources(main, build.replace('.commands(&[', '.commands(&["unused_command",'), capability, config), /manifest must cover/);
  for (const permission of ['allow-unused-command', 'core:default', 'core:event:allow-emit']) {
    assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, permissions: [...capability.permissions, permission] }, config), /permissions must cover/);
  }
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, permissions: [...capability.permissions, 'allow-open-local-raster'] }, config), /contains duplicates/);
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, windows: ['main', 'other'] }, config), /scoped to the main window/);
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, webviews: ['*'] }, config), /webview scope/);
  assert.throws(() => verifyDesktopAclSources(main, build, { ...capability, local: false }, config), /Bundled local content/);
  const extraCapability = { ...config, app: { ...config.app, security: { ...config.app.security, capabilities: ['main-window', 'remote-window'] } } };
  assert.throws(() => verifyDesktopAclSources(main, build, capability, extraCapability), /additional desktop capability/);
});
