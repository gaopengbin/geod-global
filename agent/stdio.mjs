import { AgentService } from './service.mjs';
import { testModelConnection } from './connection-test.mjs';

const [home, executable] = process.argv.slice(2);
if (!home || !executable) throw new Error('Agent runtime arguments are missing.');
let sequence = 0, buffer = '', closing = false;
const pending = new Map();
const send = value => process.stdout.write(`${JSON.stringify(value)}\n`);
let changeTimer, latestRevision;
const service = await new AgentService({ home, executable, onChange(revision) {
  latestRevision = revision;
  if (closing || changeTimer) return;
  changeTimer = setTimeout(() => {
    changeTimer = undefined;
    if (!closing) send({ method:'geod.changed', params:{revision:latestRevision} });
  }, 32);
  changeTimer.unref();
}, callTool(name, args, scope) {
  return new Promise((resolve, reject) => {
    const id = `tool-${++sequence}`;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error('GeoD tool timed out.')); }, 60_000);
    pending.set(id, { resolve, reject, timer }); send({ id, method: 'geod.tool', params: { name, arguments: args, ...scope } });
  });
} }).open();

async function message(value) {
  if (typeof value.id === 'string' && !value.method) {
    const entry = pending.get(value.id); if (!entry) return;
    pending.delete(value.id); clearTimeout(entry.timer);
    value.error ? entry.reject(new Error(typeof value.error.message === 'string' ? value.error.message.slice(0,240) : 'GeoD tool failed.')) : entry.resolve(value.result); return;
  }
  if (!Number.isSafeInteger(value.id) || !value.method || closing) return;
  try {
    let result;
    switch (value.method) {
      case 'configure': result = await service.configure(value.params.config, value.params.definitions); break;
      case 'testModel':
        if (service.snapshot().busy) throw new Error('Stop the Agent response before testing a connection.');
        result = await testModelConnection(value.params.config); break;
      case 'snapshot': result = service.snapshot(); break;
      case 'recordControl': result = await service.recordControl(value.params); break;
      case 'acknowledgeView':result=await service.acknowledgeView(value.params);break;
      case 'select': result = await service.select(value.params.id); break;
      case 'imageReferences': result = await service.imageReferences(); break;
      case 'attachmentReferences': result = await service.attachmentReferences(); break;
      case 'recordPlanRevision': result = await service.recordPlanRevision(value.params); break;
      case 'send': result = await service.send(value.params); break;
      case 'compact': result = await service.compact(value.params); break;
      case 'goalControl': result = await service.goalControl(value.params); break;
      case 'interrupt': result = await service.interrupt(); break;
      default: throw new Error('Unknown Agent operation.');
    }
    send({ id: value.id, result });
  } catch (error) { send({ id: value.id, error: { message: error.message } }); }
}
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {
  buffer += chunk;
  if (Buffer.byteLength(buffer) > 1_000_000) { close().then(() => process.exit(1)); return; }
  let index;
  while ((index = buffer.indexOf('\n')) !== -1) {
    const line = buffer.slice(0, index); buffer = buffer.slice(index + 1);
    try { void message(JSON.parse(line)); } catch { close().then(() => process.exit(1)); return; }
  }
});
async function close() {
  if (closing) return; closing = true;
  clearTimeout(changeTimer);
  for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(new Error('Agent stopped.')); } pending.clear();
  await service.close();
}
process.stdin.once('end', () => close().then(() => process.exit(0), () => process.exit(1)));
process.once('SIGTERM', () => close().then(() => process.exit(0), () => process.exit(1)));
