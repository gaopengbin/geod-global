import React from 'react';
import {expect,it,vi} from 'vitest';
import {render,screen,within,waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {MapError} from './workspace-map.jsx';

vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:text=>text})}));

it('keeps a retryable map notice compact and opens technical text outside the panel',async()=>{
  const user=userEvent.setup(),retry=vi.fn();
  const {rerender}=render(<MapError message="Local service connection refused" onRetry={retry}/>);
  const alert=screen.getByRole('alert'),info=screen.getByRole('button',{name:'Technical details'});
  expect(alert.querySelector('[data-slot=disclosure]')).toBeNull();
  expect(info.textContent).toBe('');
  expect(screen.queryByText('Local service connection refused')).toBeNull();
  await user.click(info);
  expect(within(screen.getByRole('dialog',{name:'Technical details'})).getByText('Local service connection refused')).toBeTruthy();
  expect(within(alert).queryByText('Local service connection refused')).toBeNull();
  await user.keyboard('{Escape}');await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
  expect(document.activeElement).toBe(info);
  await user.click(screen.getByRole('button',{name:'Retry map request'}));
  expect(retry).toHaveBeenCalledOnce();
  rerender(<MapError message="Local service connection refused" onRetry={retry} busy/>);
  expect(screen.getByRole('button',{name:'Retry map request'}).disabled).toBe(true);
  expect(screen.getByRole('button',{name:'Technical details'}).disabled).toBe(false);
});
