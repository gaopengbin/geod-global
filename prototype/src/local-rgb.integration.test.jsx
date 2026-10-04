import React from 'react';
import { beforeEach, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { RuntimeJobRows } from './runtime-ui.jsx';
import { normalizeScene } from './catalog.js';
import { runtimeRequest } from './runtime-client.js';
import { localRgbGroups } from './local-rgb.js';

const scene = normalizeScene(JSON.parse(readFileSync('prototype/public/samples/landsat-response.json','utf8')).features[0], 'planetary-landsat');
const jobs = ['red','green','blue'].map((key, index) => ({ id: `band-${index}`, kind:'download', itemId:scene.id, assetKey:key, href:scene.assets[key].href, mediaType:scene.assets[key].type, status:'succeeded', bytesDownloaded:1000, sha256:String(index + 1).repeat(64) }));
const metadata = { width:3,height:2,bandCount:3,dataType:'UInt16',crs:'EPSG:32610',bounds:[500000,4199940,500090,4200000],pixelSize:[30,30],pixelInterpretation:'PixelIsArea',nodata:0,previewWidth:3,previewHeight:2,previewDataUrl:'data:image/png;base64,AAAA',composite:{product:'landsat-c2-l2',sources:jobs.map(job=>({jobId:job.id,sha256:job.sha256,band:job.assetKey})),scale:.0000275,offset:-.2,displayRanges:[[100,200],[100,200],[100,200]],sampleCount:6,validSampleCount:6}};
beforeEach(()=> {
  vi.restoreAllMocks();
  delete window.__TAURI__;
  Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}});
});
it('complete triplets open local RGB, while incomplete downloads keep their single-band entry', async()=> {
  const view = render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs,projects:[]}}><RuntimeJobRows jobs={jobs} library/></RuntimeContext.Provider></I18nProvider>);
  const links = screen.getAllByRole('link',{name:'Open local RGB'});
  expect(links).toHaveLength(3);
  expect(links.every(link => link.hash === '#Workspace?rgb=band-0')).toBe(true);
  expect(localRgbGroups(jobs)).toHaveLength(1);
  view.unmount();
  render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:jobs.slice(0,2),projects:[]}}><RuntimeJobRows jobs={jobs.slice(0,2)} library/></RuntimeContext.Provider></I18nProvider>);
  expect(screen.getAllByRole('link',{name:'Open in workspace'})).toHaveLength(2);
  expect(screen.queryByRole('link',{name:'Open local RGB'})).toBeNull();
});
it('desktop IPC and browser API send only the three managed job IDs and validate source metadata', async()=> {
  const payload = { jobIds: jobs.map(job=>job.id) };
  const invoke = vi.fn().mockResolvedValue(metadata);
  window.__TAURI__ = {core:{invoke}};
  expect(await runtimeRequest('composite',payload)).toEqual(metadata);
  expect(invoke).toHaveBeenCalledWith('inspect_composite',{request:payload});
  await runtimeRequest('compositePixel',{...payload,x:500015,y:4199985});
  expect(invoke).toHaveBeenLastCalledWith('sample_composite',{request:{...payload,x:500015,y:4199985}});
  delete window.__TAURI__;
  const fetch = vi.spyOn(globalThis,'fetch').mockResolvedValue({ok:true,json:async()=>metadata});
  expect(await runtimeRequest('composite',payload)).toEqual(metadata);
  expect(fetch.mock.calls[0][0]).toBe('http://127.0.0.1:4318/rasters/composite');
  expect(fetch.mock.calls[0][1].method).toBe('POST');
  expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(payload);
});
