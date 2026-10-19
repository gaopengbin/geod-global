import { t, localize } from "./i18n";
// i18n: presentation strings migrated
import { Activity, ArrowDownToLine, ArrowRight, Bot, Check, CheckCircle2, ChevronDown, ChevronRight, CircleAlert, Copy, FolderOpen, Terminal } from "./icons";
import "./codex-work.css";
import { Fragment, memo, useEffect, useId, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Message, MessageContent, MessageGroup, MessageHeader, MessageScroller, MessageTyping } from "@/components/agents/message";
import { MessageBubble, MessageBubbleContent } from "@/components/agents/message-bubble";
import { Button } from "@/components/motion/button/base";
import { UiTooltip } from "./ui-tooltip";
import { ScrollArea } from "@/components/ui/scroll-area";
import { api, errorMessage, type SourceRegistrationDraft } from "./api";
import type { DisplayMessage, ExtensionProposal } from "./pending-generations";
import {ImageAttachments} from "./image-attachments";
import {DocumentAttachments} from "./document-attachments";
import { collapsePollingHistory, type BackgroundSnapshot } from "./background-jobs";
import { groupWorkRecords, isTurnWork, projectWorkActivities, uniqueDisplayMessages, workRecordState, type TurnWork, type WorkActivity } from "./chat-work";
import {UserInputRequest} from './user-input-request';
import type {UserInputDraft} from './user-input-records';
import {McpKeyDialog} from './mcp-key-dialog';
import {mcpProvider} from './mcp-provider-presets';
import {conversationBusyFailure} from './codex-recovery';
import {turnUsageSources,type TurnTokenUsage} from './turn-token-usage';
import {TurnTokenUsageFooter} from './turn-token-usage-footer';

// Streaming and layout updates must not reparse every completed Markdown block.
const MarkdownContent = memo(function MarkdownContent({ children }: { children: string }) {
  return <ReactMarkdown remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>;
});

function PollingHistory({ item }: { item: DisplayMessage }) {
  const [open, setOpen] = useState(false);
  return <section className="background-monitor-history">
    <Button variant="ghost" size="sm" className="background-job-trigger" aria-expanded={open} onClick={() => setOpen(!open)}><Activity size={17}/><span className="background-job-title">{localize(item.content)}</span><ChevronDown size={15} className={open ? "is-open" : ""}/></Button>
    <AnimatePresence initial={false}>{open && <motion.div className="background-job-details" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: .25 }}><div className="background-job-detail-content">{item.monitorTrace?.map(entry => <p key={entry.id}>{entry.content}</p>)}</div></motion.div>}</AnimatePresence>
  </section>;
}

function CopyMessage({ content, label = "消息" }: { content: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch { /* Clipboard access may be unavailable in browser preview. */ }
  }
  return <UiTooltip content={copied ? t("已复制") : t("复制")}>
    <Button type="button" variant="ghost" size="sm" className="chat-copy" onClick={copy} aria-label={copied ? t("已复制{0}", {"0": label}) : t("复制{0}", {"0": label})}>{copied ? <Check size={16} /> : <Copy size={16} />}</Button>
  </UiTooltip>;
}

function WorkDetailSection({ label, content }: { label: string; content: string }) {
  const empty = label === "输入" && content.trim() === "{}";
  let format = "文本";
  try { JSON.parse(content); format = "JSON"; } catch { /* Preserve command and text output. */ }
  return <div className={`agent-work-code-panel${empty ? " is-empty" : ""}`}>
    <div className="agent-work-code-heading"><span>{localize(label)}</span><small>{empty ? t("无参数") : localize(format)}</small><CopyMessage content={content} label={localize(label)}/></div>
    {!empty && <ScrollArea className="agent-work-code-area" viewportProps={{"aria-label":`${label}详情`}}><pre className="agent-work-code"><code>{content}</code></pre></ScrollArea>}
  </div>;
}

function WorkDetails({ item }: { item: DisplayMessage }) {
  if (item.itemType === "commandExecution") return <div className="agent-command-panel">
    <div className="agent-work-code-heading"><span>Shell</span><CopyMessage content={item.content} label={t("命令")}/></div>
    <ScrollArea className="agent-work-code-area" viewportProps={{"aria-label":t("命令与输出")}}><div className="agent-command-content"><pre className="agent-command-line"><code>$ {item.content}</code></pre>{item.details && <pre className="agent-command-output"><code>{item.details}</code></pre>}</div></ScrollArea>
    <footer className={`agent-command-result ${item.toolStatus === "attention" ? "attention" : ""}`}>
      {item.durationMs != null && Number.isFinite(item.durationMs) && item.durationMs >= 0 && <span>{t("用时 {0} 秒", {"0":(item.durationMs / 1000).toLocaleString(undefined,{maximumFractionDigits:1})})}</span>}
      <span>{item.exitCode != null ? t("退出码 {0}", {"0":String(item.exitCode)}) : t(item.toolStatus === "running" ? "运行中" : item.toolStatus === "success" ? "已完成" : item.toolStatus === "attention" ? "未完成" : "执行记录")}</span>
    </footer>
  </div>;
  if (item.monitorTrace || item.itemType === "reasoning") return <ScrollArea className="agent-work-reasoning-area" viewportProps={{ "aria-label": t(item.monitorTrace ? "执行记录详情" : "模型思考详情") }}>
    <div className="agent-work-reasoning agent-work-content geod-message-body">
      {item.monitorTrace
        ? item.monitorTrace.map(entry => <MarkdownContent key={entry.id}>{entry.content}</MarkdownContent>)
        : <MarkdownContent>{item.details ?? ""}</MarkdownContent>}
    </div>
  </ScrollArea>;
  const sections: { label: string; content: string }[] = [];
  if (item.itemType === "commandExecution") sections.push({ label: "命令", content: item.content });
  if (item.details) {
    let payload: Record<string, unknown> | undefined;
    try {
      const value = JSON.parse(item.details);
      if (value && typeof value === "object" && !Array.isArray(value) && ("arguments" in value || "result" in value || "error" in value)) payload = value;
    } catch { /* Command output and plain-text tool results are not JSON. */ }
    if (payload) {
      for (const [key, label] of [["arguments", "输入"], ["result", "结果"], ["error", "错误"]]) {
        if (payload[key] != null) sections.push({ label, content: typeof payload[key] === "string" ? payload[key] as string : JSON.stringify(payload[key], null, 2) });
      }
    } else sections.push({ label: item.itemType === "commandExecution" ? "输出" : "详情", content: item.details });
  }
  return <div className="agent-work-detail-sections">{sections.map(section => <WorkDetailSection key={section.label} {...section}/>)}</div>;
}

function workTitle(item: DisplayMessage, active = true) {
  if(item.itemType==='reasoning'&&item.toolStatus==='attention')return t('思考已停止');
  if(item.itemType==='reasoning')return t('思考摘要');
  if (item.itemType === "commandExecution") {
    const state = workRecordState(item, active);
    return t(state === "running" ? "正在运行命令" : state === "success" ? "命令已完成" : state === "attention" ? "命令未完成" : "运行命令");
  }
  // A native MCP server may be named by its local connection ID.
  if (item.itemType === "mcpToolCall" && /^[a-f0-9-]{36} · /i.test(item.content)) return item.content.replace(/^[a-f0-9-]{36} · /i, "MCP 工具 · ");
  return localize(item.content);
}

function TurnOutcomeCard({item,onContinue,canContinue}:{item:DisplayMessage;onContinue?:()=>void;canContinue:boolean}) {
  const outcome=item.turnOutcome!;
  return <section className="agent-turn-outcome" role="status" aria-label={t(outcome.status==='interrupted'?'本轮已停止':'本轮未完成')}>
    <CircleAlert size={17}/><div><strong>{t(outcome.status==='interrupted'?'本轮已停止':'本轮未完成')}</strong><p>{localize(outcome.message)}</p>{onContinue&&<Button variant="outline" size="sm" disabled={!canContinue} onClick={onContinue}>{t('继续处理')}<ArrowRight size={15}/></Button>}</div>
  </section>;
}

function WorkRecord({ item, active, onInspect, initiallyOpen = false }: { item: DisplayMessage; active: boolean; onInspect: () => void; initiallyOpen?: boolean }) {
  const [open, setOpen] = useState(initiallyOpen);
  const detailId = useId();
  const state = workRecordState(item, active);
  const running = state === "running";
  const attention = state === "attention";
  const reasoning = item.itemType === "reasoning";
  const hasDetails = !!item.details || item.itemType === "commandExecution" || !!item.monitorTrace?.length;
  const icon = attention ? <CircleAlert size={15}/> : reasoning ? <Activity size={15}/> : item.itemType === "commandExecution" ? <Terminal size={15}/> : state === "success" ? <CheckCircle2 size={15}/> : <Activity size={15}/>;
  if (item.role === "assistant") return <div className={`agent-work-note ${item.streaming ? "is-streaming" : ""}`}>
    <div className="agent-work-content geod-message-body"><MarkdownContent>{item.content}</MarkdownContent></div>
  </div>;
  const row = <><span className={`agent-work-record-icon ${running ? "is-running" : ""}`}>{icon}</span><span className="agent-work-record-title">{workTitle(item, active)}</span>{hasDetails && <ChevronDown size={14} className={open ? "is-open" : ""}/>}</>;
  return <div className={`agent-work-record ${attention ? "attention" : ""} ${running ? "is-running" : ""}`} data-work-record={item.id}>
    {hasDetails ? <Button variant="ghost" size="sm" whileHover={undefined} whileTap={undefined} className="agent-work-record-trigger" aria-expanded={open} aria-controls={detailId} onClick={() => { if (!open) onInspect(); setOpen(!open); }}>{row}</Button> : <div className="agent-work-record-heading">{row}</div>}
    <div id={detailId} className="agent-work-record-details" hidden={!open}>{open && <WorkDetails item={item}/>}</div>
  </div>;
}

function ActivityGroup({ activity, active, onInspect }: { activity: WorkActivity; active: boolean; onInspect: () => void }) {
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  const { items, kind } = activity;
  const running = items.filter(item => workRecordState(item, active) === "running");
  const attention = items.filter(item => workRecordState(item, active) === "attention");
  const succeeded = items.every(item => workRecordState(item, active) === "success");
  const label = kind === "exploration"
    ? t(running.length ? "正在检查 {0} 项资料" : succeeded ? "已检查 {0} 项资料" : "检查 {0} 项资料", {"0":String(items.length)})
    : running.length ? workTitle(running.at(-1)!) : items.length === 1 ? workTitle(items[0], active) : t("使用 {0} 项工具", {"0":String(items.length)});
  return <section className={`agent-activity-group ${running.length ? "is-running" : ""}`} data-activity-group={kind}>
    <Button variant="ghost" size="sm" whileHover={undefined} whileTap={undefined} className="agent-activity-trigger" aria-expanded={open} aria-controls={bodyId} onClick={() => { if (!open) onInspect(); setOpen(!open); }}>
      {kind === "exploration" ? <FolderOpen size={15}/> : <Activity size={15}/>}
      <span className="agent-activity-label">{label}</span>
      {attention.length > 0 && <span className="agent-activity-attention"><CircleAlert size={13}/>{attention.length}</span>}
      <ChevronDown size={14} className={open ? "is-open" : ""}/>
    </Button>
    <div id={bodyId} className="agent-activity-body" hidden={!open}>
      {open && items.map(item => <WorkRecord key={item.id} item={item} active={active} onInspect={onInspect}/>)}
    </div>
  </section>;
}

function LiveActivity({ label, reasoning, onInspect, onOpenChange }: { label: string; reasoning?: DisplayMessage; onInspect?: () => void; onOpenChange?: (open:boolean) => void }) {
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  const content = <><span className="agent-activity-shimmer">{label}</span>{reasoning?.details && <ChevronDown size={14} className={open ? "is-open" : ""}/>}</>;
  return <div className="agent-live-status" role="status" aria-label={t("当前执行状态")}>
    {reasoning?.details ? <><Button variant="ghost" size="sm" whileHover={undefined} whileTap={undefined} className="agent-live-trigger" aria-expanded={open} aria-controls={bodyId} onClick={() => { if (!open) onInspect?.(); onOpenChange?.(!open); setOpen(!open); }}>{content}</Button><div id={bodyId} hidden={!open}>{open && <WorkDetails item={reasoning}/>}</div></> : <span>{content}</span>}
  </div>;
}

function WorkRecords({ group, active, writing, activityLabel, renderInput }: { group: TurnWork; active: boolean; writing: boolean; activityLabel: string; renderInput: (item: DisplayMessage) => ReactNode }) {
  // An explicit disclosure choice survives completion and continuation. Merely
  // running a turn uses the automatic expanded -> collapsed transition.
  const [openChoice, setOpenChoice] = useState<boolean | null>(null);
  const [inspectedReasoning, setInspectedReasoning] = useState<string>();
  const open = openChoice ?? active;
  const bodyId = useId();
  const { items } = group;
  const operations = items.filter(item => item.role === "tool" && item.itemType !== "reasoning" && !item.userInput).length;
  const failed = items.some(item => item.toolStatus === "attention" || (!active && item.toolStatus === "running"));
  const activities = projectWorkActivities(items, active);
  const runningTool = activities.some(activity => activity.items.some(item => item.role === "tool" && workRecordState(item, active) === "running"));
  const reasoning = [...items].reverse().find(item => item.itemType === "reasoning" && item.toolStatus === "running");
  const label = active ? t("查看执行过程") : t(group.outcome === "completed" ? "处理完成" : group.outcome === "interrupted" ? "执行已停止" : group.outcome ? "本轮未完成" : "执行记录");
  const onInspect = () => setOpenChoice(true);
  return <section className={`agent-work-records ${active ? "is-active" : ""}`} data-work-turn={group.id} aria-label={t("本轮思考与执行")}>
    {(!active || !open) && <Button variant="ghost" size="sm" whileHover={undefined} whileTap={undefined} className="agent-work-records-trigger" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpenChoice(!open)}>
      <span className="agent-work-records-label">{label}</span>
      {operations > 0 && <span className="agent-work-count">{operations} {t(" 项操作")}</span>}
      {failed && <span className="agent-work-warning"><CircleAlert size={13}/>{t("有异常记录")}</span>}
      <ChevronRight size={14} className={open ? "is-open" : ""}/>
    </Button>}
    <div id={bodyId} className="agent-work-records-body" hidden={!open}>
      {open && <div className="agent-work-flow">{activities.map(activity => activity.kind === "input" ? <Fragment key={activity.id}>{renderInput(activity.items[0])}</Fragment>
        : activity.kind === "exploration" || activity.kind === "tools" ? <ActivityGroup key={activity.id} activity={activity} active={active} onInspect={onInspect}/>
        : <WorkRecord key={activity.id} item={activity.items[0]} active={active} onInspect={onInspect} initiallyOpen={activity.id === inspectedReasoning}/>)}</div>
      }
    </div>
    {active && !writing && (!open || !runningTool) && <LiveActivity key={reasoning?.id ?? "live"} label={activityLabel} reasoning={reasoning} onInspect={onInspect} onOpenChange={value => setInspectedReasoning(value ? reasoning?.id : undefined)}/>}
  </section>;
}

function activeWorkId(entries: ReturnType<typeof groupWorkRecords>, busy: boolean, activeTurnId?: string | null) {
  if (!busy) return undefined;
  if (activeTurnId) return entries.find(entry => isTurnWork(entry) && (entry.id === activeTurnId || entry.turnIds.includes(activeTurnId)))?.id ?? activeTurnId;
  for (let index = entries.length - 1; index >= 0; index--) {
    const entry = entries[index];
    if (isTurnWork(entry)) return entry.id;
    if (entry.role === "user" || (entry.role === "assistant" && !entry.streaming)) return undefined;
  }
  return undefined;
}

function ExtensionReview({ proposal, busy, onApprove,onConfigured,conversationId }: { proposal: ExtensionProposal; busy: boolean; onApprove: (proposal: ExtensionProposal) => void;onConfigured?:(old:ExtensionProposal,next:ExtensionProposal)=>void;conversationId:string }) {
  // A saved setup card stays in the transcript; only its button opens the credential form.
  const [configureOpen,setConfigureOpen]=useState(false);
  const [preview, setPreview] = useState("");
  const [previewError, setPreviewError] = useState("");
  const [loading, setLoading] = useState(false);
  const credentialLabel=mcpProvider(proposal.detail)?.label??'服务凭据';
  async function showPreview() {
    setLoading(true);
    setPreviewError("");
    try { setPreview(await api.skillPreview(proposal.id)); }
    catch (cause) { setPreviewError(errorMessage(cause)); }
    finally { setLoading(false); }
  }
  return <div className="agent-source-review">
    <strong>{proposal.kind === "mcp" ? t("MCP 连接器") : "Skill"} · {proposal.name}</strong>
    <span>{proposal.detail}</span>
    {proposal.description && <small>{proposal.description}</small>}
    {proposal.toolNames?.length ? <small>{t("提供工具：")}{proposal.toolNames.join("、")}</small> : null}
    {proposal.sha256 && <small>{t("内容 SHA-256：")}{proposal.sha256.slice(0, 16)}…</small>}
    <small>{proposal.kind === "mcp" ? t("启用后，Agent 可以调用此连接器提供的工具。请确认你信任这个服务。") : t("启用后，Agent 可以读取这份 Skill 的工作说明。请先核对来源和内容。")}</small>
    {proposal.kind === "skill" && <Button type="button" size="sm" variant="ghost" disabled={loading} onClick={() => void showPreview()}>{loading ? t("正在读取…") : t("查看完整 SKILL.md")}</Button>}
    {previewError && <small role="alert">{previewError}</small>}
    {preview && <pre className="agent-skill-preview">{preview}</pre>}
    {proposal.requiresKey?<><small>{t('在本机填写 {0}。关闭后可随时重新打开。',{'0':t(credentialLabel)})}</small><Button type="button" size="sm" variant="outline" onClick={()=>setConfigureOpen(true)}>{t('配置并连接')}<ArrowRight size={16}/></Button>{configureOpen&&<McpKeyDialog proposal={proposal} conversationId={conversationId} onClose={()=>setConfigureOpen(false)} onConnected={(connector,tools)=>onConfigured?.(proposal,{...proposal,id:connector.id,detail:connector.url,requiresKey:false,toolNames:tools.tools.slice(0,8).map(tool=>tool.name)})}/>}</>:<Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => onApprove(proposal)}>{t("确认启用")}<ArrowRight size={16} /></Button>}
  </div>;
}

export function ChatTranscript({ conversationId, messages, busy, activity, activeTurnId, onReviewSource, onApproveExtension, onConfigureExtension, afterEntry, children, openInputId, onOpenInput, onInputReply, onInputDraft, canAnswerInput, onContinueTurn, canContinueTurn=true, onTokenUsage }: { conversationId: string; messages: DisplayMessage[]; busy: boolean; activity: string; activeTurnId?: string | null; onReviewSource: (draft: SourceRegistrationDraft) => void; onApproveExtension: (proposal: ExtensionProposal) => void; onConfigureExtension?:(old:ExtensionProposal,next:ExtensionProposal)=>void; backgroundJobs?: Record<string, BackgroundSnapshot>; onOpenJob?: (jobId: string) => void; afterEntry?: (id: string) => ReactNode; planTitles?: Record<string, string>; children?: ReactNode;openInputId?:string|null;onOpenInput?:(id:string|null)=>void;onInputReply?:(id:string,value:unknown)=>Promise<void>;onInputDraft?:(id:string,value:UserInputDraft)=>void;canAnswerInput?:(id:string)=>boolean;onContinueTurn?:()=>void;canContinueTurn?:boolean;onTokenUsage?:(id:string,usage:TurnTokenUsage)=>void }) {
  const [following, setFollowing] = useState(true);
  const [resumeKey, setResumeKey] = useState(0);
  const entries = groupWorkRecords(collapsePollingHistory(uniqueDisplayMessages(messages)));
  const usageSources=turnUsageSources(messages);
  const tokenFooter=(item:DisplayMessage)=>!item.streaming&&!(activeTurnId&&usageSources[item.id]?.runIds.includes(activeTurnId))&&<TurnTokenUsageFooter key={`usage-${item.id}`} conversationId={conversationId} messageId={item.id} source={usageSources[item.id]??{runIds:[],receipts:[]}} saved={item.tokenUsage} onUsage={onTokenUsage}/>;
  const executionActive = busy || !!activeTurnId;
  const followKey = messages.filter(item => item.role === "user" || item.userInput?.status === "answered").map(item => item.id).join("|") + `|resume-${resumeKey}`;
  const liveWorkId = activeWorkId(entries, executionActive, activeTurnId);
  const activityLabel = localize(activity) || t("正在处理…");
  const writing = messages.some(item => item.role === "assistant" && item.phase === "final" && item.streaming);
  const hasLiveGroup = entries.some(item => isTurnWork(item) && item.id === liveWorkId);
  const lastCompletedAnswer = [...messages].reverse().find(item => item.role === "assistant" && item.phase !== "progress" && !item.streaming)?.id;
  // Close a reviewed answer once on completion; users can reopen it afterwards.
  useEffect(() => {
    if (messages.some(item => item.id === openInputId && item.userInput && item.userInput.status !== "pending")) onOpenInput?.(null);
  }, [conversationId, lastCompletedAnswer]);
  useEffect(() => { setFollowing(true); }, [conversationId]);
  const renderInput = (item: DisplayMessage) => <UserInputRequest record={item.userInput!} open={openInputId===item.id} onOpen={open=>onOpenInput?.(open?item.id:null)} onReply={value=>onInputReply?.(item.id,value)??Promise.resolve()} onDraft={value=>onInputDraft?.(item.id,value)} disabled={canAnswerInput?!canAnswerInput(item.id):false}/>;
  let resumableOutcome:string|undefined;
  for(let index=messages.length-1;index>=0;index--){
    const item=messages[index];
    if(item.role==='user'||item.role==='assistant'&&item.phase==='final')break;
    if(item.turnOutcome){if(!conversationBusyFailure(item.turnOutcome))resumableOutcome=item.id;break;}
  }
  return <div className="agent-transcript-view"><MessageScroller key={conversationId} className="agent-messages" viewportClassName="agent-messages-viewport" contentClassName="agent-messages-content" label={t("GeoD Agent 工作记录")} busy={executionActive} followKey={followKey} onFollowChange={setFollowing}>
    <div className="agent-transcript-stage">
    {messages.length ? <MessageGroup spacing="default" className="geod-message-group">
      {entries.map(item => <Fragment key={item.id}>{isTurnWork(item)
        ? <WorkRecords key={`work-${conversationId}-${item.items[0]?.id}`} group={item} active={item.id === liveWorkId} writing={writing} activityLabel={activityLabel} renderInput={renderInput}/>
        : item.turnOutcome
        ? conversationBusyFailure(item.turnOutcome) ? null : <div><TurnOutcomeCard item={item} onContinue={item.id===resumableOutcome?onContinueTurn:undefined} canContinue={canContinueTurn&&!executionActive}/><div className="agent-message-actions">{tokenFooter(item)}</div></div>
        : item.userInput
        ? renderInput(item)
        : item.monitorTrace
        ? <PollingHistory key={item.id} item={item}/>
        : item.backgroundJob
        ? null
        : item.role === "user"
        ? <Message key={item.id} from="user" className="geod-message geod-message-user" animateIn={busy}>
            <MessageContent><MessageHeader>{t("你")}</MessageHeader>{item.images?.length?<ImageAttachments images={item.images}/>:null}{item.documents?.length?<DocumentAttachments documents={item.documents}/>:null}<MessageBubble variant="soft" animateIn={false} className="geod-message-bubble"><MessageBubbleContent className="geod-message-body">{item.content}</MessageBubbleContent></MessageBubble></MessageContent>
          </Message>
        : item.role === "tool"
          ? <motion.div className={`agent-step ${item.toolStatus === "attention" ? "attention" : ""} ${item.toolStatus === "running" ? "agent-step-running" : ""}`} key={item.id} role="status" initial={busy ? { opacity: 0, x: -22, y: 8, scale: 0.96 } : false} animate={{ opacity: 1, x: 0, y: 0, scale: 1 }} transition={{ type: "spring", stiffness: 370, damping: 27, mass: 0.7 }}>
              <span className="agent-step-icon">{item.toolStatus === "running" ? <MessageTyping label={t("正在调用工具")} /> : item.toolStatus === "attention" ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}</span>
              <div className="agent-step-content"><span>{item.content}</span>{item.sourceDraft && <div className="agent-source-review"><strong>{item.sourceDraft.source.name}</strong><span>{item.sourceDraft.source.urlTemplate}</span><small>{t("已整理图源配置。请核对服务地址和参数。")}</small><Button type="button" size="sm" variant="outline" onClick={() => onReviewSource(item.sourceDraft!)}>{t("查看配置")}<ArrowRight size={16} /></Button></div>}{item.extensionProposal && <ExtensionReview proposal={item.extensionProposal} busy={busy} onApprove={onApproveExtension} conversationId={conversationId} onConfigured={onConfigureExtension} />}</div>
            </motion.div>
          : <article className={`agent-work-entry ${item.phase === "progress" ? "progress" : "final"} ${busy && messages.at(-1)?.id === item.id ? "is-live" : ""} ${item.streaming ? "is-streaming" : ""}`} key={item.id} data-slot="message" data-from="assistant">
              <div className="agent-work-content geod-message-body"><MarkdownContent>{item.content}</MarkdownContent></div>
              {item.phase !== "progress" && !item.streaming && <div className="agent-message-actions"><CopyMessage content={item.content}/>{tokenFooter(item)}</div>}
            </article>}{afterEntry?.(item.id)}</Fragment>)}
      {executionActive && !hasLiveGroup && !writing && <LiveActivity label={activityLabel}/>}
    </MessageGroup> : <div className="agent-welcome"><Bot size={28} /><h3>{t("描述你要获取的影像")}</h3><p>{t("也可以直接说需要什么 Skill 或 MCP 能力，Agent 会查找并准备接入。")}</p></div>}
    {children}
    </div>
  </MessageScroller>
    {!following && <Button variant="outline" size="sm" whileHover={undefined} whileTap={undefined} className="agent-jump-latest" onClick={() => { setFollowing(true); setResumeKey(key => key + 1); }}><ArrowDownToLine size={15}/>{t("回到最新")}</Button>}
  </div>;
}
