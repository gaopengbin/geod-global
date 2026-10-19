import React from 'react';
import {expect,it} from 'vitest';
import {render,screen,within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {UsageSettings} from './usage-ui.jsx';
import {createUsageClient} from './usage-client.js';
import {I18nProvider} from './i18n.jsx';

it('shows the enabled default and its off switch, explains local-only status and clears when disabled',async()=>{
  const user=userEvent.setup(),client=createUsageClient({storage:null,privacy:()=>false});
  render(<I18nProvider><UsageSettings client={client}/></I18nProvider>);
  const toggle=screen.getByRole('switch',{name:/Record usage statistics|记录使用统计/});
  expect(toggle.getAttribute('aria-checked')).toBe('true');
  expect(screen.getByText(/On by default|默认开启，可随时关闭/)).toBeTruthy();
  expect(screen.queryByRole('switch',{name:/Share anonymous usage|分享匿名使用统计/})).toBeNull();
  await user.click(screen.getByRole('button',{name:/About usage statistics|使用统计说明/}));
  const dialog=screen.getByRole('dialog');expect(within(dialog).getByText(/No collection service|尚未配置统计接收服务/)).toBeTruthy();
  await user.keyboard('{Escape}');await user.click(toggle);
  expect(client.report().events).toHaveLength(0);expect(client.snapshot().enabled).toBe(false);
});

it('defaults anonymous sharing on with a configured receiver and preserves the off choice',async()=>{
  const user=userEvent.setup(),client=createUsageClient({storage:null,privacy:()=>false,endpoint:'https://geod-global.laogao.xyz/api/usage/v1/events'});
  const view=render(<I18nProvider><UsageSettings client={client}/></I18nProvider>);
  const share=screen.getByRole('switch',{name:/Share anonymous usage|分享匿名使用统计/});
  expect(share.disabled).toBe(false);expect(share.getAttribute('aria-checked')).toBe('true');
  await user.click(share);expect(client.snapshot().sharing).toBe(false);expect(client.snapshot().enabled).toBe(true);
  view.unmount();const blocked=createUsageClient({storage:null,privacy:()=>true});
  render(<I18nProvider><UsageSettings client={blocked}/></I18nProvider>);
  expect(screen.getByRole('switch',{name:/Record usage statistics|记录使用统计/}).disabled).toBe(true);
});
