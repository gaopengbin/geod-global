import React from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StacRasterInspection } from './stac-ui.jsx';
import { stacRequest } from './stac-client.js';
import actual from './stac-map-next-ghsl.fixture.json';

const capture=vi.hoisted(()=>({maps:[],sources:[],layers:[],fits:[]}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:(text,values={})=>text.replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),number:String,date:String,locale:'en'})}));
vi.mock('./stac-client.js',async original=>({...await original(),stacRequest:vi.fn()}));
// Check the real map component's anchoring, clip, fit and click wiring. These
// OpenLayers boundary doubles cannot claim a rendered live browser map.
vi.mock('ol/source/ImageStatic.js',()=>({default:class{constructor(options){capture.sources.push(options);}}}));
vi.mock('ol/layer/Image.js',()=>({default:class{constructor(options){capture.layers.push(options);}}}));
vi.mock('ol/View.js',()=>({default:class{constructor(options){this.options=options;}fit(bounds){capture.fits.push(bounds);}getZoom(){return 3;}setZoom(){}}}));
vi.mock('ol/Map.js',()=>({default:class{constructor(options){this.options=options;this.handlers={};capture.maps.push(this);}getView(){return this.options.view;}on(event,handler){this.handlers[event]=handler;}updateSize(){}setTarget(){}}}));

const job={id:actual.inspection.jobId,sha256:actual.inspection.sha256,title:'GHSL · Residential population · 2020',
  href:actual.snapshot.assets[0].href,stacSource:{snapshotId:actual.snapshot.id,assetKey:'raster'}};
beforeEach(()=>{
  vi.clearAllMocks(); for(const values of Object.values(capture))values.length=0;
  stacRequest.mockImplementation(async(operation,payload)=>{
    if(operation==='inspect')return actual.inspection;
    if(operation==='snapshot')return actual.snapshot;
    if(operation==='pixel')return actual.pixels.find(p=>p.column===payload.column&&p.row===payload.row);
    throw new Error('Unexpected map playback request '+operation);
  });
});

it('actual POP result anchors the original PNG, clips only the legal display and reads the unchanged original coordinate',async()=>{
  render(<StacRasterInspection job={job} onClose={vi.fn()}/>);
  await screen.findByRole('button',{name:'Fit original raster'});
  const legal=[-180,actual.inspection.bounds[1],180,actual.inspection.bounds[3]];
  expect(capture.sources).toHaveLength(1);
  expect(capture.sources[0].imageExtent).toEqual(actual.inspection.bounds);
  expect(capture.sources[0].imageExtent).not.toEqual(legal);
  expect(capture.sources[0].url).toBe(actual.inspection.previewDataUrl);
  expect(capture.layers[0].extent).toEqual(legal); expect(capture.fits[0]).toEqual(legal);
  expect(screen.queryByText(/Map placement is unavailable/)).toBeNull();
  expect(screen.getByRole('note').textContent).toContain('all 43202 columns remain unchanged');
  const column=35568,row=5903,data=actual.inspection;
  capture.maps[0].handlers.singleclick({coordinate:[data.transform[2]+(column+0.5)*data.transform[0],data.transform[5]+(row+0.5)*data.transform[4]]});
  await waitFor(()=>expect(document.querySelector('.stac-pixel-result strong')?.textContent).toBe('4307.731181065241'));
  expect(stacRequest.mock.calls.filter(([op])=>op==='pixel').at(-1)[1]).toEqual({id:job.id,column,row});
  expect(screen.getByRole('textbox',{name:'Pixel column'}).value).toBe('35568');
  expect(screen.getByRole('textbox',{name:'Pixel row'}).value).toBe('5903');
});

it('the clipped display leaves the published edge columns readable with valid zero and original metadata visible',async()=>{
  const user=userEvent.setup(); render(<StacRasterInspection job={job} onClose={vi.fn()}/>);
  await screen.findByRole('button',{name:'Fit original raster'});
  for(const pixel of actual.pixels.slice(1)){
    await user.clear(screen.getByRole('textbox',{name:'Pixel column'}));
    await user.type(screen.getByRole('textbox',{name:'Pixel column'}),String(pixel.column));
    await user.clear(screen.getByRole('textbox',{name:'Pixel row'}));
    await user.type(screen.getByRole('textbox',{name:'Pixel row'}),String(pixel.row));
    await user.click(screen.getByRole('button',{name:'Read raw pixel'}));
    await waitFor(()=>expect(document.querySelector('.stac-pixel-result strong')?.textContent).toBe('0'));
    expect(stacRequest.mock.calls.filter(([op])=>op==='pixel').at(-1)[1]).toEqual({id:job.id,column:pixel.column,row:pixel.row});
  }
  await user.click(screen.getByRole('button',{name:'File metadata and display'}));
  expect(screen.getByText(actual.inspection.bounds.join(', '))).toBeTruthy();
  expect(screen.getByText(actual.inspection.transform.join(', '))).toBeTruthy();
  expect(stacRequest.mock.calls.some(([op])=>['project','downloads'].includes(op))).toBe(false);
});
