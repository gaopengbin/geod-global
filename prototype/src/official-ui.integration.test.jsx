import React from 'react';
import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,test,vi} from 'vitest';
import {I18nProvider} from './i18n.jsx';
import {ConversationMonitorReviews,ImageryMonitoringPage,MembershipPanel,OfficialServicesProvider} from './official-ui.jsx';
import {AccountPage} from './account-ui.jsx';
const identity=vi.hoisted(()=>({snapshot:{user:{id:'a'.repeat(32)}}}));
vi.mock('./identity-ui.jsx',()=>({useIdentity:()=>identity,AccountAvatar:()=> <span/>}));
const geometry={type:'Polygon',coordinates:[[[116,40],[116.1,40],[116.1,40.1],[116,40.1],[116,40]]]};
let account,monitors,quotes,notifications,orders,caps,invoke;
const envelope=value=>({product:'xyz.laogao.geod.global',version:1,...value});
const view=props=><I18nProvider><OfficialServicesProvider><ImageryMonitoringPage initialArea={{geometry,bounds:[116,40,116.1,40.1],place:{name:'Selected region'}}} onPreviewArea={vi.fn()} {...props}/></OfficialServicesProvider></I18nProvider>;
const membershipView=()=><I18nProvider><OfficialServicesProvider><MembershipPanel/></OfficialServicesProvider></I18nProvider>;
beforeEach(()=>{
 const storage=new Map([['geod-global-locale','en']]);Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:k=>storage.get(k)??null,setItem:(k,v)=>storage.set(k,String(v))}});
 identity.snapshot={user:{id:'a'.repeat(32)}};
 account={plan:'pro',mode:'beta-test-credits',proExpiresAt:1791532800,buckets:[{kind:'trial',availableMicros:100000000,reservedMicros:0}],ledger:[]};monitors=[];quotes=[];notifications=[];orders=[];
 caps={mode:'beta-test-credits',sources:[{id:'earth-search',name:'Sentinel-2'}],checkout:{available:false,products:[{id:'pro-monthly',priceMinor:1900,purchasable:false}]}};
 invoke=vi.fn(async(command,{operation,payload}={})=>{
  if(command!=='official_request')return undefined;
  if(operation==='capabilities')return envelope(caps);
  if(operation==='account')return envelope({account});
  if(operation==='monitors')return envelope({monitors});
  if(operation==='quotes')return envelope({quotes});
  if(operation==='notifications')return envelope({notifications});
  if(operation==='orders')return envelope({orders});
  if(operation==='quoteMonitor'){
   const quote={id:'c'.repeat(32),hash:'h'.repeat(64),config:payload,checkCostMicros:1000000,expiresAt:1791532900};quotes=[quote];return envelope({quote});
  }
  if(operation==='acceptMonitor'){
   const monitor={id:'d'.repeat(32),config:quotes[0].config,state:'active',runs:[],last_check:null};monitors=[monitor];quotes=[];return envelope({monitor});
  }
  if(operation==='checkout')return envelope({order:{id:'e'.repeat(32),browserOpened:true}});
  return envelope({ok:true});
 });
 window.__TAURI__={core:{invoke}};
});
test('monitoring review preserves polygon and requires explicit human confirmation',async()=>{
 render(view());
 const add=await screen.findByRole('button',{name:'Add monitored area'});
 await waitFor(()=>expect(add.disabled).toBe(false));fireEvent.click(add);
 fireEvent.change(screen.getByLabelText('Task name'),{target:{value:'Monitor selected region'}});
 fireEvent.click(screen.getByRole('button',{name:'Prepare monitoring review'}));
 await screen.findByRole('button',{name:'Confirm monitoring task'});
 const preparation=invoke.mock.calls.find(([,p])=>p?.operation==='quoteMonitor')[1].payload;
 expect(preparation.geometry).toEqual(geometry);expect(preparation).not.toHaveProperty('isPro');
 expect(invoke.mock.calls.some(([,p])=>p?.operation==='acceptMonitor')).toBe(false);
 expect(screen.getByText(/does not authorize imagery purchases or downloads/)).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'Confirm monitoring task'}));
 await screen.findByText('Monitoring is active. New matches will appear in your notifications.');
 expect(invoke.mock.calls.find(([,p])=>p?.operation==='acceptMonitor')[1].payload).toEqual({id:'c'.repeat(32),hash:'h'.repeat(64)});
 expect(invoke.mock.calls.every(([command])=>['official_request','set_desktop_locale'].includes(command))).toBe(true);
});
test('Free and unavailable checkout remain usable without misleading upgrade actions',async()=>{
 account.plan='free';render(view());
 await screen.findByText('Imagery monitoring is a Pro feature in testing.');
 expect(screen.getByRole('button',{name:'Add monitored area'}).disabled).toBe(true);
 expect(screen.queryByRole('button',{name:'Service credits and sandbox payments'})).toBeNull();
 expect(screen.queryByRole('button',{name:'Test checkout'})).toBeNull();
 expect(invoke.mock.calls.some(([,p])=>p?.operation==='checkout')).toBe(false);
 expect(screen.getByRole('link',{name:'View membership'}).getAttribute('href')).toBe('#Account');
});
test('sandbox checkout opens only after an explicit review and never activates client Pro',async()=>{
 account.plan='free';account.sandboxCheckoutAllowed=true;caps.checkout={available:true,products:[{id:'credits-1000',priceMinor:1000,purchasable:true}]};render(membershipView());
 await screen.findByText('Free');
 fireEvent.click(screen.getByRole('button',{name:'Service credits and sandbox payments'}));
 fireEvent.click(screen.getByRole('button',{name:'Test checkout'}));
 await screen.findByRole('dialog',{name:'Confirm sandbox checkout'});
 expect(invoke.mock.calls.some(([,p])=>p?.operation==='checkout')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'Open test checkout'}));
 await screen.findByText(/Complete the test payment in your browser/);
 expect(screen.getByText('Free')).toBeTruthy();
});
test('late account responses cannot leak a previous users balance after account switching',async()=>{
 let release;const original=invoke.getMockImplementation();let first=true;
 invoke.mockImplementation(async(command,params)=>{if(params?.operation==='account'&&first){first=false;return new Promise(resolve=>{release=()=>resolve(envelope({account:{...account,buckets:[{availableMicros:999000000}]}}));});}return original(command,params);});
 const page=render(membershipView());await waitFor(()=>expect(release).toBeTruthy());
 identity.snapshot={user:{id:'b'.repeat(32)}};account={...account,plan:'free',buckets:[{availableMicros:1000000}]};page.rerender(membershipView());
 await screen.findByText('1',{selector:'.membership-balance strong'});await act(async()=>release());
 expect(screen.queryByText('999',{selector:'.membership-balance strong'})).toBeNull();
});
test('normal accounts never expose payment tests even when the provider is configured',async()=>{
 account.sandboxCheckoutAllowed=false;caps.checkout.available=true;render(membershipView());
 await screen.findByText('Pro');
 expect(screen.queryByRole('button',{name:'Test checkout'})).toBeNull();
 expect(screen.queryByRole('button',{name:'Service credits and sandbox payments'})).toBeNull();
});
test('unavailable monitoring offers local imagery actions without a billing console',async()=>{
 const original=invoke.getMockImplementation();invoke.mockImplementation((command,params)=>params?.operation==='capabilities'?Promise.reject(Error('unavailable')):original(command,params));
 const onOpenAgent=vi.fn(),onExplore=vi.fn();render(view({onOpenAgent,onExplore}));
 await screen.findByText('Monitoring is being prepared. You can search and download imagery with Agent now.');
 fireEvent.click(screen.getByRole('button',{name:'Find imagery with Agent'}));fireEvent.click(screen.getByRole('button',{name:'Explore imagery'}));
 expect(onOpenAgent).toHaveBeenCalledOnce();expect(onExplore).toHaveBeenCalledOnce();
 expect(screen.queryByRole('alert')).toBeNull();expect(screen.queryByText('Membership')).toBeNull();
});
test('membership and usage are reachable through the account page without reopening OAuth',async()=>{
 identity.snapshot.user={...identity.snapshot.user,name:'Test Global User',email:'fixture@example.invalid'};identity.run=vi.fn();
 render(<I18nProvider><OfficialServicesProvider><AccountPage/></OfficialServicesProvider></I18nProvider>);
 await screen.findByText('Test Global User');await screen.findByText('100',{selector:'.membership-balance strong'});
 expect(screen.getByRole('region',{name:'Membership and usage'})).toBeTruthy();
 expect(screen.queryByRole('button',{name:'Continue with Google'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'Sign out of this device'}));expect(identity.run).toHaveBeenCalledWith('logout');
});
test('Agent review appears only in its conversation and starts only after a human click',async()=>{
 quotes=[{id:'c'.repeat(32),hash:'h'.repeat(64),conversationId:'current',config:{...geometry,name:'Agent prepared area',sourceId:'earth-search',intervalHours:24,lookbackDays:30,cloudMax:5,dailyCreditLimit:5,totalCreditLimit:100,geometry},checkCostMicros:1000000,expiresAt:1791532900}];
 const renderConversation=id=><I18nProvider><OfficialServicesProvider><ConversationMonitorReviews conversationId={id}/></OfficialServicesProvider></I18nProvider>;
 const page=render(renderConversation('different'));
 await waitFor(()=>expect(invoke.mock.calls.some(([,p])=>p?.operation==='quotes')).toBe(true));
 expect(screen.queryByRole('button',{name:'Confirm monitoring task'})).toBeNull();
 page.rerender(renderConversation('current'));
 fireEvent.click(await screen.findByRole('button',{name:'Confirm monitoring task'}));
 await waitFor(()=>expect(invoke.mock.calls.find(([,p])=>p?.operation==='acceptMonitor')[1].payload).toEqual({id:'c'.repeat(32),hash:'h'.repeat(64)}));
 await waitFor(()=>expect(screen.queryByRole('button',{name:'Confirm monitoring task'})).toBeNull());
});
