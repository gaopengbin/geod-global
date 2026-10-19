import React from 'react';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {cleanup,render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {I18nProvider} from './i18n.jsx';
import AgentArchiveResult from './agent-archive-result.jsx';
import {inspectGSCloudOriginal} from './gscloud-client.js';
vi.mock('./gscloud-client.js',()=>({inspectGSCloudOriginal:vi.fn()}));
const id='a1234567-1234-1234-1234-123456789abc';
const archive={kind:'archive',jobId:id,itemId:'ASTGTM2_N40E116',productId:'421',bytes:17773361,sha256:'a'.repeat(64),members:[{name:'ASTGTM2_N40E116_dem.tif',bytes:25934402},{name:'ASTGTM2_N40E116_num.tif',bytes:25934402},{name:'README.pdf',bytes:272476}]};
describe('original archive result',()=>{
  beforeEach(()=>{vi.clearAllMocks();inspectGSCloudOriginal.mockResolvedValue(archive);});
  afterEach(cleanup);
  it('displays the native manifest and offers a folder action without inventing raster preview',async()=>{
    render(<I18nProvider><AgentArchiveResult id={id}/></I18nProvider>);
    expect(await screen.findByText('ASTGTM2_N40E116_dem.tif')).toBeTruthy();
    expect(screen.getByText('ASTGTM2_N40E116_num.tif')).toBeTruthy();expect(screen.getByText('README.pdf')).toBeTruthy();
    expect(screen.getByRole('button',{name:'Open folder'})).toBeTruthy();
    expect(screen.queryByRole('button',{name:'Map preview'})).toBeNull();
    expect(inspectGSCloudOriginal).toHaveBeenCalledWith(id);
  });
  it('keeps an inspection failure visible and retries verification before showing files',async()=>{
    inspectGSCloudOriginal.mockRejectedValueOnce(Error('GSCloud archive SHA-256 changed.'));
    render(<I18nProvider><AgentArchiveResult id={id}/></I18nProvider>);
    expect((await screen.findByRole('alert')).textContent).toContain('SHA-256');
    expect(screen.queryByText('README.pdf')).toBeNull();
    await userEvent.click(screen.getByRole('button',{name:'Retry verification'}));
    expect(await screen.findByText('README.pdf')).toBeTruthy();
  });
});
