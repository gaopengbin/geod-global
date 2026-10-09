import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

function unique(values, label) {
  assert.equal(new Set(values).size, values.length, `${label} contains duplicates`);
  return [...values].sort();
}

// Compare the real registered commands with both parts of Tauri's app ACL.
// A generated handler alone cannot authorize a command once an app ACL exists.
export function verifyDesktopAclSources(main, build, capability, config) {
  const handlers = [...main.matchAll(/tauri::generate_handler!\[([\s\S]*?)\]/g)];
  assert.equal(handlers.length, 1, 'Expected one explicit desktop command registration');
  const registered = handlers[0][1].replace(/\/\/[^\n]*/g, '').split(',').map(value => value.trim()).filter(Boolean);
  assert(registered.length > 0 && registered.every(value => /^[a-z_][a-z_0-9]*$/.test(value)), 'Unsupported desktop command registration syntax');
  const declarations = [...main.matchAll(/#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(/g)].map(match => match[1]);
  assert.deepEqual(unique(declarations, 'Desktop command declarations'), unique(registered, 'Desktop command handlers'), 'Every declared Tauri command must be registered');
  const manifests = [...build.matchAll(/\.commands\(\s*&\s*\[([\s\S]*?)\]\s*\)/g)];
  assert.equal(manifests.length, 1, 'Expected one explicit Tauri app command manifest');
  const generated = [...manifests[0][1].matchAll(/"([a-z_][a-z_0-9]*)"/g)].map(match => match[1]);
  assert.deepEqual(unique(generated, 'App ACL manifest'), unique(registered, 'Desktop command handlers'), 'App ACL manifest must cover exactly the registered desktop commands');
  const expected = [...registered.map(command => 'allow-' + command.replaceAll('_', '-')),
    'decoration:default', 'core:window:allow-start-dragging', 'core:window:allow-internal-toggle-maximize',
    // The main window listens for the bounded Agent revision notification.
    // Event emission from JavaScript remains unavailable.
    'core:event:allow-listen', 'core:event:allow-unlisten'];
  assert.deepEqual(unique(capability.permissions, 'Main-window permissions'), unique(expected, 'Expected permissions'), 'Main-window permissions must cover exactly the registered desktop commands');
  assert.equal(capability.identifier, 'main-window');
  assert.deepEqual(capability.windows, ['main'], 'Desktop commands must remain scoped to the main window');
  assert.equal(capability.remote, undefined, 'Desktop command capability must not authorize remote origins');
  assert.equal(capability.webviews, undefined, 'Unexpected change to desktop webview scope');
  assert.notEqual(capability.local, false, 'Bundled local content must retain access');
  assert.deepEqual(config.app.security.capabilities, ['main-window'], 'Unexpected additional desktop capability');
  return { desktopCommandAcl: 'passed', registeredCommands: registered.length, window: 'main', remoteOrigins: false };
}

export async function verifyDesktopAcl(root) {
  const files = await Promise.all(['src-tauri/src/main.rs', 'src-tauri/build.rs', 'src-tauri/capabilities/main-window.json', 'src-tauri/tauri.conf.json','src-tauri/src/distribution.rs','src-tauri/src/identity.rs'].map(file => readFile(path.join(root, file), 'utf8')));
  return verifyDesktopAclSources(files[0]+'\n'+files[4]+'\n'+files[5], files[1], JSON.parse(files[2]), JSON.parse(files[3]));
}
