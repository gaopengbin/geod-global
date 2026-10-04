// Live public PNG tiles in the production renderer. Native data reads use a
// private empty store; this does not operate a desktop window or create jobs.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createServer as createNetServer } from 'node:net';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

assert(process.argv[2], 'Pass this repository\'s runtime executable as the first argument.');
const workspace = process.cwd(), exe = path.resolve(process.argv[2]);
assert(exe.startsWith(workspace + path.sep), 'Runtime must belong to this repository.');
const renderer = path.resolve(process.argv[3] || 'prototype/dist');
const root = path.join(workspace, '.verification', 'vegetation-preview-' + new Date().toISOString().replace(/[:.]/g, '-'));
await mkdir(path.join(root, 'evidence'), { recursive: true });
const config = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'));
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const freePort = async () => {
  const server = createNetServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
};
const runtimePort = await freePort(), base = `http://127.0.0.1:${runtimePort}`;
const report = { status: 'pending', checkedAt: new Date().toISOString(), runtimeSha256: sha256(await readFile(exe)),
  nativeWindowTested: false, usedUserDesktop: false, originalDownloadsCreated: 0,
  cases: [], requests: [], abortedResponses: [], commands: [], errors: [], cspErrors: [] };
const runtime = spawn(exe, ['serve', '--data-dir', path.join(root, 'store'), '--port', String(runtimePort)], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
let stderr = '', origin, browser, page;
runtime.stdout.on('data', () => {}); runtime.stderr.on('data', bytes => stderr += bytes);
const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, origin), file = path.resolve(renderer, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    assert(file.startsWith(renderer + path.sep));
    const bytes = await readFile(file);
    res.writeHead(200, { 'Content-Type': ({ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.geojson': 'application/json', '.woff2': 'font/woff2' })[path.extname(file)] || 'application/octet-stream', 'Content-Security-Policy': config.app.security.csp });
    res.end(bytes);
  } catch { res.writeHead(404); res.end(); }
});
const paths = { health: '/health', list_jobs: '/jobs', list_projects: '/projects', list_recipes: '/recipes', get_proxy_settings: '/proxy', list_provider_accounts: '/accounts', list_vectors: '/vectors', list_feature_services: '/feature-services', list_map_services: '/map-services', list_map_images: '/map-images', list_tile_sources: '/tile-sources', list_tile_packages: '/tile-packages', list_stac_connections: '/stac/connections', list_wcs_connections: '/wcs/connections', list_three_d: '/three-d/packages' };
async function api(route) { const response = await fetch(base + route), body = await response.json(); assert(response.ok, JSON.stringify(body)); return body; }
async function settled() {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(document.getAnimations().filter(animation => Number.isFinite(animation.effect?.getComputedTiming().endTime)).map(animation => animation.finished.catch(() => {})));
  });
}
async function visibleIndex(index) {
  await page.waitForFunction(index => {
    const map = document.querySelector('.explore-map-root');
    return map?.dataset.previewIndex === index && map.dataset.mapReady === 'true' && !document.querySelector('.explore-map-progress') && !document.querySelector('.explore-map-error');
  }, index, { timeout: 60000 });
  await settled();
  const pixels = await page.locator('.explore-index-layer canvas').evaluate(canvas => {
    const image = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height), colors = new Set();
    let valid = 0;
    for (let i = 0; i < image.data.length; i += 4) if (image.data[i + 3]) { valid++; colors.add(image.data[i] + ',' + image.data[i + 1] + ',' + image.data[i + 2]); }
    return { width: canvas.width, height: canvas.height, valid, colors: colors.size, bytes: [...image.data] };
  });
  const digest = sha256(Buffer.from(pixels.bytes)); delete pixels.bytes;
  assert(pixels.valid > 500 && pixels.colors > 20, JSON.stringify(pixels));
  const geometry = await page.evaluate(() => {
    const legend = document.querySelector('.vegetation-preview-legend').getBoundingClientRect();
    const attribution = document.querySelector('.map-attribution').getBoundingClientRect();
    return { legendBottom: legend.bottom, attributionTop: attribution.top };
  });
  assert(geometry.legendBottom <= geometry.attributionTop - 8, JSON.stringify(geometry));
  return { ...pixels, sha256: digest, item: await page.locator('.explore-map-root').getAttribute('data-preview-item') };
}
async function capture(name) {
  await settled();
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: path.join(root, name + '.png') });
  console.log(JSON.stringify({ screenshot: name }));
}
try {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  for (let n = 0; n < 80; n++) {
    try { await api('/health'); break; }
    catch (error) { assert.equal(runtime.exitCode, null, stderr); if (n === 79) throw error; await new Promise(resolve => setTimeout(resolve, 100)); }
  }
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  for (const [width, height, locale, theme, clock] of [[1440, 960, 'en', 'light', '2025-07-15T12:00:00Z'], [900, 680, 'zh-CN', 'dark', null]]) {
    const context = await browser.newContext({ viewport: { width, height } });
    const suffix = `${width}-${locale}-${theme}`, label = (en, zh) => locale === 'en' ? en : zh;
    let failTiles = false, rapid = false;
    const receipts = [];
    await context.exposeBinding('__previewNative', async (_source, command) => {
      report.commands.push(command);
      if (command === 'activate_desktop_frame') return 'custom';
      if (['set_desktop_locale', 'set_desktop_appearance'].includes(command)) return null;
      assert(paths[command], 'Unexpected mutation/command: ' + command);
      return api(paths[command]);
    });
    await context.exposeBinding('__savePreviewBytes', async (_source, receipt) => {
      const url = new URL(receipt.url);
      assert.equal(url.origin, 'https://planetarycomputer.microsoft.com');
      assert(url.pathname.startsWith('/api/data/v1/item/tiles/WebMercatorQuad/'));
      const bytes = Buffer.from(receipt.data, 'base64'), hash = sha256(bytes);
      if (receipt.status === 200) assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
      const file = path.join(root, 'evidence', hash + (receipt.status === 200 ? '.png' : '.json'));
      await writeFile(file, bytes);
      report.requests.push({ url: url.href, status: receipt.status, path: file, sha256: hash, bytes: bytes.length, capturedFrom: 'browser-fetch-response-clone' });
    });
    await context.addInitScript(({ locale, theme, clock }) => {
      localStorage.setItem('geod-global-locale', locale); localStorage.setItem('geod-design-theme', JSON.stringify(theme));
      window.__TAURI__ = { core: { invoke: command => window.__previewNative(command) } };
      window.__CSP_ERRORS = []; document.addEventListener('securitypolicyviolation', event => window.__CSP_ERRORS.push(event.violatedDirective));
      // Playwright's CDP response.body() can be empty after an aborted image
      // stream. Capture a clone of the actual browser fetch, keeping real
      // network/CORS/CSP behavior rather than substituting Node responses.
      window.__PREVIEW_RECEIPTS = [];
      const fetchOriginal = window.fetch.bind(window);
      window.fetch = async (...args) => {
        const response = await fetchOriginal(...args), url = String(args[0] instanceof Request ? args[0].url : args[0]);
        if (url.startsWith('https://planetarycomputer.microsoft.com/api/data/v1/item/tiles/WebMercatorQuad/')) {
          const save = response.clone().arrayBuffer().then(bytes => {
            const data = new Uint8Array(bytes); let binary = '';
            for (let offset = 0; offset < data.length; offset += 8192) binary += String.fromCharCode(...data.subarray(offset, offset + 8192));
            return window.__savePreviewBytes({ url, status: response.status, data: btoa(binary) });
          }).catch(error => { if (error.name !== 'AbortError') throw error; });
          window.__PREVIEW_RECEIPTS.push(save);
        }
        return response;
      };
      if (clock) { const OriginalDate = Date; window.Date = class extends OriginalDate { constructor(...args) { super(...(args.length ? args : [clock])); } }; }
    }, { locale, theme, clock });
    await context.route('**/*', async route => {
      const request = route.request(), url = new URL(request.url());
      if (url.origin === origin) return route.continue();
      const tiles = url.origin === 'https://planetarycomputer.microsoft.com' && url.pathname.startsWith('/api/data/v1/item/tiles/WebMercatorQuad/');
      const allowed = request.method() === 'GET' && (tiles || (url.origin === 'https://planetarycomputer.microsoft.com' && ['/api/stac/v1/search', '/api/data/v1/item/preview.png'].includes(url.pathname)) || (url.origin === 'https://earth-search.aws.element84.com' && url.pathname === '/v1/search'));
      assert(allowed, 'Unexpected network request: ' + url.href);
      if (tiles && failTiles) return route.fulfill({ status: 503, contentType: 'application/json', body: '{"detail":"deliberate transport failure for retry verification"}', headers: { 'access-control-allow-origin': '*' } });
      if (tiles && rapid) await new Promise(resolve => setTimeout(resolve, 250));
      return route.continue();
    });
    page = await context.newPage();
    page.on('pageerror', error => report.errors.push(error.message));
    page.on('response', response => {
      const url = new URL(response.url());
      if (url.origin !== 'https://planetarycomputer.microsoft.com') return;
      const receipt = (async () => {
        try {
          const bytes = await response.body(), hash = sha256(bytes), tiles = url.pathname.includes('/tiles/');
          if (tiles || url.pathname !== '/api/stac/v1/search') return;
          if (!bytes.length) { report.abortedResponses.push({ url: url.href, status: response.status() }); return; }
          if (tiles && response.ok() && bytes.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a') { report.errors.push('Preview returned an invalid PNG: ' + url.href); return; }
          const file = path.join(root, 'evidence', hash + (tiles ? response.ok() ? '.png' : '.json' : '.json'));
          await writeFile(file, bytes);
          report.requests.push({ url: url.href, status: response.status(), path: file, sha256: hash, bytes: bytes.length });
        } catch { /* A switch intentionally aborts prior tile requests. */ }
      })(); receipts.push(receipt);
    });
    await page.goto(origin + '/#Explore');
    await page.locator('.catalog-source-panel').getByRole('combobox').click();
    await page.getByRole('option', { name: 'MODIS NDVI / EVI · Planetary Computer', exact: true }).click();
    const ndvi = await visibleIndex('ndvi'); await capture('ndvi-' + suffix);
    await page.getByRole('radio', { name: 'EVI', exact: true }).click();
    const evi = await visibleIndex('evi'); await capture('evi-' + suffix);
    assert.equal(ndvi.item, evi.item); assert.notEqual(ndvi.sha256, evi.sha256);
    assert((await page.getByRole('region', { name: label('Index preview legend', '指数预览图例') }).innerText()).includes(label('no quality mask applied', '未应用质量掩膜')));
    report.cases.push({ name: suffix, ndvi, evi });
    if (clock) {
      const dates = page.locator('.timeline-track [aria-pressed]');
      assert(await dates.count() > 2, 'Historical live search must return multiple composite dates.');
      const oldItem = evi.item;
      await dates.nth(1).click(); const dated = await visibleIndex('evi');
      const firstDateItem = dated.item;
      await dates.nth(2).click(); const nextDate = await visibleIndex('evi');
      assert.notEqual(firstDateItem, nextDate.item);
      await capture('timeline-date-' + suffix);
      report.cases.push({ name: 'date-switch', oldItem, firstDateItem, nextDateItem: nextDate.item });
    }
    failTiles = true;
    await page.getByRole('radio', { name: 'NDVI', exact: true }).click();
    await page.locator('.explore-map-error').waitFor(); await capture('network-error-' + suffix);
    assert((await page.locator('.explore-map-error').innerText()).includes(label('index preview tiles could not load', '指数预览加载失败')));
    failTiles = false;
    await page.getByRole('button', { name: label('Retry map', '重试地图'), exact: true }).click();
    await visibleIndex('ndvi'); report.cases.push({ name: 'error-retry-' + suffix, passed: true });
    rapid = true;
    await page.getByRole('radio', { name: 'EVI', exact: true }).click();
    await page.getByRole('radio', { name: 'NDVI', exact: true }).click();
    await visibleIndex('ndvi'); rapid = false; report.cases.push({ name: 'rapid-index-switch-' + suffix, passed: true });
    await page.locator('.catalog-source-panel').getByRole('combobox').click();
    await page.getByRole('option', { name: 'Earth Search', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('.vegetation-preview-controls') && !document.querySelector('.explore-index-layer'));
    report.cases.push({ name: 'source-switch-cleans-index-layer-' + suffix, passed: true });
    report.cspErrors.push(...await page.evaluate(() => window.__CSP_ERRORS));
    await Promise.allSettled(receipts);
    await page.evaluate(() => Promise.all(window.__PREVIEW_RECEIPTS));
    await context.close();
  }
  assert.equal(report.errors.length, 0, JSON.stringify(report.errors)); assert.equal(report.cspErrors.length, 0, JSON.stringify(report.cspErrors));
  assert(report.requests.some(request => request.status === 200 && request.url.includes('assets=250m_16_days_NDVI')));
  assert(report.requests.some(request => request.status === 200 && request.url.includes('assets=250m_16_days_EVI')));
  assert.deepEqual(await api('/jobs'), []);
  report.status = 'passed'; console.log(JSON.stringify({ status: report.status, cases: report.cases.length, liveResponses: report.requests.length, root }));
} catch (error) {
  report.status = 'failed'; report.failure = error.stack; await page?.screenshot({ path: path.join(root, 'failure.png') }).catch(() => {}); throw error;
} finally {
  await browser?.close(); server.close();
  if (runtime.exitCode === null) { runtime.kill(); await new Promise(resolve => runtime.once('exit', resolve)); }
  await writeFile(path.join(root, 'verification.json'), JSON.stringify(report, null, 2)); await writeFile(path.join(root, 'runtime.stderr.log'), stderr);
}
