import test from 'node:test';
import assert from 'node:assert/strict';
import { formatDate, formatNumber, initialLocale, LOCALE_STORAGE_KEY, saveLocale, translate } from './i18n-core.js';

test('explicit language survives reload and wins over browser preference', () => {
  const data = new Map();
  const storage = { getItem: key => data.get(key), setItem: (key, value) => data.set(key, value) };
  assert.equal(initialLocale(storage, ['zh-CN', 'en-US']), 'zh-CN');
  assert.equal(saveLocale(storage, 'en'), true);
  assert.equal(initialLocale(storage, ['zh-CN']), 'en');
  assert.equal(saveLocale(storage, 'unsupported'), false);
  assert.equal(data.get(LOCALE_STORAGE_KEY), 'en');
  data.set(LOCALE_STORAGE_KEY, 'unsupported');
  assert.equal(initialLocale(storage, ['fr', 'zh-Hans-SG']), 'zh-CN');
});

test('unavailable storage and unsupported languages have safe fallbacks', () => {
  const storage = { getItem() { throw new Error('denied'); }, setItem() { throw new Error('denied'); } };
  assert.equal(initialLocale(storage, ['en-GB']), 'en');
  assert.equal(initialLocale(storage, ['fr', 'zh-Hant']), 'en');
  assert.equal(saveLocale(storage, 'zh-CN'), false);
});

test('translations interpolate values without reinterpreting source IDs or user text', () => {
  const dictionary = { '{count} scenes': '{count} 景影像', 'Saved {name}': '已保存 {name}' };
  assert.equal(translate('zh-CN', '{count} scenes', { count: 0 }, dictionary), '0 景影像');
  assert.equal(translate('zh-CN', 'Saved {name}', { name: '<script>{count}</script>' }, dictionary), '已保存 <script>{count}</script>');
  assert.equal(translate('en', '{count} scenes', { count: 12 }, dictionary), '12 scenes');
  assert.equal(translate('zh-CN', 'S2C_10SEG_20250707_0_L2A', {}, dictionary), 'S2C_10SEG_20250707_0_L2A');
  assert.equal(translate('zh-CN', 'constructor', {}, dictionary), 'constructor');
  assert.equal(translate('zh-CN', '{count} scenes', {}, dictionary), '{count} 景影像');
});

test('locale-aware acquisition dates keep the UTC observation day and handle missing metadata', () => {
  const acquired = '2025-07-07T23:59:59Z';
  assert.equal(formatDate('en', acquired), 'Jul 7, 2025');
  assert.equal(formatDate('zh-CN', acquired), '2025年7月7日');
  assert.equal(formatDate('zh-CN', null), '—');
  assert.equal(formatDate('en', 'invalid'), '—');
  assert.equal(formatNumber('en', 12345.5, { maximumFractionDigits: 0 }), '12,346');
  assert.equal(formatNumber('zh-CN', null), '—');
});

test('RTC catalog validation errors explain the failure in the selected language', async () => {
  const { default: main } = await import('./locales/main.zh-CN.js');
  const message = 'The radar catalog record does not match its source product, polarization or raster format. Try searching again.';
  assert.equal(translate('zh-CN', message, {}, main), '雷达影像的目录记录与源产品编号、极化通道或栅格规格不一致，已停止加载。请重新检索。');
  assert.equal(translate('en', message, {}, main), message);
});

test('clearing imagery selection remains distinct from the MODIS clear-sky state', async () => {
  const { default: main } = await import('./locales/main.zh-CN.js');
  const { default: modis } = await import('./locales/modis.zh-CN.js');
  const dictionary = { ...main, ...modis };
  assert.equal(translate('zh-CN', 'Clear selection', {}, dictionary), '清空');
  assert.equal(translate('zh-CN', 'Clear', {}, dictionary), '晴空');
  assert.equal(translate('en', 'Clear selection', {}, dictionary), 'Clear selection');
});

test('locale resources preserve interpolation contracts and shared messages agree', async () => {
  const { default: main } = await import('./locales/main.zh-CN.js');
  const { default: runtime } = await import('./locales/runtime.zh-CN.js');
  const { default: processing } = await import('./locales/processing.zh-CN.js');
  const { default: artifact } = await import('./locales/artifact.zh-CN.js');
  const { default: workspace } = await import('./locales/workspace.zh-CN.js');
  const parameters = text => [...new Set([...text.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/g)].map(match => match[1]))].sort();
  for (const [source, translated] of [...Object.entries(main), ...Object.entries(runtime), ...Object.entries(processing), ...Object.entries(artifact), ...Object.entries(workspace)]) {
    assert.equal(typeof translated, 'string', `Invalid translation: ${source}`);
    assert.ok(translated.trim(), `Empty translation: ${source}`);
    assert.deepEqual(parameters(translated), parameters(source), `Missing/renamed parameters: ${source}`);
  }
  const shared = {};
  for (const resource of [main, runtime, processing, artifact, workspace]) {
    for (const [source, translated] of Object.entries(resource)) {
      if (Object.hasOwn(shared, source)) assert.equal(shared[source], translated, `Conflicting shared translation: ${source}`);
      shared[source] = translated;
    }
  }
});
