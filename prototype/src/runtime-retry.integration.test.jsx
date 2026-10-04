import React, { useState } from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RuntimeProvider, RuntimeTasks } from './runtime-ui.jsx';
import { runtimeRequest } from './runtime-client.js';
import { I18nProvider } from './i18n.jsx';

vi.mock('./runtime-client.js', async importOriginal => ({ ...await importOriginal(), runtimeRequest: vi.fn() }));
vi.mock('./file-thumbnail.jsx', () => ({ FileThumbnail: () => <span>Test preview</span> }));

const task = (id, status = 'interrupted', extra = {}) => ({
  id, status, kind: 'download', itemId: `SCENE_${id}`, title: `Source ${id}`,
  assetKey: 'visual', bytesDownloaded: 0, attempts: 1,
  error: status === 'interrupted' ? 'The application exited before this operation completed. Retry restarts the operation.' : null,
  ...extra,
});
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

function service(initial, retry = async () => {}) {
  let jobs = initial;
  let online = true;
  const submissions = [];
  vi.mocked(runtimeRequest).mockImplementation(async (operation, payload) => {
    if (!online) throw new Error('Test service unavailable');
    if (operation === 'health') return { ok: true };
    if (operation === 'list') return jobs.map(job => ({ ...job }));
    if (operation === 'projects') return [];
    if (operation !== 'retry') throw new Error(`Unexpected test operation: ${operation}`);
    submissions.push(payload.id);
    await retry(payload.id, { offline: () => { online = false; } });
    jobs = jobs.map(job => job.id === payload.id ? { ...job, status: 'queued', error: null, bytesDownloaded: 0, attempts: job.attempts + 1 } : job);
    return jobs.find(job => job.id === payload.id);
  });
  return { submissions, jobs: () => jobs, reconnect: () => { online = true; }, update: (id, changes) => { jobs = jobs.map(job => job.id === id ? { ...job, ...changes } : job); } };
}

function renderTasks(children = <RuntimeTasks/>) {
  return render(<I18nProvider><RuntimeProvider>{children}</RuntimeProvider></I18nProvider>);
}

async function showAttention(count) {
  await userEvent.click(await screen.findByRole('radio', { name: `Needs attention · ${count}` }));
}

beforeEach(() => {
  vi.mocked(runtimeRequest).mockReset();
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'en', setItem: vi.fn() } });
});

it('queues a batch of 16 interrupted/failed jobs on their existing IDs, leaving completed, cancelled and live work intact', async () => {
  const candidates = Array.from({ length: 16 }, (_, index) => task(String(index), index % 2 ? 'failed' : 'interrupted', index === 3 ? { kind: 'raster_mosaic' } : {}));
  const completed = task('complete', 'succeeded', { bytesDownloaded: 800, outputPath: '/retained/source.tif', sha256: 'a'.repeat(64) });
  const untouched = [completed, task('cancelled', 'cancelled'), task('running', 'running'), task('queued', 'queued')];
  const backend = service([...candidates, ...untouched]);
  renderTasks();
  await showAttention(16);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 16' }));
  await screen.findByText('Queued again: 16 / 16.');
  expect(backend.submissions).toEqual(candidates.map(job => job.id));
  expect(backend.jobs().filter(job => candidates.some(source => source.id === job.id)).every(job => job.status === 'queued' && job.attempts === 2 && job.bytesDownloaded === 0)).toBe(true);
  expect(backend.jobs().slice(16)).toEqual(untouched);
  expect(screen.queryByRole('button', { name: 'Retry all · 0' })).toBeNull();
  expect(screen.getByRole('radio', { name: 'Needs attention · 0' }).getAttribute('data-state')).toBe('on');
  await userEvent.click(screen.getByRole('button', { name: 'View running tasks' }));
  expect(screen.getByText('Source 3')).toBeTruthy();
});

it('shows resumed bytes only after a valid accepted recovery, never for a candidate or a fresh transfer', async () => {
  service([
    task('resumed', 'running', { bytesDownloaded: 2097152, totalBytes: 4194304, transfer: { mode: 'resumed', resumedBytes: 1048576 } }),
    task('fresh', 'running', { bytesDownloaded: 1048576, transfer: { mode: 'fresh', resumedBytes: 0 } }),
    task('candidate', 'running', { bytesDownloaded: 1048576 }),
    task('invalid', 'running', { bytesDownloaded: 5, transfer: { mode: 'resumed', resumedBytes: 1048576 } }),
  ]);
  renderTasks();
  await screen.findByText('Source resumed');
  expect(screen.getAllByText(/ · Resumed$/)).toHaveLength(1);
  await userEvent.click(screen.getAllByRole('button', { name: 'Task details' })[0]);
  expect(screen.getByText('Resumed 1.0 MiB of verified source bytes.')).toBeTruthy();
});

it('reports partial rejection without claiming a finished download or hiding the rejected task', async () => {
  const backend = service([task('first'), task('rejected'), task('last')], async id => {
    if (id === 'rejected') throw new Error('The local queue is full (64 jobs)');
  });
  renderTasks();
  await showAttention(3);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 3' }));
  await screen.findByText('Queued again: 2 / 3.');
  expect(backend.submissions).toEqual(['first', 'rejected', 'last']);
  expect(screen.getByText('Source rejected')).toBeTruthy();
  expect(screen.getByRole('alert').textContent).toContain('Some tasks could not be queued again.');
  expect(screen.getByRole('button', { name: 'Retry all · 1' }).disabled).toBe(false);
});

it('stops submitting when the connection is lost and waits for an explicit retry after reconnection', async () => {
  let disconnect = true;
  const backend = service([task('first'), task('second'), task('third')], async (_id, controls) => {
    if (disconnect) { controls.offline(); throw new Error('Test connection lost'); }
  });
  renderTasks();
  await showAttention(3);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 3' }));
  await screen.findByText('The connection was lost. Remaining tasks were not submitted; reconnect to retry them.');
  expect(backend.submissions).toEqual(['first']);
  expect(screen.getByRole('button', { name: 'Retry all · 3' }).disabled).toBe(true);
  disconnect = false;
  backend.reconnect();
  await userEvent.click(screen.getByRole('button', { name: 'Reconnect task service' }));
  await waitFor(() => expect(screen.getByRole('button', { name: 'Retry all · 3' }).disabled).toBe(false));
  expect(backend.submissions).toEqual(['first']);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 3' }));
  await screen.findByText('Queued again: 3 / 3.');
  expect(backend.submissions).toEqual(['first', 'first', 'second', 'third']);
});

it('continues the requested batch when the task page is left, and keeps its result when the page is reopened', async () => {
  const pending = deferred();
  const backend = service([task('first'), task('second')], async id => { if (id === 'first') await pending.promise; });
  function Navigation() {
    const [tasks, setTasks] = useState(true);
    return <><button onClick={() => setTasks(value => !value)}>Toggle task page</button>{tasks && <RuntimeTasks/>}</>;
  }
  renderTasks(<Navigation/>);
  await showAttention(2);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 2' }));
  await userEvent.click(screen.getByRole('button', { name: 'Toggle task page' }));
  await act(async () => pending.resolve());
  await waitFor(() => expect(backend.submissions).toEqual(['first', 'second']));
  await userEvent.click(screen.getByRole('button', { name: 'Toggle task page' }));
  await screen.findByText('Queued again: 2 / 2.');
});

it('guards double-clicks and disables individual retries until the batch submission finishes', async () => {
  const pending = deferred();
  const backend = service([task('first'), task('second')], async id => { if (id === 'first') await pending.promise; });
  renderTasks();
  await showAttention(2);
  await userEvent.dblClick(screen.getByRole('button', { name: 'Retry all · 2' }));
  expect(backend.submissions).toEqual(['first']);
  expect(screen.getAllByRole('button', { name: 'Retry download' }).every(button => button.disabled)).toBe(true);
  await act(async () => pending.resolve());
  await screen.findByText('Queued again: 2 / 2.');
  expect(backend.submissions).toEqual(['first', 'second']);
});

it('does not enqueue an individual retry a second time when a batch is started', async () => {
  const pending = deferred();
  const backend = service([task('first'), task('second')], async id => { if (id === 'first') await pending.promise; });
  renderTasks();
  await showAttention(2);
  await userEvent.click(screen.getAllByRole('button', { name: 'Retry download' })[0]);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 2' }));
  await screen.findByText('Queued again: 1 / 2.');
  expect(screen.getByText('Already handled or changed status: 1. No duplicate submission.')).toBeTruthy();
  await act(async () => pending.resolve());
  expect(backend.submissions).toEqual(['first', 'second']);
});

it('uses the latest service status before submitting each remaining retry', async () => {
  const backend = service([task('first'), task('second')], async id => {
    if (id === 'first') backend.update('second', { status: 'running', error: null });
  });
  renderTasks();
  await showAttention(2);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 2' }));
  await screen.findByText('Queued again: 1 / 2.');
  expect(backend.submissions).toEqual(['first']);
  expect(screen.getByText('Already handled or changed status: 1. No duplicate submission.')).toBeTruthy();
  expect(backend.jobs()[1].status).toBe('running');
});

it('stops scheduling further tasks if the provider is unmounted', async () => {
  const pending = deferred();
  const first = task('first');
  const backend = service([first, task('second')], async () => pending.promise);
  const view = renderTasks();
  await showAttention(2);
  await userEvent.click(screen.getByRole('button', { name: 'Retry all · 2' }));
  expect(backend.submissions).toEqual(['first']);
  view.unmount();
  await act(async () => pending.resolve());
  expect(backend.submissions).toEqual(['first']);
});

it('describes interruption and cancellation as retained work, and separates mosaic failure from clipping', async () => {
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
  service([task('interrupted', 'interrupted'), task('cancelled', 'cancelled'), task('mosaic', 'failed', { kind: 'raster_mosaic', error: 'Test source missing' })]);
  renderTasks();
  await userEvent.click(await screen.findByRole('radio', { name: '待处理 · 2' }));
  expect(screen.queryByRole('note')).toBeNull();
  expect(screen.queryByRole('alert')).toBeNull();
  for (const button of screen.getAllByRole('button', { name: '任务详情' })) await userEvent.click(button);
  expect(screen.getByRole('note').textContent).toContain('下载已中断。重试时会检查已保存部分，数据源支持时从断点继续。');
  expect(screen.getByRole('alert').textContent).toContain('拼接处理未完成，请检查源文件后重试。');
  expect(screen.queryByText('本次下载未完成。请在数据源与本地服务恢复可用后重新下载。')).toBeNull();
  await userEvent.click(screen.getByRole('radio', { name: '历史记录 · 1' }));
  await userEvent.click(screen.getByRole('button', { name: '任务详情' }));
  expect(screen.getByRole('note').textContent).toContain('这项任务已取消，可从头重试。');
  expect(screen.getByRole('button', { name: '重试下载' })).toBeTruthy();
});

it('explains the old mosaic limit in Chinese and retries the same job without downloading sources', async () => {
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
  const backend = service([task('large-mosaic', 'failed', { kind: 'raster_mosaic', error: 'Project output exceeds 8 million pixels; choose a smaller area or process smaller groups' })]);
  renderTasks();
  await userEvent.click(await screen.findByRole('radio', { name: '待处理 · 1' }));
  await userEvent.click(screen.getByRole('button', { name: '任务详情' }));
  expect(screen.getByRole('alert').textContent).toContain('此任务触发了旧版处理上限。重试将使用分块拼接，并复用已下载源文件。');
  await userEvent.click(screen.getByRole('button', { name: '技术详情' }));
  expect(screen.getByRole('alert').textContent).toContain('此任务触发了旧版 800 万像元上限。请重试，现已支持按原分辨率分块拼接。');
  expect(screen.queryByText('拼接处理未完成，请检查源文件后重试。')).toBeNull();
  await userEvent.click(screen.getByRole('button', { name: '从头重试' }));
  await waitFor(() => expect(backend.submissions).toEqual(['large-mosaic']));
});

it('shows actionable disk space information with readable sizes', async () => {
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
  service([task('disk-mosaic', 'failed', { kind: 'raster_mosaic', error: 'Insufficient workspace disk space for mosaic: need 1073741824 bytes, available 536870912 bytes' })]);
  renderTasks();
  await userEvent.click(await screen.findByRole('radio', { name: '待处理 · 1' }));
  await userEvent.click(screen.getByRole('button', { name: '任务详情' }));
  await userEvent.click(screen.getByRole('button', { name: '技术详情' }));
  expect(screen.getByRole('alert').textContent).toContain('请释放工作空间所在磁盘的空间后重试，已下载源文件会继续复用。');
  expect(screen.getByRole('alert').textContent).toContain('工作空间剩余空间不足：预计需要 1.0 GiB，当前可用 512.0 MiB。');
});
