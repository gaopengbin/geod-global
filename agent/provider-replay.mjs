// Protocol state only: closed vendor signatures keyed by output identity and a
// content digest. No prompts, tool results, API keys, or second execution loop.
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const MAX_BYTES = 8_000_000, MAX_ENTRIES = 4096;
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key,stable(value[key])]));
  return value;
}
function binding(item) {
  if (item.type === 'function_call') return hash([item.type,item.call_id,item.name,stable(JSON.parse(item.arguments || '{}'))]);
  return hash([item.type,item.role ?? null,(item.content ?? item.summary ?? []).map(part => part.text).join('\n'),
    ...(item.type==='reasoning' && item.encrypted_content!=null ? [item.encrypted_content] : [])]);
}
function checkedOptions(protocol, metadata) {
  if (!metadata) return {};
  if(protocol==='openai-responses') {
    const options=metadata.openai;
    if(Object.keys(metadata).some(key=>key!=='openai') || !options || typeof options!=='object' || Array.isArray(options)
      || Object.keys(options).some(key=>!['itemId','reasoningEncryptedContent','phase'].includes(key))
      || options.itemId!=null && (typeof options.itemId!=='string' || !/^[A-Za-z0-9_-]{1,256}$/.test(options.itemId))
      || options.reasoningEncryptedContent!=null && (typeof options.reasoningEncryptedContent!=='string' || !options.reasoningEncryptedContent || options.reasoningEncryptedContent.length>131072)
      || options.phase!=null && !['commentary','final_answer'].includes(options.phase))throw new Error('Unsupported native provider metadata.');
    return {openai:Object.fromEntries(Object.entries(options).filter(([,value])=>value!=null))};
  }
  const name = protocol === 'anthropic-messages' ? 'anthropic' : 'google';
  const allowed = name === 'anthropic' ? ['signature','redactedData'] : ['thoughtSignature'];
  const options = metadata[name];
  // Do not replay provider-side tools, compaction, citations or arbitrary
  // options as ordinary text. These require a separate reviewed contract.
  if (Object.keys(metadata).some(key => key !== name) || !options || typeof options !== 'object'
    || Object.keys(options).some(key => !allowed.includes(key))
    || Object.values(options).some(value => typeof value !== 'string' || !value || value.length > 131072)
    || options.signature && options.redactedData) throw new Error('Unsupported native provider metadata.');
  return {[name]:{...options}};
}
export class ProviderReplay {
  constructor(protocol, file = null, secret = '') { this.protocol=protocol; this.file=file; this.secret=secret; this.entries=new Map(); this.queue=Promise.resolve(); }
  static async open({config, home, identity}) {
    if (config.protocol === 'openai-compatible') return null;
    const file=home ? join(home,'provider-replay',`${hash([identity ?? null,config.protocol,config.baseUrl,config.model])}.json`) : null;
    const replay=new ProviderReplay(config.protocol,file,config.apiKey);
    if (file) {
      try {
        if ((await stat(file)).size > MAX_BYTES) throw new Error('Invalid native provider replay state.');
        const value=JSON.parse(await readFile(file,'utf8'));
        if (value.version !== 1 || value.protocol !== config.protocol || Object.keys(value).some(key => !['version','protocol','entries'].includes(key))
          || !Array.isArray(value.entries) || value.entries.length > MAX_ENTRIES) throw new Error('Invalid native provider replay state.');
        for (const entry of value.entries) {
          if (!Array.isArray(entry) || entry.length !== 2 || !/^[a-f0-9]{64}$/.test(entry[0]) || replay.entries.has(entry[0])
            || entry[1] !== null && (Object.keys(entry[1]).some(key=>!['binding','options'].includes(key)) || !/^[a-f0-9]{64}$/.test(entry[1].binding))) throw new Error('Invalid native provider replay state.');
          if (entry[1]) checkedOptions(config.protocol,Object.keys(entry[1].options).length ? entry[1].options : undefined);
          replay.entries.set(...entry);
        }
      } catch (error) { if (error.code !== 'ENOENT') throw new Error('Native provider history could not be restored. Start a new conversation or restore its protocol state.'); }
    }
    return replay;
  }
  options(metadata) {
    const options=checkedOptions(this.protocol, metadata);
    if(this.secret && JSON.stringify(options).includes(this.secret)) throw new Error('Unsupported native provider metadata.');
    return options;
  }
  remember(item, options) {
    const digest=binding(item), value={binding:digest,options};
    const identity=item.id ?? (item.type==='function_call' ? item.call_id : null);
    const keys=[...(identity ? [hash(['identity',item.type,identity])] : []),hash(['content',digest])];
    if (item.type==='function_call') keys.push(hash(['call',item.call_id]));
    for (const key of keys) {
      const previous=this.entries.get(key);
      // If Codex omits an item ID, repeated identical plaintext with different
      // signatures is ambiguous. Fail on replay rather than inventing one.
      this.entries.set(key, previous === null || previous && JSON.stringify(previous)!==JSON.stringify(value) ? null : value);
    }
    if (this.entries.size > MAX_ENTRIES) throw new Error('Native provider history storage limit reached.');
  }
  restore(item) {
    const digest=binding(item), identity=item.id ?? null;
    const keys=[...(identity ? [hash(['identity',item.type,identity])] : []),
      ...(item.type==='function_call' ? [hash(['call',item.call_id])] : []),hash(['content',digest])];
    for (const key of keys) {
      if (!this.entries.has(key)) continue;
      const entry=this.entries.get(key);
      if (!entry || entry.binding!==digest) throw new Error('Native provider history does not match its saved signature.');
      return structuredClone(entry.options);
    }
    if (item.type==='reasoning' || item.type==='function_call') throw new Error('Native provider history is missing its saved signature.');
    return {};
  }
  async flush() {
    if (!this.file) return;
    const content=JSON.stringify({version:1,protocol:this.protocol,entries:[...this.entries]});
    if (Buffer.byteLength(content)>MAX_BYTES) throw new Error('Native provider history storage limit reached.');
    this.queue=this.queue.then(async()=>{
      await mkdir(join(this.file,'..'),{recursive:true});
      const temporary=`${this.file}.${randomUUID()}.tmp`;
      await writeFile(temporary,content,{mode:0o600}); await rename(temporary,this.file);
    });
    await this.queue;
  }
}
