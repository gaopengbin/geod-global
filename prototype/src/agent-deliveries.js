// Only native references from the selected conversation may become deliveries.
// Assistant text, other conversations and unfinished jobs are never file links.
const ready = job => job.status === 'succeeded' && job.settled === true;
export function conversationPlans(session, plans = []) {
  const ids = new Set([
    ...(session?.goal?.planIds ?? []),
    ...(session?.goal?.outputs ?? []).map(output => output.planId),
    ...(session?.entries ?? []).filter(entry => entry.type === 'tool').flatMap(entry =>
      (entry.references ?? []).filter(reference => reference.kind === 'plan').map(reference => reference.id)),
  ]);
  return plans.filter(plan => ids.has(plan.planId));
}
export function conversationDeliveries(session, plans = []) {
  const related = conversationPlans(session, plans);
  const items = new Map();
  const add = item => {
    const key = `${item.kind}:${item.id}`;
    if(item.kind==='raster'&&items.has(`archive:${item.id}`))return;
    if (!items.has(key)) items.set(key, {...item, key});
  };
  const required = new Set((session?.goal?.outputs ?? []).filter(output => output.state === 'verified').flatMap(output => output.verifiedIds ?? []));
  const hasDerived = related.some(plan => ['clip','mosaic','rgb'].includes(plan.kind) && (plan.jobs ?? []).some(ready));
  for (const plan of related.filter(plan => plan.status === 'submitted')) {
    if (plan.kind === 'vector' && plan.vector?.verified) add({kind:'vector', id:plan.vector.id, label:plan.vectorReview.name, bytes:plan.vector.bytes, count:plan.vector.featureCount, source:plan.source, planId:plan.planId});
    for (const job of (plan.jobs ?? []).filter(ready)) {
      const file = plan.files?.[0];
      add({kind:plan.archive?'archive':'raster', id:job.id, label:job.title, bytes:job.bytesDownloaded, sha256:job.sha256,
        assetKey:plan.kind === 'rgb' ? 'reflectance_rgb' : file?.assetKey,
        format:plan.format ?? 'GeoTIFF', source:plan.source, width:file?.width, height:file?.height,
        planId:plan.planId, projectId:plan.project?.id,
        packageKind:plan.kind === 'rgb' ? 'raster_rgb' : plan.kind === 'clip' ? 'raster_clip' : undefined,
        intermediate:plan.kind === 'download' && (required.size ? !required.has(job.id) : hasDerived),
      });
    }
    if (plan.kind === 'project' && plan.project?.committed) add({kind:'project', id:plan.project.id, label:plan.project.name, source:plan.source, planId:plan.planId});
  }
  for (const entry of session?.entries ?? []) {
    if (entry.type !== 'tool' || entry.status !== 'completed') continue;
    for (const reference of entry.references ?? []) {
      if (reference.kind === 'job' && (entry.summary?.kind === 'file' || entry.summary?.kind === 'job' && ready(entry.summary))) {
        add({kind:'raster', id:reference.id, label:reference.label, sha256:entry.summary.sha256});
      }
      if (reference.kind === 'vector' && entry.summary?.kind === 'vector' && entry.summary.verified) {
        add({kind:'vector', id:reference.id, label:reference.label, count:entry.summary.count});
      }
    }
  }
  if (session?.workspaceView?.verified) {
    const view = session.workspaceView;
    add({kind:view.kind === 'vector' ? 'vector' : view.kind === 'archive' ? 'archive' : 'raster', id:view.id, label:'Verified result'});
  }
  return [...items.values()].sort((a,b) => Number(Boolean(a.intermediate)) - Number(Boolean(b.intermediate)) || Number(a.kind === 'project') - Number(b.kind === 'project'));
}
