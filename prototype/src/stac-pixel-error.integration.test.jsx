import React from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StacRasterInspection } from './stac-ui.jsx';
import { stacRequest } from './stac-client.js';
import { stacInspection, stacSnapshot, stacJob } from './stac-fixtures.js';
import chinese from './locales/stac.zh-CN.js';

const language=vi.hoisted(()=>({value:'en'}));
const actualRequest=vi.hoisted(()=>({pixel:null}));
vi.mock('./i18n.jsx',()=>({useI18n:()=>({
  t:(text,values={})=>(language.value==='zh-CN'?chinese[text]||text:text).replace(/\{(\w+)\}/g,(_,key)=>values[key]??''),
  date:String,number:String,locale:language.value,
})}));
vi.mock('./stac-client.js',async original=>{
  const module=await original(); actualRequest.pixel=module.stacRequest;
  return {...module,stacRequest:vi.fn()};
});
vi.mock('./stac-map.jsx',()=>({default:()=> <div aria-label="Georeferenced original raster"/>}));
beforeEach(()=>{
  vi.clearAllMocks(); delete window.__TAURI__;
  stacRequest.mockImplementation(async(operation,payload)=>operation==='inspect'?stacInspection
    :operation==='snapshot'?stacSnapshot:actualRequest.pixel(operation,payload));
});
afterEach(()=>vi.unstubAllGlobals());

it.each([
  ['zh-CN','读取原始像素','该位置的原始像元无法解码，请选择其他位置查看。'],
  ['en','Read raw pixel','The original pixel at this position could not be decoded. Choose another position to inspect.'],
])('a real-style HTTP 400 LZW pixel response is localized in %s and the next read remains usable',async(locale,button,message)=>{
  language.value=locale;
  const fetch=vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({error:'no lzw end code found'}),{status:400}))
    .mockResolvedValueOnce(new Response(JSON.stringify({jobId:stacJob.id,sha256:stacJob.sha256,column:0,row:0,values:[42],noData:[false]}),{status:200}));
  vi.stubGlobal('fetch',fetch);
  const user=userEvent.setup(); render(<StacRasterInspection job={stacJob} onClose={vi.fn()}/>);
  await user.click(await screen.findByRole('button',{name:button}));
  expect((await screen.findByRole('alert')).textContent).toBe(message);
  expect(screen.queryByText('no lzw end code found')).toBeNull();
  expect(fetch.mock.calls[0][0]).toBe(`http://127.0.0.1:4318/stac/jobs/${stacJob.id}/pixel?column=0&row=0`);
  expect(screen.getByRole('button',{name:button}).disabled).toBe(false);
  await user.click(screen.getByRole('button',{name:button}));
  await waitFor(()=>expect(document.querySelector('.stac-pixel-result strong')?.textContent).toBe('42'));
  expect(screen.queryByRole('alert')).toBeNull();
  expect(fetch.mock.calls.every(([,options])=>options.method==='GET')).toBe(true);
  expect(stacRequest.mock.calls.some(([op])=>['project','downloads'].includes(op))).toBe(false);
});
