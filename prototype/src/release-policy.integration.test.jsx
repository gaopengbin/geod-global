import React from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { I18nProvider } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { DownloadAssetButton } from './runtime-ui.jsx';
import { PROVIDERS } from './providers.js';
import { PROTECTED_ORIGINAL_NOTICE } from './release-policy.js';
import { runtimeRequest } from './runtime-client.js';
vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn() }));
beforeEach(() => {
  Object.defineProperty(window, 'localStorage', { configurable:true, value:{getItem:()=> 'en',setItem:vi.fn()} });
  runtimeRequest.mockClear();
});
it.each(PROVIDERS.filter(p=>p.account))('withholds $id downloads while retaining the correct authorization route', source => {
  render(<I18nProvider><RuntimeContext.Provider value={{health:{},jobs:[],refresh:vi.fn()}}>
    <DownloadAssetButton scene={{provider:source.id}}/>
  </RuntimeContext.Provider></I18nProvider>);
  expect(screen.getByText(PROTECTED_ORIGINAL_NOTICE)).toBeTruthy();
  expect(screen.getByRole('link',{name:'Manage authorization'}).getAttribute('href')).toBe('#Settings?account='+source.account);
  expect(screen.queryByRole('button',{name:/download/i})).toBeNull();
  expect(runtimeRequest).not.toHaveBeenCalled();
});
