/* GeoD adapters for Beautiful UI (MIT, Shane Levine) and shadcn/ui (MIT).
 * Exact upstream snapshots, hashes, licenses and adaptations: third-party/.
 * Radix owns interactive state, focus, keyboard and form integration.
 */
import React, { forwardRef, useId, useRef, useState } from 'react';
import { cva } from 'class-variance-authority';
import { clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { Slot } from '@radix-ui/react-slot';
import * as DialogPrimitive from '@radix-ui/react-dialog';
import * as CheckboxPrimitive from '@radix-ui/react-checkbox';
import * as SliderPrimitive from '@radix-ui/react-slider';
import * as SwitchPrimitive from '@radix-ui/react-switch';
import * as ProgressPrimitive from '@radix-ui/react-progress';
import * as CollapsiblePrimitive from '@radix-ui/react-collapsible';
import * as ToggleGroupPrimitive from '@radix-ui/react-toggle-group';
import * as ToastPrimitive from '@radix-ui/react-toast';
import * as SelectPrimitive from '@radix-ui/react-select';
import { Check, ChevronDown, CircleAlert, Clock3, LoaderCircle, X } from 'lucide-react';
import './styles.css';

export const cn = (...inputs) => twMerge(clsx(inputs));

// Adapted from Beautiful UI components/atoms/Button.tsx. Its original variant
// classes remain the canonical button skin; additions cover native app APIs.
const filledShadow = 'shadow-[inset_0_1px_0_rgba(255,255,255,0.14)]';
export const buttonVariants = cva(
  'inline-flex items-center justify-center font-medium select-none transition-[transform,background-color,opacity] duration-150 ease-out active:scale-[0.96] disabled:opacity-50 disabled:pointer-events-none',
  { variants: {
    variant: {
      primary: `bg-ink text-canvas hover:opacity-90 dark:bg-ink dark:text-canvas ${filledShadow}`,
      secondary: 'bg-surface text-ink shadow-btn hover:bg-inset aria-expanded:bg-hover',
      ghost: 'bg-hover-2 text-ink hover:bg-line-strong',
      accent: `bg-accent text-white hover:bg-accent-ink ${filledShadow}`,
      success: `bg-green text-white hover:brightness-95 ${filledShadow}`,
      quiet: 'text-ink hover:bg-hover',
      link: 'text-accent-ink hover:underline underline-offset-4',
      destructive: 'bg-red-tint text-red hover:bg-red hover:text-white',
    },
    size: {
      xs: 'h-7 rounded-full px-2.5 text-[12px] font-normal leading-none gap-1',
      sm: 'h-[27px] px-3 text-[13px] leading-none rounded-full gap-1.5',
      md: 'px-4 py-[9px] text-sm leading-none rounded-full gap-2',
      icon: 'size-8 p-0 rounded-control gap-0',
      row: 'w-full h-auto rounded-card px-3 py-2 text-sm gap-2 text-left',
    },
  }, defaultVariants: { variant: 'secondary', size: 'md' } },
);

export const Button = forwardRef(function Button({ variant, size, primary, selected, icon: Icon, className = '', asChild = false, children, type, ...props }, ref) {
  const legacy = className.split(/\s+/);
  const resolvedVariant = variant || (primary || legacy.includes('primary') ? 'primary' : legacy.some(x => ['icon-btn', 'nav-item'].includes(x)) ? 'quiet' : legacy.includes('text-link') ? 'link' : 'secondary');
  const Comp = asChild ? Slot : 'button';
  return <Comp ref={ref} type={asChild ? undefined : type || 'button'} data-slot="button" data-variant={resolvedVariant} data-selected={selected || undefined} aria-pressed={selected === undefined ? undefined : selected} className={cn('bui-button', buttonVariants({ variant: resolvedVariant, size: size || (legacy.includes('icon-btn') ? 'icon' : 'md') }), className)} {...props}>{asChild ? children : <>{Icon && <Icon size={16} aria-hidden="true" />}{children}</>}</Comp>;
});

// Badge/input/textarea/native select/table structure follows the pinned shadcn
// sources. All shadcn palette aliases are replaced with Beautiful UI tokens.
export function Badge({ tone, variant, className, asChild = false, ...props }) {
  const Comp = asChild ? Slot : 'span';
  return <Comp data-slot="badge" data-tone={tone || variant || 'neutral'} className={cn('bui-badge inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap', className)} {...props} />;
}

const fieldClasses = 'w-full min-w-0 rounded-control border border-line-strong bg-surface px-3 py-2 text-sm text-ink transition-[color,box-shadow] placeholder:text-ink-3 disabled:cursor-not-allowed disabled:opacity-50';
const changeEvent = (type, name, value, checked) => {
  const target = { type, name, value: String(value), valueAsNumber: Number(value), checked };
  return { target, currentTarget: target, type: 'change', preventDefault() {}, stopPropagation() {} };
};
function notifyChange(onChange, onInput, event) {
  onInput?.(event);
  if (onChange !== onInput) onChange?.(event);
}

const RangeInput = forwardRef(function RangeInput({ value, defaultValue, min = 0, max = 100, step = 1, onChange, onInput, name, disabled, readOnly, className, id, 'aria-label': ariaLabel, 'aria-labelledby': ariaLabelledBy, ...props }, ref) {
  return <SliderPrimitive.Root data-slot="slider" className={cn('bui-slider', className)} min={Number(min)} max={Number(max)} step={Number(step)} value={readOnly ? [Number(value ?? defaultValue ?? min)] : value === undefined ? undefined : [Number(value)]} defaultValue={defaultValue === undefined ? [Number(min)] : [Number(defaultValue)]} name={name} disabled={disabled} onValueChange={([next]) => { if (!readOnly) notifyChange(onChange, onInput, changeEvent('range', name, next)); }} {...props}>
    <SliderPrimitive.Track className="bui-slider-track"><SliderPrimitive.Range className="bui-slider-range" /></SliderPrimitive.Track>
    <SliderPrimitive.Thumb ref={ref} id={id} className="bui-slider-thumb" aria-label={ariaLabel} aria-labelledby={ariaLabelledBy} aria-readonly={readOnly || undefined} />
  </SliderPrimitive.Root>;
});

const CheckboxInput = forwardRef(function CheckboxInput({ checked, defaultChecked, onChange, onInput, name, value = 'on', disabled, readOnly, className, ...props }, ref) {
  return <CheckboxPrimitive.Root ref={ref} data-slot="checkbox" className={cn('bui-checkbox', className)} checked={readOnly ? Boolean(checked ?? defaultChecked) : checked} defaultChecked={defaultChecked} name={name} value={value} disabled={disabled} aria-readonly={readOnly || undefined} onCheckedChange={next => { if (!readOnly) notifyChange(onChange, onInput, changeEvent('checkbox', name, value, next === true)); }} {...props}><CheckboxPrimitive.Indicator className="bui-checkbox-indicator"><Check size={13} aria-hidden="true" /></CheckboxPrimitive.Indicator></CheckboxPrimitive.Root>;
});

export const Input = forwardRef(function Input({ type = 'text', className, ...props }, ref) {
  if (type === 'range') return <RangeInput ref={ref} className={className} {...props} />;
  if (type === 'checkbox') return <CheckboxInput ref={ref} className={className} {...props} />;
  return <input ref={ref} type={type} data-slot="input" className={cn('bui-input', type === 'radio' ? 'bui-radio' : fieldClasses, className)} {...props} />;
});
export const Textarea = forwardRef(function Textarea({ className, ...props }, ref) {
  return <textarea ref={ref} data-slot="textarea" className={cn('bui-textarea flex min-h-24', fieldClasses, className)} {...props} />;
});
function selectOptions(children) {
  return React.Children.toArray(children).flatMap(child => {
    if (!React.isValidElement(child)) return [];
    if (child.type === 'option') return [{ value: String(child.props.value ?? child.props.children), label: child.props.children, disabled: child.props.disabled }];
    return selectOptions(child.props.children);
  });
}
export const Select = forwardRef(function Select({ className, children, value, defaultValue, onChange, onInput, name, required, disabled, readOnly, form, id, ...props }, ref) {
  const options = selectOptions(children);
  const emptyKey = `__geod_empty_${useId()}`;
  const [internal, setInternal] = useState(() => String(defaultValue ?? options[0]?.value ?? ''));
  const actual = value === undefined ? internal : String(value);
  const selectRef = useRef(null);
  const triggerRef = useRef(null);
  const selected = options.find(option => option.value === actual);
  const update = next => {
    if (readOnly) return;
    const logical = next === emptyKey ? '' : next;
    setInternal(logical);
    const target = selectRef.current;
    if (target) target.value = logical;
    notifyChange(onChange, onInput, target ? { target, currentTarget: target, type: 'change', preventDefault() {}, stopPropagation() {} } : changeEvent('select-one', name, logical));
  };
  return <div data-slot="select-wrapper" className="bui-select-wrapper relative min-w-0">
    <SelectPrimitive.Root value={actual === '' ? emptyKey : actual} onValueChange={update} disabled={disabled || readOnly}>
      <SelectPrimitive.Trigger ref={node => { triggerRef.current = node; if (typeof ref === 'function') ref(node); else if (ref) ref.current = node; }} id={id} data-slot="select-trigger" className={cn('bui-select flex items-center justify-between gap-2', fieldClasses, className)} aria-required={required || undefined} {...props}><span className="min-w-0 flex-1 truncate text-left"><SelectPrimitive.Value>{selected?.label ?? ''}</SelectPrimitive.Value></span><SelectPrimitive.Icon asChild><ChevronDown className="shrink-0" size={15} aria-hidden="true" /></SelectPrimitive.Icon></SelectPrimitive.Trigger>
      <SelectPrimitive.Portal><SelectPrimitive.Content data-slot="select-content" className="bui-select-content" position="popper" sideOffset={5} collisionPadding={12}><SelectPrimitive.ScrollUpButton className="bui-select-scroll"><ChevronDown size={14} className="rotate-180" /></SelectPrimitive.ScrollUpButton><SelectPrimitive.Viewport className="bui-select-viewport">{options.map(option => <SelectPrimitive.Item key={option.value} value={option.value === '' ? emptyKey : option.value} disabled={option.disabled} className="bui-select-item"><SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText><SelectPrimitive.ItemIndicator><Check size={14} aria-hidden="true" /></SelectPrimitive.ItemIndicator></SelectPrimitive.Item>)}</SelectPrimitive.Viewport><SelectPrimitive.ScrollDownButton className="bui-select-scroll"><ChevronDown size={14} /></SelectPrimitive.ScrollDownButton></SelectPrimitive.Content></SelectPrimitive.Portal>
    </SelectPrimitive.Root>
    <select ref={selectRef} data-slot="select-form-control" className="bui-form-control" tabIndex={-1} aria-hidden="true" name={name} value={actual} form={form} required={required} disabled={disabled} onChange={event => update(event.target.value)} onInvalid={() => triggerRef.current?.focus()}>{children}</select>
  </div>;
});

export function Switch({ className, ...props }) {
  return <SwitchPrimitive.Root data-slot="switch" className={cn('bui-switch', className)} {...props}><SwitchPrimitive.Thumb className="bui-switch-thumb" /></SwitchPrimitive.Root>;
}
export function Progress({ value, max = 100, className, ...props }) {
  const safeMax = Number(max) > 0 ? Number(max) : 100;
  const actual = value == null || !Number.isFinite(Number(value)) ? null : Math.min(safeMax, Math.max(0, Number(value)));
  return <ProgressPrimitive.Root data-slot="progress" className={cn('bui-progress', className)} value={actual} max={safeMax} {...props}><ProgressPrimitive.Indicator className="bui-progress-indicator" style={actual == null ? undefined : { transform: `translateX(-${100 - actual / safeMax * 100}%)` }} /></ProgressPrimitive.Root>;
}

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;
export const DialogTitle = DialogPrimitive.Title;
export const DialogDescription = DialogPrimitive.Description;
export const DialogContent = forwardRef(function DialogContent({ className, children, ...props }, ref) {
  return <DialogPrimitive.Portal><DialogPrimitive.Overlay data-slot="dialog-overlay" className="bui-dialog-overlay" /><DialogPrimitive.Content ref={ref} data-slot="dialog-content" className={cn('bui-dialog-content', className)} {...props}>{children}</DialogPrimitive.Content></DialogPrimitive.Portal>;
});
export function Modal({ title, description, onClose, closeDisabled = false, closeLabel = 'Close', wide, children, className, ...props }) {
  const returnFocus = useRef(typeof document === 'undefined' ? null : document.activeElement);
  return <DialogPrimitive.Root open onOpenChange={open => { if (!open && !closeDisabled) onClose?.(); }}><DialogContent className={cn(wide && 'bui-dialog-wide', className)} {...(!description ? { 'aria-describedby': undefined } : {})} onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus(); }} onEscapeKeyDown={event => { if (closeDisabled) event.preventDefault(); }} onPointerDownOutside={event => { if (closeDisabled) event.preventDefault(); }} {...props}>
    <header className="bui-dialog-header"><div><DialogPrimitive.Title className="bui-dialog-title">{title}</DialogPrimitive.Title>{description && <DialogPrimitive.Description className="bui-dialog-description">{description}</DialogPrimitive.Description>}</div><DialogPrimitive.Close asChild><Button variant="quiet" size="icon" disabled={closeDisabled} aria-label={closeLabel}><X size={18} aria-hidden="true" /></Button></DialogPrimitive.Close></header>
    <div className="bui-dialog-body">{children}</div>
  </DialogContent></DialogPrimitive.Root>;
}

export function Table({ className, ...props }) { return <div data-slot="table-container" className="bui-table-container relative w-full overflow-x-auto"><table data-slot="table" className={cn('bui-table w-full caption-bottom text-sm', className)} {...props} /></div>; }
export function THead({ className, ...props }) { return <thead data-slot="table-header" className={cn('bui-table-head', className)} {...props} />; }
export function TBody({ className, ...props }) { return <tbody data-slot="table-body" className={className} {...props} />; }
export function TR({ className, ...props }) { return <tr data-slot="table-row" className={cn('bui-table-row', className)} {...props} />; }
export function TH({ className, ...props }) { return <th data-slot="table-head" className={cn('bui-table-th', className)} {...props} />; }
export function TD({ className, ...props }) { return <td data-slot="table-cell" className={cn('bui-table-td', className)} {...props} />; }

export function Disclosure({ summary, children, open, defaultOpen, onOpenChange, onToggle, className, ...props }) {
  return <CollapsiblePrimitive.Root data-slot="disclosure" open={open} defaultOpen={defaultOpen} onOpenChange={next => { onOpenChange?.(next); onToggle?.({ target: { open: next }, currentTarget: { open: next } }); }} className={cn('bui-disclosure', className)} {...props}><CollapsiblePrimitive.Trigger data-slot="disclosure-trigger" className="bui-disclosure-trigger"><ChevronDown size={15} aria-hidden="true" /><span>{summary}</span></CollapsiblePrimitive.Trigger><CollapsiblePrimitive.Content data-slot="disclosure-content" className="bui-disclosure-content">{children}</CollapsiblePrimitive.Content></CollapsiblePrimitive.Root>;
}
export function Surface({ as: Comp = 'section', variant = 'card', className, ...props }) { return <Comp data-slot="surface" data-variant={variant} className={cn('bui-surface', className)} {...props} />; }
export function EmptyState({ icon: Icon, title, description, children, action, className, ...props }) {
  return <section data-slot="empty-state" className={cn('bui-empty-state', className)} {...props}>{Icon && <span className="bui-empty-icon">{React.isValidElement(Icon) ? Icon : <Icon size={24} aria-hidden="true" />}</span>}{title && <h3>{title}</h3>}{description && <p>{description}</p>}{children}{action && <div className="bui-empty-action">{action}</div>}</section>;
}
export function Spinner({ className, 'aria-label': label, ...props }) { return <LoaderCircle data-slot="spinner" role={label ? 'status' : undefined} aria-label={label} aria-hidden={label ? undefined : true} className={cn('bui-spinner', className)} size={16} {...props} />; }

export function SegmentedControl({ value, onValueChange, items, className, ...props }) {
  return <ToggleGroupPrimitive.Root data-slot="segmented-control" type="single" value={value} onValueChange={next => { if (next) onValueChange?.(next); }} className={cn('bui-segmented', className)} {...props}>{items.map(({ value: key, label, icon: Icon, ...item }) => <ToggleGroupPrimitive.Item data-slot="segmented-item" className="bui-segmented-item" key={key} value={key} {...item}>{Icon && <Icon size={15} aria-hidden="true" />}{label}</ToggleGroupPrimitive.Item>)}</ToggleGroupPrimitive.Root>;
}
export function Toast({ message, onDismiss, closeLabel = 'Close', duration = 4500 }) {
  return <ToastPrimitive.Provider duration={duration} swipeDirection="right"><ToastPrimitive.Root key={message} data-slot="toast" className="bui-toast" open={Boolean(message)} onOpenChange={open => { if (!open) onDismiss?.(); }}><ToastPrimitive.Description>{message}</ToastPrimitive.Description><ToastPrimitive.Close asChild><Button variant="quiet" size="icon" aria-label={closeLabel}><X size={15} /></Button></ToastPrimitive.Close></ToastPrimitive.Root><ToastPrimitive.Viewport data-slot="toast-viewport" className="bui-toast-viewport" /></ToastPrimitive.Provider>;
}

// Adapted from Beautiful UI SidebarNav's RailButton layout and token classes.
// The demo workspace/chat/account menus and commercial icons are omitted;
// route state and accessible labels are supplied by the host application.
export function SidebarNav({ items = [], footerItems = [], brand, footer, ariaLabel, className, ...props }) {
  const itemView = ({ id, label, icon: Icon, href, active, badge, onClick, ...item }) => {
    const Comp = href ? 'a' : 'button';
    return <Comp key={id} data-slot="sidebar-item" data-active={active || undefined} href={href} type={href ? undefined : 'button'} onClick={onClick} aria-current={active ? 'page' : undefined} aria-label={label} title={label} className="bui-sidebar-row relative z-10 mx-2 flex h-9 items-center rounded-control px-2 text-left transition-[background-color,color,transform] duration-150 active:scale-[0.98]" {...item}><span className="flex size-5 shrink-0 items-center justify-center">{Icon && (React.isValidElement(Icon) ? Icon : <Icon size={18} aria-hidden="true" />)}</span><span className="bui-sidebar-copy ml-2 min-w-0 flex-1 truncate text-sm font-medium">{label}</span>{badge != null && <span className="bui-sidebar-badge text-xs tabular-nums">{badge}</span>}</Comp>;
  };
  return <aside data-slot="sidebar" aria-label={ariaLabel} className={cn('bui-sidebar', className)} {...props}><div className="bui-sidebar-brand">{brand}</div><nav className="bui-sidebar-items" aria-label={ariaLabel}>{items.map(itemView)}</nav><div className="bui-sidebar-bottom">{footerItems.map(itemView)}{footer && <div className="bui-sidebar-footer">{footer}</div>}</div></aside>;
}

// Adapted from Beautiful UI TaskRows. Removed useTick, all demo records and
// staged status transitions. Status/progress/actions are controlled inputs.
export function TaskRows({ items = [], className, ariaLabel }) {
  return <div data-slot="task-rows" role="list" aria-label={ariaLabel} className={cn('bui-task-rows', className)}>{items.map(item => <TaskRow key={item.id} item={item} />)}</div>;
}
function TaskRow({ item }) {
  const { title, description, meta, status, statusLabel, progress, progressLabel, details, actions, icon: Icon } = item;
  const StatusIcon = Icon || (status === 'succeeded' ? Check : ['failed', 'interrupted'].includes(status) ? CircleAlert : status === 'cancelled' ? X : Clock3);
  const tone = status === 'succeeded' ? 'green' : ['failed', 'interrupted'].includes(status) ? 'red' : status === 'running' ? 'blue' : 'neutral';
  return <article data-slot="task-row" role="listitem" data-status={status} className="bui-task-row"><div className="bui-task-main"><span className="bui-task-icon" data-tone={tone}>{status === 'running' ? <Spinner /> : <StatusIcon size={17} aria-hidden="true" />}</span><div className="bui-task-copy"><strong>{title}</strong>{description && <p>{description}</p>}</div>{meta && <span className="bui-task-meta">{meta}</span>}{statusLabel && <Badge tone={tone}>{statusLabel}</Badge>}</div>{status === 'running' && <Progress value={progress} aria-label={progressLabel} />}{details && <div className="bui-task-details">{details}</div>}{actions && <div className="bui-task-actions">{actions}</div>}</article>;
}
