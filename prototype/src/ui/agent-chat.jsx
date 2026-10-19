// GeoD Agent 0.2.5 chat-ui.tsx / codex-work.css port, 693d6a5.
// Versioned local adaptation: licenses/GeoD-Agent-Chat-MIT.txt.
import React, { useEffect, useId, useRef, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'motion/react';
import { Activity, Check, CheckCircle2, ChevronDown, ChevronRight, CircleAlert, Copy, FolderOpen } from 'lucide-react';
import { Button, MessageText } from './index.jsx';
import { useI18n } from '../i18n.jsx';
import { projectWorkActivities, workRecordState } from '../agent-chat-work.js';
import './agent-chat.css';
const disclosureMotion = {initial:{height:0,opacity:0},animate:{height:'auto',opacity:1},exit:{height:0,opacity:0},transition:{duration:.26,ease:[.22,1,.36,1]}};
export function TranscriptStage({children,conversationId}) {
  const reduce=useReducedMotion();
  return <motion.div key={conversationId} className="agent-transcript-stage" initial={reduce?false:{opacity:0,x:24,scale:.985}} animate={{opacity:1,x:0,scale:1}} transition={{duration:reduce?0:.42,ease:[.22,1,.36,1]}}>{children}</motion.div>;
}
export function MessageTyping({label, ...props}) {
  const reduce=useReducedMotion();
  return <span className="agent-typing" data-slot="message-typing" {...props}><span className="sr-only">{label}</span>{[0,1,2].map(index=><motion.span key={index} aria-hidden="true" animate={reduce?{opacity:.45}:{opacity:[.28,.85,.28],y:[0,-2,0]}} transition={{duration:1.05,ease:[.22,1,.36,1],repeat:Infinity,delay:index*.14}}/>)}</span>;
}
export function CopyMessage({content,label, className=''}) {
  const {t}=useI18n(),[copied,setCopied]=useState(false),[failed,setFailed]=useState(false),timer=useRef(null);
  useEffect(()=>()=>clearTimeout(timer.current),[]);
  const copy=async()=>{try{await navigator.clipboard.writeText(content);setCopied(true);setFailed(false);clearTimeout(timer.current);timer.current=setTimeout(()=>setCopied(false),1800);}catch{setFailed(true);}};
  return <><Button variant="quiet" size="icon" className={`chat-copy ${className}`} icon={copied?Check:Copy} aria-label={t(copied?'Response copied':label||'Copy response')} tooltip={t(copied?'Response copied':label||'Copy response')} onClick={copy}/>{failed&&<small role="alert">{t('Could not copy. Select the text and copy it.')}</small>}</>;
}
export function ChatMessage({entry,streaming=false,progress=false,active=false,children,footer}) {
  const {t}=useI18n(),reduce=useReducedMotion(),user=entry.type==='user',system=entry.type==='system';
  const initial=active&&!reduce?(user?{opacity:0,x:30,y:16,scale:.9}:{opacity:0,y:24,scale:.975,filter:'blur(5px)'}):false;
  const transition=reduce?{duration:0}:user?{type:'spring',stiffness:390,damping:24,mass:.72}:{type:'spring',stiffness:310,damping:28,mass:.85};
  if(progress)return <div className={`agent-work-note ${streaming?'is-streaming':''}`}><div className="agent-work-content geod-message-body"><MessageText text={entry.text} streaming={streaming}/></div></div>;
  return <motion.article className={`agent-message agent-message-${entry.type} ${!user&&!system?'agent-work-entry final':''} ${streaming?'is-streaming':''}`} data-slot="message" data-from={user?'user':'assistant'} data-status={entry.status} initial={initial} animate={{opacity:1,x:0,y:0,scale:1,filter:'blur(0px)'}} transition={transition}>
    {user?<div className="geod-message-user-content" data-slot="message-content">{children}<div className="geod-message-bubble" data-slot="message-bubble-content"><div className="agent-message-body geod-message-body">{entry.text}</div></div></div>:<>
      {children}<div className="agent-message-body agent-work-content geod-message-body">{entry.text?(system?t(entry.text):<MessageText text={entry.text} streaming={streaming}/>):streaming?<MessageTyping label={t('Writing response…')}/>:null}</div>
      {!system&&!streaming&&entry.text&&entry.status!=='running'&&<div className="agent-message-actions"><CopyMessage content={entry.text}/>{footer}</div>}
    </>}
  </motion.article>;
}
export function WorkDetailsSection({label,content}) {
  const {t}=useI18n(),empty=label==='Input summary'&&content.trim()==='{}';
  let format='Text';try{JSON.parse(content);format='JSON';}catch{/* Preserve plain text output. */}
  return <div className={`agent-work-code-panel ${empty?'is-empty':''}`}><div className="agent-work-code-heading"><span>{t(label)}</span><small>{t(empty?'No displayed parameters':format)}</small><CopyMessage content={content} label={`Copy ${label.toLowerCase()}`}/></div>{!empty&&<div className="agent-work-code-area" role="region" aria-label={t('{label} data',{label:t(label)})} tabIndex={0}><pre className="agent-work-code"><code>{content}</code></pre></div>}</div>;
}
export function WorkRecord({title,status,active=false,hasDetails=false,children,onOpenChange,onInspect,className='',duration,detail}) {
  const {t}=useI18n(),[open,setOpen]=useState(false),detailId=useId(),reduce=useReducedMotion();
  const running=active&&status==='running',attention=['failed','interrupted'].includes(status)||!active&&status==='running';
  const icon=running?<MessageTyping label={t('Performing native task')}/>:attention?<CircleAlert size={16}/>:<CheckCircle2 size={16}/>;
  const row=<><span className={`agent-work-record-icon ${running?'is-running':''}`}>{icon}</span><span className="agent-work-record-title">{title}</span>{duration!==undefined&&<small className="agent-work-duration">{duration}</small>}{hasDetails&&<ChevronDown size={14} className={open?'is-open':''}/>}</>;
  return <motion.div className={`agent-work-record ${attention?'attention':''} ${className}`} data-status={running?'running':status==='running'?'interrupted':status} initial={active&&!reduce?{opacity:0,y:6}:false} animate={{opacity:1,y:0}} transition={{duration:reduce?0:.24}}>
    {hasDetails?<Button variant="quiet" size="sm" className="agent-work-record-trigger" data-slot="disclosure-trigger" aria-expanded={open} aria-controls={detailId} onClick={()=>{if(!open)onInspect?.();setOpen(!open);onOpenChange?.(!open);}}>{row}</Button>:<div className="agent-work-record-heading">{row}</div>}
    {attention&&detail&&<p className="agent-work-failure">{detail}</p>}
    <AnimatePresence initial={false}>{open&&<motion.div id={detailId} className="agent-work-record-details" {...disclosureMotion} transition={reduce?{duration:0}:disclosureMotion.transition}>{children}</motion.div>}</AnimatePresence>
  </motion.div>;
}
function ActivityGroup({activity,active,renderEntry,onInspect}) {
  const {t}=useI18n(),[open,setOpen]=useState(false),bodyId=useId();
  const running=activity.entries.filter(entry=>workRecordState(entry,active)==='running');
  const attention=activity.entries.filter(entry=>workRecordState(entry,active)==='attention');
  const succeeded=activity.entries.every(entry=>workRecordState(entry,active)==='success');
  const exploration=activity.kind==='exploration';
  return <section className={`agent-activity-group ${running.length?'is-running':''}`} data-activity-group={activity.kind}>
    <Button variant="quiet" size="sm" className="agent-activity-trigger" aria-expanded={open} aria-controls={bodyId} onClick={()=>{if(!open)onInspect();setOpen(!open);}}>
      {exploration?<FolderOpen size={15}/>:<Activity size={15}/>}
      <span className="agent-activity-label">{t(exploration?running.length?'Checking {count} resources':succeeded?'Checked {count} resources':'Check {count} resources':running.length?'Running {count} tools':'Used {count} tools',{count:activity.entries.length})}</span>
      {attention.length>0&&<span className="agent-activity-attention"><CircleAlert size={13}/>{attention.length}</span>}
      <ChevronDown size={14} className={open?'is-open':''}/>
    </Button>
    <div id={bodyId} className="agent-activity-body" hidden={!open}>{open&&activity.entries.map(entry=>renderEntry(entry,active,onInspect))}</div>
  </section>;
}
export function WorkRecords({id,turnId,entries,active,writing=false,activity,completed,expandRequest,keepVisible,renderEntry,footer,attachments,keepAttachmentsVisible=false}) {
  const {t}=useI18n(),[openChoice,setOpenChoice]=useState(null),bodyId=useId(),reduce=useReducedMotion();
  const open=openChoice??Boolean(active),onInspect=()=>setOpenChoice(true);
  const count=entries.filter(entry=>entry.type==='tool'&&!entry.decision).length;
  const failed=entries.some(entry=>workRecordState(entry,active)==='attention');
  const runningTool=entries.some(entry=>entry.type==='tool'&&!entry.decision&&workRecordState(entry,active)==='running');
  useEffect(()=>{if(expandRequest)setOpenChoice(true);},[expandRequest]);
  const content=items=>projectWorkActivities(items).map(segment=>['exploration','tools'].includes(segment.kind)
    ?<ActivityGroup key={segment.id} activity={segment} active={active} renderEntry={renderEntry} onInspect={onInspect}/>
    :<div key={segment.id} className={segment.kind==='attachment'?'agent-work-attachment':'agent-work-flow-entry'}>{renderEntry(segment.entries[0],active,onInspect)}</div>);
  return <section className={`agent-work-records ${active?'is-active':''}`} data-work-turn={turnId??id} data-work-id={id} data-active={active||undefined} aria-label={t('Execution records')}>
    <Button variant="quiet" size="sm" className="agent-work-records-trigger" data-slot="disclosure-trigger" aria-expanded={open} aria-controls={bodyId} onClick={()=>setOpenChoice(!open)}>
      <span className="agent-work-records-label">{t(active?open?'Hide execution process':'View execution process':completed?'Processing complete':'Execution records')}</span>
      {count>0&&<span className="agent-work-count">{t('{count} operations',{count})}</span>}
      {!active&&failed&&<span className="agent-work-warning"><CircleAlert size={14}/>{t('Tasks need attention')}</span>}
      <ChevronRight size={14} className={open?'is-open':''}/>
    </Button>
    {open?<motion.div id={bodyId} className="agent-work-records-body agent-work-flow" {...disclosureMotion} transition={reduce?{duration:0}:disclosureMotion.transition}>{content(entries)}</motion.div>:
      <div className="agent-work-attachments">{content(entries.filter(entry=>keepVisible?.(entry)))}</div>}
    {(open||keepAttachmentsVisible)&&attachments}
    {active&&!writing&&(!open||!runningTool)&&<LiveStatus activity={activity??t('Working…')}/>}
    {footer&&<div className="agent-message-actions">{footer}</div>}
  </section>;
}
export function LiveStatus({activity}) {
  const {t}=useI18n();return <div className="agent-current-activity" role="status" aria-live="polite" aria-atomic="true" aria-label={t('Current activity')}><span aria-hidden="true"><MessageTyping/></span><span>{activity}</span></div>;
}
