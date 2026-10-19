export function deliveryState(jobs = [], expectedIds) {
  const ids=expectedIds?new Set(expectedIds):null;
  const rows=[...new Map(jobs.filter(job=>!ids||ids.has(job.id)).map(job=>[job.id,job])).values()];
  const total=ids?ids.size:rows.length;
  const done=rows.filter(job=>job.status==='succeeded'&&job.settled!==false).length;
  const active=rows.filter(job=>['queued','running'].includes(job.status)).length;
  const attention=rows.filter(job=>['failed','interrupted','cancelled'].includes(job.status)).length;
  const missing=Math.max(0,total-rows.length);
  return {rows,total,done,active,attention,missing,
    status:total>0&&done===total?'ready':active?'active':attention?(done?'partial':'attention'):total?'pending':'saved'};
}
export const taskHref = ({projectId,jobId}={}) => '#Tasks'+(jobId?'?job='+encodeURIComponent(jobId):projectId?'?project='+encodeURIComponent(projectId):'');
