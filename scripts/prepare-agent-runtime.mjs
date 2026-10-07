import { build } from 'esbuild';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, copyFile, readdir, stat } from 'node:fs/promises';
import { join, resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const root = fileURLToPath(new URL('../', import.meta.url));
export const runtimeVersions = Object.freeze({ node: '24.14.0', codex: '0.159.2' });
const { node: nodeVersion, codex: codexVersion } = runtimeVersions;
const nodeHash = '63c259c81e5d472b5f11c8d506070130cb04a1ecf84b80377a34ed6ec9048088';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

async function boundedNotice(path) {
  const info = await stat(path);
  if (!info.isFile() || info.size < 20 || info.size > 2_000_000) throw new Error('Agent runtime notice is missing or outside its size limit.');
  const bytes = await readFile(path);
  if (bytes.length !== info.size) throw new Error('Agent runtime notice changed while reading.');
  return bytes;
}

// Notice preparation is offline and independent of model/user configuration.
// The bundle metafile, rather than the whole development dependency tree,
// determines which npm package attributions belong to agent.mjs.
export async function prepareRuntimeNotices({ repository = root, output, metafile = { inputs: {} }, versions = runtimeVersions }) {
  const sourceDirectory = join(repository, 'licenses', 'agent-runtime');
  const reviewed = JSON.parse(await readFile(join(sourceDirectory, 'sources.json'), 'utf8'));
  if (reviewed.schema !== 'geod-agent-runtime-notices/v1'
      || versions.node !== runtimeVersions.node || versions.codex !== runtimeVersions.codex
      || reviewed.runtimeVersions?.node !== versions.node || reviewed.runtimeVersions?.codex !== versions.codex) {
    throw new Error('Unreviewed Agent runtime notice versions.');
  }
  const sourceRecords = new Map(), pending = [], entries = [];
  const add = (filename, bytes, metadata) => {
    if (!/^[A-Za-z0-9@_.-]+$/.test(filename) || filename.includes('..') || pending.some(file => file.filename === filename)) throw new Error('Invalid or duplicate Agent runtime notice filename.');
    pending.push({ filename, bytes });
    entries.push({ ...metadata, file: `licenses/${filename}`, sha256: digest(bytes), bytes: bytes.length });
  };
  if (!Array.isArray(reviewed.records) || reviewed.records.length !== 6) throw new Error('Agent runtime notice inventory is incomplete.');
  for (const record of reviewed.records) {
    if (!/^[A-Za-z0-9.-]+\.txt$/.test(record.file ?? '') || record.file.includes('..')
        || !/^[a-f0-9]{64}$/.test(record.sha256 ?? '') || !Number.isInteger(record.bytes)
        || !/^https:\/\/raw\.githubusercontent\.com\/(nodejs\/node|openai\/codex|ratatui\/ratatui|vercel\/ai)\//.test(record.source ?? '')) {
      throw new Error('Invalid Agent runtime notice source record.');
    }
    const bytes = await boundedNotice(join(sourceDirectory, record.file));
    if (bytes.length !== record.bytes || digest(bytes) !== record.sha256) throw new Error(`Agent runtime notice checksum mismatch: ${record.file}`);
    sourceRecords.set(record.file, { record, bytes });
    add(record.file, bytes, { component: record.component, version: record.version, license: record.license, kind: record.kind, source: record.source });
  }
  const project = JSON.parse(await readFile(join(repository, 'package.json'), 'utf8'));
  const lock = JSON.parse(await readFile(join(repository, 'package-lock.json'), 'utf8'));
  const firstParty = await boundedNotice(join(repository, 'LICENSE'));
  if (project.license !== 'GPL-3.0-only' || lock.packages?.['']?.license !== project.license
      || !firstParty.toString('utf8').includes('GNU GENERAL PUBLIC LICENSE')) throw new Error('Agent first-party license metadata or terms are missing.');
  add('GeoD-Global-LICENSE.txt', firstParty, { component: 'GeoD Global Agent bundle', version: project.version, license: project.license, kind: 'complete first-party license' });
  const packagePaths = new Set();
  for (const input of Object.keys(metafile.inputs ?? {})) {
    const path = relative(repository, resolve(repository, input)).split(sep).join('/');
    const match = path.match(/^(node_modules\/(?:@[^/]+\/)?[^/]+)\//);
    if (match) packagePaths.add(match[1]);
    else if (path.includes('node_modules/')) throw new Error('Agent bundle dependency is outside the owned root package.');
  }
  const npmPackages = [];
  for (const packagePath of [...packagePaths].sort()) {
    const directory = join(repository, packagePath), metadata = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
    const locked = lock.packages?.[packagePath];
    if (!locked || locked.version !== metadata.version || locked.license !== metadata.license
        || !/^(@[A-Za-z0-9_.-]+\/)?[A-Za-z0-9_.-]+$/.test(metadata.name ?? '')
        || !/^[A-Za-z0-9_.+-]+$/.test(metadata.version ?? '') || !['MIT', 'Apache-2.0'].includes(metadata.license)) {
      throw new Error('Unreviewed or changed Agent bundle dependency.');
    }
    const notices = [];
    for (const file of (await readdir(directory)).sort()) {
      if (!/^(?:licen[cs]e|notice|copying)(?:[.-].*)?$/i.test(file) || !(await stat(join(directory, file))).isFile()) continue;
      const filename = `${metadata.name.replace('@', '').replace('/', '--')}--${metadata.version}--${file}`;
      add(filename, await boundedNotice(join(directory, file)), { component: metadata.name, version: metadata.version, license: metadata.license, kind: 'unchanged published package notice', source: `npm:${metadata.name}@${metadata.version}` });
      notices.push(`licenses/${filename}`);
    }
    if (!notices.length && metadata.name === '@ai-sdk/provider-utils' && metadata.version === '5.0.53' && metadata.license === 'Apache-2.0') {
      if (!sourceRecords.has('AI-provider-utils-v5.0.53-LICENSE.txt')) throw new Error('Reviewed AI SDK attribution is missing.');
      notices.push('licenses/AI-provider-utils-v5.0.53-LICENSE.txt');
    }
    if (!notices.length) throw new Error(`Agent bundle dependency has no reviewed license notice: ${metadata.name}`);
    npmPackages.push({ name: metadata.name, version: metadata.version, license: metadata.license, notices,
      ...(metadata.license === 'Apache-2.0' ? { completeTerms: 'licenses/Codex-v0.159.2-LICENSE.txt' } : {}) });
  }
  const inventory = { schema: reviewed.schema, runtimeVersions: { node: versions.node, codex: versions.codex },
    codexCommit: reviewed.codexCommit, entries, npmPackages,
    scope: 'Unchanged upstream runtime notices and actual JavaScript bundle attributions. Complete transitive native-binary audit, corresponding sources and installed release acceptance remain separate.' };
  const notice = ['GeoD Global owned Agent development runtime', '',
    `Node.js ${versions.node}: full upstream combined license, including bundled dependency notices.`,
    `OpenAI Codex ${versions.codex}: complete Apache-2.0 LICENSE and unchanged upstream NOTICE.`,
    'Ratatui and vendored WezTerm MIT terms are included.',
    'agent.mjs: GeoD Global GPL-3.0-only; actual npm dependencies and attributions are listed in licenses/inventory.json.',
    'Complete Apache-2.0 terms are in licenses/Codex-v0.159.2-LICENSE.txt.',
    'Shell/code-mode helper and voice binaries are not included.', '', inventory.scope, ''].join('\n');
  // Verify every input before writing any notice. A missing attribution must not
  // leave a new manifest claiming preparation succeeded.
  await mkdir(join(output, 'licenses'), { recursive: true });
  for (const file of pending) await writeFile(join(output, 'licenses', file.filename), file.bytes);
  await writeFile(join(output, 'licenses', 'inventory.json'), JSON.stringify(inventory, null, 2) + '\n');
  await writeFile(join(output, 'RUNTIME-NOTICES.txt'), notice);
  return inventory;
}

export async function prepareRuntime() {
  if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('Agent desktop runtime preparation currently supports Windows x64.');
  const output = join(root, '.agent-runtime', 'win32-x64');
  await mkdir(output, { recursive: true });
  const nodeFile = join(output, 'node.exe');
  let cached = false;
  try { cached = digest(await readFile(nodeFile)) === nodeHash; } catch { /* First preparation. */ }
  if (!cached) {
    const args = ['--fail', '--silent', '--show-error', '--max-time', '180', '--output', nodeFile, `https://nodejs.org/dist/v${nodeVersion}/win-x64/node.exe`];
    if (process.env.HTTPS_PROXY) args.unshift('--proxy', process.env.HTTPS_PROXY);
    const download = spawnSync('curl.exe', args, { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
    if (download.status !== 0) throw new Error('Pinned Node runtime download failed.');
  }
  if (digest(await readFile(nodeFile)) !== nodeHash) throw new Error('Node runtime checksum does not match the official pinned release.');
  const require = createRequire(join(root, 'package.json'));
  const codexPackage = require.resolve('@openai/codex-win32-x64/package.json');
  const packageMetadata = JSON.parse(await readFile(codexPackage, 'utf8'));
  if (packageMetadata.version !== `${codexVersion}-win32-x64` || packageMetadata.license !== 'Apache-2.0') throw new Error('Incorrect pinned Codex package.');
  const vendor = resolve(codexPackage, '..', 'vendor', 'x86_64-pc-windows-msvc');
  // Shell, voice and code-mode hosts are deliberately not included in this stage.
  const codexSource = join(vendor, 'bin', 'codex.exe'), codexTarget = join(output, 'codex.exe');
  let currentCodex = false;
  try { currentCodex = digest(await readFile(codexTarget)) === digest(await readFile(codexSource)); } catch { /* First preparation. */ }
  // A running development conversation holds this executable open on Windows.
  // Reuse only byte-identical pinned binaries while refreshing the JS adapter.
  if (!currentCodex) await copyFile(codexSource, codexTarget);
  const bundled = await build({ absWorkingDir: root, entryPoints: ['agent/stdio.mjs'], bundle: true, minify: true, keepNames: true, platform: 'node', format: 'esm', target: 'node24', outfile: join(output, 'agent.mjs'),
    banner: { js: 'import { createRequire as __createRequire } from "node:module"; const require = __createRequire(import.meta.url);' }, legalComments: 'linked', sourcemap: false, metafile: true });
  const licenses = await prepareRuntimeNotices({ output, metafile: bundled.metafile });
  const files = {};
  const names = ['node.exe', 'codex.exe', 'agent.mjs', 'RUNTIME-NOTICES.txt', 'licenses/inventory.json', ...licenses.entries.map(entry => entry.file)];
  // esbuild emits this only when retained upstream comments exist.
  try { await stat(join(output, 'agent.mjs.LEGAL.txt')); names.push('agent.mjs.LEGAL.txt'); } catch { /* No legal-comment output in this bundle. */ }
  for (const name of names.sort()) {
    const bytes = await readFile(join(output, name)); files[name] = { sha256: digest(bytes), bytes: bytes.length };
  }
  const manifest = { version: 1, platform: 'win32-x64', nodeVersion, codexVersion, aiSdkVersion: '7.0.127', adapterVersion: '3.0.62', files,
    adapters: { 'openai-compatible':'3.0.62', 'openai-responses':'4.0.83', 'anthropic-messages':'4.0.71', 'google-generative-ai':'4.0.87' },
    sources: { node: `https://nodejs.org/dist/v${nodeVersion}/SHASUMS256.txt`, codex: '@openai/codex@0.159.2-win32-x64 (npm lockfile integrity)' },
    notices: { inventory: 'licenses/inventory.json', npmPackages: licenses.npmPackages.length },
    scope: 'Development Agent runtime with verified upstream notices. Complete native dependency/source audit and installed distribution require separate acceptance.' };
  await writeFile(join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  console.log(JSON.stringify({ prepared: true, nodeVersion, codexVersion, aiSdkVersion: manifest.aiSdkVersion, notices: manifest.notices, files: Object.keys(files), runtime: '.agent-runtime/win32-x64' }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await prepareRuntime();
