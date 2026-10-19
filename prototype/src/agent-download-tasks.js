// Display-only projection. Native plans and job IDs continue to own execution.
export function downloadWork(entries = [], plans = []) {
  const byId=new Map(plans.map(plan=>[plan.planId,plan])),downloads=new Map();
  const remaining=entries.flatMap(entry=>{
    if(entry.type!=='reviews')return [entry];
    const references=entry.references.filter(ref=>{
      const plan=byId.get(ref.id);
      if(plan?.kind!=='download'||plan.status!=='submitted')return true;
      downloads.set(plan.planId,plan);return false;
    });
    return references.length?[{...entry,references}]:[];
  });
  return {entries:remaining,plans:[...downloads.values()]};
}

export function downloadTaskSummary(plans = []) {
  const jobs=[...new Map(plans.flatMap(plan=>plan.jobs??[]).map(job=>[job.id,job])).values()];
  const missing=plans.filter(plan=>!plan.jobs?.length).reduce((count,plan)=>count+(plan.files?.length??0),0);
  const ready=jobs.filter(job=>job.status==='succeeded'&&job.settled===true).length;
  const failed=jobs.filter(job=>['failed','interrupted'].includes(job.status));
  const running=jobs.filter(job=>job.status==='running').length;
  const queued=jobs.filter(job=>job.status==='queued').length;
  const finalizing=jobs.some(job=>job.status==='succeeded'&&job.settled!==true);
  const complete=jobs.length>0&&missing===0&&ready===jobs.length;
  const measured=jobs.length>0&&missing===0&&jobs.every(job=>Number.isFinite(job.totalBytes)&&job.totalBytes>0);
  const transferred=jobs.reduce((sum,job)=>sum+(Number.isFinite(job.bytesDownloaded)?Math.max(0,job.bytesDownloaded):0),0);
  const totalBytes=measured?jobs.reduce((sum,job)=>sum+job.totalBytes,0):null;
  const progressBytes=measured?jobs.reduce((sum,job)=>sum+Math.min(job.totalBytes,Math.max(0,job.bytesDownloaded??0)),0):null;
  const status=failed.length?'Needs attention':complete?'Execution complete':running?'Tasks in progress':finalizing?'Finalizing output':queued?'Queued task':jobs.some(job=>job.status==='cancelled')?'Tasks stopped':'Submitted';
  return {jobs,total:jobs.length+missing,ready,failed,running,queued,complete,transferred,totalBytes,progressBytes,status,
    needsAttention:failed.length>0,unfinished:!complete};
}
