// Two fixed, independent diagnostic requests through the same pinned adapter.
// This is not an Agent loop: no execute callbacks, GeoD tools, session or files.
import { randomUUID } from 'node:crypto';
import { generateText, tool, jsonSchema } from 'ai';
import { modelAdapter } from './providers.mjs';
import { validateModelConfig, modelOutputBudget, safeError } from './protocol.mjs';

export async function testModelConnection(value, { generate = generateText, timeoutMs = 75_000 } = {}) {
  const config = validateModelConfig(value);
  const started = Date.now(), checkedAt = new Date().toISOString();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const adapter = modelAdapter(config);
  let text = false, functionCalls = false;
  const result = (status, message) => ({version:1,status,text,functionCalls,latencyMs:Date.now()-started,checkedAt,...(message ? {message} : {})});
  try {
    // Match conversation output capacity: thinking tokens share this budget.
    const options = {...adapter,maxRetries:0,maxOutputTokens:modelOutputBudget(config),abortSignal:controller.signal};
    const answer = await generate({...options,prompt:'This is a connection test. Reply with only: GeoD connection ready.'});
    if (answer.finishReason === 'length') return result('failed','The model reached its output limit during the connection test.');
    text = typeof answer.text === 'string' && Boolean(answer.text.trim());
    if (!text) return result('failed','The model returned an empty response. Please retry.');
    const nonce = randomUUID();
    const probe = await generate({...options,
      prompt:`This is a harmless connection test. Call geod_connection_probe once with nonce ${nonce}. Do not do anything else.`,
      tools:{geod_connection_probe:tool({description:'Echo a diagnostic nonce; no data or workspace access.',inputSchema:jsonSchema({type:'object',properties:{nonce:{type:'string'}},required:['nonce'],additionalProperties:false})})},
      // Agent conversations use automatic selection. DeepSeek thinking mode
      // rejects forced named tools with HTTP 400; still require a valid call.
      toolChoice:'auto',
    });
    if (probe.finishReason === 'length') return result('failed','The model reached its output limit during the connection test.');
    const calls = probe.toolCalls;
    functionCalls = Array.isArray(calls) && calls.length===1 && !calls[0].invalid
      && calls[0].toolName==='geod_connection_probe' && calls[0].input?.nonce===nonce
      && Object.keys(calls[0].input).length===1;
    return functionCalls ? result('passed') : result('failed','The model responded, but the test tool call was not valid.');
  } catch (error) {
    const status = error?.statusCode ?? error?.status;
    return result('failed',controller.signal.aborted ? 'Connection test timed out. Check the endpoint and try again.'
      : status === 400 ? text ? 'Text response works, but the model rejected the tool-call test (HTTP 400).'
        : 'The model rejected the test request (HTTP 400). Check protocol and tool-call compatibility.'
      : safeError(error,[config.apiKey]));
  } finally { clearTimeout(timer); }
}
