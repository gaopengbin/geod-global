import React, { useState } from 'react';
import { expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DatePicker } from './ui/index.jsx';
import { CatalogFilters } from './catalog-filters.jsx';
import { I18nProvider } from './i18n.jsx';

const values = { bbox: '-122.55, 37.68, -122.32, 37.84', start: '2026-09-02', end: '2026-10-01', cloudMin: 0, cloud: 60, limit: 100 };
const filters = props => render(<I18nProvider><CatalogFilters initialValues={values} {...props}/></I18nProvider>);

it('selects a day across a month boundary by keyboard and submits its exact ISO calendar date', async () => {
  const user = userEvent.setup();
  let submitted;
  function Form() {
    const [date, setDate] = useState('2026-09-30');
    return <form onSubmit={event => { event.preventDefault(); submitted = new FormData(event.currentTarget).get('start'); }}>
      <DatePicker name="start" value={date} onChange={event => setDate(event.currentTarget.value)} aria-label="Start date"/>
      <button type="submit">Submit</button>
    </form>;
  }
  render(<Form/>);
  const trigger = screen.getByRole('button', { name: 'Start date' });
  await user.click(trigger);
  await user.keyboard('{ArrowRight}{Enter}');
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  expect(document.activeElement).toBe(trigger);
  expect(trigger.textContent).toContain('10/01/2026');
  await user.click(screen.getByRole('button', { name: 'Submit' }));
  expect(submitted).toBe('2026-10-01');
});

it('dismisses the nested calendar with Escape without closing the filters or applying a draft', async () => {
  const user = userEvent.setup(), onClose = vi.fn(), onApply = vi.fn();
  filters({ onClose, onApply });
  const trigger = screen.getByRole('button', { name: 'Search start date' });
  await user.click(trigger);
  expect(screen.getAllByRole('dialog')).toHaveLength(2);
  await user.keyboard('{Escape}');
  await waitFor(() => expect(screen.getAllByRole('dialog')).toHaveLength(1));
  expect(document.activeElement).toBe(trigger);
  expect(onClose).not.toHaveBeenCalled();
  expect(onApply).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(onClose).toHaveBeenCalledOnce();
});

it('keeps invalid cloud ranges in the dialog and applies corrected values only on search', async () => {
  const user = userEvent.setup(), onApply = vi.fn();
  filters({ onApply });
  const minimum = screen.getByRole('slider', { name: 'Minimum cloud cover' });
  minimum.focus();
  await user.keyboard('{End}');
  await user.click(screen.getByRole('button', { name: 'Search catalog' }));
  expect(screen.getByRole('alert').textContent).toContain('minimum no greater than maximum');
  expect(onApply).not.toHaveBeenCalled();
  minimum.focus();
  await user.keyboard('{Home}{ArrowRight}');
  await user.click(screen.getByRole('button', { name: 'Search catalog' }));
  expect(onApply).toHaveBeenCalledOnce();
  expect(onApply.mock.calls[0][0]).toMatchObject({ start: values.start, end: values.end, cloudMin: '1', cloud: '60', bbox: values.bbox });
});

it('passes the unsaved filter dates into map area selection without submitting a search', async () => {
  const user = userEvent.setup(), onArea = vi.fn(), onApply = vi.fn();
  filters({ onArea, onApply });
  await user.click(screen.getByRole('button', { name: 'Search start date' }));
  await user.keyboard('{ArrowRight}{Enter}');
  await user.click(screen.getByRole('button', { name: 'Draw area on map' }));
  expect(onApply).not.toHaveBeenCalled();
  expect(onArea.mock.calls[0][0].start).toBe('2026-09-03');
});
