import React, { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Button, Input, Select, Switch, Modal, Progress, Disclosure, SegmentedControl } from './ui/index.jsx';

describe('Shared UI behavior used by catalog, recipes and workspace', () => {
  it('associates wrapping field labels with the visible select instead of its hidden form control', async () => {
    const user = userEvent.setup();
    const changes = [];
    render(<label>输入坐标<Select defaultValue="source" onChange={event => changes.push(event.currentTarget.value)}><option value="source">源坐标系</option><option value="world">WGS84</option></Select></label>);
    const trigger = screen.getByRole('combobox', { name: '输入坐标' });
    expect(screen.getByLabelText('输入坐标')).toBe(trigger);
    await user.pointer({ target: screen.getByText('输入坐标', { exact: true }), keys: '[MouseLeft]', coords: { clientX: 20, clientY: 20 } });
    expect(screen.getByRole('listbox')).toBeTruthy();
    // A label forwards a click (after pointerup) to the trigger. Radix guards
    // the next pointerup against accidental selection until the pointer has
    // moved 10px. jsdom has no layout, so user.click's default (0, 0) for both
    // controls incorrectly models no movement between the label and option.
    await user.pointer([
      { target: screen.getByRole('option', { name: 'WGS84' }), coords: { clientX: 40, clientY: 80 } },
      { keys: '[MouseLeft]' },
    ]);
    await waitFor(() => expect(screen.getByRole('combobox', { name: '输入坐标' }).textContent).toContain('WGS84'));
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(changes).toEqual(['world']);
    expect(document.activeElement).toBe(trigger);
  });

  it('submits the selected catalog values through native FormData', async () => {
    const user = userEvent.setup();
    let submitted;
    function Form() {
      const [limit, setLimit] = useState('10');
      return <form onSubmit={event => { event.preventDefault(); submitted = Object.fromEntries(new FormData(event.currentTarget)); }}>
        <Input aria-label="Bounds" name="bbox" defaultValue="-123,37,-122,38" />
        <Input aria-label="Start" name="start" type="date" defaultValue="2025-06-01" />
        <Select aria-label="Scenes per page" name="limit" value={limit} onChange={event => setLimit(event.currentTarget.value)}>
          <option value="10">10 scenes</option><option value="20">20 scenes</option>
        </Select>
        <Button type="submit">Search</Button>
      </form>;
    }
    render(<Form />);
    await user.click(screen.getByRole('combobox', { name: 'Scenes per page' }));
    await user.click(screen.getByRole('option', { name: '20 scenes' }));
    await user.click(screen.getByRole('button', { name: 'Search' }));
    expect(submitted).toEqual({ bbox: '-123,37,-122,38', start: '2025-06-01', limit: '20' });
  });

  it('keeps empty selection values and translated options usable', async () => {
    const user = userEvent.setup();
    let current;
    function Field() {
      const [value, setValue] = useState('raster');
      return <Select aria-label="图层选择" name="layer" value={value} onChange={event => { current = event.target.value; setValue(current); }}>
        <option value="">请选择图层</option><option value="raster">本地场景分类栅格</option><option value="blocked" disabled>暂不可用</option>
      </Select>;
    }
    render(<Field />);
    await user.click(screen.getByRole('combobox', { name: '图层选择' }));
    expect(screen.getByRole('option', { name: '暂不可用' }).getAttribute('aria-disabled')).toBe('true');
    await user.click(screen.getByRole('option', { name: '请选择图层' }));
    expect(current).toBe('');
    expect(screen.getByRole('combobox', { name: '图层选择' }).textContent).toContain('请选择图层');
  });

  it('restores focus to the action that opened a modal after Escape', async () => {
    const user = userEvent.setup();
    function Panel() {
      const [open, setOpen] = useState(false);
      return <><Button onClick={() => setOpen(true)}>查看处理计划</Button>{open && <Modal title="处理计划" closeLabel="关闭" onClose={() => setOpen(false)}>
        <Input aria-label="配方名称" defaultValue="测试" /><Button>检查计划</Button>
      </Modal>}</>;
    }
    render(<Panel />);
    const trigger = screen.getByRole('button', { name: '查看处理计划' });
    await user.click(trigger);
    expect(screen.getByRole('dialog', { name: '处理计划' }).contains(document.activeElement)).toBe(true);
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it('keeps a busy processing dialog open until its operation allows close', async () => {
    const user = userEvent.setup();
    let closes = 0;
    render(<Modal title="Saving recipe" closeDisabled closeLabel="Close" onClose={() => closes++}><p>Writing the pinned recipe</p></Modal>);
    await user.keyboard('{Escape}');
    expect(closes).toBe(0);
    expect(screen.getByRole('dialog', { name: 'Saving recipe' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Close' }).disabled).toBe(true);
  });

  it('lets the modal exit before notifying its parent, and cancels pending dismissal on unmount', () => {
    vi.useFakeTimers();
    try {
      const onClose = vi.fn();
      const panel = render(<Modal title="Preview" closeLabel="Close preview" onClose={onClose}><p>Local file</p></Modal>);
      fireEvent.click(screen.getByRole('button', { name: 'Close preview' }));
      expect(onClose).not.toHaveBeenCalled();
      act(() => vi.advanceTimersByTime(180));
      expect(onClose).toHaveBeenCalledTimes(1);
      panel.unmount();

      const cancelled = vi.fn();
      const second = render(<Modal title="Preview" closeLabel="Close preview" onClose={cancelled}><p>Local file</p></Modal>);
      fireEvent.click(screen.getByRole('button', { name: 'Close preview' }));
      second.unmount();
      act(() => vi.advanceTimersByTime(180));
      expect(cancelled).not.toHaveBeenCalled();
    } finally { vi.useRealTimers(); }
  });

  it('supports keyboard changes and native form values for slider and checkbox', async () => {
    const user = userEvent.setup();
    let submitted;
    function Fields() {
      const [cloud, setCloud] = useState(60);
      const [checked, setChecked] = useState(false);
      return <form onSubmit={event => { event.preventDefault(); submitted = Object.fromEntries(new FormData(event.currentTarget)); }}>
        <Input aria-label="Maximum cloud cover" type="range" name="cloud" min="0" max="100" step="1" value={cloud} onChange={event => setCloud(Number(event.currentTarget.value))}/>
        <Input aria-label="Visible" type="checkbox" name="visible" checked={checked} onChange={event => setChecked(event.currentTarget.checked)}/>
        <Button type="submit">Apply</Button>
      </form>;
    }
    render(<Fields />);
    screen.getByRole('slider', { name: 'Maximum cloud cover' }).focus();
    await user.keyboard('{ArrowRight}');
    await user.click(screen.getByRole('checkbox', { name: 'Visible' }));
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(submitted.cloud).toBe('61');
    expect(submitted.visible).toBe('on');
  });

  it('supports switches, disclosure and exclusive segment selection without page-local handlers', async () => {
    const user = userEvent.setup();
    function Controls() {
      const [on, setOn] = useState(false);
      const [mode, setMode] = useState('preview');
      return <><Switch checked={on} onCheckedChange={setOn} aria-label="Analytics"/>
        <Disclosure summary="Source evidence"><p>SHA-256 verified</p></Disclosure>
        <SegmentedControl aria-label="Preview mode" value={mode} onValueChange={setMode} items={[{ value: 'preview', label: 'Preview' }, { value: 'compare', label: 'Compare' }]}/>
      </>;
    }
    render(<Controls />);
    const control = screen.getByRole('switch', { name: 'Analytics' });
    control.focus(); await user.keyboard(' ');
    expect(control.getAttribute('aria-checked')).toBe('true');
    await user.click(screen.getByRole('button', { name: 'Source evidence' }));
    expect(screen.getByText('SHA-256 verified')).toBeTruthy();
    await user.click(screen.getByRole('radio', { name: 'Compare' }));
    expect(screen.getByRole('radio', { name: 'Compare' }).getAttribute('aria-checked')).toBe('true');
    expect(screen.getByRole('radio', { name: 'Preview' }).getAttribute('aria-checked')).toBe('false');
  });

  it('does not invent numeric progress when a processing total is unknown', () => {
    const { rerender } = render(<Progress aria-label="Processing"/>);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBeNull();
    rerender(<Progress aria-label="Processing" value={40} max={100}/>);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('40');
  });
});
