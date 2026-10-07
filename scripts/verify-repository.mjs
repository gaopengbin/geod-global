import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { gunzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { verifyDesktopAcl } from './verify-desktop-acl.mjs';
import { verifyUiSystem } from './verify-ui-system.mjs';
import { verifyBrand } from './verify-brand.mjs';
import { verifyDesktopCsp } from './verify-desktop-csp.mjs';

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
const safeFixtures = path.join(root, 'crates/geod-runtime/fixtures/safe');
const safeSource = JSON.parse(await readFile(path.join(safeFixtures, 'SOURCE.json'), 'utf8'));
assert.equal(safeSource.kind, 'synthetic test fixture; not a downloaded Copernicus product');
assert.match(safeSource.generator, /GDAL JP2OpenJPEG/);
for (const fixture of [safeSource, safeSource.negativeFixture]) {
  assert.match(fixture.file, /^[a-z0-9-]+\.(?:zip|jp2)$/);
  const target = await realpath(path.join(safeFixtures, fixture.file));
  assert(inside(target), 'SAFE fixture must remain inside this repository');
  assert.equal(createHash('sha256').update(await readFile(target)).digest('hex'), fixture.sha256,
    'Changed synthetic SAFE fixture: ' + fixture.file);
}
const basemap = path.join(samples, 'basemaps/natural-earth-50m-land.geojson');
const viirsFixtures = path.join(root, 'crates/geod-runtime/fixtures/viirs');
const viirsExpected = JSON.parse(await readFile(path.join(viirsFixtures, 'expected.json'), 'utf8'));
const viirsSummaries = JSON.parse(await readFile(path.join(root, 'prototype/qa/viirs-science-summary-fixtures.json'), 'utf8'));
assert.match(viirsExpected.provenance, /Synthetic/);
assert.match(viirsSummaries.provenance, /Synthetic/);
assert.equal(viirsExpected.fixtures.length, 3);
assert.equal(viirsSummaries.summaries.length, 3);
for (const fixture of viirsExpected.fixtures) {
  assert.match(fixture.file, /^synthetic-(?:vnp09a1|vj109a1|vj209a1)\.h5\.gz$/);
  const target = await realpath(path.join(viirsFixtures, fixture.file));
  assert(inside(target), 'VIIRS synthetic fixture must remain inside this repository');
  const bytes = gunzipSync(await readFile(target), { maxOutputLength: 24 * 1024 * 1024 });
  assert.equal(bytes.length, fixture.byteCount);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), fixture.sourceSha256);
  const summary = viirsSummaries.summaries.find(s => s.itemId === fixture.itemId);
  assert.equal(summary?.sourceSha256, fixture.sourceSha256);
  assert.equal(summary?.schemaVersion, 'geod-viirs-science/v1');
  assert.equal(summary?.qualityMaskApplied, false);
  for (const [index, band] of fixture.bands.entries()) {
    assert.equal(summary.bands[index].samplesSha256, band.samplesSha256);
    assert.equal(summary.bands[index].noDataCount, band.noDataCount);
    assert.equal(summary.bands[index].outsideValidRangeCount, band.outsideValidRangeCount);
    assert.equal(summary.bands[index].sampleCount, 1440000);
  }
}
assert.equal(viirsExpected.negatives.length, 7);
const preparedViirs = JSON.parse(await readFile(path.join(root, 'prototype/qa/viirs-prepared-fixture.json'), 'utf8'));
assert.match(preparedViirs.provenance, /Synthetic/);
const preparedSummary = viirsSummaries.summaries.find(s => s.itemId === preparedViirs.source.itemId);
assert.deepEqual(preparedViirs.source.viirsScience, preparedSummary);
assert.equal(preparedViirs.jobs.length, 3);
for (const [index, job] of preparedViirs.jobs.entries()) {
  assert.equal(job.assetKey, ['red','green','blue'][index]);
  assert.equal(job.viirsPrepare.sourceJobId, preparedViirs.source.id);
  assert.equal(job.viirsPrepare.sourceSha256, preparedSummary.sourceSha256);
  assert.deepEqual(job.viirsPrepare.science, preparedSummary);
  assert.equal(preparedViirs.inspections[index].sha256, job.sha256);
  assert.equal(preparedViirs.inspections[index].reflectance.product, 'viirs-09a1-v002');
  assert.equal(preparedViirs.inspections[index].crs, 'VIIRS:Sinusoidal');
  assert.equal(preparedViirs.thumbnails[index].sha256, job.sha256);
  assert.equal(preparedViirs.composite.composite.sources[index].sha256, job.sha256);
}
for (const fixture of viirsExpected.negatives) {
  assert.match(fixture.file, /^negative-[a-z-]+\.h5\.gz$/);
  assert(inside(await realpath(path.join(viirsFixtures, fixture.file))), 'Negative VIIRS fixture escaped this repository');
}
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

const admin1Directory = path.join(samples, 'basemaps/admin1-10m');
const admin1Manifest = JSON.parse(await readFile(path.join(admin1Directory, 'manifest.json'), 'utf8'));
assert.equal(admin1Manifest.version, '5.1.1');
assert.equal(admin1Manifest.sourceSha256, 'efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05');
assert.equal(admin1Manifest.featureCount, 4596);
assert.equal(admin1Manifest.countryCount, 251);
const admin1IndexBytes = await readFile(path.join(admin1Directory, 'index.json'));
assert.equal(createHash('sha256').update(admin1IndexBytes).digest('hex'), admin1Manifest.indexSha256);
const admin1Index = JSON.parse(admin1IndexBytes.toString('utf8'));
assert.equal(admin1Index.areas.length, 4596);
assert.equal(new Set(admin1Index.areas.map(area => area.code)).size, 4596);
const agentAdminAliasBytes = await readFile(path.join(admin1Directory, 'agent-aliases.json'));
assert.equal(createHash('sha256').update(agentAdminAliasBytes).digest('hex'), '5ccfa93fb593d011b0d6b5b5e2561714969372627b0df1ba718117781355ab67');
const agentAdminAliases = JSON.parse(agentAdminAliasBytes.toString('utf8'));
assert.equal(agentAdminAliases.sourceSha256, admin1Manifest.sourceSha256);
assert.equal(agentAdminAliases.version, admin1Manifest.version);
assert.deepEqual(new Set(Object.keys(agentAdminAliases.aliases)), new Set(admin1Index.areas.map(area => area.code)));
assert(Object.values(agentAdminAliases.aliases).every(names => Array.isArray(names) && names.length <= 40 && names.every(name => typeof name === 'string' && name && name.trim() === name && [...name].length <= 200 && !/\p{Cc}/u.test(name))));
assert.deepEqual(admin1Manifest.clipLimitations, { polar: 2, 'date-line': 5, complex: 1 });
for (const [reason, count] of Object.entries(admin1Manifest.clipLimitations))
  assert.equal(admin1Index.areas.filter(area => area.clipLimitation === reason).length, count);
let admin1FeatureCount = 0;
const admin1GeometryCodes = new Set();
for (const [country, record] of Object.entries(admin1Manifest.countries)) {
  assert.match(country, /^[A-Z]{3}$/);
  assert.equal(record.file, `${country}.geojson`);
  const target = path.join(admin1Directory, record.file);
  assert(inside(await realpath(target)), `Admin-1 geometry must remain inside this repository: ${country}`);
  const bytes = await readFile(target);
  assert.equal(bytes.length, record.bytes, `Admin-1 geometry size changed: ${country}`);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), record.sha256, `Admin-1 geometry changed: ${country}`);
  const features = JSON.parse(bytes.toString('utf8')).features;
  assert.equal(features.length, record.features, `Admin-1 coverage changed: ${country}`);
  assert(features.every(feature => feature.properties.adm0_a3 === country && ['Polygon', 'MultiPolygon'].includes(feature.geometry.type)));
  for (const feature of features) admin1GeometryCodes.add(feature.properties.adm1_code);
  admin1FeatureCount += features.length;
}
assert.equal(admin1FeatureCount, 4596);
assert.deepEqual(admin1GeometryCodes, new Set(admin1Index.areas.map(area => area.code)));

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
const brand = await verifyBrand(root);
const desktopCsp = await verifyDesktopCsp(root);
console.log(JSON.stringify({dependencyIsolation:'passed',resolutions,verifiedThumbnails:7,verifiedSyntheticSafeFixtures:2,verifiedAoiBasemap:'passed',verifiedAdministrativeLayers:2,verifiedGlobalAdmin1:{features:admin1FeatureCount,countries:admin1Manifest.countryCount},relativeDocumentationLinks:'passed',...desktopAcl,...sharedUi,...brand,...desktopCsp},null,2));
