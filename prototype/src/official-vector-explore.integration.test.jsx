
async function acquire(user,{wait=true}={}){
  await user.click(await screen.findByRole('button',{name:/^(Save and download|保存与下载)$/}));
  await user.click(screen.getByRole('button',{name:'Confirm and download'}));
  if(wait){
    await waitFor(()=>expect(screen.getAllByRole('button',{name:/^(Close|关闭)$/})[0].disabled).toBe(false));
    await user.click(screen.getAllByRole('button',{name:/^(Close|关闭)$/})[0]);
    await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Confirm download'})).toBeNull());
  }
}
import React from 'react';import {beforeEach,afterEach,expect,it,vi} from 'vitest';import {cleanup,render,screen,waitFor} from '@testing-library/react';import userEvent from '@testing-library/user-event';
import assets from './official-vector-native.fixture.json';import {OfficialVectorExplorer} from './official-vector-explore.jsx';import {officialVectorRequest} from './official-vector-client.js';import {vectorRequest,exportVector} from './vector-client.js';
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(s,v={})=>s.replace(/\{(\w+)\}/g,(_,k)=>v[k]??''),number:String})}));
vi.mock('./official-vector-client.js',async load=>({...await load(),officialVectorRequest:vi.fn()}));vi.mock('./vector-client.js',async load=>({...await load(),vectorRequest:vi.fn(),exportVector:vi.fn()}));
vi.mock('./explore-source-workspace.jsx',()=>({ExploreSourceFrame:({children,actions,map})=><main><aside>{children}{actions}</aside>{map}</main>}));vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:({entries,areaBounds})=><output data-testid="map">{JSON.stringify({areaBounds,ids:entries.map(e=>e.key)})}</output>}));
const asset=assets[0],packageInfo=asset.officialPackage.package,bounds=asset.officialPackage.requestedBounds;
beforeEach(()=>{vi.clearAllMocks();officialVectorRequest.mockImplementation(async operation=>operation==='search'?{items:[packageInfo]}:asset);vectorRequest.mockResolvedValue({asset,geojson:{type:'FeatureCollection',features:[]}});});afterEach(cleanup);
it('uses the existing area to select a whole official partition and exports the original bytes',async()=>{const user=userEvent.setup();render(<OfficialVectorExplorer datasetId="ms-buildings" areaBounds={bounds}/>);await user.click(await screen.findByRole('checkbox',{name:'Select result '+packageInfo.title}));await acquire(user);await waitFor(()=>expect(screen.getByTestId('map').textContent).toContain(asset.id));expect(officialVectorRequest).toHaveBeenCalledWith('download',{datasetId:'ms-buildings',packageId:packageInfo.id,bounds},expect.any(AbortSignal));await user.click(screen.getByRole('button',{name:'Export original '+packageInfo.title}));expect(exportVector).toHaveBeenCalledWith(asset.id,true);});
it('does not retain results or late downloaded geometry after the selected area changes',async()=>{let finish;const pending=new Promise(r=>finish=r);officialVectorRequest.mockImplementation(async operation=>operation==='search'?{items:[packageInfo]}:pending);const user=userEvent.setup(),view=render(<OfficialVectorExplorer datasetId="ms-buildings" areaBounds={bounds}/>);await user.click(await screen.findByRole('checkbox'));await acquire(user,{wait:false});view.rerender(<OfficialVectorExplorer datasetId="ms-buildings" areaBounds={[10,10,11,11]}/>);finish(asset);await waitFor(()=>expect(screen.getByTestId('map').textContent).toContain('"areaBounds":[10,10,11,11]'));expect(screen.getByTestId('map').textContent).toContain('"ids":[]');});
it('clearing the area removes old official packages and never invents a substitute area',async()=>{const view=render(<OfficialVectorExplorer datasetId="ms-buildings" areaBounds={bounds}/>);await screen.findByRole('checkbox');view.rerender(<OfficialVectorExplorer datasetId="ms-buildings"/>);await waitFor(()=>expect(screen.queryByRole('checkbox')).toBeNull());expect(screen.getByText('Select a query region on Explore first.')).toBeTruthy();expect(officialVectorRequest).toHaveBeenCalledTimes(1);});
