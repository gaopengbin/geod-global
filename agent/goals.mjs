import { randomUUID } from 'node:crypto';
import { pendingDecision } from './decisions.mjs';

import { UUID, KEY, HASH, KINDS, object, text, schema, validGoalInput, validEngineGoal, publicGoal } from './goal-contract.mjs';
export { validGoal, validEngineGoal, publicGoal } from './goal-contract.mjs';
export const GOAL_TOOLS = Object.freeze([
  { name: 'geod_goal_define', description: 'For a human request to acquire, prepare or deliver data, save the whole objective and ALL required outputs before creating plans. Do not use for greetings, explanation or status-only questions. Include unavailable requirements rather than silently dropping them. This sets a real Codex thread goal; it never approves execution. Each output is one complete native plan or one complete metadata read, not an individual job. Changes require a new human request; automatic continuations cannot replace the manifest.',
    inputSchema: schema({ objective: { type: 'string', minLength: 1, maxLength: 1200 }, outputs: { type: 'array', minItems: 1, maxItems: 16, items: schema({ id: { type: 'string', pattern: KEY.source }, label: { type: 'string', minLength: 1, maxLength: 240 }, kind: { type: 'string', enum: KINDS } }, ['id', 'label', 'kind']) } }, ['objective', 'outputs']) },
  { name: 'geod_goal_bind', description: 'Bind a required output to an actual native plan recorded in this conversation, or a completed metadata read entry. Bind download/processing outputs to the final delivery plan, not its input download. The goal checks every job in that plan and verifies actual files. Binding is not proof of completion and never approves a plan. Superseded reviews require the replacement plan.',
    inputSchema: schema({ outputId: { type: 'string', pattern: KEY.source }, planId: { type: 'string', format: 'uuid' }, entryId: { type: 'string', format: 'uuid' } }, ['outputId']) },
  { name: 'geod_goal_check', description: 'Read fresh native plan status and verify every required output. Returns missing, waiting, failed and verified results separately. Only all required outputs passing their native checks can complete the Codex goal. File verification does not establish scientific accuracy, full geographic coverage or product entitlement.', inputSchema: schema({}, []) },
]);
export const GOAL_INSTRUCTIONS = `For an actual data acquisition, processing or delivery request, call geod_goal_define before preparing plans. Preserve the complete human objective and list all required deliveries, including unsupported statistics or formats as unavailable. Do not create goals for explanation, greetings or simple status questions. After preparing a plan bind its actual planId to the matching required output with geod_goal_bind. Input downloads are not the final processed output. One output binds one entire native plan; do not omit files by binding its first child job. A project output proves saved metadata only. Metadata outputs bind completed native read entry IDs and require exhausted pages. Necessary decisions still use geod_request_decision; native download confirmation still applies. Run geod_goal_check before claiming the whole task complete. Native results decide completion; assistant text, plan approval and a completed reply do not. Automatic continuations must retain the same manifest and execute only remaining original work. Unsupported requirements stay visible and prevent a complete status. Native goal controls never grant download permission.`;
const auditBinding=goal=>JSON.stringify([goal.id,goal.planIds,goal.outputs.map(({id,kind,planId,entryId})=>({id,kind,planId,entryId}))]);

// Codex owns persisted goal lifecycle and usage. GeoD owns artifact checks and
// dispatches through the existing single conversation/workflow coordinator.
export class GoalCoordinator {
  constructor(service) { this.service = service; }
  async sync(session, status) {
    if (!session.goal || !this.service.host || !session.threadId) return;
    const goal=session.goal;
    const result = await this.service.host.rpc('thread/goal/set', { threadId: session.threadId, objective: goal.objective, status });
    if (!validEngineGoal(result?.goal, session.threadId) || result.goal.objective !== goal.objective || result.goal.status !== status) throw Error('Codex returned invalid goal state.');
    if(session.goal===goal)goal.engine = result.goal;
  }
  async restore(session) {
    if (!session.goal) return;
    const result = await this.service.host.rpc('thread/goal/get', { threadId: session.threadId });
    if (result?.goal != null && !validEngineGoal(result.goal, session.threadId)) throw Error('Codex returned invalid goal state.');
    if (result?.goal?.objective === session.goal.objective) session.goal.engine = result.goal;
    await this.sync(session, session.goal.status === 'complete' ? 'complete' : session.goal.status === 'active' ? 'active' : 'paused');
  }
  async define(active, input) {
    if (!validGoalInput(input) || active.origin === 'workflow' || active.goalDefined || !text(active.humanText, 8000)) throw Error('Define the complete goal once for an actual human task request.');
    const previous = active.session.goal;
    input={objective:this.service.redact(input.objective),outputs:input.outputs.map(output=>({...output,label:this.service.redact(output.label)}))};
    active.session.goal = { version: 1, id: randomUUID(), ...structuredClone(input), requestText: active.humanText, planIds:[],
      outputs: input.outputs.map(output => ({ ...output, planId: null, entryId: null, state: output.kind === 'unavailable' ? 'unavailable' : 'missing', verifiedIds: [] })),
      status: 'active', engine: null, continuations: 0, updatedAt: new Date().toISOString(), checkedAt: null, reason: null };
    try { await this.sync(active.session, 'active'); await this.service.save(); }
    catch (error) {
      active.session.goal = previous;
      try{if(previous)await this.sync(active.session,'paused');else await this.service.host.rpc('thread/goal/clear',{threadId:active.session.threadId});}
      catch{await this.service.closeRuntime();}
      throw error;
    }
    active.goalDefined = true; active.goalProgress = true;
    return { goal: publicGoal(active.session.goal), entryId: active.session.entries.at(-1)?.id, note: 'Goal saved. Outputs are not complete; native plans and verification are still required.' };
  }
  async bind(active, input) {
    const goal = active.session.goal;
    if (!goal || goal.status === 'complete' || !object(input, ['outputId', 'planId', 'entryId']) || !KEY.test(input.outputId)
      || Boolean(input.planId) === Boolean(input.entryId) || !UUID.test(input.planId ?? input.entryId)) throw Error('Bind an actual goal output to one native reference.');
    const output = goal.outputs.find(output => output.id === input.outputId);
    if (!output || output.kind === 'unavailable') throw Error('Choose an available required goal output.');
    if(goal.outputs.some(other=>other!==output&&(input.planId&&other.planId===input.planId||input.entryId&&other.entryId===input.entryId)))throw Error('Each required delivery needs its own native reference. Combine duplicate requirements instead of counting the same result twice.');
    if (input.planId) {
      if (output.kind === 'metadata' || !active.session.entries.some(entry => entry.status === 'completed' && entry.references?.some(ref => ref.kind === 'plan' && ref.id === input.planId))) throw Error('Use a native plan recorded in this conversation.');
      const plan = await this.service.callTool('geod_plan_status', { planId: input.planId }, { sessionId: active.session.id, context: null });
      if (plan.planId !== input.planId || plan.replacedBy || (output.kind === 'project') !== (plan.kind === 'project') || (output.kind === 'vector') !== (plan.kind === 'vector') || (output.kind === 'rgb') !== (plan.kind === 'rgb')) throw Error('Choose the matching current delivery plan, not a superseded or different output.');
    } else {
      const entry = active.session.entries.find(entry => entry.id === input.entryId);
      if (output.kind !== 'metadata' || !entry || entry.type !== 'tool' || entry.status !== 'completed' || !entry.metadataComplete) throw Error('Use a completed, exhausted native metadata read.');
    }
    const previous=structuredClone(goal);
    Object.assign(output, { planId: input.planId ?? null, entryId: input.entryId ?? null, state: 'missing', verifiedIds: [] });
    goal.status = 'active'; goal.checkedAt = null; goal.updatedAt = new Date().toISOString(); active.goalProgress = true;
    try{await this.service.save();}catch(error){active.session.goal=previous;throw error;}
    return { output: structuredClone(output), note: 'Bound to native data. Fresh result verification is still required.' };
  }
  async check(session) {
    const original = session.goal;
    if (!original) return { goal: null };
    // Never expose partially reset verified IDs while native file checks await.
    // Snapshots retain the last complete check until all outputs are audited.
    const goal=structuredClone(original),binding=auditBinding(original);
    const scope = { sessionId: session.id, context: null, readOnly: true };
    goal.reason=null;
    let waitingConfirmation = false, waitingJobs = false;
    // Intermediate project/download reviews can be necessary even before the
    // final delivery plan exists. Wait for those too; do not repeat planning.
    for(const planId of goal.planIds){
      try{
        const plan=await this.service.callTool('geod_plan_status',{planId},scope);
        if(plan.planId!==planId)throw Error('Invalid plan.');
        if(plan.replacedBy)continue;
        if(plan.status==='pending'&&(!plan.areaCoverage||plan.areaCoverage.status==='complete'))waitingConfirmation=true;
        if(plan.status==='pending'&&plan.areaCoverage&&plan.areaCoverage.status!=='complete')goal.reason='The imagery review needs complete area coverage. Search again before confirming.';
        if(plan.status==='submitted'&&plan.jobs?.some(job=>['queued','running'].includes(job.status)||job.status==='succeeded'&&!job.settled))waitingJobs=true;
      }catch{goal.reason='A native task could not be checked. Continue explicitly to inspect its state.';}
    }
    const checkFailure=goal.reason;
    for (const output of goal.outputs) {
      output.verifiedIds = [];
      if (output.kind === 'unavailable') { output.state = 'unavailable'; continue; }
      if (output.entryId) {
        const entry = session.entries.find(entry => entry.id === output.entryId);
        output.state = entry?.status === 'completed' && entry.metadataComplete === true ? 'verified' : 'failed'; continue;
      }
      if (!output.planId) { output.state = 'missing'; continue; }
      try {
        const plan = await this.service.callTool('geod_plan_status', { planId: output.planId }, scope);
        if (plan.planId !== output.planId || plan.replacedBy) throw Error('Review changed.');
        if (plan.status === 'pending') {
          if(plan.areaCoverage&&plan.areaCoverage.status!=='complete') { output.state='failed'; continue; }
          output.state = 'waiting'; waitingConfirmation = true; continue;
        }
        if (plan.status !== 'submitted') throw Error('Plan is not submitted.');
        if (output.kind === 'project') {
          if (!UUID.test(plan.project?.id) || plan.project.saved !== true) throw Error('Project not saved.');
          const project = await this.service.callTool('geod_project_get', { id: plan.project.id }, scope);
          if (project.id !== plan.project.id) throw Error('Project changed.');
          output.verifiedIds = [project.id];
        } else if (output.kind === 'vector') {
          if (!UUID.test(plan.vector?.id) || plan.vector.verified !== true) throw Error('Vector not saved.');
          const file = await this.service.callTool('geod_vector_inspect', { id: plan.vector.id }, scope);
          if (file.verified !== true || file.asset?.id !== plan.vector.id || !HASH.test(file.asset.geojsonSha256)) throw Error('Vector verification failed.');
          output.verifiedIds = [file.asset.id];
        } else {
          if (!Array.isArray(plan.jobs) || !plan.jobs.length || plan.jobs.length > 32 || plan.jobs.some(job => !UUID.test(job.id))) throw Error('Plan has no actual output tasks.');
          if (plan.jobs.some(job => ['failed', 'cancelled', 'interrupted'].includes(job.status))) throw Error('Task failed or stopped.');
          if (plan.jobs.some(job => job.status !== 'succeeded' || job.settled !== true)) { output.state = 'waiting'; waitingJobs = true; continue; }
          const tool = { raster: 'geod_raster_inspect', rgb: 'geod_rgb_inspect', stac: 'geod_stac_inspect', wcs: 'geod_wcs_inspect' }[output.kind];
          for (const job of plan.jobs) {
            const file = await this.service.callTool(tool, { id: job.id }, scope);
            if (!HASH.test(file.sha256 ?? file.artifact?.sha256) || file.jobId !== undefined && file.jobId !== job.id || file.artifact?.jobId !== undefined && file.artifact.jobId !== job.id) throw Error('File verification failed.');
            output.verifiedIds.push(job.id);
          }
        }
        output.state = 'verified';
      } catch { output.state = 'failed'; output.verifiedIds = []; }
    }
    goal.checkedAt = goal.updatedAt = new Date().toISOString(); goal.reason = checkFailure??null;
    if(session.goal!==original||auditBinding(original)!==binding)return {goal:publicGoal(session.goal)};
    goal.engine=structuredClone(original.engine);
    if(original.status==='paused')goal.status='paused';
    else if (pendingDecision(session)) goal.status = 'waiting_input';
    else if (checkFailure || goal.outputs.some(output => ['failed', 'unavailable'].includes(output.state))) goal.status = 'needs_attention';
    else if (waitingConfirmation) goal.status = 'waiting_confirmation';
    else if (waitingJobs) goal.status = 'waiting_jobs';
    else if (goal.outputs.every(output => output.state === 'verified')) goal.status = 'complete';
    else goal.status = 'active';
    if (goal.engine && ['usageLimited', 'budgetLimited'].includes(goal.engine.status) && !['complete','paused'].includes(goal.status)) goal.status = 'budget_limited';
    session.goal=goal;
    await this.sync(session, goal.status === 'complete' ? 'complete' : 'paused');
    await this.service.save(); return { goal: publicGoal(session.goal), verificationScope: 'Native delivery plans, settlement and file validity only. Scientific accuracy, complete coverage and entitlement require separate evidence.' };
  }
  async afterTurn(active, status) {
    if (!active.session.goal || active.kind === 'compaction') return;
    if (status !== 'completed') { await this.pause(active.session); return; }
    if (active.session.goal.status === 'paused') return;
    await this.check(active.session);
    if (active.session.goal.status === 'active' && (!active.goalProgress || active.goalMayContinue===false&&!active.goalDefined)) {
      active.session.goal.status = 'needs_attention'; active.session.goal.reason = 'No new native progress. Continue explicitly to review the missing outputs.';
      await this.service.save();
    }
  }
  async pause(session) {
    if (!session?.goal || session.goal.status === 'complete') return;
    session.goal.status = 'paused'; session.goal.updatedAt = new Date().toISOString();
    await this.sync(session, 'paused'); await this.service.save();
  }
}
