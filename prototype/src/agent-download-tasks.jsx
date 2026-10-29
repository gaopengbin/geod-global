import React,{useEffect,useState} from 'react';
import {CircleAlert,Download,ListTodo} from 'lucide-react';
import {Badge,Button,Disclosure,Progress} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {formatBytes} from './runtime-client.js';
import {downloadTaskSummary} from './agent-download-tasks.js';
import {downloadQueueCounts} from './download-observation.js';
import './agent-download-tasks.css';

export function AgentDownloadTasks({plans,onOpenTasks,renderPlan,expandRequest,focusPlanId}) {
  const {t,number,locale}=useI18n(),summary=downloadTaskSummary(plans);
  const queue=downloadQueueCounts(summary.jobs);
  const [open,setOpen]=useState(false),[detail,setDetail]=useState(null);
  useEffect(()=>{if(expandRequest&&plans.some(plan=>plan.planId===focusPlanId)){setOpen(true);setDetail(focusPlanId);}},[expandRequest,focusPlanId]);
  const preferred=summary.failed[0]??summary.jobs.find(job=>job.status==='running')??summary.jobs[0];
  return <section className="agent-download-tasks" aria-label={t('Download tasks')} data-attention={summary.needsAttention||undefined}>
    <div className="agent-download-heading"><Download size={15}/><strong>{t('Download tasks')}</strong><Badge tone={summary.needsAttention?'warning':summary.complete?'neutral':'blue'}>{t(summary.status)}</Badge>{preferred&&<Button variant="quiet" size="icon" icon={ListTodo} aria-label={t('Open task')} tooltip={t('Open task')} onClick={()=>onOpenTasks(preferred.id)}/>}</div>
    <div className="agent-download-totals"><span>{t('{completed}/{total} files ready',{completed:number(summary.ready),total:number(summary.total)})}{queue.running+queue.queued>0&&<> · {t('{running} downloading · {queued} queued',{running:number(queue.running),queued:number(queue.queued)})}</>}</span><span>{formatBytes(summary.transferred,locale)}{summary.totalBytes!==null?` / ${formatBytes(summary.totalBytes,locale)}`:` · ${t('Total size unavailable')}`}</span></div>
    {!summary.complete&&<Progress value={summary.totalBytes!==null?summary.progressBytes:summary.running?null:0} max={summary.totalBytes??100} aria-label={t('Download progress')}/>}
    {summary.needsAttention&&<div className="agent-download-attention" role="status"><CircleAlert size={13}/><span>{t('{count} downloads need attention',{count:number(summary.failed.length)})}</span><Button size="xs" variant="quiet" onClick={()=>onOpenTasks(summary.failed[0].id)}>{t('View failed task')}</Button></div>}
    <Disclosure className="agent-download-details" open={open} onOpenChange={setOpen} summary={t('Download details · {count} plans',{count:number(plans.length)})}>
      <div className="agent-download-list">{plans.map(plan=>{
        const state=downloadTaskSummary([plan]);
        const assets=[...new Set(plan.files.map(file=>(file.originalAssetKey??file.assetKey).toUpperCase()))].join(' / ');
        return <Disclosure key={plan.planId} className="agent-download-item" open={detail===plan.planId} onOpenChange={next=>setDetail(next?plan.planId:null)} summary={<span className="agent-download-item-summary"><span title={plan.project?.name??plan.source}>{plan.project?.name??t(plan.source)} · {assets}</span><small>{t('{completed}/{total} files ready',{completed:number(state.ready),total:number(state.total)})} · {t(state.status)}</small></span>}>
          {renderPlan(plan)}
        </Disclosure>;
      })}</div>
    </Disclosure>
  </section>;
}
