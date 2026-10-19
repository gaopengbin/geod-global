
async function acquire(user,{wait=true}={}){
  await user.click(await screen.findByRole('button',{name:/^(Save and download|保存与下载)$/}));
  await user.click(screen.getByRole('button',{name:'Confirm and download'}));
  if(wait){
    await waitFor(()=>expect(screen.getAllByRole('button',{name:/^(Close|关闭)$/})[0].disabled).toBe(false));
    await user.click(screen.getAllByRole('button',{name:/^(Close|关闭)$/})[0]);
    await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Confirm download'})).toBeNull());
  }
}
import React from 'react';import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {cleanup,render,screen,waitFor,within} from '@testing-library/react';import userEvent from '@testing-library/user-event';
import fixture from './overture-native.fixture.json';
import {OvertureExplorer} from './overture-explore-ui.jsx';
import {overtureRequest,exportOvertureOriginal} from './overture-client.js';import {vectorRequest} from './vector-client.js';
import chinese from './locales/public-dataset.zh-CN.js';
const language=vi.hoisted(()=>({value:'en'}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:s=>language.value==='zh-CN'?(chinese[s]||s):s,number:String})}));
vi.mock('./overture-client.js',async load=>({...await load(),overtureRequest:vi.fn(),exportOvertureOriginal:vi.fn()}));
vi.mock('./vector-client.js',async load=>({...await load(),vectorRequest:vi.fn()}));
vi.mock('./explore-source-workspace.jsx',()=>({ExploreSourceFrame:({children,actions,map})=><main><aside>{children}{actions}</aside>{map}</main>}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:({entries,areaBounds})=><output data-testid="map">{JSON.stringify({areaBounds,features:entries[0]?.data.geojson.features.length??0})}</output>}));
const bounds=[-71.5,40.5,-70.5,41.5];
beforeEach(()=>{language.value='en';vi.clearAllMocks();vectorRequest.mockImplementation(async operation=>operation==='list'?[]:fixture);overtureRequest.mockImplementation(async operation=>operation==='search'?{items:[{id:'base-bathymetry-2026-09-23.1-part-00000'}]}:fixture.asset);});
afterEach(cleanup);
it('uses distinct wrapping text actions in Chinese and original export never invokes the adjacent properties action',async()=>{
  language.value='zh-CN';const user=userEvent.setup(),view=render(<OvertureExplorer areaBounds={bounds}/>);
  await screen.findByText(/完整 GeoParquet 原件/);expect(screen.getByText(/原始行/)).toBeTruthy();
  await acquire(user);await screen.findByTestId('overture-preview-count');
  const actions=view.container.querySelector('.overture-original-actions');expect(actions).toBeTruthy();expect(view.container.querySelector('.service-result-actions')).toBeNull();expect(within(actions).getAllByRole('button')).toHaveLength(3);
  await user.click(within(actions).getByRole('button',{name:'导出完整原件'}));await waitFor(()=>expect(exportOvertureOriginal).toHaveBeenCalledWith(fixture.asset));expect(overtureRequest.mock.calls.some(([operation])=>operation==='rows')).toBe(false);
  expect(screen.getByRole('button',{name:'Files and area'})).toBeTruthy();
});
it('keeps original/preview counts distinct and shows the actual selected geometry',async()=>{
  const user=userEvent.setup();render(<OvertureExplorer areaBounds={bounds}/>);
  await screen.findByText(/407277 Original rows/);
  await acquire(user);
  await screen.findByTestId('overture-preview-count');expect(screen.getByTestId('overture-preview-count').textContent).toContain('11 Features');
  expect(screen.getByTestId('map').textContent).toContain('"features":11');
  expect(overtureRequest).toHaveBeenCalledWith('download',expect.objectContaining({bounds}),expect.any(AbortSignal));
  await user.click(screen.getByRole('button',{name:'Export complete original'}));
  await waitFor(()=>expect(exportOvertureOriginal).toHaveBeenCalledWith(fixture.asset));
});
it('reopens a matching saved preview without downloading another original',async()=>{
  vectorRequest.mockImplementation(async operation=>operation==='list'?[fixture.asset]:fixture);
  const user=userEvent.setup();render(<OvertureExplorer areaBounds={bounds}/>);
  await user.click(await screen.findByRole('button',{name:'Reopen saved preview'}));
  await waitFor(()=>expect(screen.getByTestId('map').textContent).toContain('"features":11'));
  expect(overtureRequest.mock.calls.some(([operation])=>operation==='download')).toBe(false);
});
it('preserves the new area and never attaches a late preview from the previous area',async()=>{
  let finish;const pending=new Promise(resolve=>{finish=resolve;});
  overtureRequest.mockImplementation(async operation=>operation==='search'?{items:[{}]}:pending);
  const user=userEvent.setup(),view=render(<OvertureExplorer areaBounds={bounds}/>);
  await acquire(user,{wait:false});
  view.rerender(<OvertureExplorer areaBounds={[1,2,3,4]}/>);finish(fixture.asset);
  await waitFor(()=>expect(screen.getByTestId('map').textContent).toContain('"areaBounds":[1,2,3,4]'));
  expect(screen.getByTestId('map').textContent).toContain('"features":0');
});
