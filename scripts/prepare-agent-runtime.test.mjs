import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, cp, copyFile, readFile, writeFile, access, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { build } from 'esbuild';
import { prepareRuntimeNotices, runtimeVersions } from './prepare-agent-runtime.mjs';

const repository = resolve(import.meta.dirname, '..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'geod-agent-notices-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await cp(join(repository, 'licenses', 'agent-runtime'), join(root, 'licenses', 'agent-runtime'), { recursive: true });
  await copyFile(join(repository, 'LICENSE'), join(root, 'LICENSE'));
  await writeFile(join(root, 'package.json'), JSON.stringify({ name: 'geod-global', version: 'test', license: 'GPL-3.0-only' }));
  await writeFile(join(root, 'package-lock.json'), JSON.stringify({ packages: { '': { license: 'GPL-3.0-only' } } }));
  return { repository: root, output: join(root, 'owned-runtime') };
}

test('actual Agent bundle gets every published dependency notice and the reviewed missing attribution', async t => {
  const output = await mkdtemp(join(tmpdir(), 'geod-agent-bundle-notices-'));
  t.after(() => rm(output, { recursive: true, force: true }));
  const bundled = await build({ absWorkingDir: repository, entryPoints: ['agent/stdio.mjs'], bundle: true,
    platform: 'node', format: 'esm', target: 'node24', outfile: join(output, 'agent.mjs'), write: false, metafile: true });
  const inventory = await prepareRuntimeNotices({ repository, output, metafile: bundled.metafile });
  assert.deepEqual(inventory.runtimeVersions, runtimeVersions);
  const names = inventory.npmPackages.map(value => value.name);
  for (const required of ['ai', '@ai-sdk/openai', '@ai-sdk/openai-compatible', '@ai-sdk/provider-utils', '@ai-sdk/provider', 'zod']) assert(names.includes(required));
  const missingPublishedLicense = inventory.npmPackages.find(value => value.name === '@ai-sdk/provider-utils');
  assert.equal(missingPublishedLicense.version, '5.0.53');
  assert.deepEqual(missingPublishedLicense.notices, ['licenses/AI-provider-utils-v5.0.53-LICENSE.txt']);
  assert.equal(missingPublishedLicense.completeTerms, 'licenses/Codex-v0.159.2-LICENSE.txt');
  for (const entry of inventory.entries) {
    const bytes = await readFile(join(output, entry.file));
    assert.equal(bytes.length, entry.bytes);
    assert.equal(digest(bytes), entry.sha256);
  }
  const serialized = await readFile(join(output, 'licenses', 'inventory.json'), 'utf8');
  assert.deepEqual(JSON.parse(serialized), inventory);
  assert(!serialized.includes(repository));
  assert(!serialized.includes(output));
  assert(!serialized.includes('apiKey'));
  assert.equal((await readFile(join(output, 'licenses', 'Node-v24.14.0-LICENSE.txt'))).length, 156926);
  assert.match(await readFile(join(output, 'licenses', 'Codex-v0.159.2-NOTICE.txt'), 'utf8'), /Ratatui/);
  assert.match(await readFile(join(output, 'licenses', 'Codex-v0.159.2-LICENSE.txt'), 'utf8'), /TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION/);
});

test('changed or incomplete upstream notices fail before any successful inventory is written', async t => {
  const options = await fixture(t);
  const notice = join(options.repository, 'licenses', 'agent-runtime', 'Node-v24.14.0-LICENSE.txt');
  await writeFile(notice, 'Short MIT declaration is not the complete Node license.');
  await assert.rejects(prepareRuntimeNotices(options), /notice checksum mismatch/);
  await assert.rejects(access(options.output));
});

test('runtime updates and source-record path substitution require a fresh review', async t => {
  const options = await fixture(t);
  await assert.rejects(prepareRuntimeNotices({ ...options, versions: { node: '24.15.0', codex: runtimeVersions.codex } }), /Unreviewed Agent runtime notice versions/);
  const path = join(options.repository, 'licenses', 'agent-runtime', 'sources.json');
  const sources = JSON.parse(await readFile(path, 'utf8'));
  sources.records[0].file = '../private.txt';
  await writeFile(path, JSON.stringify(sources));
  await assert.rejects(prepareRuntimeNotices(options), /Invalid Agent runtime notice source record/);
  await assert.rejects(access(options.output));
});

test('a newly bundled dependency without license terms or an explicit reviewed fallback stops preparation', async t => {
  const options = await fixture(t), packagePath = 'node_modules/new-dependency';
  await mkdir(join(options.repository, packagePath), { recursive: true });
  const metadata = { name: 'new-dependency', version: '1.0.0', license: 'MIT' };
  await writeFile(join(options.repository, packagePath, 'package.json'), JSON.stringify(metadata));
  await writeFile(join(options.repository, 'package-lock.json'), JSON.stringify({ packages: { '': { license: 'GPL-3.0-only' }, [packagePath]: metadata } }));
  await assert.rejects(prepareRuntimeNotices({ ...options, metafile: { inputs: { [`${packagePath}/index.js`]: {} } } }), /no reviewed license notice/);
  await assert.rejects(access(options.output));
});

test('installed package version drift and cross-checkout bundle paths are rejected', async t => {
  const options = await fixture(t), packagePath = 'node_modules/drifted';
  await mkdir(join(options.repository, packagePath), { recursive: true });
  await writeFile(join(options.repository, packagePath, 'package.json'), JSON.stringify({ name: 'drifted', version: '2.0.0', license: 'MIT' }));
  await writeFile(join(options.repository, 'package-lock.json'), JSON.stringify({ packages: { '': { license: 'GPL-3.0-only' }, [packagePath]: { version: '1.0.0', license: 'MIT' } } }));
  await assert.rejects(prepareRuntimeNotices({ ...options, metafile: { inputs: { [`${packagePath}/index.js`]: {} } } }), /changed Agent bundle dependency/);
  await assert.rejects(prepareRuntimeNotices({ ...options, metafile: { inputs: { '../other-checkout/node_modules/borrowed/index.js': {} } } }), /outside the owned root package/);
  await assert.rejects(access(options.output));
});
