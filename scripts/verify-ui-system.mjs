import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';

// UI source is distributed by upstream as copyable components. Keep those
// components and behavior adapters behind one boundary instead of recreating
// controls in each application screen.
export async function verifyUiSystem(root) {
  const base = path.join(root, 'prototype/src');
  const violations = [];
  let screens = 0;
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.name === 'ui' || entry.name === 'locales' || entry.name === '__tests__') continue;
      const target = path.join(directory, entry.name);
      if (entry.isDirectory()) { await visit(target); continue; }
      if (!/\.[jt]sx$/.test(entry.name) || /\.(?:test|spec)\./.test(entry.name)) continue;
      const source = await readFile(target, 'utf8');
      screens += 1;
      for (const match of source.matchAll(/<(button|input|select|textarea|dialog|progress|table|thead|tbody|tr|th|td|details|summary)\b/g)) {
        const line = source.slice(0, match.index).split('\n').length;
        violations.push(`${path.relative(base, target)}:${line}: native <${match[1]}> bypasses the shared UI library`);
      }
      if (/function\s+(?:Btn|Modal|Badge|Switch|Select|Input)\s*\(/.test(source)) {
        violations.push(`${path.relative(base, target)}: page-local generic control`);
      }
    }
  }
  await visit(base);
  assert.deepEqual(violations, [], 'All generic controls must use the shared Beautiful UI system');
  const main = await readFile(path.join(base, 'main.jsx'), 'utf8');
  assert(main.includes('"./ui/foundation.css"'), 'The application must load the shared foundation');
  assert(main.includes('classList.toggle("dark"'), 'Both themes must switch the Beautiful UI foundation');
  for (const name of ['styles.css', 'catalog.css', 'runtime.css', 'processing.css', 'workspace-map.css']) {
    const css = await readFile(path.join(base, name), 'utf8');
    assert(!/--(?:surface|ink|accent|page|canvas|line|blue|text|muted)\s*:/.test(css), `${name} redefines global theme tokens`);
  }
  return { sharedUiBoundary: 'passed', checkedUiModules: screens };
}
