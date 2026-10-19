import React from 'react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {render,screen,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import fixture from './sinolc-reuse.fixture.json';
import {PublicDatasetExplorer} from './public-dataset-ui.jsx';
import {RuntimeContext} from './runtime-context.js';
import {publicDatasetRequest} from './public-dataset-client.js';
import {stacRequest,validateStacInspection} from './stac-client.js';

const rendered=vi.hoisted(()=>vi.fn());
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),number:String,locale:'en'})}));
vi.mock('./public-dataset-client.js',async original=>({...await original(),publicDatasetRequest:vi.fn()}));
vi.mock('./stac-client.js',async original=>({...await original(),stacRequest:vi.fn()}));
vi.mock('./public-dataset-explore-map.jsx',()=>({PublicDatasetExploreMap:()=> <section aria-label="Product footprint map"/>}));
vi.mock('./native-raster-explore-map.jsx',()=>({NativeRasterExploreMap:props=>{rendered(props);return <section aria-label="Loaded original map">{props.entries.map(e=><span key={e.key}>{e.key}</span>)}</section>;}}));

const clone=value=>JSON.parse(JSON.stringify(value));
let current,original,inspection;
beforeEach(()=>{
  vi.clearAllMocks();original=clone(fixture.snapshot);inspection=clone(fixture.inspection);
  current={...clone(original),id:'c'.repeat(64),retrievedAt:'2026-10-14T13:00:00+00:00'};
  const proof=current.properties.datasetPreset;
  const source={id:proof.sourceId,title:proof.sourceTitle,bounds:proof.sourceBounds,stac:{url:current.assets[0].href,kind:'raster',name:'SinoLC reviewed original'}};
  const plan={dataset:proof.dataset,request:proof.request,sources:[source],warnings:[]};
  publicDatasetRequest.mockImplementation(async operation=>{
    if(operation==='list')return [proof.dataset];
    if(operation==='connect')return {complete:true,plan,wcsConnections:[],stacConnections:[{id:current.connectionId,url:source.stac.url,name:source.stac.name,kind:'raster',snapshotIds:[current.id],metadataSha256:[current.documentSha256]}]};
    throw new Error('Unexpected dataset mutation '+operation);
  });
  stacRequest.mockImplementation(async(operation,payload)=>{
    if(operation==='snapshot')return payload.id===current.id?current:original;
    if(operation==='inspect')return validateStacInspection(inspection,payload.id);
    throw new Error('Unexpected download or write '+operation);
  });
});
afterEach(()=>vi.restoreAllMocks());
const mount=()=>render(<RuntimeContext.Provider value={{projects:[],jobs:[fixture.job]}}><PublicDatasetExplorer datasetId="sinolc-1" areaBounds={current.properties.datasetPreset.request.bounds} autoSearch/></RuntimeContext.Provider>);
const select=async user=>user.click(await screen.findByRole('checkbox',{name:`Select asset · ${current.itemId} · raster`}));

it('fresh Explore metadata enables loading the saved whole original without queuing another job',async()=>{
  const user=userEvent.setup();mount();await select(user);
  const load=screen.getByRole('button',{name:'Load selected files to map'});
  await user.click(screen.getByRole('button',{name:'Preview details'}));
  expect(screen.getByText('Loading checks the saved whole original source and separate ZIP/TIFF checksum receipts; it does not start another download.')).toBeTruthy();
  expect(screen.queryByText('Download the complete original ZIP first. The map reads the separately verified whole TIFF member; both original checksums are retained.')).toBeNull();
  expect(screen.queryByText('Some selected originals have no online TIFF preview. They remain available for saving and download.')).toBeNull();
  await user.keyboard('{Escape}');expect(load.disabled).toBe(false);await user.click(load);
  expect(await screen.findByRole('region',{name:'Loaded original map'})).toBeTruthy();
  const entry=rendered.mock.calls.at(-1)[0].entries[0];
  expect(entry.job.id).toBe(fixture.job.id);expect(entry.item.id).toBe(fixture.snapshot.id);
  expect(entry.data.archiveOriginal.sourcePin).toEqual(fixture.job.stacSource);
  expect(stacRequest.mock.calls.map(([operation])=>operation)).toEqual(['snapshot','snapshot','inspect']);
  expect(publicDatasetRequest.mock.calls.map(([operation])=>operation)).toEqual(['list','connect']);
});

it('a changed original source proof fails reuse and removes the previous original map',async()=>{
  const user=userEvent.setup();mount();await select(user);
  await user.click(screen.getByRole('button',{name:'Load selected files to map'}));
  await screen.findByRole('region',{name:'Loaded original map'});
  original.assets[0].metadata.datasetPreset.request.bounds=[121.4,31.1,121.6,31.3];
  await user.click(screen.getByRole('button',{name:'Load selected files to map'}));
  expect(await screen.findByRole('alert')).toHaveProperty('textContent','The original ZIP and TIFF member do not match their saved dual-checksum receipt.');
  await waitFor(()=>expect(screen.queryByRole('region',{name:'Loaded original map'})).toBeNull());
  expect(stacRequest.mock.calls.filter(([operation])=>operation==='inspect')).toHaveLength(1);
});
