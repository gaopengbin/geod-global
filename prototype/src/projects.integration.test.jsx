import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { DownloadAssetButton } from './runtime-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';

vi.mock('./runtime-client.js', async importOriginal => ({ ...await importOriginal(), runtimeRequest: vi.fn() }));

const href = 'https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2026/9/S2C_TEST/';
const scene = { id: 'S2C_TEST', date: '2026-09-28T00:00:00Z', bbox: [-123, 37, -122, 38], crs: 'EPSG:32610', assets: {
  scl: { href: `${href}SCL.tif`, type: 'image/tiff; application=geotiff' },
  visual: { href: `${href}TCI.tif`, type: 'image/tiff; application=geotiff' },
} };
const project = { id: 'p1', name: '湾区工程', bounds: scene.bbox, scenes: [{ itemId: scene.id, date: scene.date, crs: scene.crs, assets: scene.assets }] };
const context = overrides => ({ health: { storageRoot: String.raw`\\?\G:\workspace` }, jobs: [], refresh: vi.fn().mockResolvedValue(), act: vi.fn(), ...overrides });
const wrap = (component, value = context()) => render(<I18nProvider><RuntimeContext.Provider value={value}>{component}</RuntimeContext.Provider></I18nProvider>);
beforeEach(() => {
  vi.resetAllMocks();
  // Node's experimental storage can shadow jsdom's storage in this runtime.
  Object.defineProperty(window, 'localStorage', { configurable: true, value: { getItem: () => 'zh-CN', setItem: vi.fn() } });
  location.hash = 'Explore';
});

describe('Named project download and scoped processing flow', () => {
  it('creates the named project with both sources, stays in Explore and opens only on explicit click', async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    runtimeRequest.mockImplementation(async (operation, payload) => operation === 'createProject' ? { id: 'p1', ...payload } : { jobs: [{ id: payload.assetKey }] });
    wrap(<DownloadAssetButton scene={scene} areaBounds={scene.bbox} areaName="Bay" onOpenProject={open}/>);
    await user.click(screen.getByRole('button', { name: '新建工程并下载' }));
    expect(screen.queryByText(String.raw`\\?\G:\workspace`)).toBeNull();
    const input = screen.getByRole('textbox', { name: '工程名称' });
    await user.clear(input); await user.type(input, '我的湾区影像');
    await user.click(screen.getByRole('combobox', { name: '下载内容' }));
    await user.click(screen.getByRole('option', { name: '真彩色 + SCL 分类栅格' }));
    await user.click(screen.getByRole('button', { name: '创建工程并开始下载' }));
    await screen.findByRole('heading', { name: '我的湾区影像' });
    expect(runtimeRequest.mock.calls.map(([operation]) => operation)).toEqual(['createProject', 'downloadProject', 'downloadProject']);
    expect(runtimeRequest.mock.calls[0][1].name).toBe('我的湾区影像');
    expect(runtimeRequest.mock.calls[0][1].bounds).toEqual(scene.bbox);
    expect(location.hash).toBe('#Explore');
    expect(open).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '打开工程' }));
    expect(open).toHaveBeenCalledWith('p1');
  });

  it('retries a queue failure in the already saved project without creating another project', async () => {
    const user = userEvent.setup();
    let attempts = 0;
    runtimeRequest.mockImplementation(async operation => {
      if (operation === 'createProject') return project;
      if (++attempts === 1) throw new Error('Queue offline');
      return { jobs: [{ id: 'job' }] };
    });
    wrap(<DownloadAssetButton scene={scene} areaBounds={scene.bbox} areaName="Bay"/>);
    await user.click(screen.getByRole('button', { name: '新建工程并下载' }));
    await user.click(screen.getByRole('button', { name: '创建工程并开始下载' }));
    await screen.findByRole('button', { name: '重试下载' });
    await user.click(screen.getByRole('button', { name: '重试下载' }));
    await waitFor(() => expect(screen.queryByRole('button', { name: '重试下载' })).toBeNull());
    expect(runtimeRequest.mock.calls.filter(([operation]) => operation === 'createProject')).toHaveLength(1);
    expect(runtimeRequest.mock.calls.at(-1)).toEqual(['downloadProject', { id: 'p1', assetKey: 'visual' }]);
  });

  it('shows only the opened project files and saves an edited project name', async () => {
    const user = userEvent.setup();
    runtimeRequest.mockResolvedValue([project, { ...project, id: 'other', name: '另一个工程' }]);
    const act = vi.fn().mockImplementation(async (_, payload) => ({ ...project, name: payload.name }));
    const jobs = [
      { id: 'j1', kind: 'download', itemId: scene.id, assetKey: 'visual', href: scene.assets.visual.href, status: 'succeeded', title: '工程真彩色', updatedAt: scene.date },
      { id: 'j2', kind: 'download', itemId: 'unrelated', assetKey: 'visual', href: 'other', status: 'succeeded', title: '无关文件', updatedAt: scene.date },
    ];
    wrap(<ProjectsLibrary focusedProjectId="p1"/>, context({ jobs, act }));
    await screen.findByRole('heading', { name: '湾区工程' });
    expect(screen.queryByText('另一个工程')).toBeNull();
    expect(screen.getByText('工程真彩色')).toBeTruthy();
    expect(screen.queryByText('无关文件')).toBeNull();
    await user.click(screen.getByRole('button', { name: '修改工程名称：湾区工程' }));
    const input = screen.getByRole('textbox', { name: '工程名称' });
    await user.clear(input); await user.type(input, '已改名的工程');
    await user.click(screen.getByRole('button', { name: '保存工程名称' }));
    await screen.findByRole('heading', { name: '已改名的工程' });
    expect(act).toHaveBeenCalledWith('renameProject', { id: 'p1', name: '已改名的工程' });
  });
});
