import React from 'react';
import {beforeEach,afterEach,it,expect,vi} from 'vitest';
import {cleanup,render,screen,waitFor,within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {DistributionProvider,UpdatesPanel,NotificationCenter} from './distribution-ui.jsx';
import {emptyDistribution,validateDistribution} from './distribution-client.js';
vi.mock('./runtime-client.js',()=>({desktopAvailable:()=>true,syncDesktopLocale:async()=>{}}));
let backend,invoke,off;
beforeEach(()=>{
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:()=>{}}});
  backend={...emptyDistribution(),updateConfigured:true,messagesConfigured:true,development:false,automaticChecks:false,notificationsEnabled:false};off=vi.fn();
  invoke=vi.fn(async(command,args)=>{
    if(command==='update_check'){backend.update={state:'available',version:'0.1.1',notes:'Signed release notes'};return backend.update;}
    if(command==='update_download'){backend.update={state:'ready',version:args.version,verified:true};return backend.update;}
    if(command==='distribution_preferences')Object.assign(backend,args);
    if(command==='notifications_read')for(const item of backend.items)if(args.ids.includes(item.id))item[args.seenOnly?'seen':'read']=true;
    backend.unreadCount=backend.items.filter(i=>!i.read).length;return structuredClone(backend);
  });window.__TAURI__={core:{invoke},event:{listen:vi.fn(async()=>off)}};
});
afterEach(()=>{cleanup();delete window.__TAURI__;vi.restoreAllMocks();});
const mount=()=>render(<I18nProvider><DistributionProvider><UpdatesPanel/><NotificationCenter/></DistributionProvider></I18nProvider>);
const notice=(id='notice-1')=>({id,revision:1,title:{en:'Product news','zh-CN':'产品公告'},body:{en:'Public release notice.','zh-CN':'公开发行通知。'},priority:'normal',publishedAt:'2026-01-01T00:00:00Z',expiresAt:null,minVersion:null,maxVersion:null,action:null,read:false,seen:false});
it('does not silently download or install; inspection, verified download and install confirmation are separate',async()=>{
  mount();await waitFor(()=>expect(screen.getByText('Installed version')).toBeTruthy());
  expect(invoke.mock.calls.every(([name])=>name==='distribution_snapshot')).toBe(true);
  await userEvent.click(screen.getByRole('button',{name:'Check for updates'}));await screen.findByText('Signed release notes');
  expect(invoke).not.toHaveBeenCalledWith('update_download',expect.anything());
  await userEvent.click(screen.getByRole('button',{name:'Download update'}));await screen.findByText('Update verified and ready to install.');
  await userEvent.click(screen.getByRole('button',{name:'Install and restart'}));const dialog=await screen.findByRole('dialog');expect(invoke).not.toHaveBeenCalledWith('update_install',expect.anything());
  await userEvent.click(within(dialog).getByRole('button',{name:'Later'}));expect(invoke).not.toHaveBeenCalledWith('update_install',expect.anything());
});
it('development builds can verify downloads but cannot install',async()=>{
  backend.development=true;backend.update={state:'ready',version:'0.1.1',verified:true};mount();
  await screen.findByText('Development builds do not install updates.');expect(screen.getByRole('button',{name:'Install and restart'}).disabled).toBe(true);
});
it('shows pending channel explicitly without reporting up-to-date',async()=>{
  backend.updateConfigured=false;backend.messagesConfigured=false;mount();
  await screen.findByText('No signed version has been published to this channel yet. This is not an up-to-date result.');expect(screen.queryByText('You are up to date.')).toBeNull();
});
it('marks specific revisions as read and keeps an empty unread view',async()=>{
  backend.items=[notice()];backend.unreadCount=1;mount();await userEvent.click(await screen.findByRole('button',{name:'Notifications · 1 unread'}));
  await screen.findByText('Product news');await userEvent.click(screen.getByRole('button',{name:'Mark as read'}));await waitFor(()=>expect(screen.getByRole('button',{name:'Mark all as read'}).disabled).toBe(true));
  await userEvent.click(screen.getByRole('radio',{name:'Unread'}));await screen.findByText('No unread notifications');
  expect(invoke).toHaveBeenCalledWith('notifications_read',{ids:['notice-1'],seenOnly:false});
});
it('retains cached announcements if refreshing the server fails',async()=>{
  backend.items=[notice()];backend.unreadCount=1;const original=invoke.getMockImplementation();invoke.mockImplementation((command,args)=>command==='notifications_refresh'?Promise.reject(new Error('Could not refresh notifications.')):original(command,args));
  mount();await userEvent.click(await screen.findByRole('button',{name:'Notifications · 1 unread'}));await userEvent.click(screen.getByRole('button',{name:'Refresh notifications'}));
  await screen.findByText('Could not refresh notifications.');expect(screen.getByText('Product news')).toBeTruthy();
});
it('an update reminder opens settings without triggering installation',async()=>{
  backend.update={state:'available',version:'0.1.1'};mount();await userEvent.click(await screen.findByRole('button',{name:'Notifications · 1 unread'}));
  await userEvent.click(screen.getByRole('button',{name:'Open updates'}));expect(location.hash).toBe('#Settings');expect(invoke).not.toHaveBeenCalledWith('update_install',expect.anything());
});
it('persists automatic check preference through the native store and disposes listeners',async()=>{
  const view=mount();await screen.findByText('Installed version');await userEvent.click(screen.getByRole('switch',{name:'Automatically check for updates'}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('distribution_preferences',{automaticChecks:true,notificationsEnabled:false}));view.unmount();expect(off).toHaveBeenCalledOnce();
});
it('rejects cross-product or inconsistent native message snapshots',()=>{
  expect(()=>validateDistribution({...backend,product:'GeoD Agent'})).toThrow(/invalid/);
  expect(()=>validateDistribution({...backend,items:[notice()],unreadCount:0})).toThrow(/invalid/);
  expect(()=>validateDistribution({...backend,items:[{...notice(),action:'javascript:alert(1)'}],unreadCount:1})).toThrow(/invalid/);
});
