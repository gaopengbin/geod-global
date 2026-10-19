// Shared by the connection form and runtime. Only a documented model on its
// official endpoint gets a capacity preset; gateway aliases remain unknown.
// Checked 2026-10-09: https://api-docs.deepseek.com/api/list-models/
export const MIN_CONTEXT_WINDOW = 8_192;
export const MAX_CONTEXT_WINDOW = 4_194_304;
export const FALLBACK_CONTEXT_WINDOW = 32_768;
export const validContextWindow = value => value == null
  || Number.isSafeInteger(value) && value >= MIN_CONTEXT_WINDOW && value <= MAX_CONTEXT_WINDOW;

export function defaultContextWindow(config = {}) {
  let endpoint;
  try { endpoint = new URL(config.baseUrl); } catch { /* Unsaved or unknown endpoint. */ }
  const known = endpoint?.protocol === 'https:' && endpoint.hostname === 'api.deepseek.com'
    && !endpoint.port && !endpoint.username && !endpoint.password && !endpoint.search && !endpoint.hash
    && ['/', '/v1', '/v1/'].includes(endpoint.pathname)
    && ['openai-compatible', 'openai-responses'].includes(config.protocol)
    && ['deepseek-flash', 'deepseek-v4-flash', 'deepseek-v4-flash-vision-exp', 'deepseek-v4-pro'].includes(config.model);
  return { contextWindow: known ? 1_048_576 : FALLBACK_CONTEXT_WINDOW, known };
}

export function modelContextPolicy(config = {}) {
  if (!validContextWindow(config.contextWindow)) throw new Error('Invalid Agent context window size.');
  const contextWindow = config.contextWindow ?? defaultContextWindow(config).contextWindow;
  // Thinking and visible output share this budget on DeepSeek. Smaller custom
  // windows also need enough room for inputs and the tool definitions.
  const preferredOutput = config.protocol === 'openai-compatible' && /^deepseek(?:[-/]|$)/i.test(config.model) ? 16_384 : 4_096;
  const outputTokens = Math.min(preferredOutput, Math.floor(contextWindow / 4));
  const autoCompactTokenLimit = Math.min(Math.floor(contextWindow * .85), contextWindow - outputTokens - 1_024);
  return { contextWindow, autoCompactTokenLimit, outputTokens };
}
