export const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
export const KEY = /^[a-z][a-z0-9_]{0,31}$/;
export const HASH = /^[a-f0-9]{64}$/;
export const KINDS = ['raster', 'rgb', 'stac', 'wcs', 'vector', 'project', 'metadata', 'unavailable'];
const STATES = ['active', 'waiting_input', 'waiting_confirmation', 'waiting_jobs', 'needs_attention', 'paused', 'complete', 'budget_limited'];
const ENGINE_STATES = ['active', 'paused', 'blocked', 'usageLimited', 'budgetLimited', 'complete'];
export const object = (value, keys) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).every(key => keys.includes(key));
export const text = (value, max) => typeof value === 'string' && value.trim().length > 0 && value.length <= max && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(value);
const date = value => typeof value === 'string' && value.length <= 64 && Number.isFinite(Date.parse(value));
export const schema = (properties, required) => ({ type: 'object', properties, required, additionalProperties: false });
export function validGoalInput(value) {
  return object(value, ['objective', 'outputs']) && text(value.objective, 1200) && Array.isArray(value.outputs) && value.outputs.length > 0 && value.outputs.length <= 16
    && new Set(value.outputs.map(output => output?.id)).size === value.outputs.length
    && value.outputs.every(output => object(output, ['id', 'label', 'kind']) && KEY.test(output.id) && text(output.label, 240) && KINDS.includes(output.kind));
}
export function validEngineGoal(value, threadId) {
  return object(value, ['threadId', 'objective', 'status', 'tokenBudget', 'tokensUsed', 'timeUsedSeconds', 'createdAt', 'updatedAt'])
    && UUID.test(value.threadId) && (!threadId || value.threadId === threadId) && text(value.objective, 4000) && ENGINE_STATES.includes(value.status)
    && (value.tokenBudget == null || Number.isSafeInteger(value.tokenBudget) && value.tokenBudget > 0)
    && ['tokensUsed', 'timeUsedSeconds', 'createdAt', 'updatedAt'].every(key => Number.isSafeInteger(value[key]) && value[key] >= 0);
}
export function validGoal(value) {
  return object(value, ['version', 'id', 'objective', 'requestText', 'outputs', 'planIds', 'status', 'engine', 'continuations', 'updatedAt', 'checkedAt', 'reason']) && value.version === 1 && UUID.test(value.id)
    && text(value.requestText, 8000) && validGoalInput({ objective: value.objective, outputs: value.outputs?.map(({ id, label, kind }) => ({ id, label, kind })) })
    && STATES.includes(value.status) && (value.engine === null || validEngineGoal(value.engine)) && Number.isSafeInteger(value.continuations) && value.continuations >= 0 && value.continuations <= 8
    && Array.isArray(value.planIds) && value.planIds.length<=32 && value.planIds.every(id=>UUID.test(id)) && new Set(value.planIds).size===value.planIds.length
    && date(value.updatedAt) && (value.checkedAt === null || date(value.checkedAt)) && (value.reason === null || text(value.reason, 240))
    && value.outputs.every(output => object(output, ['id', 'label', 'kind', 'planId', 'entryId', 'state', 'verifiedIds'])
      && ['missing', 'waiting', 'failed', 'verified', 'unavailable'].includes(output.state)
      && (output.planId == null || UUID.test(output.planId)) && (output.entryId == null || UUID.test(output.entryId)) && !(output.planId && output.entryId)
      && Array.isArray(output.verifiedIds) && output.verifiedIds.length <= 32 && output.verifiedIds.every(id => UUID.test(id))
      && (output.state!=='verified' || (output.kind==='metadata'?UUID.test(output.entryId)&&output.verifiedIds.length===0:UUID.test(output.planId)&&output.verifiedIds.length>0))
      && (output.kind!=='unavailable'||output.state==='unavailable'&&!output.planId&&!output.entryId&&output.verifiedIds.length===0))
    && new Set(value.outputs.map(output=>output.planId??output.entryId).filter(Boolean)).size===value.outputs.filter(output=>output.planId||output.entryId).length
    && (value.status !== 'complete' || value.checkedAt !== null && value.outputs.every(output => output.state === 'verified'));
}
export function publicGoal(goal) { return goal ? structuredClone(goal) : null; }
export function goalPrompt(goal){return JSON.stringify({objective:goal.objective,outputs:goal.outputs,status:goal.status,reason:goal.reason});}
