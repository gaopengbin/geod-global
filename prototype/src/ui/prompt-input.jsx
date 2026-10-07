// Adapted from GeoD Agent 0.2.3's beUI PromptInput (MIT).
// Source and retained notice: licenses/GeoD-Agent-Composer-MIT.txt.
// The measured textarea, toolbar order and icon sizes are retained. Existing
// Beautiful UI adapters replace the source's motion button/popover adapters.
import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowUp, Plus, Square } from 'lucide-react';
import { Button, Popover } from './index.jsx';
import './prompt-input.css';

export function PromptInput({ value = '', onValueChange, onSubmit, onStop, loading = false, disabled = false,
  canSubmit = false, stopDisabled = false, actions = [], onAction, actionsDisabled = false,
  toolbarContent, modelPicker, children, minRows = 2, maxRows = 6,
  placeholder, label, addLabel, sendLabel, stopLabel, className = '', maxLength = 8000 }) {
  const textareaRef = useRef(null), measurementRef = useRef(null), composing = useRef(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const resizeTextarea = useCallback(() => {
    const textarea = textareaRef.current, measurement = measurementRef.current;
    if (!textarea || !measurement || textarea.value !== value) return;
    const height = `${Math.min(Math.max(measurement.scrollHeight, minRows * 24), maxRows * 24)}px`;
    if (textarea.style.height !== height) textarea.style.height = height;
  }, [value, minRows, maxRows]);
  useLayoutEffect(resizeTextarea, [resizeTextarea]);
  useEffect(() => {
    if (!textareaRef.current || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(resizeTextarea);
    observer.observe(textareaRef.current);
    let current = true;
    document.fonts?.ready.then(() => { if (current) resizeTextarea(); });
    return () => { current = false; observer.disconnect(); };
  }, [resizeTextarea]);
  const submit = event => {
    event?.preventDefault();
    if (!canSubmit || disabled || loading) return;
    onSubmit?.(value.trim());
    textareaRef.current?.focus({preventScroll:true});
  };
  return <form data-slot="prompt-input" className={`bui-prompt-input ${className}`} onSubmit={submit}>
    {children}
    <div ref={measurementRef} aria-hidden="true" className="bui-prompt-measurement">{`${value}\u200b`}</div>
    <textarea ref={textareaRef} className="bui-prompt-textarea" value={value} disabled={disabled} placeholder={placeholder}
      aria-label={label} rows={minRows} maxLength={maxLength} onChange={event => onValueChange?.(event.target.value)}
      onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
      onKeyDown={event => {
        if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing || composing.current || event.keyCode === 229) return;
        event.preventDefault(); submit();
      }}/>
    <div className="prompt-input-toolbar">
      {actions.length > 0 && <Popover open={actionsOpen} onOpenChange={setActionsOpen} align="start" side="top" className="prompt-input-actions"
        trigger={<Button data-prompt-control="attachment" variant="quiet" size="icon" className="prompt-input-add" aria-label={addLabel} tooltip={addLabel} disabled={disabled || loading || actionsDisabled} aria-expanded={actionsOpen}>
          <span className="prompt-input-plus" data-open={actionsOpen || undefined}><Plus size={16} aria-hidden="true"/></span>
        </Button>}>
        {actions.map(action => <Button key={action.value} variant="quiet" size="row" className="prompt-input-action" disabled={action.disabled}
          onClick={() => { onAction?.(action.value); setActionsOpen(false); }}>
          {action.icon && <span className="prompt-input-action-icon"><action.icon size={16} aria-hidden="true"/></span>}
          <span className="prompt-input-action-copy"><span>{action.label}</span>{action.description && <small>{action.description}</small>}</span>
        </Button>)}
      </Popover>}
      {toolbarContent}
      {modelPicker}
      <Button data-prompt-control="submit" type={loading ? 'button' : 'submit'} primary size="icon" className="prompt-input-submit"
        aria-label={loading ? stopLabel : sendLabel} tooltip={loading ? stopLabel : sendLabel}
        disabled={loading ? stopDisabled || !onStop : disabled || !canSubmit} onClick={loading ? onStop : undefined}>
        <span key={loading ? 'stop' : 'send'} className="prompt-input-submit-icon">{loading ? <Square size={12} fill="currentColor" aria-hidden="true"/> : <ArrowUp size={16} aria-hidden="true"/>}</span>
      </Button>
    </div>
  </form>;
}
