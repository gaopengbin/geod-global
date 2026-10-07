// Endpoint presets checked against the providers' own documentation on 2026-10-05.
// https://developers.openai.com/api/reference/resources/chat
// https://api-docs.deepseek.com/
// https://ai-sdk.dev/providers/ai-sdk-providers/anthropic
// https://ai-sdk.dev/providers/ai-sdk-providers/google
// These are protocol presets, not model availability or upstream identity claims.
export const MODEL_PROTOCOLS = Object.freeze([
  {id:'openai-compatible',label:'OpenAI-compatible Chat Completions'},
  {id:'openai-responses',label:'OpenAI Responses'},
  {id:'anthropic-messages',label:'Anthropic Messages'},
  {id:'google-generative-ai',label:'Google Generative AI'},
]);
export const MODEL_PROVIDERS = Object.freeze([
  { id:'openai', label:'OpenAI', protocol:'openai-responses', baseUrl:'https://api.openai.com/v1', protocols:['openai-responses','openai-compatible'] },
  { id:'deepseek', label:'DeepSeek', protocol:'openai-compatible', baseUrl:'https://api.deepseek.com' },
  { id:'anthropic', label:'Anthropic', protocol:'anthropic-messages', baseUrl:'https://api.anthropic.com/v1' },
  { id:'google', label:'Google', protocol:'google-generative-ai', baseUrl:'https://generativelanguage.googleapis.com/v1beta' },
  { id:'custom', label:'Custom connection', protocol:'openai-compatible', baseUrl:'' },
]);
export const validProviderProtocol = (provider, protocol) => MODEL_PROTOCOLS.some(value=>value.id===protocol)
  && (provider==='custom' || (MODEL_PROVIDERS.find(value=>value.id===provider)?.protocols ?? [MODEL_PROVIDERS.find(value=>value.id===provider)?.protocol]).includes(protocol));
export const validModelId = (protocol, model) => typeof model==='string'
  && (protocol==='google-generative-ai' ? /^[A-Za-z0-9][A-Za-z0-9_.+-]{0,159}$/ : /^[A-Za-z0-9][A-Za-z0-9_./:@+-]{0,159}$/).test(model);
export const MODEL_CAPABILITIES = Object.freeze({ text:true, functionCalls:true, plaintextReasoning:true, images:true, encryptedReasoning:false, contextCompaction:true });
export const modelCapabilities = protocol => ({...MODEL_CAPABILITIES,encryptedReasoning:protocol==='openai-responses'});
export const providerLabel = id => MODEL_PROVIDERS.find(provider => provider.id === id)?.label ?? MODEL_PROVIDERS.at(-1).label;
export function groupedConnections(connections = []) {
  return MODEL_PROVIDERS.map(provider => ({ ...provider, connections:connections.filter(connection => connection.provider === provider.id) })).filter(provider => provider.connections.length);
}
