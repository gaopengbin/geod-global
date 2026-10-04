import React, { forwardRef, useRef, useState } from 'react';
import * as Popover from '@radix-ui/react-popover';
import { DayPicker } from 'react-day-picker';
import { enUS, zhCN } from 'react-day-picker/locale';
import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-react';
import { Button, Select } from './index.jsx';
import './date-picker.css';

// A search date is a calendar day in UTC, not an instant. Keep its ISO date
// unchanged when the computer's local timezone differs from the search timezone.
function calendarDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value || '')) return undefined;
  const [year, month, day] = value.split('-').map(Number);
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : undefined;
}
function isoDate(date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
function CalendarDropdown({ options, className, ...props }) {
  return <Select {...props}>{options?.map(option => <option key={option.value} value={option.value} disabled={option.disabled}>{option.label}</option>)}</Select>;
}
const PreviousMonthButton = forwardRef(function PreviousMonthButton(props, ref) {
  return <Button {...props} ref={ref} variant="quiet" size="icon"><ChevronLeft size={16} aria-hidden="true" /></Button>;
});
const NextMonthButton = forwardRef(function NextMonthButton(props, ref) {
  return <Button {...props} ref={ref} variant="quiet" size="icon"><ChevronRight size={16} aria-hidden="true" /></Button>;
});

export function DatePicker({ name, value, onChange, locale = 'en-US', disabled, id, 'aria-label': label }) {
  const [open, setOpen] = useState(false);
  const calendar = useRef(null);
  const selected = calendarDate(value);
  return <Popover.Root open={open} onOpenChange={setOpen}>
    <input type="hidden" name={name} value={value || ''} disabled={disabled} />
    <Popover.Trigger asChild>
      <Button id={id} className="bui-date-trigger" disabled={disabled} aria-label={label}>
        <span>{selected ? new Intl.DateTimeFormat(locale, { year: 'numeric', month: '2-digit', day: '2-digit' }).format(selected) : '—'}</span>
        <CalendarDays size={16} aria-hidden="true" />
      </Button>
    </Popover.Trigger>
    <Popover.Portal>
      <Popover.Content ref={calendar} className="bui-calendar-popover" align="start" sideOffset={6} collisionPadding={12} aria-label={label}
        onOpenAutoFocus={event => {
          event.preventDefault();
          // Start on DayPicker's keyboard target, rather than the previous-month
          // button that Radix would choose as the first focusable element.
          calendar.current?.querySelector('.rdp-day_button[tabindex="0"]')?.focus();
        }}>
        <DayPicker className="bui-calendar" mode="single" required selected={selected}
          defaultMonth={selected} locale={locale === 'zh-CN' ? zhCN : enUS} autoFocus
          captionLayout="dropdown" navLayout="around" showOutsideDays fixedWeeks
          startMonth={new Date(1900, 0)} endMonth={new Date(2100, 11)}
          components={{ Dropdown: CalendarDropdown, PreviousMonthButton, NextMonthButton }}
          onSelect={date => {
            if (!date) return;
            const target = { name, value: isoDate(date) };
            onChange?.({ target, currentTarget: target, type: 'change' });
            setOpen(false);
          }} />
      </Popover.Content>
    </Popover.Portal>
  </Popover.Root>;
}
