import type { DisplayMessage } from "./pending-generations";

export interface TurnWork {
  kind: "work";
  id: string;
  turnId?: string;
  turnIds: string[];
  items: DisplayMessage[];
  outcome?: "completed" | "interrupted" | "incomplete" | "failed";
}
export type TranscriptEntry = DisplayMessage | TurnWork;
export const isTurnWork = (entry: TranscriptEntry): entry is TurnWork => "kind" in entry && entry.kind === "work";

/** Replayed stream events update one row; they never create another copy of it. */
export function uniqueDisplayMessages(messages: DisplayMessage[]): DisplayMessage[] {
  const records = new Map<string, DisplayMessage>();
  for (const item of messages) records.set(item.id, item.monitorTrace ? { ...item, monitorTrace: uniqueDisplayMessages(item.monitorTrace) } : item);
  return [...records.values()];
}

/** Commentary and execution belong to the same turn, even when a card separates them. */
export function groupWorkRecords(messages: DisplayMessage[]): TranscriptEntry[] {
  const records = uniqueDisplayMessages(messages);
  const completedUsers = new Set<string>();
  const completedTurns = new Set<string>();
  let userId: string | undefined;
  for (const item of records) {
    if (item.role === "user") userId = item.id;
    if ((item.role === "assistant" && item.phase !== "progress" && !item.streaming) || item.turnOutcome) {
      if (userId) completedUsers.add(userId);
      if (item.turnId) completedTurns.add(item.turnId);
    }
  }
  const entries: TranscriptEntry[] = [];
  const turns = new Map<string, TurnWork>();
  let legacyTurn = "legacy-start";
  let currentGroup: TurnWork | undefined;
  userId = undefined;
  for (const item of records) {
    if (item.role === "user") {
      userId = item.id;
      legacyTurn = `legacy-${item.id}`;
      currentGroup = undefined;
    }
    const historicalInput = item.userInput && item.userInput.status !== "pending"
      && (completedUsers.has(item.userInput.userMessageId ?? userId ?? "") || completedTurns.has(item.turnId ?? ""));
    const work = (item.role === "tool" || item.phase === "progress") && item.role !== "user"
      && !item.sourceDraft && !item.extensionProposal && !item.backgroundJob && (!item.userInput || historicalInput) && !item.turnOutcome;
    if (work) {
      const turnId = item.turnId ?? item.monitorTrace?.[0]?.turnId;
      const id = turnId ?? legacyTurn;
      // Local tools can omit a run ID; SDK continuations can introduce another ID.
      // Both belong to the current answer until a new user message or final reply.
      let group = turns.get(id) ?? currentGroup;
      if (!group) {
        group = { kind: "work", id, turnId, turnIds: [], items: [] };
        entries.push(group);
      }
      if (turnId && !group.turnIds.includes(turnId)) {
        group.turnIds.push(turnId);
        if (!group.turnId) { group.turnId = turnId; group.id = turnId; }
      }
      turns.set(id, group);
      currentGroup = group;
      group.items.push(item);
    } else {
      const terminal = item.turnOutcome?.status ?? (item.role === "assistant" && item.phase !== "progress" && !item.streaming ? "completed" : undefined);
      if (terminal) {
        const group = (item.turnId ? turns.get(item.turnId) : undefined) ?? currentGroup;
        if (group) group.outcome = terminal;
      }
      entries.push(item);
      if ((item.role === "assistant" && item.phase !== "progress") || item.turnOutcome) {
        legacyTurn = `legacy-after-${item.id}`;
        currentGroup = undefined;
      }
    }
  }
  return entries;
}

export interface WorkActivity {
  kind: "note" | "record" | "exploration" | "tools" | "input";
  id: string;
  items: DisplayMessage[];
}

// Presentation categories come from tool contracts, not from matching prose or
// guessing whether an arbitrary shell command is safe/read-only.
const explorationTools = new Set([
  "workspace_status", "sources_list", "workspace_skills_list", "workspace_boundaries_list",
  "boundaries_list", "skill_read", "skill_catalog_search", "skill_source_inspect",
  "mcp_registry_search", "extensions_list", "agent_tasks_read_file", "artifacts_inspect",
]);

export function isExplorationRecord(item: DisplayMessage): boolean {
  if (item.itemType === "commandExecution") {
    return !!item.commandActions?.length && item.commandActions.every(action => ["read", "listFiles", "search"].includes(action.type));
  }
  return item.readOnlyHint === true || explorationTools.has(item.toolName ?? "");
}

/** Keep commentary in the reading column and coalesce adjacent activity rows. */
export function projectWorkActivities(items: DisplayMessage[], active: boolean): WorkActivity[] {
  const activities: WorkActivity[] = [];
  for (const item of uniqueDisplayMessages(items)) {
    // Only the server's visible summary is available. Empty reasoning events
    // aren't another "thinking" placeholder; active reasoning uses one live row.
    if (item.itemType === "reasoning" && (!item.details?.trim() || active && item.toolStatus === "running")) continue;
    const kind = item.userInput ? "input" : item.role === "assistant" ? "note"
      : isExplorationRecord(item) ? "exploration"
      : item.itemType === "reasoning" || item.itemType === "commandExecution" || item.monitorTrace ? "record" : "tools";
    const last = activities.at(-1);
    if ((kind === "exploration" || kind === "tools") && last?.kind === kind) last.items.push(item);
    else activities.push({ kind, id: item.id, items: [item] });
  }
  return activities;
}

export function workRecordState(item: DisplayMessage, active: boolean): "running" | "attention" | "success" | "recorded" {
  if (item.toolStatus === "attention" || !active && item.toolStatus === "running") return "attention";
  if (active && (item.toolStatus === "running" || item.streaming)) return "running";
  return item.toolStatus === "success" ? "success" : "recorded";
}
