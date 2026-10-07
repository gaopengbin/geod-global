import React, { useId, useState } from 'react';
import { Check, MessageSquare } from 'lucide-react';
import { Badge, Button, Input, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { validAnswers } from '../../agent/decisions.mjs';

export function AgentDecisionCard({ decision, disabled, submitting, onAnswer }) {
  const { t } = useI18n(), prefix = useId(), [draft, setDraft] = useState([]);
  const pending = decision.status === 'pending', answers = decision.status === 'answered' ? decision.answers : draft;
  const locked = !pending || disabled || submitting;
  const setAnswer = answer => setDraft(current => [...current.filter(value => value.questionId !== answer.questionId), answer]);
  return <section className="agent-plan agent-decision" aria-label={decision.title} data-decision-id={decision.id}>
    <div className="agent-plan-heading"><span><MessageSquare size={16}/><strong>{decision.title}</strong></span><Badge tone={pending ? 'blue' : 'neutral'}>{t(pending ? 'Choose your preferences' : decision.status === 'answered' ? 'Choices recorded' : decision.status==='superseded' ? 'Boundary ready' : 'Automatic choices enabled')}</Badge></div>
    {decision.status!=='superseded' && decision.questions.map(question => {
      const answer = answers.find(value => value.questionId === question.id);
      return <div key={question.id} className="agent-decision-question" role="group" aria-labelledby={`${prefix}-${question.id}`}>
        <h3 id={`${prefix}-${question.id}`}>{question.prompt}</h3>
        {pending ? <><div className="agent-decision-options">{question.options.map(option => <Button key={option.id} variant="secondary" className="agent-decision-option" aria-pressed={answer?.optionId === option.id} disabled={locked} onClick={() => setAnswer({ questionId: question.id, optionId: option.id })}>
          <span><strong>{option.label}</strong>{question.recommendedOptionId === option.id && <Badge>{t('Recommended')}</Badge>}<small>{option.description}</small></span>
          {answer?.optionId === option.id && <Check size={16} aria-hidden="true"/>}
        </Button>)}</div><Input aria-label={t('Other preference for {question}', { question: question.prompt })} placeholder={t('Or describe your preference')} value={answer?.text ?? ''} maxLength={600} disabled={locked} onChange={event => setAnswer({ questionId: question.id, text: event.target.value })}/></> : decision.status==='answered' && <p className="agent-decision-answer">{answer.text ?? question.options.find(option=>option.id===answer.optionId).label}</p>}
      </div>;
    })}
    {pending && <div className="agent-decision-footer"><p>{t('Your choices do not approve a download. Review the complete task before execution.')}</p><Button primary icon={submitting ? undefined : Check} disabled={locked || !validAnswers(decision, answers)} onClick={() => onAnswer(decision.id, answers)}>{submitting && <Spinner size={14}/>} {t(submitting ? 'Submitting…' : 'Submit choices')}</Button></div>}
    {decision.status === 'skipped' && <p className="agent-plan-note">{t('You enabled automatic execution. Supported defaults will be used without further questions.')}</p>}
    {decision.status === 'superseded' && <p className="agent-plan-note">{t('The source boundary is ready. The earlier missing-boundary question no longer applies. No choice was recorded; the complete task still needs confirmation.')}</p>}
  </section>;
}
