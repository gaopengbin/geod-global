import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyDesktopAcl } from './verify-desktop-acl.mjs';
import { verifyUiSystem } from './verify-ui-system.mjs';

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
const basemap = path.join(samples, 'basemaps/natural-earth-50m-land.geojson');
assert(inside(await realpath(basemap)), 'AOI reference map must remain inside this repository');
assert.equal(createHash('sha256').update(await readFile(basemap)).digest('hex'),
  'e874b27a51d146452be360cafb3cc50c86001074a67d534113e6534682f9826b',
  'Changed Natural Earth AOI reference map');
for (const [name, hash, count] of [
  ['natural-earth-50m-admin-0-countries.geojson', '3e458fc036ad0a66411f2c1e6cac49c5d7bfb81cb1123bc513b22511a2b7fdeb', 242],
  ['natural-earth-50m-admin-1-states-provinces.geojson', '69a0e06e640b2d505858ae1cb63034e4677f3000b35a98e16312932b98c426b9', 294],
]) {
  const target = path.join(samples, 'basemaps', name);
  assert(inside(await realpath(target)), `Administrative reference layer must remain inside this repository: ${name}`);
  const bytes = await readFile(target);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), hash, `Changed administrative reference layer: ${name}`);
  const features = JSON.parse(bytes.toString('utf8')).features;
  assert.equal(features.length, count, `Unexpected administrative coverage: ${name}`);
  if (name.includes('admin-1')) assert.equal(new Set(features.map(feature => feature.properties.adm0_a3)).size, 9,
    'This 1:50m province file only covers nine countries; do not advertise worldwide province coverage');
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
const sharedUi = await verifyUiSystem(root);
console.log(JSON.stringify({dependencyIsolation:'passed',resolutions,verifiedThumbnails:7,verifiedAoiBasemap:'passed',verifiedAdministrativeLayers:2,relativeDocumentationLinks:'passed',...desktopAcl,...sharedUi},null,2));
