// Ported from GeoD Agent 0.2.5 chat-work.ts at 693d6a5.
// Retained source/license: licenses/GeoD-Agent-Chat-MIT.txt.
export function uniqueDisplayMessages(messages = []) {
  const records = new Map();
  for (const item of messages) records.set(item.id, item);
  return [...records.values()];
}
// Native read contracts, not prose or a guessed classification of tool names.
// Reviews, execution, decisions and goal mutations deliberately stay separate.
const explorationTools = new Set([
  'geod_health','geod_sources_list','geod_jobs_list','geod_job_status',
  'geod_projects_list','geod_project_get','geod_recipes_list','geod_plan_status',
  'geod_place_search','geod_workspace_context','geod_region_search','geod_region_levels',
  'geod_boundary_read','geod_scene_search','geod_scene_search_more','geod_scene_coverage',
  'geod_catalog_selection','geod_raster_inspect','geod_raster_pixel','geod_rgb_inspect','geod_rgb_pixel',
  'geod_feature_services','geod_feature_collections','geod_vectors_list','geod_vector_inspect',
  'geod_vector_features','geod_vector_node','geod_stac_connections','geod_stac_catalog',
  'geod_stac_snapshot','geod_stac_assets','geod_stac_inspect','geod_stac_pixel','geod_stac_search',
  'geod_wcs_connections','geod_wcs_coverages','geod_wcs_description','geod_wcs_inspect','geod_wcs_pixel',
  'geod_execution_policy','geod_official_services','geod_monitor_list','geod_monitor_notifications',
  'geod_dataset_catalog','geod_dataset_resolve',
]);
export function projectWorkActivities(entries = []) {
  const activities = [];
  for (const entry of uniqueDisplayMessages(entries)) {
    const kind = entry.type === 'reviews' || entry.decision ? 'attachment'
      : entry.type === 'assistant' || entry.type === 'system' ? 'note'
      : explorationTools.has(entry.name) ? 'exploration' : entry.type === 'tool' ? 'tools' : 'record';
    const last = activities.at(-1);
    if (['exploration','tools'].includes(kind) && last?.kind === kind) last.entries.push(entry);
    else activities.push({id:entry.id,kind,entries:[entry]});
  }
  return activities;
}
export function workRecordState(entry, active) {
  if (['failed','interrupted'].includes(entry.status) || !active && entry.status === 'running') return 'attention';
  if (active && entry.status === 'running') return 'running';
  return entry.status === 'completed' ? 'success' : 'recorded';
}
const continuationNotice = entry => entry.type === 'system' && entry.origin === 'desktop'
  && /^(Background tasks settled\.|Some background tasks need attention\.|Resuming your request\.|Plans submitted to the native task system\.)/.test(entry.text ?? '');

function requestState(request, state, latest) {
  if (!latest) return {backgroundRunning:false};
  const {entries,user} = request;
  const planIds = new Set(entries.flatMap(entry => (entry.references ?? []).filter(ref => ref.kind === 'plan').map(ref => ref.id)));
  const related = (state.plans ?? []).filter(plan => planIds.has(plan.planId));
  const goalOwned = !state.goal?.requestText || state.goal.requestText === user?.text
    || state.goal.planIds?.some(id=>planIds.has(id));
  const workflowOwned = related.some(plan=>plan.status === 'submitted') || entries.some(continuationNotice) || Boolean(state.goal && goalOwned);
  const waitingForUser = entries.some(entry => entry.decision?.status === 'pending')
    || related.some(plan => plan.status === 'pending') || goalOwned && state.goal?.status === 'waiting_confirmation';
  const stopped = ['failed','interrupted'].includes(state.status) || Boolean(state.error)
    || workflowOwned && ['paused','failed'].includes(state.workflow?.status) || goalOwned && ['paused','needs_attention'].includes(state.goal?.status);
  const backgroundRunning = !stopped && !waitingForUser && (
    workflowOwned && ['waiting','ready'].includes(state.workflow?.status)
    || goalOwned && ['active','waiting_jobs'].includes(state.goal?.status)
    || related.some(plan => plan.status === 'submitted' && plan.jobs?.some(job => !job.settled || ['queued','running'].includes(job.status)))
  );
  // Goal verification can finish before the model finishes streaming its
  // delivery summary. Native job receipts, rather than text, own completion.
  const verified = goalOwned && state.goal?.status === 'complete' && state.goal.outputs?.every(output => output.state === 'verified')
    && !related.some(plan => plan.jobs?.some(job => !job.settled || ['queued','running'].includes(job.status)));
  return {backgroundRunning:backgroundRunning && !verified};
}

export function transcriptRows(messages = [], activity, state = {}) {
  const entries = uniqueDisplayMessages(messages), rows = [], requests = [];
  const latestPlans = new Map();
  for (const item of entries) for (const ref of item.references ?? []) {
    if (ref.kind === 'plan') latestPlans.set(ref.id, item.id);
  }
  let request;
  for (const entry of entries) {
    if (entry.type === 'user' || !request) {
      request = {id:entry.id,user:entry.type==='user'?entry:null,entries:[]};
      requests.push(request);
    }
    if (entry.type !== 'user') request.entries.push(entry);
  }
  for (const [requestIndex, request] of requests.entries()) {
    if (request.user) rows.push(request.user);
    const {entries} = request;
    const lifecycle = requestState(request,state,requestIndex === requests.length - 1);
    const progress = (entry,index) => entry.type === 'assistant' && (
      ['progress','commentary'].includes(entry.phase)
      || entry.phase == null && activity?.kind === 'turn' && entry.turnId === activity.turnId
      || entry.phase == null && entries[index+1]?.type === 'tool' && entries[index+1].origin !== 'desktop' && entries[index+1].name !== 'geod_plan_execute'
    );
    const answerIndex = entries.findLastIndex((entry,index) => entry.type === 'assistant' && !progress(entry,index));
    const laterActivity = entries.slice(answerIndex+1).some(entry => entry.type === 'tool' || entry.type === 'assistant' || continuationNotice(entry));
    const reply = answerIndex >= 0 && !laterActivity && !lifecycle.backgroundRunning ? entries[answerIndex] : null;
    const turnIds = [...new Set(entries.map(entry=>entry.turnId).filter(Boolean))];
    const group = {id:`work-request-${request.id}`,type:'work',requestId:request.id,turnId:turnIds.at(-1),turnIds,entries:[],...lifecycle};
    for (const [index,entry] of entries.entries()) {
      const references=(entry.references??[]).filter(ref=>ref.kind==='plan'&&latestPlans.get(ref.id)===entry.id);
      if (entry === reply) {
        group.finalId=entry.id;
        if(references.length)group.entries.push({id:`reviews-${entry.id}`,type:'reviews',entry,references});
        continue;
      }
      // Informational settings notices after a reply retain their chronological
      // position. Automatic workflow wakeups belong to this request's activity.
      if (reply && index > answerIndex && entry.type === 'system') continue;
      group.entries.push(entry);
      if(references.length)group.entries.push({id:`reviews-${entry.id}`,type:'reviews',entry,references});
    }
    if(group.entries.length)rows.push(group);
    if(reply){rows.push(reply);rows.push(...entries.slice(answerIndex+1));}
  }
  return rows;
}
export function workEntryNeedsAttention(entry, plans = []) {
  if (entry.decision) return true;
  if (entry.type !== 'reviews') return false;
  return entry.references.some(ref => {
    const plan = plans.find(plan => plan.planId === ref.id);
    return plan && (plan.status !== 'submitted' || plan.jobs?.some(job => job.status !== 'succeeded' || job.settled !== true));
  });
}
export function activeWorkId(rows, busy, activity) {
  const last = rows.findLast(row=>row.type==='work');
  if (!last || rows.slice(rows.indexOf(last)+1).some(row=>row.type==='user')) return undefined;
  if (last.backgroundRunning) return last.id;
  if (!busy) return undefined;
  if(activity!==undefined){
    if(activity?.kind!=='turn'||!activity.turnId)return undefined;
    return last.turnIds.includes(activity.turnId) ? last.id : undefined;
  }
  for (let index = rows.length - 1; index >= 0; index--) {
    const row = rows[index];
    if (row.type === 'reviews' || row.decision) continue;
    if (row.type === 'work') return row.id;
    if (row.type === 'user' || row.type === 'assistant' && row.status !== 'running') return undefined;
  }
  return undefined;
}
