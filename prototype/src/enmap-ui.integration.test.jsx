import React from 'react';
import {render,screen,cleanup,within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {afterEach,expect,test,vi} from 'vitest';
import {SpectralBandSelect,SpectralPixelTable} from './enmap-ui.jsx';
import {enmapEncoding} from './enmap-client.js';
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:text=>text})}));
afterEach(cleanup);
test('all wavelengths remain selectable and waiting disables repeat band changes',async()=>{
  const user=userEvent.setup(),change=vi.fn(),encoding=enmapEncoding();
  const {rerender}=render(<SpectralBandSelect data={{spectralEncoding:encoding,displayBand:50}} onChange={change}/>);
  await user.click(screen.getByRole('combobox',{name:'Display spectral band'}));
  expect(screen.getAllByRole('option')).toHaveLength(224);
  await user.click(screen.getByRole('option',{name:'Band 224 · 2445.3 nm'}));expect(change).toHaveBeenCalledWith(224);
  rerender(<SpectralBandSelect data={{spectralEncoding:encoding,displayBand:224}} disabled onChange={change}/>);
  expect(screen.getByRole('combobox',{name:'Display spectral band'}).disabled).toBe(true);
});
test('compact spectral rows separate raw samples, header scaling and NoData',()=>{
  const encoding=enmapEncoding(),values=[-32768,-100,20000,0,...Array.from({length:220},(_,i)=>i)];
  render(<SpectralPixelTable encoding={encoding} pixel={{values,noData:values.map(v=>v===-32768)}}/>);
  const rows=within(screen.getByRole('table')).getAllByRole('row');expect(rows).toHaveLength(225);
  expect(rows[1].textContent).toContain('-32768');expect(rows[1].textContent).toContain('NoData');
  expect(rows[2].textContent).toContain('-0.0100');expect(rows[3].textContent).toContain('2.0000');expect(rows[4].textContent).toContain('0.0000');
});
