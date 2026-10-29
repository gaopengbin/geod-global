import React from 'react';
import {beforeEach,afterEach,it,expect,vi} from 'vitest';
import {act,cleanup,render,screen,waitFor,within} from '@testing-library/react';
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
afterEach(()=>{cleanup();delete window.__TAURI__;vi.restoreAllMocks();vi.useRealTimers();});
const mount=()=>render(<I18nProvider><DistributionProvider><UpdatesPanel/><NotificationCenter/></DistributionProvider></I18nProvider>);
it('a transient automatic check failure gets a bounded later retry and never starts downloading',async()=>{
 vi.useFakeTimers();vi.spyOn(document,'hidden','get').mockReturnValue(false);backend.automaticChecks=true;backend.messagesConfigured=false;
 const original=invoke.getMockImplementation();let attempts=0;
 invoke.mockImplementation(async(command,args)=>{if(command==='update_check'){
   attempts++;if(attempts===1)throw '[update_check|network|unknown] Temporary update network failure.';
   backend.lastUpdateCheck=new Date().toISOString();backend.update={state:'upToDate'};return backend.update;
 }return original(command,args);});
 mount();await act(async()=>{await vi.advanceTimersByTimeAsync(0);});await act(async()=>{await vi.advanceTimersByTimeAsync(2500);});expect(attempts).toBe(1);
 await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(attempts).toBe(2);
 expect(invoke.mock.calls.some(([name])=>['update_download','update_install'].includes(name))).toBe(false);
});
it('announces an automatic update once per session and never downloads or installs without confirmation',async()=>{
  backend.automaticChecks=true;backend.update={state:'available',version:'0.2.1',notes:'Verified release'};
  mount();const prompt=await screen.findByRole('dialog');
  expect(invoke).not.toHaveBeenCalledWith('update_download',expect.anything());
  await userEvent.click(within(prompt).getByRole('button',{name:'Later'}));
  await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
  expect(invoke).not.toHaveBeenCalledWith('update_install',expect.anything());
});
it('downloads from the update prompt, then requires a separate explicit restart confirmation',async()=>{
  backend.automaticChecks=true;backend.update={state:'available',version:'0.2.1'};
  mount();let prompt=await screen.findByRole('dialog');
  await userEvent.click(within(prompt).getByRole('button',{name:'Download update'}));
  await waitFor(()=>expect(within(screen.getByRole('dialog')).getByText('Update verified and ready to install.')).toBeTruthy());
  await userEvent.click(within(screen.getByRole('dialog')).getByRole('button',{name:'Install and restart'}));
  expect(invoke).not.toHaveBeenCalledWith('update_install',expect.anything());
  await userEvent.click(within(screen.getByRole('dialog')).getByRole('button',{name:'Confirm upgrade and restart'}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('update_install',{version:'0.2.1'}));
});
const notice=(id='notice-1')=>({id,revision:1,title:{en:'Product news','zh-CN':'产品公告'},body:{en:'Public release notice.','zh-CN':'公开发行通知。'},priority:'normal',publishedAt:'2026-01-01T00:00:00Z',expiresAt:null,minVersion:null,maxVersion:null,action:null,read:false,seen:false});
it('does not silently download or install; inspection, verified download and install confirmation are separate',async()=>{
  mount();await waitFor(()=>expect(screen.getByText('Installed version')).toBeTruthy());
  expect(invoke.mock.calls.every(([name])=>name==='distribution_snapshot')).toBe(true);
  await userEvent.click(screen.getByRole('button',{name:'Check for updates'}));expect(screen.queryByText('Signed release notes')).toBeNull();await userEvent.click(await screen.findByRole('button',{name:'Release notes'}));await screen.findByText('Signed release notes');await userEvent.keyboard('{Escape}');
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
