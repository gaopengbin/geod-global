// Conversation choices never grant execution permission or approve a data plan.
const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const KEY = /^[a-z][a-z0-9_]{0,31}$/;
const HASH = /^[a-f0-9]{64}$/;
export const validDecisionBounds = value => Array.isArray(value) && value.length === 4 && value.every(Number.isFinite)
  && value[0] >= -180 && value[0] < value[2] && value[2] <= 180 && value[1] >= -90 && value[1] < value[3] && value[3] <= 90;
const object = (value, fields) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).every(key => fields.includes(key));
const text = (value, max) => typeof value === 'string' && value.trim().length > 0 && value.length <= max && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(value);
export const DECISION_TOOL = Object.freeze({
  name: 'geod_request_decision',
  description: 'Present 1–3 necessary human decisions as selectable conversation cards. Give 2–5 distinct options with concise tradeoffs for each question; an optional recommendation is not a selection. Returns pending questions, never answers or execution approval. Stop dependent planning until the human answers through the card. Do not ask for passwords or tokens. Native full-access permission skips questions and lets you use supported defaults; never invent unavailable data.',
  inputSchema: { type: 'object', additionalProperties: false, properties: {
    title: { type: 'string', minLength: 1, maxLength: 120 },
    questions: { type: 'array', minItems: 1, maxItems: 3, items: { type: 'object', additionalProperties: false, properties: {
      id: { type: 'string', pattern: KEY.source }, prompt: { type: 'string', minLength: 1, maxLength: 600 },
      recommendedOptionId: { type: 'string', pattern: KEY.source },
      options: { type: 'array', minItems: 2, maxItems: 5, items: { type: 'object', additionalProperties: false, properties: {
        id: { type: 'string', pattern: KEY.source }, label: { type: 'string', minLength: 1, maxLength: 80 }, description: { type: 'string', minLength: 1, maxLength: 240 },
      }, required: ['id', 'label', 'description'] } },
    }, required: ['id', 'prompt', 'options'] } },
  }, required: ['title', 'questions'] },
});
export function validDecisionInput(value) {
  return object(value, ['title', 'questions']) && text(value.title, 120) && Array.isArray(value.questions)
    && value.questions.length >= 1 && value.questions.length <= 3 && new Set(value.questions.map(q => q?.id)).size === value.questions.length
    && value.questions.every(q => object(q, ['id', 'prompt', 'options', 'recommendedOptionId']) && KEY.test(q.id) && text(q.prompt, 600)
      && Array.isArray(q.options) && q.options.length >= 2 && q.options.length <= 5 && new Set(q.options.map(o => o?.id)).size === q.options.length
      && q.options.every(o => object(o, ['id', 'label', 'description']) && KEY.test(o.id) && text(o.label, 80) && text(o.description, 240))
      && (q.recommendedOptionId === undefined || q.options.some(o => o.id === q.recommendedOptionId)));
}
export function validAnswers(decision, answers) {
  return Array.isArray(answers) && answers.length === decision.questions.length && new Set(answers.map(a => a?.questionId)).size === answers.length
    && answers.every(a => object(a, ['questionId', 'optionId', 'text']) && decision.questions.some(q => q.id === a.questionId
      && (a.text === undefined && q.options.some(o => o.id === a.optionId) || a.optionId === undefined && text(a.text, 600))));
}
export function validDecision(value) {
  return object(value, ['version', 'id', 'title', 'questions', 'status', 'answers', 'boundaryScope', 'resolution']) && value.version === 1 && UUID.test(value.id)
    && validDecisionInput({ title: value.title, questions: value.questions }) && ['pending', 'answered', 'skipped', 'superseded'].includes(value.status)
    && (value.status === 'answered' ? validAnswers(value, value.answers) : value.answers === undefined)
    && (value.boundaryScope === undefined || object(value.boundaryScope, ['bounds']) && validDecisionBounds(value.boundaryScope.bounds))
    && (value.status === 'superseded' ? object(value.resolution, ['reason', 'boundary', 'bounds'])
      && value.resolution.reason === 'source-boundary-ready' && validDecisionBounds(value.resolution.bounds)
      && object(value.resolution.boundary, ['id', 'sha256']) && UUID.test(value.resolution.boundary.id) && HASH.test(value.resolution.boundary.sha256)
      : value.resolution === undefined);
}
export function pendingDecision(session) {
  return session?.entries.findLast(entry => entry.name === DECISION_TOOL.name && entry.decision?.status === 'pending')?.decision;
}
// A UI explanation only; native confirmation repeats this check before commit.
export function decisionReviewIssue(entries, planId) {
  if (entries.some(entry => entry.decision?.status === 'pending')) return 'Answer the pending decision card before confirming the download task.';
  const refers = entry => entry.references?.some(ref => ref.kind === 'plan' && ref.id === planId);
  const first = entries.findIndex(refers);
  if (first < 0) return null;
  const review = entries.findLast(entry => entry.taskContext && refers(entry));
  return entries.slice(first + 1).some(entry => entry.decision?.status === 'answered' && !review?.taskContext.choices.some(choice => choice.decisionId === entry.decision.id))
    ? 'Choices changed. Prepare or revise the complete task card before confirming.' : null;
}
export function taskContext(session, requestText) {
  const control = /^(?:重试|再试试|再试一次|retry|try again|确认(?:执行|方案|全部|全部方案)?|执行方案|开始执行|执行吧|同意执行|就这么做吧|继续任务|继续执行|(?:开启|启用|允许|关闭)自动执行|不用问我|不用询问我|不需要我确认|不要再问我，直接执行|confirm(?: all)? plans?|enable automatic execution|disable automatic execution|don't ask me, execute automatically|resume task)[。！!.]*$/i;
  const lastRequest = session.entries.findLastIndex(entry => entry.type === 'user' && entry.origin !== 'desktop' && !control.test(entry.text.trim()));
  const choices = session.entries.slice(lastRequest + 1).flatMap(entry => entry.decision?.status === 'answered' ? entry.decision.questions.map(question => {
    const answer = entry.decision.answers.find(value => value.questionId === question.id);
    return { decisionId: entry.decision.id, questionId: question.id, prompt: question.prompt, answer: answer.text?.trim() ?? question.options.find(option => option.id === answer.optionId).label };
  }) : []);
  return { requestText: requestText ?? session.entries[lastRequest]?.text ?? '', choices };
}
export function validTaskContext(value) {
  return object(value, ['requestText', 'choices']) && typeof value.requestText === 'string' && value.requestText.length <= 8000
    && Array.isArray(value.choices) && value.choices.length <= 30 && value.choices.every(choice => object(choice, ['decisionId', 'questionId', 'prompt', 'answer'])
      && UUID.test(choice.decisionId) && KEY.test(choice.questionId) && text(choice.prompt, 600) && text(choice.answer, 600));
}
export function decisionReply(session, reply) {
  if (!object(reply, ['decisionId', 'answers']) || !UUID.test(reply.decisionId)) throw Error('Invalid decision answer.');
  const entry = session?.entries.find(entry => entry.name === DECISION_TOOL.name && entry.decision?.id === reply.decisionId);
  if (!entry || entry.decision.status !== 'pending' || !validAnswers(entry.decision, reply.answers)) throw Error('Answer the current decision using its actual options.');
  const answers = entry.decision.questions.map(q => {
    const answer = reply.answers.find(a => a.questionId === q.id);
    return { prompt: q.prompt, answer: answer.text?.trim() ?? q.options.find(o => o.id === answer.optionId).label };
  });
  return { entry, decision: { ...entry.decision, status: 'answered', answers: structuredClone(reply.answers) },
    text: `Human decision answers (preferences only; not execution approval):\n${answers.map(a => `${a.prompt}\n${a.answer}`).join('\n\n')}` };
}
