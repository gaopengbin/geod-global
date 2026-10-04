import React from 'react';
import { it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WcsSourceDialog, WcsRasterInspection, WcsProjectAssets } from './wcs-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';
import { RuntimeContext } from './runtime-context.js';
import { wcsRequest } from './wcs-client.js';
import { runtimeRequest } from './runtime-client.js';
import { wcsConnection, wcsDescription, wcsPlan, wcsProject, wcsJob, wcsInspection } from './wcs-fixtures.js';
vi.mock('./i18n.jsx', () => ({ useI18n: () => ({ t: (s, v={}) => s.replace(/\{(\w+)\}/g, (_, k) => v[k] ?? ''), date: value => value || '—', number: String, locale: 'en' }) }));
vi.mock('./wcs-client.js', async original => ({ ...await original(), wcsRequest: vi.fn() }));
vi.mock('./runtime-client.js', async original => ({ ...await original(), runtimeRequest: vi.fn() }));
vi.mock('./file-thumbnail.jsx', () => ({ FileThumbnail: () => <span>Local preview</span> }));
vi.mock('./stac-map.jsx', () => ({ default: ({ onPixel, kind }) => <div aria-label={kind}><button onClick={() => onPixel({ column: 2, row: 3 })}>Read map position</button></div> }));
const refresh = vi.fn(async () => {}), context = { jobs: [wcsJob], projects: [wcsProject], health: {}, refresh, act: vi.fn() };
beforeEach(() => { vi.clearAllMocks(); runtimeRequest.mockResolvedValue([wcsProject]); wcsRequest.mockImplementation(async (op, payload) => ({ list: [wcsConnection], connect: wcsConnection, describe: wcsDescription, plan: wcsPlan, savedPlan: wcsPlan, project: wcsProject, inspect: wcsInspection, downloads: { projectId: wcsProject.id, assetKey: 'wcs_coverage', jobs: [{ ...wcsJob, status: 'queued' }] }, pixel: { jobId: wcsJob.id, sha256: wcsJob.sha256, column: payload?.column, row: payload?.row, values: [278.25], noData: [false] } })[op]); });
function dialog() { return render(<RuntimeContext.Provider value={context}><WcsSourceDialog areaBounds={wcsPlan.requestedBounds} onClose={() => {}}/></RuntimeContext.Provider>); }
async function choose(user) { await waitFor(() => expect(screen.getByRole('combobox', { name: 'Saved coverage service' }).disabled).toBe(false)); await user.click(screen.getByRole('combobox', { name: 'Saved coverage service' })); await user.click(screen.getByRole('option', { name: wcsConnection.name })); }
it('requires explicit coverage description and bounded plan, retaining declarations and saving before downloading', async () => {
  const user = userEvent.setup(); dialog(); expect(screen.getByRole('textbox', { name: 'WCS service URL' }).value).toBe('');
  await choose(user); expect(wcsRequest.mock.calls.some(([op]) => op === 'describe')).toBe(false); expect(screen.getByRole('button', { name: 'Plan coverage subset' }).disabled).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Read coverage description' })); await screen.findByText('100 × 100 · EPSG:4326 · 1 range fields');
  await user.click(screen.getByRole('button', { name: 'Declared range fields · 1' })); expect(screen.getByText('K')).toBeTruthy(); expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeTruthy(); expect(document.querySelector('img')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Plan coverage subset' })); await screen.findByText('20 × 10 · EPSG:4326');
  expect(wcsRequest.mock.calls.find(([op]) => op === 'plan')[1]).toEqual({ descriptionId: wcsDescription.id, bounds: wcsPlan.requestedBounds });
  await user.type(screen.getByRole('textbox', { name: 'Coverage project name' }), 'Temperature subset'); await user.click(screen.getByRole('button', { name: 'Save coverage plan' }));
  await screen.findByRole('button', { name: 'Download coverage subset' }); expect(wcsRequest.mock.calls.some(([op]) => op === 'downloads')).toBe(false);
  expect(wcsRequest.mock.calls.find(([op]) => op === 'project')[1]).toEqual({ name: 'Temperature subset', bounds: wcsPlan.requestedBounds, selections: [{ planId: wcsPlan.id }] });
  await user.click(screen.getByRole('button', { name: 'Download coverage subset' })); await screen.findByRole('button', { name: 'Subset download queued' });
});
it('editing the region invalidates the saved plan and download action while retaining the coverage description', async () => {
  const user = userEvent.setup(); dialog(); await choose(user); await user.click(screen.getByRole('button', { name: 'Read coverage description' })); await user.click(await screen.findByRole('button', { name: 'Plan coverage subset' })); await screen.findByText('Planned coverage subset');
  await user.clear(screen.getByRole('textbox', { name: 'Coverage subset bounds' }));
  expect(screen.queryByText('Planned coverage subset')).toBeNull(); expect(screen.queryByRole('button', { name: 'Save coverage plan' })).toBeNull(); expect(screen.getByRole('button', { name: 'Plan coverage subset' }).disabled).toBe(true);
  expect(screen.getByText('100 × 100 · EPSG:4326 · 1 range fields')).toBeTruthy();
});
it('filtering the coverage list never leaves an invisible coverage selected for a new request', async () => {
  const original = wcsRequest.getMockImplementation();
  wcsRequest.mockImplementation((op, payload) => op === 'list' ? Promise.resolve([{ ...wcsConnection, coverages: Array.from({ length: 13 }, (_, i) => ({ id: `coverage-${i}`, title: `Field ${i}`, subtype: 'RectifiedGridCoverage' })) }]) : original(op, payload));
  const user = userEvent.setup(); dialog(); await choose(user);
  const filter = screen.getByRole('searchbox', { name: 'Find a coverage' });
  await user.type(filter, 'Field 12');
  await user.click(screen.getByRole('button', { name: 'Read coverage description' }));
  expect(wcsRequest.mock.calls.find(([op]) => op === 'describe')[1].coverageId).toBe('coverage-12');
  await user.clear(filter); await user.type(filter, 'Missing coverage');
  expect(screen.getByRole('button', { name: 'Read coverage description' }).disabled).toBe(true);
  expect(screen.getByRole('button', { name: 'Plan coverage subset' }).disabled).toBe(true);
});
it('coverage-only projects expose subsets and file inspection without sensor or original-product actions', async () => {
  render(<RuntimeContext.Provider value={context}><ProjectsLibrary focusedProjectId={wcsProject.id}/></RuntimeContext.Provider>);
  await screen.findByText('1 coverage subsets'); expect(screen.getByText('Coverage subset · GeoTIFF')).toBeTruthy(); expect(screen.getByRole('button', { name: 'Download missing subsets' }).disabled).toBe(true);
  expect(screen.getByRole('button', { name: 'Inspect coverage subset' })).toBeTruthy(); expect(screen.queryByRole('button', { name: 'Inspect original raster' })).toBeNull(); expect(screen.queryByText('Prepare SCL')).toBeNull();
});
it('coverage recovery verifies completed plan pins and accurately reports queued repairs', async () => {
  const user = userEvent.setup(); render(<RuntimeContext.Provider value={context}><WcsProjectAssets project={wcsProject} jobs={[wcsJob]}/></RuntimeContext.Provider>);
  await user.click(screen.getByRole('button', { name: 'Verify downloaded subsets' })); await screen.findByText('0 subsets verified · 1 downloads queued · 0 running');
  expect(wcsRequest).toHaveBeenCalledWith('downloads', { projectId: wcsProject.id, selections: [{ planId: wcsPlan.id }] });
});
it('saved coverage plans can be reviewed without describing the service again or starting a download', async () => {
  const user = userEvent.setup(); render(<RuntimeContext.Provider value={context}><WcsProjectAssets project={wcsProject} jobs={[]}/></RuntimeContext.Provider>);
  await user.click(screen.getByRole('button', { name: 'Review coverage subsets · 1' }));
  await user.click(screen.getByRole('button', { name: 'Review saved plan · Temperature field' }));
  await screen.findByRole('dialog', { name: 'Saved coverage plan' });
  expect(wcsRequest.mock.calls).toEqual([['savedPlan', { id: wcsPlan.id }]]);
  expect(screen.getByText('20 × 10 · EPSG:4326')).toBeTruthy();
});
it('WCS inspection shares raw-pixel rendering but describes a service subset with its own plan provenance', async () => {
  const user = userEvent.setup(); render(<WcsRasterInspection job={wcsJob} onClose={() => {}}/>);
  await user.click(await screen.findByRole('button', { name: 'Read map position' })); await screen.findByText('278.25');
  expect(wcsRequest).toHaveBeenCalledWith('pixel', { id: wcsJob.id, column: 2, row: 3 });
  expect(screen.getByRole('dialog', { name: 'Inspect coverage subset' })).toBeTruthy(); expect(screen.queryByText('Original item metadata')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Coverage subset source details' })); expect(screen.getByText(wcsDescription.descriptionSha256)).toBeTruthy(); expect(screen.getByText(wcsPlan.requestUrl)).toBeTruthy();
});
