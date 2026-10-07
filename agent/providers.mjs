import { createOpenAICompatible } from '@ai-sdk/openai-compatible';
import { createOpenAI } from '@ai-sdk/openai';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createGoogleGenerativeAI } from '@ai-sdk/google';

export const PROTOCOLS = Object.freeze(['openai-compatible', 'openai-responses', 'anthropic-messages', 'google-generative-ai']);
export const PROVIDERS = Object.freeze(['openai', 'deepseek', 'anthropic', 'google', 'custom']);
export const validProviderProtocol = (provider, protocol) => PROTOCOLS.includes(protocol)
  && (provider === 'custom' || provider === 'openai' && ['openai-compatible','openai-responses'].includes(protocol)
    || ({deepseek:'openai-compatible', anthropic:'anthropic-messages', google:'google-generative-ai'})[provider] === protocol);
// Gemini puts the model ID in the request path. Accept one model name, never a
// caller-supplied relative path that could escape the configured endpoint.
export const validModelId = (protocol, model) => typeof model === 'string'
  && (protocol === 'google-generative-ai' ? /^[A-Za-z0-9][A-Za-z0-9_.+-]{0,159}$/ : /^[A-Za-z0-9][A-Za-z0-9_./:@+-]{0,159}$/).test(model);

export function modelAdapter(config) {
  const settings = {baseURL:config.baseUrl, apiKey:config.apiKey};
  if (config.protocol === 'openai-responses') return {
    model:createOpenAI(settings).responses(config.model),
    providerOptions:{openai:{store:false,include:['reasoning.encrypted_content'],parallelToolCalls:false,strictJsonSchema:false}},
  };
  if (config.protocol === 'anthropic-messages') return {
    model:createAnthropic(settings).languageModel(config.model),
    providerOptions:{anthropic:{disableParallelToolUse:true}},
  };
  if (config.protocol === 'google-generative-ai') return {
    model:createGoogleGenerativeAI({...settings,fetch:async(input,init)=>{
      // Inline files grow when base64-encoded. Bound the complete serialized
      // request (text, tools and files), not just the original attachment sizes.
      if(typeof init.body!=='string' || Buffer.byteLength(init.body)>20_000_000)throw Error('Google inline request exceeds 20 MB. Start a new conversation with smaller attachments.');
      // The pinned SDK inserts this documented sentinel when a Gemini 3 tool
      // signature is missing. Our bridge must fail rather than bypass vendor
      // validation; unsigned legacy models do not require this sentinel.
      const body=JSON.parse(init.body);
      if(body.contents?.some(message=>message.role==='model' && message.parts?.some(part=>part.functionCall && part.thoughtSignature==='skip_thought_signature_validator'))) {
        throw new Error('Native provider history is missing its saved signature.');
      }
      return fetch(input,init);
    }}).languageModel(config.model), providerOptions:{},
  };
  if (config.protocol === 'openai-compatible') return {
    model:createOpenAICompatible({...settings, name:'geodModel', includeUsage:true}).chatModel(config.model),
    // These current DeepSeek models default to high thinking effort. Use the
    // documented low setting for interactive task planning, retaining thinking
    // and its full tool history without changing the chosen model or endpoint.
    providerOptions:{geodModel:{parallel_tool_calls:false,
      ...(/^deepseek(?:-v4)?-(?:flash|pro)$/i.test(config.model)?{reasoningEffort:'low'}:{})}},
  };
  throw new Error('Invalid Agent model protocol.');
}
