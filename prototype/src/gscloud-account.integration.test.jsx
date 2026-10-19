import React from 'react';
import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {I18nProvider} from './i18n.jsx';
import {GSCloudAccountDialog} from './gscloud-account.jsx';
import {gscloudAuth} from './gscloud-client.js';
vi.mock('./gscloud-client.js',async original=>({...await original(),gscloudAuth:vi.fn()}));
beforeEach(()=>{Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});gscloudAuth.mockReset().mockImplementation(async op=>({status:op==='finish'?'connected':op==='cancel'?'cancelled':'pending',verified:op==='finish',downloadVerified:false}));});

it('uses website sign-in without collecting credentials and connects only after verification',async()=>{
  const onConnected=vi.fn(),onClose=vi.fn();
  render(<I18nProvider><GSCloudAccountDialog onClose={onClose} onConnected={onConnected}/></I18nProvider>);
  expect(screen.queryByRole('textbox')).toBeNull();expect(screen.queryByLabelText('Password')).toBeNull();
  expect(gscloudAuth).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button',{name:'Open official sign-in window'}));
  await screen.findByRole('button',{name:'Verify website sign-in'});
  expect(onConnected).not.toHaveBeenCalled();expect(onClose).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button',{name:'Verify website sign-in'}));
  await waitFor(()=>expect(onConnected).toHaveBeenCalledWith({status:'connected',verified:true,downloadVerified:false}));
  expect(onClose).toHaveBeenCalledOnce();expect(gscloudAuth.mock.calls).toEqual([['begin'],['finish']]);
});

it('verification failure preserves the website window for a recoverable retry',async()=>{
  const onConnected=vi.fn();gscloudAuth.mockImplementation(async op=>{if(op==='finish')throw Error('Complete sign-in in the GSCloud website window first.');return {status:'pending'};});
  render(<I18nProvider><GSCloudAccountDialog onClose={vi.fn()} onConnected={onConnected}/></I18nProvider>);
  fireEvent.click(screen.getByRole('button',{name:'Open official sign-in window'}));
  fireEvent.click(await screen.findByRole('button',{name:'Verify website sign-in'}));
  await screen.findByRole('alert');expect(onConnected).not.toHaveBeenCalled();
  expect(screen.getByRole('button',{name:'Show official sign-in window'}).disabled).toBe(false);
  expect(gscloudAuth.mock.calls.some(([op])=>op==='cancel')).toBe(false);
});

it('closing the dialog cancels the website window, including a late opening result',async()=>{
  let finish;gscloudAuth.mockImplementation(op=>op==='begin'?new Promise(resolve=>{finish=resolve;}):Promise.resolve({status:'cancelled'}));
  const view=render(<I18nProvider><GSCloudAccountDialog onClose={vi.fn()}/></I18nProvider>);
  fireEvent.click(screen.getByRole('button',{name:'Open official sign-in window'}));await waitFor(()=>expect(gscloudAuth).toHaveBeenCalledWith('begin'));
  view.unmount();await act(async()=>finish({status:'pending'}));
  expect(gscloudAuth).toHaveBeenCalledWith('cancel');
});
