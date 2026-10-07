// Native jobs own the work. This observer wakes the same Codex conversation
// after a state change; it never polls the model while downloads are running.
const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const STATES = ['waiting', 'ready', 'paused', 'failed', 'completed'];
export const WORKFLOW_LIMITS = Object.freeze({ plans: 10, continuations: 8, intervalMs: 2500 });
const timestamp = value => typeof value === 'string' && value.length <= 64 && Number.isFinite(Date.parse(value));
const keys = (value, allowed) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).every(key => allowed.includes(key));
function validReceipt(value) {
  return keys(value, ['planId', 'kind', 'status', 'jobs', 'projectId', 'vectorId', 'hasFailure'])
    && UUID.test(value.planId) && ['project', 'download', 'clip', 'mosaic', 'rgb', 'vector'].includes(value.kind) && value.status === 'settled'
    && Array.isArray(value.jobs) && value.jobs.length <= 32 && value.jobs.every(job => keys(job, ['id', 'status', 'settled', 'bytesDownloaded'])
      && UUID.test(job.id) && ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.status) && job.settled === true
      && Number.isSafeInteger(job.bytesDownloaded) && job.bytesDownloaded >= 0)
    && (value.projectId === undefined || UUID.test(value.projectId)) && (value.vectorId === undefined || UUID.test(value.vectorId))
    && value.hasFailure === value.jobs.some(job => job.status !== 'succeeded');
}
export function validWorkflow(value) {
  return keys(value, ['version', 'status', 'pendingPlans', 'receipts', 'context', 'continuations', 'updatedAt'])
    && value.version === 1 && STATES.includes(value.status)
    && Array.isArray(value.pendingPlans) && value.pendingPlans.length <= WORKFLOW_LIMITS.plans && value.pendingPlans.every(id => UUID.test(id))
    && new Set(value.pendingPlans).size === value.pendingPlans.length && Number.isSafeInteger(value.continuations)
    && value.continuations >= 0 && value.continuations <= WORKFLOW_LIMITS.continuations && timestamp(value.updatedAt)
    && Array.isArray(value.receipts) && value.receipts.length <= WORKFLOW_LIMITS.plans && value.receipts.every(validReceipt)
    && (value.context === null || keys(value.context, ['page', 'provider', 'bounds', 'start', 'end', 'cloudMax', 'projectId', 'geometry'])
      && JSON.stringify(value.context).length <= 100_000);
}
export function publicWorkflow(value) {
  return value ? { status: value.status, waitingPlans: value.pendingPlans.length, continuations: value.continuations, updatedAt: value.updatedAt } : null;
}
export class WorkflowMonitor {
  constructor(service, { intervalMs = WORKFLOW_LIMITS.intervalMs } = {}) {
    this.service = service; this.intervalMs = intervalMs; this.timer = null; this.polling = false; this.closed = false;
  }
  schedule() {
    if (this.closed || this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.tick().catch(async () => {
        for (const session of this.service.sessions) if (['waiting', 'ready'].includes(session.workflow?.status)) await this.pause(session);
      }).catch(() => {}).finally(() => this.schedule());
    }, this.intervalMs);
    this.timer.unref?.();
  }
  async register(session, plan, context) {
    if (!UUID.test(plan?.planId) || plan.status !== 'submitted') return;
    const workflow = session.workflow ?? { version: 1, status: 'waiting', pendingPlans: [], receipts: [], context: null, continuations: 0, updatedAt: new Date().toISOString() };
    if (!workflow.pendingPlans.includes(plan.planId)) {
      if (workflow.pendingPlans.length >= WORKFLOW_LIMITS.plans) throw Error('This workflow reached its task limit. Continue explicitly in chat.');
      workflow.pendingPlans.push(plan.planId);
    }
    workflow.status = 'waiting'; workflow.context = structuredClone(context ?? workflow.context); workflow.updatedAt = new Date().toISOString(); session.workflow = workflow;
    await this.service.save(); this.schedule();
  }
  async pause(session) {
    if(session?.goal && session.goal.status!=='paused')await this.service.goals.pause(session);
    if (session?.workflow && session.workflow.status !== 'completed') {
      session.workflow.status = 'paused'; session.workflow.updatedAt = new Date().toISOString(); await this.service.save();
    }
  }
  async resume(session) {
    if (!session?.workflow) throw Error('There is no background workflow in this conversation.');
    session.workflow.status = session.workflow.pendingPlans.length ? 'waiting' : 'ready';
    // A new explicit human request renews the bounded continuation allowance.
    session.workflow.continuations = 0; session.workflow.updatedAt = new Date().toISOString(); await this.service.save(); this.schedule();
  }
  async tick() {
    if (this.polling || this.closed || this.service.closing || !this.service.config) return;
    this.polling = true;
    try {
      for (const session of this.service.sessions) {
        const workflow = session.workflow;
        if(session.goal?.status==='active' && !workflow?.pendingPlans.length && !workflow?.receipts.length && this.service.selectedId===session.id && this.service.compatible(session)
          && !this.service.active && !this.service.preparing){
          if(session.goal.continuations>=WORKFLOW_LIMITS.continuations){session.goal.status='needs_attention';session.goal.reason='Automatic continuation reached its limit. Continue explicitly to review remaining outputs.';await this.service.goals.pause(session);session.goal.status='needs_attention';await this.service.save();continue;}
          const permission=await this.service.callTool('geod_execution_policy',{}, {sessionId:session.id,context:null}).catch(()=>null);
          if(!permission || !['confirm-each','full-access'].includes(permission.mode)){await this.service.goals.pause(session);continue;}
          if(this.service.active||this.service.preparing||session.goal.status!=='active'||this.service.selectedId!==session.id)continue;
          session.goal.continuations++;await this.service.save();
          try{await this.service.continueWorkflow(session,[],permission.mode,workflow?.context??null);}catch{await this.service.goals.pause(session);}
          continue;
        }
        if (!workflow || !['waiting', 'ready'].includes(workflow.status) || !this.service.compatible(session)) continue;
        for (const planId of [...workflow.pendingPlans]) {
          let plan;
          try { plan = await this.service.callTool('geod_plan_status', { planId }, { sessionId: session.id, context: null }); }
          catch {
            await this.pause(session);
            await this.service.desktopNotice(session, 'The workflow paused because its native task could not be read. Continue explicitly in chat.');
            await this.service.save(); break;
          }
          if (plan.planId !== planId || plan.status !== 'submitted' || !Array.isArray(plan.jobs)) {
            await this.pause(session); break;
          }
          if (plan.kind === 'vector' && plan.vector?.verified !== true) continue;
          if (plan.jobs.some(job => !job.settled || ['queued', 'running'].includes(job.status))) continue;
          const receipt = { planId, kind: plan.kind, status: 'settled', jobs: plan.jobs.map(job => ({ id: job.id, status: job.status, settled: job.settled, bytesDownloaded: job.bytesDownloaded })),
            ...(plan.project?.id ? { projectId: plan.project.id } : {}), ...(plan.vector?.verified ? { vectorId: plan.vector.id } : {}), hasFailure: plan.jobs.some(job => job.status !== 'succeeded') };
          if (!validReceipt(receipt)) { await this.pause(session); break; }
          workflow.pendingPlans = workflow.pendingPlans.filter(id => id !== planId);
          workflow.receipts = [...workflow.receipts.filter(receipt => receipt.planId !== planId), receipt].slice(-WORKFLOW_LIMITS.plans);
          workflow.updatedAt = new Date().toISOString(); await this.service.save();
        }
        if (!['waiting', 'ready'].includes(workflow.status) || workflow.pendingPlans.length || this.service.active || this.service.preparing || this.service.selectedId !== session.id || !workflow.receipts.length) continue;
        if (workflow.continuations >= WORKFLOW_LIMITS.continuations || session.goal?.continuations >= WORKFLOW_LIMITS.continuations) {
          await this.pause(session);
          if(session.goal){session.goal.status='needs_attention';session.goal.reason='Automatic continuation reached its limit. Continue explicitly to review remaining outputs.';}
          await this.service.desktopNotice(session, 'Automatic continuation reached its limit. Continue explicitly in chat.'); await this.service.save(); continue;
        }
        const permission = await this.service.callTool('geod_execution_policy', {}, { sessionId: session.id, context: null }).catch(() => null);
        if (!permission || !['confirm-each', 'full-access'].includes(permission.mode)) { await this.pause(session); continue; }
        if (!['waiting', 'ready'].includes(workflow.status) || this.service.active || this.service.preparing || this.service.selectedId !== session.id) continue;
        const receipts = workflow.receipts; workflow.receipts = []; workflow.continuations++;
        if(session.goal)session.goal.continuations++;
        workflow.status = receipts.some(receipt => receipt.hasFailure) ? 'failed' : 'ready'; workflow.updatedAt = new Date().toISOString();
        try {
          await this.service.save(); await this.service.continueWorkflow(session, receipts, permission.mode, workflow.context);
        } catch {
          workflow.receipts = receipts; workflow.continuations--; if(session.goal)session.goal.continuations--; await this.pause(session);
          await this.service.desktopNotice(session, 'The workflow paused before continuing. Your completed tasks remain saved. Continue explicitly in chat.'); await this.service.save();
        }
      }
    } finally { this.polling = false; }
  }
  async turnFinished(session, status) {
    const workflow = session.workflow; if (!workflow) return;
    if (status !== 'completed') { await this.pause(session); return; }
    if (!['paused', 'failed'].includes(workflow.status)) workflow.status = workflow.pendingPlans.length ? 'waiting' : workflow.receipts.length ? 'ready' : 'completed';
    workflow.updatedAt = new Date().toISOString(); await this.service.save(); this.schedule();
  }
  close() { this.closed = true; clearTimeout(this.timer); this.timer = null; }
}
