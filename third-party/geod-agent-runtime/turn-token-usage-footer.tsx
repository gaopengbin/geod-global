import { useEffect, useRef, useState } from 'react';
import { api, desktopAvailable } from './api';
import { t, useLocale } from './i18n';
import { Button } from '@/components/motion/button/base';
import { UiTooltip } from './ui-tooltip';
import { ChartNoAxesColumn } from './icons';
import { loadTurnTokenUsage, usageSourceKey, type TurnTokenUsage, type TurnUsageSources } from './turn-token-usage';

export function TurnTokenUsageFooter({conversationId,messageId,source,saved,onUsage}:{conversationId:string;messageId:string;source:TurnUsageSources;saved?:TurnTokenUsage;onUsage?:(id:string,usage:TurnTokenUsage)=>void}) {
  const locale=useLocale(), sourceKey=usageSourceKey(source);
  const [usage,setUsage]=useState(saved?.sourceKey===sourceKey?saved:undefined);
  const [attempt,setAttempt]=useState(0), [loading,setLoading]=useState(false);
  const notify=useRef(onUsage);notify.current=onUsage;
  useEffect(()=>{
    if(saved?.sourceKey===sourceKey&&saved.status==='complete'){setUsage(saved);return;}
    if(!desktopAvailable)return;
    let cancelled=false;
    setLoading(true);
    void loadTurnTokenUsage(source,conversationId,api).then(value=>{
      if(cancelled)return;
      setUsage(value);notify.current?.(messageId,value);
    }).finally(()=>{if(!cancelled)setLoading(false);});
    return()=>{cancelled=true;};
  },[conversationId,messageId,sourceKey,attempt]);
  useEffect(()=>{
    // Bounded reconciliation for a late settlement, not polling during model execution.
    if(!desktopAvailable||loading||attempt>=2||!usage||!usage.pendingRequests&&usage.status!=='pending')return;
    const timer=window.setTimeout(()=>setAttempt(value=>value+1),attempt?10000:3000);
    return()=>window.clearTimeout(timer);
  },[usage,loading,attempt]);
  const value=usage?.sourceKey===sourceKey?usage:undefined;
  const format=(n:number)=>n.toLocaleString(locale);
  const hasCounts=value&&(value.knownRequests>0||value.status==='complete');
  const statusLabel=t(loading||!value||value.status==='pending'?'用量待核对':'用量不可用');
  const details=<div className="agent-token-usage-tooltip">
    <strong className="agent-token-usage-heading">{hasCounts?t(value.status==='complete'?'本轮消耗':'已知用量'):statusLabel}</strong>
    {hasCounts&&<>
      <div className="agent-token-usage-total"><strong>{format(value.totalTokens)}</strong><span>tokens</span></div>
      <dl className="agent-token-usage-breakdown">
        <div><dt>{t('输入')}</dt><dd>{format(value.inputTokens)}</dd></div>
        <div><dt>{t('输出')}</dt><dd>{format(value.outputTokens)}</dd></div>
        <div><dt>{t('缓存输入')}</dt><dd>{value.cachedInputTokens!=null?format(value.cachedInputTokens):t('不可用')}</dd></div>
        <div><dt>{t('推理')}</dt><dd>{value.reasoningTokens!=null?format(value.reasoningTokens):t('不可用')}</dd></div>
      </dl>
      <p>{t('累计 {0} 次模型请求，{1} 次已有实际用量。',{'0':format(value.requests),'1':format(value.knownRequests)})}</p>
      <p className="agent-token-usage-note">{t('缓存已包含在输入中，推理已包含在输出中。')}</p>
    </>}
    {!hasCounts&&<p>{t(loading||!value?'正在读取本轮实际用量。':'尚未取得本轮实际用量。')}</p>}
    {value&&value.status!=='complete'&&<p className="agent-token-usage-note">{hasCounts&&t('部分请求尚未取得实际用量，总量可能增加。')}{t('点击统计图标可重新核对用量。')}</p>}
  </div>;
  return <UiTooltip content={details} align="start"><Button type="button" variant="ghost" size="sm" className="agent-turn-token-usage" aria-label={t('本轮 Token 用量')} aria-busy={loading} data-usage-status={value?.status??'pending'} whileHover={undefined} whileTap={undefined} onClick={()=>{if(!loading&&value?.status!=='complete')setAttempt(attempt=>attempt+1);}}><ChartNoAxesColumn size={16}/></Button></UiTooltip>;
}
