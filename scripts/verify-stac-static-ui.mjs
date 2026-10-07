// Renderer QA using captured real native metadata, with a read-only fixture bridge.
// Headless Edge; no user desktop, live provider request, project write or transfer.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { chromium } from 'playwright';

const root = path.resolve(import.meta.dirname, '..');
const evidence = path.resolve(root, process.argv[2] || '.verification/stac-static-20261005-c');
const output = path.join(evidence, 'renderer');
const connection = JSON.parse(await fs.readFile(path.join(evidence, 'connection.json')));
const pages = JSON.parse(await fs.readFile(path.join(evidence, 'pages.json')));
const receipt = JSON.parse(await fs.readFile(path.join(evidence, 'acceptance.json')));
assert.equal(receipt.status, 'passed');
const chosen = connection.catalogNodes.find(node => node.id === receipt.directoryId);
await fs.mkdir(output, { recursive: true });
const filename = `.stac-static-qa-${process.pid}.html`;
const harness = path.join(root, 'prototype', filename);
const report = { status: 'pending', scope: 'Headless renderer with captured real metadata and read-only fixture bridge', nativeWindowTested: false, originalDownloads: 0, screenshots: [], errors: [], cases: [] };
let browser, server;
try {
  await fs.writeFile(harness, `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div><script type="module">
import React from 'react'; import {createRoot} from 'react-dom/client';
import {I18nProvider} from '/src/i18n.jsx'; import {RuntimeContext} from '/src/runtime-context.js';
import {StacSourceDialog} from '/src/stac-ui.jsx';
import '/src/ui/foundation.css'; import '/src/styles.css';
createRoot(document.getElementById('root')).render(React.createElement(I18nProvider,null,
 React.createElement(RuntimeContext.Provider,{value:{projects:[],refresh:async()=>{}}},
 React.createElement(StacSourceDialog,{areaBounds:[-10,51,-9,52],onClose:()=>{}}))));
</script></body></html>`, { flag: 'wx' });
  // Use the one authorized preview port; never commandeer an existing listener.
  let occupied = false;
  try { occupied = (await fetch('http://127.0.0.1:4317')).ok; } catch {}
  assert.equal(occupied, false, 'Preview port is already in use; keep its owner untouched');
  server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'prototype', '--config', 'prototype/vite.config.mjs'], { cwd: root, windowsHide: true, stdio: 'ignore' });
  for (let attempts = 0; attempts < 100; attempts++) {
    try { if ((await fetch(`http://127.0.0.1:4317/${filename}`)).ok) break; } catch {}
    assert.equal(server.exitCode, null, 'Owned preview did not start');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  for (const [width, locale, theme] of [[1440, 'zh-CN', 'light'], [900, 'en', 'dark']]) {
    const context = await browser.newContext({ viewport: { width, height: 950 }, locale });
    await context.route('**/*', route => new URL(route.request().url()).origin === 'http://127.0.0.1:4317' ? route.continue() : route.abort());
    const page = await context.newPage();
    page.on('pageerror', error => report.errors.push(error.message));
    await page.addInitScript(({ connection, pages, locale, theme }) => {
      localStorage.setItem('geod-global-locale', locale);
      document.addEventListener('DOMContentLoaded', () => { document.documentElement.dataset.theme = theme; document.documentElement.classList.toggle('dark', theme === 'dark'); });
      window.__qaCalls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.__qaCalls.push({ command, args });
        if (command === 'set_desktop_locale') return null;
        if (command === 'list_stac_connections') return [connection];
        if (command === 'search_stac') return structuredClone(pages[args.request.cursor ? 1 : 0]);
        throw new Error('Read-only renderer bridge refused ' + command);
      } } };
    }, { connection, pages, locale, theme });
    await page.goto(`http://127.0.0.1:4317/${filename}`);
    const words = locale === 'zh-CN' ? { source: '已保存的栅格来源', type: '来源类型', static: '静态 STAC 目录', collection: '栅格集合', search: '搜索栅格条目', more: '继续扫描' }
      : { source: 'Saved raster source', type: 'Source type', static: 'Static STAC catalog', collection: 'Raster collection', search: 'Search raster items', more: 'Continue scanning' };
    const source = page.getByRole('combobox', { name: words.source, exact: true });
    await source.waitFor(); await page.waitForFunction(() => !document.querySelector('[role=combobox]')?.disabled);
    await page.getByRole('combobox', { name: words.type, exact: true }).click();
    await page.getByRole('option', { name: words.static, exact: true }).click();
    await capture('connect');
    await source.click(); await page.getByRole('option', { name: connection.name, exact: true }).click();
    await page.getByRole('combobox', { name: words.collection, exact: true }).click();
    await page.getByRole('option', { name: `${chosen.title} · ${chosen.kind}`, exact: true }).click();
    await capture('directory');
    await page.getByRole('button', { name: words.search, exact: true }).click();
    await page.getByRole('button', { name: words.more, exact: true }).waitFor();
    await page.locator('.stac-search-footer').scrollIntoViewIfNeeded();
    await capture('page');
    const calls = await page.evaluate(() => window.__qaCalls);
    const search = calls.find(call => call.command === 'search_stac');
    assert.equal(search.args.request.collectionId, chosen.key);
    assert.equal(calls.some(call => /download|project|connect_stac/.test(call.command)), false);
    report.cases.push(`${width}/${locale}/${theme}: explicit static type, actual directory, real page metadata and no writes`);
    await context.close();
    async function capture(name) {
      await page.evaluate(() => document.fonts.ready);
      await page.waitForFunction(() => !document.querySelector('.stac-status'));
      await page.evaluate(() => Promise.all(document.getAnimations().filter(animation => animation.effect?.getComputedTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => {}))));
      const dialog = page.getByRole('dialog');
      const box = await dialog.boundingBox();
      assert.ok(box.x >= 0 && box.y >= 0 && box.x + box.width <= width + 1);
      const overflow = await page.locator('.bui-dialog-body').evaluate(element => element.scrollWidth > element.clientWidth + 1);
      assert.equal(overflow, false, 'Dialog has horizontal overflow');
      const screenshot = `${name}-${width}-${locale}-${theme}.png`;
      await page.screenshot({ path: path.join(output, screenshot) }); report.screenshots.push(screenshot);
    }
  }
  assert.deepEqual(report.errors, []);
  report.status = 'passed';
} catch (error) { report.status = 'failed'; report.error = error.message; throw error; }
finally {
  await browser?.close();
  if (server && server.exitCode === null) { server.kill(); await new Promise(resolve => server.once('exit', resolve)); }
  await fs.unlink(harness).catch(() => {});
  await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify(report));
