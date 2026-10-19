import React from 'react';
import { ChartNoAxesColumn } from 'lucide-react';
import { Button, Popover } from './index.jsx';
import { useI18n } from '../i18n.jsx';

export function TurnTokenUsage({usage}){
  const {t,number}=useI18n();if(!usage)return null;
  const known=usage.knownRequests>0;
  return <Popover align="start" side="top" className="agent-token-usage-panel" trigger={<Button variant="quiet" size="icon" className="agent-turn-token-usage" icon={ChartNoAxesColumn} aria-label={t('Token usage for this request')} tooltip={t('Token usage for this request')} data-usage-status={usage.status}/> }>
    <strong>{t(usage.status==='complete'?'This request’s usage':known?'Recorded usage':'Usage unavailable')}</strong>
    {known?<><div className="agent-token-usage-total"><strong>{number(usage.total)}</strong><span>tokens</span></div>
      <dl className="agent-token-usage-breakdown">{[['Input tokens',usage.input],['Output tokens',usage.output],['Cached input (included)',usage.cachedInput],['Reasoning output (included)',usage.reasoningOutput]].map(([label,value])=><div key={label}><dt>{t(label)}</dt><dd>{value==null?t('Unknown'):number(value)}</dd></div>)}</dl></>:<p>{t(usage.running?'Waiting for actual model usage.':'No actual usage receipt is available for this request.')}</p>}
    <p>{t('{known} of {count} model requests have actual usage.',{known:usage.knownRequests,count:usage.requests})}</p>
    {(usage.workflowRequests>0||usage.compactionRequests>0||usage.failedRequests>0)&&<p>{t('Includes {workflow} background continuations, {compaction} context compactions and {failed} failed requests.',{workflow:usage.workflowRequests,compaction:usage.compactionRequests,failed:usage.failedRequests})}</p>}
    {usage.maxTools!==null&&<p>{t('At most {count} tools were sent in one model request.',{count:usage.maxTools})}</p>}
    {usage.status!=='complete'&&known&&<p>{t('Only known usage is shown; incomplete or older receipts may be missing.')}</p>}
    <p>{t('Cached input and reasoning are subsets of the totals. Tokens are not a bill.')}</p>
  </Popover>;
}
