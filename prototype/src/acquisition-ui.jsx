import React,{useContext,useEffect} from 'react';
import {usage,countBucket} from './usage-client.js';
import {usageContext} from './usage-events.js';
import {CheckCircle2,FolderOpen,ListTodo,LoaderCircle,AlertCircle} from 'lucide-react';
import {Badge,Button,Input,Modal,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {deliveryState,taskHref} from './acquisition-flow.js';
import {downloadQueueCounts} from './download-observation.js';
import './acquisition.css';

export function ExploreResultRow({title,meta,selected,focused,onSelect,selectionLabel,onFocus,focusLabel,thumbnail,actions,disabled,className='',children}) {
  const focusRow = event => {
    if (!onFocus || event.defaultPrevented || event.target.closest('button,a,input,select,textarea,[role="checkbox"],[role="button"]')) return;
    onFocus();
  };
  return <article className={`explore-result-row ${className}`} data-selected={selected||undefined} data-focused={focused||undefined} data-can-focus={Boolean(onFocus)||undefined} onClick={focusRow}>
    {onSelect&&<Input type="checkbox" checked={Boolean(selected)} disabled={disabled} aria-label={selectionLabel} onChange={onSelect}/>}
    {thumbnail&&<span className="explore-result-thumbnail">{thumbnail}</span>}
    <div className="explore-result-copy"><Button variant="quiet" size="row" className="explore-result-focus" disabled={!onFocus} aria-label={focusLabel} onClick={onFocus} title={typeof title==='string'?title:undefined}>{title}</Button>{meta&&<div className="explore-result-meta">{meta}</div>}</div>
    {actions&&<div className="explore-result-actions">{actions}</div>}{children}
  </article>;
}
export function SelectionActions({count,onClear,preview,download,details,children}) {
  const {t,number}=useI18n();
  return <section className="explore-selection-bar" aria-label={t('Selected product actions')}>
    <div className="explore-selection-heading"><strong>{t('{count} selected',{count:number(count)})}</strong>{details}{onClear&&<Button variant="quiet" size="xs" disabled={!count} onClick={onClear}>{t('Clear selection')}</Button>}</div>
    <div className="explore-selection-buttons">{preview}{download}</div>{children}
  </section>;
}
// Review is shared by file downloads and service-generated results. The caller
// supplies the actual delivery kind; a map rendering is never called an original.
export function DownloadReview({count,onClose,onConfirm,busy=false,confirmDisabled=false,error,children,receipt}) {
  const {t,number}=useI18n();
  useEffect(()=>{usage.record('download_review_opened',{...usageContext(),count:countBucket(count)});},[]);
  const confirm=()=>{usage.record('download_confirmed',{...usageContext(),count:countBucket(count)});return onConfirm?.();};
  return <Modal title={t('Confirm download')} closeLabel={t('Close')} onClose={onClose} closeDisabled={busy}
    footer={<><Button disabled={busy} onClick={onClose}>{t('Close')}</Button>{!receipt&&<Button variant="primary" disabled={busy||!count||confirmDisabled} onClick={confirm}>{busy&&<Spinner size={14}/>} {t('Confirm and download')}</Button>}</>}>
    <div className="download-review"><strong>{t('{count} selected',{count:number(count)})}</strong>{children}{error&&<p role="alert" className="stac-error">{t(error)}</p>}{receipt}</div>
  </Modal>;
}
export function SavedResultsReceipt({entries=[],hrefFor,onOpen}) {
  const {t,number}=useI18n();
  if(!entries.length)return null;
  return <section className="delivery-receipt delivery-receipt-compact" aria-label={t('Download and results')} data-status="ready">
    <div className="delivery-receipt-heading"><CheckCircle2 size={16}/><strong>{t('Results saved locally')}</strong><Badge>{number(entries.length)}</Badge></div>
    <div className="delivery-receipt-actions">{entries.map(entry=>hrefFor?<Button asChild size="sm" variant="secondary" key={entry.key||entry.id}><a href={hrefFor(entry)} title={entry.title} onClick={()=>usage.record('results_opened',usageContext())}><FolderOpen size={14}/>{entries.length===1?t('View results'):entry.title}</a></Button>:<Button size="sm" variant="secondary" key={entry.key||entry.id} onClick={()=>{usage.record('results_opened',usageContext());onOpen?.(entry);}}><FolderOpen size={14}/>{entries.length===1?t('View results'):entry.title}</Button>)}</div>
  </section>;
}
export function DeliveryReceipt({project,jobIds,jobs:providedJobs,submitted=false,onOpenProject,compact=false,trackResults=true}) {
  const {t,number}=useI18n(),runtime=useContext(RuntimeContext);
  const jobs=providedJobs||runtime?.jobs||[];
  const state=deliveryState(jobs,jobIds);
  const queue=downloadQueueCounts(state.rows);
  const labels={ready:'Files ready',active:'Downloading in background',partial:'Some files ready · needs attention',attention:'Download needs attention',pending:'Waiting for task status',saved:submitted?'Waiting for task status':'Selection saved · download not started'};
  const Icon=state.status==='ready'?CheckCircle2:['partial','attention'].includes(state.status)?AlertCircle:['active','pending'].includes(state.status)?LoaderCircle:FolderOpen;
  return <section className={`delivery-receipt${compact?' delivery-receipt-compact':''}`} aria-label={t('Download and results')} data-status={state.status}>
    <div className="delivery-receipt-heading"><Icon size={16}/><strong title={project?.name}>{project?.name||t('Selected downloads')}</strong><Badge tone={state.status==='ready'?'accent':'neutral'}>{t(labels[state.status])}</Badge></div>
    {state.total>0&&<p role="status">{t('{done} of {total} files downloaded',{done:number(state.done),total:number(state.total)})}{queue.running+queue.queued>0&&<> · {t('{running} downloading · {queued} queued',{running:number(queue.running),queued:number(queue.queued)})}</>}{state.attention>0&&<> · {t('{count} need attention',{count:number(state.attention)})}</>}</p>}
    <div className="delivery-receipt-actions">{(submitted||state.total>0)&&<Button asChild variant="secondary" size="sm"><a href={taskHref({projectId:project?.id,jobId:!project&&state.rows[0]?.id})}><ListTodo size={14}/>{t('View download tasks')}</a></Button>}{project?.id&&<Button asChild variant={state.done>0?'primary':'secondary'} size="sm"><a href={`#My%20Data?project=${encodeURIComponent(project.id)}`} onClick={event=>{if(trackResults&&state.done>0)usage.record('results_opened',usageContext());if(onOpenProject){event.preventDefault();onOpenProject(project.id);}}}><FolderOpen size={14}/>{t(state.done>0?'View results':'Open project')}</a></Button>}</div>
  </section>;
}
// Global navigation is intentionally labelled as background work. It never
// claims unrelated tasks belong to the currently selected product or AOI.
export function BackgroundWorkLink() {
  const {t,number}=useI18n(),runtime=useContext(RuntimeContext);
  const active=runtime?.jobs?.filter(job=>['queued','running'].includes(job.status)).length||0;
  if(!active)return null;
  return <Button asChild variant="quiet" size="sm" className="background-work-link"><a href="#Tasks"><ListTodo size={14}/>{t('Background tasks · {count}',{count:number(active)})}</a></Button>;
}
