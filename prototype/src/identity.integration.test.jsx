import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen,waitFor,fireEvent} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {IdentityEntry,IdentityProvider,SignInPage} from './identity-ui.jsx';
import {I18nProvider} from './i18n.jsx';
import {desktopAvailable} from './runtime-client.js';
import {signedOutIdentity} from './identity-client.js';
vi.mock('./runtime-client.js',()=>({desktopAvailable:vi.fn(()=>true),syncDesktopLocale:vi.fn(async()=>{})}));
const ready=()=>({...signedOutIdentity(),configured:true,ready:true,providers:[{id:'email',available:true},{id:'google',available:true},{id:'github',available:true}]});
const account=()=>({...ready(),status:'signed-in',remembered:true,user:{id:'a'.repeat(32),provider:'google',name:'Test Global User',email:'tester@example.invalid',emailVerified:true,avatar:null,expiresAt:Math.floor(Date.now()/1000)+3600}});
let invoke,store,onContinue;
beforeEach(()=>{
 store=new Map([['geod-global-locale','en']]);
 Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:key=>store.get(key)??null,setItem:(key,value)=>store.set(key,value)}});
 desktopAvailable.mockReturnValue(true);
 invoke=vi.fn(async()=>ready());window.__TAURI__={core:{invoke}};onContinue=vi.fn();
});
function view(){return render(<I18nProvider><IdentityProvider><IdentityEntry onOpen={()=>{}}/><SignInPage onContinue={onContinue}/></IdentityProvider></I18nProvider>);}
async function loaded(){await waitFor(()=>expect(screen.getByRole('button',{name:'Continue with Google'}).disabled).toBe(false));}

it('shows branded provider buttons, guest entry, and separate data-source authorization',async()=>{
 view();await loaded();
 expect(screen.getByRole('button',{name:'Continue with GitHub'}).querySelector('img').getAttribute('src')).toBe('/account-marks/github.svg');
 expect(screen.getByText('Global sign-in is separate from NASA Earthdata and Copernicus data authorization.')).toBeTruthy();
 await userEvent.click(screen.getByRole('button',{name:'Continue without an account'}));expect(onContinue).toHaveBeenCalledOnce();
 expect(invoke.mock.calls.map(([command])=>command)).toEqual(['identity_snapshot']);
});
it('starts independent Global email handoff and accepts only a verified email identity',async()=>{
 let snapshot=ready();invoke.mockImplementation(async(command)=>{
  if(command==='identity_begin')snapshot={...ready(),status:'waiting',busy:true};return snapshot;
 });view();await loaded();
 await userEvent.click(screen.getByRole('button',{name:'GeoD Global account sign-in'}));
 expect(invoke).toHaveBeenLastCalledWith('identity_begin',{provider:'email',locale:'en',remember:true});
 snapshot={...account(),user:{...account().user,provider:'email',emailVerified:false}};
 await waitFor(()=>expect(screen.getByRole('alert').textContent).toContain('Account status could not be verified.'),{timeout:2500});
 expect(onContinue).not.toHaveBeenCalled();
 snapshot={...account(),user:{...account().user,provider:'email'}};
 await waitFor(()=>expect(onContinue).toHaveBeenCalledOnce(),{timeout:2500});
 expect(screen.getByText('GeoD Global',{selector:'.identity-user-details span'})).toBeTruthy();
});
it('keeps email unavailable until the account server advertises it',async()=>{
 invoke.mockResolvedValue({...ready(),providers:[{id:'email',available:false},{id:'google',available:true},{id:'github',available:true}]});
 view();await loaded();expect(screen.getByRole('button',{name:'GeoD Global account sign-in'}).disabled).toBe(true);
 expect(screen.getByText('Email sign-in is not available yet. Google and GitHub are still available.')).toBeTruthy();
 expect(screen.getByRole('button',{name:'Continue with GitHub'}).disabled).toBe(false);
});
it('hands off once, waits for verified native success, and never stores account secrets in the page',async()=>{
 let snapshot=ready();invoke.mockImplementation(async(command)=>{
  if(command==='identity_begin')snapshot={...ready(),status:'waiting',busy:true};return snapshot;
 });view();await loaded();
 await userEvent.click(screen.getByRole('checkbox',{name:'Keep me signed in on this device'}));
 await userEvent.dblClick(screen.getByRole('button',{name:'Continue with Google'}));
 expect(invoke.mock.calls.filter(([command])=>command==='identity_begin')).toEqual([['identity_begin',{provider:'google',locale:'en',remember:false}]]);
 expect(screen.getByText('Finish signing in in your browser')).toBeTruthy();expect(onContinue).not.toHaveBeenCalled();
 snapshot=account();await waitFor(()=>expect(onContinue).toHaveBeenCalledOnce(),{timeout:2500});
 expect(screen.getByText('tester@example.invalid')).toBeTruthy();expect(JSON.stringify([...store])).not.toMatch(/token|verifier|accessToken|tester@example/);
});
it('cancels pending authorization and offers sign-in again',async()=>{
 invoke.mockImplementation(async(command)=>command==='identity_begin'?{...ready(),status:'waiting',busy:true}:ready());
 view();await loaded();await userEvent.click(screen.getByRole('button',{name:'Continue with GitHub'}));
 await userEvent.click(screen.getByRole('button',{name:'Cancel sign-in'}));await loaded();
 expect(invoke.mock.calls.some(([command])=>command==='identity_cancel')).toBe(true);expect(onContinue).not.toHaveBeenCalled();
});
it('keeps guest use available during a slow account-service check',async()=>{
 let resolve;invoke.mockImplementation(command=>command==='identity_snapshot'?new Promise(done=>{resolve=done;}):ready());
 view();await userEvent.click(screen.getByRole('button',{name:'Continue without an account'}));
 expect(onContinue).toHaveBeenCalledOnce();resolve(ready());
});
it('shows an honest undeployed-server state and retries',async()=>{
 invoke.mockResolvedValue({...signedOutIdentity(),configured:true,error:'desktop-not-ready'});view();
 expect(await screen.findByRole('alert')).toBeTruthy();expect(screen.getByRole('button',{name:'Continue with Google'}).disabled).toBe(true);
 invoke.mockResolvedValue(ready());await userEvent.click(screen.getByRole('button',{name:'Retry'}));await loaded();
 expect(invoke).toHaveBeenLastCalledWith('identity_snapshot',{refresh:true});
});
it('rejects untrusted error text and malformed identities rather than showing a false signed-in account',async()=>{
 invoke.mockRejectedValue(Error('Bearer PRIVATE-TOKEN upstream response'));view();
 const alert=await screen.findByRole('alert');expect(alert.textContent).not.toContain('PRIVATE');
 invoke.mockResolvedValue({...account(),accessToken:'DO-NOT-EXPOSE'});await userEvent.click(screen.getByRole('button',{name:'Retry'}));
 await waitFor(()=>expect(screen.getByRole('alert').textContent).toContain('Account status could not be verified.'));
 expect(screen.queryByText('tester@example.invalid')).toBeNull();
});
it('restores a verified profile, falls back from a broken avatar, and signs out locally',async()=>{
 const state=account();state.user.avatar='data:image/png;base64,AAAA';invoke.mockResolvedValue(state);view();
 await screen.findByText('tester@example.invalid');const avatars=document.querySelectorAll('.identity-avatar img');fireEvent.error(avatars[0]);
 expect(document.querySelector('.identity-entry .identity-avatar').textContent).toBe('T');
 invoke.mockResolvedValue(ready());await userEvent.click(screen.getByRole('button',{name:'Sign out of this device'}));await loaded();
 expect(invoke).toHaveBeenLastCalledWith('identity_logout',{});
});
it('renders Chinese copy and disables native OAuth in the browser preview',async()=>{
 store.set('geod-global-locale','zh-CN');desktopAvailable.mockReturnValue(false);view();
 expect(screen.getByRole('button',{name:'使用 Google 继续'}).disabled).toBe(true);
 expect(screen.getByRole('button',{name:'暂不登录，继续使用'})).toBeTruthy();expect(invoke).not.toHaveBeenCalled();
});
