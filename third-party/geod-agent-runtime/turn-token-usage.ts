import type { DisplayMessage } from './pending-generations.ts';

export interface GenerationTokenReceipt {
  generationId: string;
  state: string;
  usageKnown?: boolean | null;
  inputTokens?: number | null;
  outputTokens?: number | null;
  cachedInputTokens?: number | null;
  reasoningTokens?: number | null;
  result?: { usage?: { cachedInputTokens?: number | null; reasoningTokens?: number | null } } | null;
}
export interface TurnUsageSources { runIds: string[]; receipts: GenerationTokenReceipt[] }
export interface TurnTokenUsage {
  sourceKey: string;
  status: 'complete' | 'partial' | 'pending' | 'unavailable';
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cachedInputTokens: number | null;
  reasoningTokens: number | null;
  requests: number;
  knownRequests: number;
  pendingRequests: number;
  unknownRequests: number;
  unavailableRuns: number;
}
const count = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0;
const known = (row: GenerationTokenReceipt) => row.usageKnown !== false && count(row.inputTokens) && count(row.outputTokens);
const pending = (row: GenerationTokenReceipt) => !['settled', 'failed'].includes(row.state);

/** Preserve only usage metadata, never model/tool output in the UI receipt. */
export function generationTokenReceipt(row: GenerationTokenReceipt): GenerationTokenReceipt {
  return { generationId: row.generationId, state: row.state, usageKnown: row.usageKnown,
    inputTokens: count(row.inputTokens) ? row.inputTokens : null,
    outputTokens: count(row.outputTokens) ? row.outputTokens : null,
    cachedInputTokens: row.cachedInputTokens ?? row.result?.usage?.cachedInputTokens ?? null,
    reasoningTokens: row.reasoningTokens ?? row.result?.usage?.reasoningTokens ?? null };
}
function mergeReceipts(rows: GenerationTokenReceipt[]): GenerationTokenReceipt[] {
  const unique = new Map<string, GenerationTokenReceipt>();
  for (const row of rows) {
    if (!row.generationId) continue;
    const previous = unique.get(row.generationId);
    // An incomplete replay cannot erase an actual settled usage receipt.
    if (!previous || known(row) || !known(previous)) unique.set(row.generationId, generationTokenReceipt(row));
  }
  return [...unique.values()];
}
export function usageSourceKey(source: TurnUsageSources): string {
  return JSON.stringify([ [...new Set(source.runIds)].sort(), mergeReceipts(source.receipts).sort((a,b)=>a.generationId.localeCompare(b.generationId)) ]);
}

/** All work since the previous answer belongs to this answer, including SDK continuations. */
export function turnUsageSources(messages: DisplayMessage[]): Record<string, TurnUsageSources> {
  const sources: Record<string, TurnUsageSources> = {};
  const rows = [...new Map(messages.map(item=>[item.id,item])).values()];
  let runs = new Set<string>(), receipts: GenerationTokenReceipt[] = [];
  for (const item of rows) {
    if (item.backgroundJob || item.monitorTrace) continue;
    if (item.turnId) runs.add(item.turnId);
    if (item.modelReceipts) receipts.push(...item.modelReceipts);
    if (item.turnOutcome?.generationId) receipts.push({generationId:item.turnOutcome.generationId,state:'unknown'});
    if (item.turnOutcome || item.role==='assistant' && item.phase!=='progress') {
      sources[item.id] = {runIds:[...runs],receipts:mergeReceipts(receipts)};
      runs = new Set(); receipts = [];
    }
  }
  return sources;
}

export function aggregateTurnTokenUsage(source: TurnUsageSources, rows: GenerationTokenReceipt[], unavailableRuns=0, running=false): TurnTokenUsage {
  const receipts = mergeReceipts(rows), actual = receipts.filter(known);
  const inputTokens = actual.reduce((sum,row)=>sum+row.inputTokens!,0);
  const outputTokens = actual.reduce((sum,row)=>sum+row.outputTokens!,0);
  const pendingRequests = receipts.filter(row=>!known(row)&&pending(row)).length;
  const unknownRequests = receipts.filter(row=>!known(row)&&!pending(row)).length;
  // Cache and reasoning are subsets. Missing detail stays unavailable, never invented as zero.
  const detail = (key: 'cachedInputTokens' | 'reasoningTokens', parent: 'inputTokens' | 'outputTokens') =>
    actual.length && actual.every(row=>count(row[key])&&row[key]!<=row[parent]!) ? actual.reduce((sum,row)=>sum+row[key]!,0) : null;
  const unresolved = unavailableRuns>0 || pendingRequests>0 || unknownRequests>0 || running;
  return { sourceKey:usageSourceKey(source), status:!unresolved ? 'complete' : actual.length ? 'partial' : pendingRequests||running ? 'pending' : 'unavailable',
    inputTokens, outputTokens, totalTokens:inputTokens+outputTokens,
    cachedInputTokens:detail('cachedInputTokens','inputTokens'),reasoningTokens:detail('reasoningTokens','outputTokens'),
    requests:receipts.length,knownRequests:actual.length,pendingRequests,unknownRequests,unavailableRuns };
}

export interface TurnUsageClient {
  billingRunSnapshot(runId:string):Promise<{conversationId:string;status:string;generations:GenerationTokenReceipt[]}>;
  agentGenerationGet(generationId:string):Promise<GenerationTokenReceipt & {conversationId:string}>;
}
/** Read account-scoped native receipts; no model requests or billing mutations. */
export async function loadTurnTokenUsage(source: TurnUsageSources, conversationId:string, client:TurnUsageClient):Promise<TurnTokenUsage> {
  const rows = [...source.receipts];
  let unavailableRuns=0, running=false;
  if (!source.runIds.length && !rows.length) return aggregateTurnTokenUsage(source,[],1);
  for (const runId of new Set(source.runIds)) {
    try {
      const proof = await client.billingRunSnapshot(runId);
      if (proof.conversationId!==conversationId) throw new Error('Usage receipt belongs to another conversation');
      rows.push(...proof.generations); running ||= proof.status==='running';
    } catch { unavailableRuns++; }
  }
  for (const row of mergeReceipts(rows)) {
    if (known(row)) continue;
    try {
      const receipt = await client.agentGenerationGet(row.generationId);
      if (receipt.conversationId===conversationId) rows.push(receipt);
    } catch { /* Preserve pending/unavailable status without fabricating usage. */ }
  }
  return aggregateTurnTokenUsage(source,rows,unavailableRuns,running);
}

/** Legacy requests lack a native run ledger. Keep their actual receipts with the user round. */
export function recordGenerationTokenUsage(messages:DisplayMessage[], receipt:GenerationTokenReceipt):DisplayMessage[] {
  let index=messages.length-1;
  while(index>=0&&messages[index].role!=='user')index--;
  if(index<0)return messages;
  return messages.map((item,i)=>i===index?{...item,modelReceipts:mergeReceipts([...(item.modelReceipts??[]),receipt])}:item);
}
