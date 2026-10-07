import React from 'react';
import { ListChecks, PanelRightClose, Pause, Play, Trash2, CheckCircle2, CircleAlert, Circle, ArrowUpRight } from 'lucide-react';
import { Button, Disclosure, Badge, Progress, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
export const GOAL_STATUS_LABELS = {
  active:'Working toward your goal', waiting_input:'Waiting for your choices', waiting_confirmation:'Waiting for task confirmation',
  waiting_jobs:'Waiting for background results', needs_attention:'Goal needs attention', paused:'Goal paused', complete:'Goal verified complete', budget_limited:'Goal stopped at its limit',
};
const OUTPUT_LABELS={missing:'Not prepared',waiting:'Awaiting execution or results',failed:'Verification failed',verified:'Verified',unavailable:'Not supported yet'};
const STEP_LABELS={project:'Create imagery project',download:'Download source imagery',mosaic:'Process project imagery',clip:'Crop imagery',rgb:'Create scientific RGB',vector:'Extract vector data'};
function executionStep(plan){
  if(plan.status==='pending'&&plan.areaCoverage&&plan.areaCoverage.status!=='complete')return {state:'failed',label:plan.areaCoverage.status==='unknown'?'Area coverage not verified':'Area coverage incomplete'};
  if(plan.status==='pending')return {state:'waiting',label:'Awaiting confirmation'};
  if(plan.status==='expired')return {state:'failed',label:'Expired'};
  if(plan.status==='unavailable')return {state:'failed',label:'Task unavailable'};
  if(plan.kind==='project')return {state:plan.project?.committed?'verified':'waiting',label:plan.project?.committed?'Project saved':'Submitted'};
  if(plan.kind==='vector')return {state:plan.vector?.verified?'verified':'waiting',label:plan.vector?.verified?'File saved':'Submitted'};
  const jobs=plan.jobs??[],completed=jobs.filter(job=>job.status==='succeeded'&&job.settled).length;
  if(jobs.some(job=>job.status==='failed'))return {state:'failed',label:'Tasks need attention'};
  if(jobs.some(job=>['cancelled','interrupted'].includes(job.status)))return {state:'failed',label:'Tasks stopped'};
  if(jobs.length&&completed===jobs.length)return {state:'verified',label:'Execution complete'};
  if(jobs.some(job=>['queued','running'].includes(job.status)))return {state:'working',label:'Tasks in progress',completed,total:jobs.length};
  if(jobs.some(job=>job.status==='succeeded'&&!job.settled))return {state:'working',label:'Finalizing output',completed,total:jobs.length};
  return {state:'waiting',label:'Submitted'};
}
function StateIcon({state}){
  if(state==='working')return <Spinner size={14}/>;
  const Icon=state==='verified'?CheckCircle2:['failed','unavailable'].includes(state)?CircleAlert:Circle;
  return <Icon size={14} aria-hidden="true"/>;
}
export function AgentGoalCard({goal,plans=[],currentActivity,inSidebar=false,onHide,busy=false,controlling=false,onControl,onShowPlan}){
  const {t,number}=useI18n();
  if(!goal)return null;
  const verified=goal.outputs.filter(output=>output.state==='verified').length;
  const related=new Set([...(goal.planIds??[]),...goal.outputs.map(output=>output.planId).filter(Boolean)]);
  const steps=plans.filter(plan=>related.has(plan.planId)&&!plan.replacedBy&&plan.status!=='superseded').map(plan=>({...executionStep(plan),plan}));
  const running=steps.find(step=>step.state==='working');
  const activity=currentActivity??(running?[t(STEP_LABELS[running.plan.kind]??'Task review'),running.total!=null?t('{completed}/{total} files ready',{completed:number(running.completed),total:number(running.total)}):null].filter(Boolean).join(' · '):null);
  const canPause=['active','waiting_jobs','waiting_input','waiting_confirmation'].includes(goal.status);
  return <section className="agent-goal-card" aria-label={t('Plan progress')}>
    <div className="agent-goal-heading"><ListChecks size={16}/><strong>{t('Plan progress')}</strong>{onHide&&<Button className="agent-goal-hide" variant="quiet" size="icon" icon={PanelRightClose} aria-label={t('Hide plan progress')} tooltip={t('Hide plan progress')} onClick={onHide}/>}</div>
    <Badge className="agent-goal-state" tone={goal.status==='complete'?'green':goal.status==='needs_attention'?'red':'blue'}>{t(GOAL_STATUS_LABELS[goal.status])}</Badge>
    <p className="agent-goal-objective" title={goal.objective}>{goal.objective}</p>
    <Progress value={verified} max={goal.outputs.length} aria-label={t('Required output progress')} getValueLabel={()=>t('Required outputs · {verified}/{total} verified',{verified:number(verified),total:number(goal.outputs.length)})}/>
    {activity&&goal.status!=='complete'&&goal.status!=='paused'&&<p className="agent-goal-activity" role="status"><Spinner size={12}/><span>{t('Current action: {action}',{action:activity})}</span></p>}
    <Disclosure defaultOpen={inSidebar} className="agent-goal-disclosure" summary={t('Required outputs · {verified}/{total} verified',{verified:number(verified),total:number(goal.outputs.length)})}>
      <div className="agent-goal-details" tabIndex={0} role="region" aria-label={t('Plan details')}>
      {goal.objective.length>160&&<p className="agent-goal-note">{goal.objective}</p>}
      <h3>{t('Required outputs')}</h3>
      <ol className="agent-goal-outputs">{goal.outputs.map(output=><li key={output.id} data-state={output.state}><StateIcon state={output.state}/><div><span>{output.label}</span><small>{t(OUTPUT_LABELS[output.state])}</small></div></li>)}</ol>
      {steps.length>0&&<><h3>{t('Execution steps')}</h3><ol className="agent-goal-steps">{steps.map(step=><li key={step.plan.planId} data-state={step.state}>
        <Button variant="quiet" size="row" className="agent-goal-step" disabled={!onShowPlan||step.plan.status==='unavailable'} onClick={()=>onShowPlan(step.plan.planId)} aria-label={t('View task: {task}',{task:t(STEP_LABELS[step.plan.kind]??'Task review')})}>
          <StateIcon state={step.state}/><span><strong>{t(STEP_LABELS[step.plan.kind]??'Task review')}</strong><small>{t(step.label)}{step.total!=null&&` · ${t('{completed}/{total} files ready',{completed:number(step.completed),total:number(step.total)})}`}</small></span><ArrowUpRight size={13} aria-hidden="true"/>
        </Button>
      </li>)}</ol></>}
      <p className="agent-goal-note">{t('Completion checks actual files and native results. Unsupported or missing required outputs keep the goal unfinished.')}</p>
      {goal.reason&&<p className="agent-goal-note" role="status">{t(goal.reason)}</p>}
      </div>
      <div className="agent-goal-actions">
        {goal.status!=='complete'&&<Button variant="quiet" size="sm" icon={canPause?Pause:Play} disabled={controlling||!canPause&&busy} onClick={()=>onControl(canPause?'pause':'resume')}>{t(canPause?'Pause goal':'Continue goal')}</Button>}
        <Button variant="quiet" size="sm" icon={Trash2} disabled={busy||controlling} onClick={()=>onControl('clear')}>{t('Clear goal')}</Button>
      </div>
    </Disclosure>
  </section>;
}
