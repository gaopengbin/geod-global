import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyDesktopAcl } from './verify-desktop-acl.mjs';

const root = await realpath(fileURLToPath(new URL('../', import.meta.url)));
const localRequire = createRequire(path.join(root, 'package.json'));
const inside = (target) => {
  const relative = path.relative(root, target);
  return !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('..' + path.sep);
};
const resolutions = {};
for (const name of ['react', 'react-dom/client', 'lucide-react', 'vite']) {
  const resolved = await realpath(localRequire.resolve(name));
  assert(inside(resolved), name + ' resolved outside this repository: ' + resolved);
  resolutions[name] = path.relative(root, resolved).split(path.sep).join('/');
}
const config = await readFile(path.join(root, 'prototype/vite.config.mjs'), 'utf8');
assert(!/GEOD_DESIGN_DEPS|tif-downloader|[A-Za-z]:[\\/]/.test(config), 'Vite contains a workstation-specific dependency');
for (const file of ['package.json']) {
  const manifest = JSON.parse(await readFile(path.join(root, file), 'utf8'));
  for (const [name, version] of Object.entries({ ...manifest.dependencies, ...manifest.devDependencies })) {
    assert(!/^(file:|link:|[A-Za-z]:[\\/])/.test(version), 'Unexpected local dependency ' + name);
  }
}
const lock = JSON.parse(await readFile(path.join(root, 'package-lock.json'), 'utf8'));
assert.equal(lock.packages?.['']?.name, 'geod-global', 'Root lockfile must describe this project');
assert(!Object.values(lock.packages).some(pkg => pkg.link), 'Root dependency tree must not require workspace symlinks');

const samples = path.join(root, 'prototype/public');
const manifest = JSON.parse(await readFile(path.join(samples, 'samples/manifest.json'), 'utf8'));
assert.equal(manifest.scenes.length, 7, 'Expected seven documented source scenes');
for (const scene of manifest.scenes) {
  const target = await realpath(path.join(samples, scene.thumbnail));
  assert(inside(target), 'Sample must remain inside this repository');
  const bytes = await readFile(target);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), scene.sha256, 'Changed sample: ' + scene.id);
}

const brokenLinks = [];
for (const name of await readdir(path.join(root, 'GeoD-Global-Spec'))) {
  if (!name.endsWith('.md')) continue;
  const doc = path.join(root, 'GeoD-Global-Spec', name);
  const content = await readFile(doc, 'utf8');
  for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
    const link = match[1];
    // Historical source references are not runtime dependencies.
    if (/^(?:[A-Za-z][\w+.-]*:|#|\/)/.test(link)) continue;
    const target = path.resolve(path.dirname(doc), link.split('#')[0]);
    if (target === path.join(root, 'GeoD-Global-Spec/contracts/verification-result.json')) continue;
    try { await realpath(target); } catch { brokenLinks.push(name + ': ' + link); }
  }
}
assert.deepEqual(brokenLinks, [], 'Broken specification links');
const desktopAcl = await verifyDesktopAcl(root);
console.log(JSON.stringify({dependencyIsolation:'passed',resolutions,verifiedThumbnails:7,relativeDocumentationLinks:'passed',...desktopAcl},null,2));
