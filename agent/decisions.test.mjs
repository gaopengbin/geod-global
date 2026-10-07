import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { DECISION_TOOL, validDecisionInput, validDecision, validAnswers, decisionReply, decisionReviewIssue } from './decisions.mjs';
const input = { title: 'Choose area and quality', questions: [{ id: 'area', prompt: 'Which boundary?', recommendedOptionId: 'polygon', options: [
  { id: 'polygon', label: 'Actual boundary', description: 'Keep the native polygon.' }, { id: 'rectangle', label: 'Bounding box', description: 'Prepare the surrounding rectangle.' },
] }] };
test('recommendation is never an answer; choice IDs, custom answers and history are strictly bound', () => {
  const decision = { version: 1, id: randomUUID(), ...structuredClone(input), status: 'pending' };
  assert(validDecision(decision)); assert(!validAnswers(decision, []));
  for (const bad of [{ ...input, executionMode: 'full-access' }, { ...input, questions: [...input.questions, ...input.questions] },
    { ...input, questions: [{ ...input.questions[0], recommendedOptionId: 'invented' }] },
    { ...input, questions: [{ ...input.questions[0], options: [...input.questions[0].options, input.questions[0].options[0]] }] }]) assert(!validDecisionInput(bad));
  for (const answers of [[{ questionId: 'other', optionId: 'polygon' }], [{ questionId: 'area', optionId: 'invented' }],
    [{ questionId: 'area', optionId: 'polygon', text: 'also custom' }], [{ questionId: 'area', text: ' ' }],
    [{ questionId: 'area', optionId: 'polygon', approve: true }]]) assert(!validAnswers(decision, answers));
  const session = { entries: [{ name: DECISION_TOOL.name, decision }] };
  assert.throws(() => decisionReply(session, { decisionId: randomUUID(), answers: [] }));
  const reply = decisionReply(session, { decisionId: decision.id, answers: [{ questionId: 'area', text: 'Use my uploaded boundary' }] });
  assert(validDecision(reply.decision)); assert.match(reply.text, /not execution approval/); assert.match(reply.text, /uploaded boundary/);
  assert.equal(decision.status, 'pending');
  session.entries[0].decision = reply.decision;
  assert.throws(() => decisionReply(session, { decisionId: decision.id, answers: [{ questionId: 'area', optionId: 'polygon' }] }));
});
test('task review explains pending or changed choices without pretending to approve a plan',()=>{
  const review={references:[{kind:'plan',id:'actual-plan'}],taskContext:{choices:[]}};
  const choice={decision:{id:randomUUID(),status:'pending'}};
  assert.match(decisionReviewIssue([review,choice],'actual-plan'),/pending decision/);
  choice.decision.status='answered';assert.match(decisionReviewIssue([review,choice],'actual-plan'),/Choices changed/);
  const revised={references:review.references,taskContext:{choices:[{decisionId:choice.decision.id}]}};
  assert.equal(decisionReviewIssue([review,choice,revised],'actual-plan'),null);
  assert.equal(decisionReviewIssue([choice,review],'actual-plan'),null);
});
test('superseded source-availability cards require bounded native evidence and never count as human answers',()=>{
  const value={version:1,id:randomUUID(),...structuredClone(input),status:'superseded',resolution:{reason:'source-boundary-ready',boundary:{id:randomUUID(),sha256:'a'.repeat(64)},bounds:[-74,40,-73,41]}};
  assert(validDecision(value));assert.equal(decisionReviewIssue([{decision:value}],'unused'),null);
  for(const bad of [{...value,answers:[{questionId:'area',optionId:'polygon'}]},{...value,resolution:undefined},
    {...value,resolution:{...value.resolution,reason:'model-picked'}},{...value,status:'pending'},
    {...value,boundaryScope:{bounds:[-74,40,-75,41]}}])assert(!validDecision(bad));
  assert.throws(()=>decisionReply({entries:[{name:DECISION_TOOL.name,decision:value}]},{decisionId:value.id,answers:[{questionId:'area',optionId:'polygon'}]}));
});
