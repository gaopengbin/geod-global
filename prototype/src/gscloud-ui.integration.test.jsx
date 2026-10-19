import React from 'react';
import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {I18nProvider} from './i18n.jsx';
import {GSCloudExplorer} from './gscloud-ui.jsx';
import {gscloudRequest} from './gscloud-client.js';
const map=vi.hoisted(()=>({current:null}));
vi.mock('./gscloud-client.js',async original=>({...await original(),gscloudRequest:vi.fn()}));
vi.mock('./public-dataset-explore-map.jsx',()=>({PublicDatasetExploreMap:props=>{map.current=props;return <section aria-label="GSCloud footprint map"/>;}}));
const product='2c79f880a0304edebac6c48592d8e088',bounds=[116.2,39.8,116.5,40.1];
const products={products:[{id:product,title:'Landsat 8-9 C2 L2',seriesId:'LANDSAT'},{id:'305',title:'SRTM DEM',seriesId:'DEM'}]};
const item=(id='scene')=>({id,productId:product,datetime:'2025-09-01T00:00:00Z',presence:'available',downloadStatus:'account-required',cloudCover:12.15,geometry:null,bbox:null});
const detail=id=>({...item(id),bbox:bounds,geometry:{type:'Polygon',coordinates:[[[116.2,39.8],[116.5,39.8],[116.5,40.1],[116.2,40.1],[116.2,39.8]]]},bytes:1000000});
const component=props=><I18nProvider><GSCloudExplorer areaBounds={bounds} {...props}/></I18nProvider>;
beforeEach(()=>{Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});map.current=null;gscloudRequest.mockReset().mockImplementation(async(op,p)=>op==='products'?products:op==='search'?{request:p,items:[item()],total:1,hasMore:false}:{item:detail(p.itemId)});});
it('searches automatically, loads selected real footprints and never labels them as downloaded imagery',async()=>{
  render(component());await screen.findByRole('checkbox',{name:'Select scene'});
  expect(map.current.items).toEqual([]);expect(screen.getByText(/File listed · sign-in required/)).toBeTruthy();
  fireEvent.click(screen.getByRole('checkbox',{name:'Select scene'}));fireEvent.click(screen.getByRole('button',{name:'Preview selected'}));
  await waitFor(()=>expect(map.current.items).toHaveLength(1));expect(map.current.items[0].geometry.type).toBe('Polygon');
  expect(gscloudRequest.mock.calls.map(([op])=>op)).toEqual(['products','search','detail']);
  fireEvent.click(screen.getByRole('button',{name:'Source and download access'}));expect(screen.getByRole('link',{name:'Sign in on GSCloud'}).href).toBe('https://www.gscloud.cn/accounts/login_user');
});
it('changing area drops old results and cannot attach a late footprint from the previous query',async()=>{
  let finish;const pending=new Promise(resolve=>{finish=resolve;});gscloudRequest.mockImplementation(async(op,p)=>op==='products'?products:op==='search'?{request:p,items:[item()],total:1,hasMore:false}:pending);
  const view=render(component());await screen.findByRole('checkbox',{name:'Select scene'});fireEvent.click(screen.getByRole('checkbox',{name:'Select scene'}));fireEvent.click(screen.getByRole('button',{name:'Preview selected'}));
  await waitFor(()=>expect(gscloudRequest.mock.calls.some(([op])=>op==='detail')).toBe(true));view.rerender(component({areaBounds:[115,39,116,40]}));
  await act(async()=>finish({item:detail('scene')}));expect(map.current.items).toEqual([]);expect(screen.getByRole('button',{name:'Preview selected'}).disabled).toBe(true);
});
it('missing footprints remain a visible per-item failure with no fabricated point or rectangle',async()=>{
  gscloudRequest.mockImplementation(async(op,p)=>op==='products'?products:op==='search'?{request:p,items:[item()],total:1,hasMore:false}:{item:item()});
  render(component());await screen.findByRole('checkbox',{name:'Select scene'});fireEvent.click(screen.getByRole('checkbox',{name:'Select scene'}));fireEvent.click(screen.getByRole('button',{name:'Preview selected'}));
  await screen.findByRole('alert');expect(screen.getByRole('alert').textContent).toContain('no published footprint');expect(map.current.items).toEqual([]);
});
it('deduplicated changing pages cannot be reported to the Agent as a complete catalogue',async()=>{
  const context=vi.fn();gscloudRequest.mockImplementation(async(op,p)=>op==='products'?products:op==='search'?{request:p,items:[item()],total:2,hasMore:p.page===1}:{});
  render(component({onContextChange:context}));await screen.findByRole('checkbox',{name:'Select scene'});fireEvent.click(screen.getByRole('button',{name:'Load more items'}));
  await screen.findByText('GSCloud pages changed while loading. These results are incomplete; search again or narrow the date range.');
  await waitFor(()=>expect(context.mock.calls.at(-1)[0].catalog.complete).toBe(false));expect(screen.getAllByRole('checkbox')).toHaveLength(1);
});
