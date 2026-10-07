// Actual owned Codex process + controlled provider stream. This tests stop/exit
// without a paid model request; it is not a model or raster acceptance claim.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { AgentService } from '../agent/service.mjs';
import { startBridge } from '../agent/protocol.mjs';

const root = path.join(process.cwd(), '.verification', `agent-stop-${Date.now()}`);
await mkdir(root);
const runtime = path.join(process.cwd(), '.agent-runtime/win32-x64');
const stream = options => ({ fullStream: (async function* () {
  yield { type: 'text-delta', text: 'Controlled provider interruption probe.' };
  await new Promise(resolve => { if (options.abortSignal.aborted) resolve(); else options.abortSignal.addEventListener('abort', resolve, { once: true }); });
  throw new DOMException('Controlled stream stopped.', 'AbortError');
})(), finishReason: Promise.resolve('length'), usage: Promise.resolve({}) });
const service = await new AgentService({ home: root, executable: path.join(runtime, 'codex.exe'),
  callTool() { throw Error('No business tool runs in this stop check.'); }, bridgeFactory: options => startBridge({ ...options, stream }) }).open();
async function until(predicate) {
  const deadline = Date.now() + 15_000;
  while (!predicate()) { if (Date.now() > deadline) throw Error('Native stop verification timed out.'); await new Promise(resolve => setTimeout(resolve, 25)); }
}
const report = { schema: 'geod-agent-stop-acceptance/v1', codex: 'actual owned process', provider: 'controlled abortable stream', modelCalls: 0, businessToolsRun: 0, usedUserDesktop: false, status: 'pending' };
try {
  await service.configure({ label: 'Controlled interruption test', protocol: 'openai-compatible', baseUrl: 'https://example.com/v1', model: 'test-model', apiKey: 'not-a-real-key' },
    [{ name: 'geod_health', description: 'Read health', inputSchema: { type: 'object', properties: {}, additionalProperties: false } }]);
  await service.send({ text: 'Controlled interruption acceptance.' });
  await until(() => service.snapshot().selected?.entries.some(entry => entry.type === 'assistant' && entry.text));
  const text = service.snapshot().selected.entries.filter(entry => entry.type === 'assistant').map(entry => entry.text).join('');
  await service.interrupt(); await until(() => !service.snapshot().busy);
  assert.equal(service.snapshot().selected.status, 'interrupted');
  assert.equal(service.snapshot().selected.entries.filter(entry => entry.type === 'assistant').map(entry => entry.text).join(''), text);
  const config = await readFile(path.join(root, 'codex/config.toml'), 'utf8');
  for (const feature of ['plugins', 'apps', 'remote_plugin', 'multi_agent', 'shell_tool', 'code_mode_host']) assert(config.includes(`${feature} = false`));
  report.interruptedTurn = true; report.frozenAnswer = true; report.disabledExtraTools = true;
  await service.send({ sessionId: service.snapshot().selected.id, text: 'Controlled application exit acceptance.' });
  await until(() => service.snapshot().selected.entries.filter(entry => entry.type === 'assistant').length === 2);
  await service.close(); assert.equal(service.snapshot().selected.status, 'interrupted'); assert.equal(service.host, null);
  report.closedActiveTurn = true; report.status = 'passed';
} catch (error) { report.status = 'failed'; report.failure = error.message; throw error; }
finally { await service.close(); await writeFile(path.join(root, 'verification.json'), JSON.stringify(report, null, 2)); console.log(JSON.stringify({ status: report.status, output: root, modelCalls: 0, usedUserDesktop: false })); }
