import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {ProxySettingsPanel} from './proxy-ui.jsx';
const request=vi.hoisted(()=>vi.fn());
vi.mock('./runtime-client.js',async original=>({...await original(),runtimeRequest:request}));
beforeEach(()=>{request.mockReset();Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
const mount=()=>render(<I18nProvider><RuntimeContext.Provider value={{health:{}}}><ProxySettingsPanel/></RuntimeContext.Provider></I18nProvider>);
it('reports the actual sample bytes and elapsed time without saving a connection change',async()=>{
 request.mockImplementation(async operation=>operation==='proxy'?{mode:'system',url:null}:{status:'connected',statusCode:206,receivedBytes:65536,elapsedMs:300});
 mount();const user=userEvent.setup();await screen.findByRole('button',{name:'Test connection'});await user.click(screen.getByRole('button',{name:'Test connection'}));
 expect(await screen.findByText('Read 64.0 KiB of source data in 300 ms.')).toBeTruthy();
 expect(request.mock.calls.some(([operation])=>operation==='saveProxy')).toBe(false);
});
it('a sample-body failure does not display a successful connection',async()=>{
 request.mockImplementation(async operation=>{if(operation==='proxy')return {mode:'system',url:null};throw new Error('The Sentinel sample response was incomplete');});
 mount();const user=userEvent.setup();await screen.findByRole('button',{name:'Test connection'});await user.click(screen.getByRole('button',{name:'Test connection'}));
 expect(await screen.findByText('Connection test failed. Check your network or proxy settings.')).toBeTruthy();
 expect(screen.queryByText(/Read .*source data/)).toBeNull();
});
