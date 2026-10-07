// Two fixed, independent diagnostic requests through the same pinned adapter.
// This is not an Agent loop: no execute callbacks, GeoD tools, session or files.
import { randomUUID } from 'node:crypto';
import { generateText, tool, jsonSchema } from 'ai';
import { modelAdapter } from './providers.mjs';
import { validateModelConfig, safeError } from './protocol.mjs';

export async function testModelConnection(value, { generate = generateText, timeoutMs = 20_000 } = {}) {
  const config = validateModelConfig(value);
  const started = Date.now(), checkedAt = new Date().toISOString();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const adapter = modelAdapter(config);
  let text = false, functionCalls = false;
  const result = (status, message) => ({version:1,status,text,functionCalls,latencyMs:Date.now()-started,checkedAt,...(message ? {message} : {})});
  try {
    const options = {...adapter,maxRetries:0,maxOutputTokens:512,abortSignal:controller.signal};
    const answer = await generate({...options,prompt:'This is a connection test. Reply with only: GeoD connection ready.'});
    text = typeof answer.text === 'string' && Boolean(answer.text.trim());
    if (!text) return result('failed','The model returned an empty response. Please retry.');
    const nonce = randomUUID();
    const probe = await generate({...options,
      prompt:`This is a harmless connection test. Call geod_connection_probe once with nonce ${nonce}. Do not do anything else.`,
      tools:{geod_connection_probe:tool({description:'Echo a diagnostic nonce; no data or workspace access.',inputSchema:jsonSchema({type:'object',properties:{nonce:{type:'string'}},required:['nonce'],additionalProperties:false})})},
      toolChoice:{type:'tool',toolName:'geod_connection_probe'},
    });
    const calls = probe.toolCalls;
    functionCalls = Array.isArray(calls) && calls.length===1 && !calls[0].invalid
      && calls[0].toolName==='geod_connection_probe' && calls[0].input?.nonce===nonce
      && Object.keys(calls[0].input).length===1;
    return functionCalls ? result('passed') : result('failed','The model responded, but the test tool call was not valid.');
  } catch (error) {
    return result('failed',controller.signal.aborted ? 'Connection test timed out. Check the endpoint and try again.' : safeError(error,[config.apiKey]));
  } finally { clearTimeout(timer); }
}
