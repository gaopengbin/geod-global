import React from 'react';
import {fireEvent,render,screen} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {I18nProvider} from './i18n.jsx';
import {GebcoExplorer} from './gebco-ui.jsx';
const native=vi.hoisted(()=>({props:null}));
vi.mock('./public-dataset-ui.jsx',()=>({PublicDatasetExplorer:p=>{native.props=p;return <section aria-label="TID original workflow">{p.workflowHeader}<button onClick={()=>p.onMutationChange(true)}>Start original download</button></section>;}}));
vi.mock('./gebco-client.js',async original=>({...await original(),gebcoRequest:vi.fn(async op=>op==='list'?[]:{})}));
vi.mock('./service-explore-map.jsx',()=>({ServiceExploreMap:()=> <section aria-label="Regional elevation map"/>}));
beforeEach(()=>{native.props=null;Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});});
it('GEBCO keeps regional elevation and whole global source-type originals distinct in the same Explore sidebar',async()=>{
  const changed=vi.fn();render(<I18nProvider><GebcoExplorer areaBounds={[116,39,117,40]} onMutationChange={changed}/></I18nProvider>);
  expect(screen.getByRole('region',{name:'Regional elevation map'})).toBeTruthy();
  fireEvent.click(screen.getByRole('combobox',{name:'GEBCO product'}));fireEvent.click(screen.getByRole('option',{name:'Global source-type grid · original tiles'}));
  expect(screen.getByRole('region',{name:'TID original workflow'})).toBeTruthy();expect(screen.queryByRole('region',{name:'Regional elevation map'})).toBeNull();
  expect(native.props).toMatchObject({datasetId:'gebco-tid',autoSearch:true,areaBounds:[116,39,117,40]});
  fireEvent.click(screen.getByRole('button',{name:'Start original download'}));expect(screen.getByRole('combobox',{name:'GEBCO product'}).disabled).toBe(true);expect(changed).toHaveBeenLastCalledWith(true);
});
