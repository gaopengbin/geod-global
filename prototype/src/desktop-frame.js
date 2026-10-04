import { desktopAvailable } from './runtime-client.js';

let frameActivation;

async function settleTitlebarLayout() {
  let previous;
  // Snap geometry is measured against the native client area. Wait for the
  // viewport and app bar to settle before asking the plugin to measure them.
  // Timers also work in a hidden/background webview, unlike animation frames.
  for (let sample = 0; sample < 6; sample++) {
    const bar = document.querySelector('.app-header')?.getBoundingClientRect();
    const current = [window.innerWidth, window.innerHeight, window.devicePixelRatio, bar?.width, bar?.height].join(':');
    if (window.innerWidth > 0 && window.innerHeight > 0 && current === previous) return;
    previous = current;
    await new Promise(resolve => window.setTimeout(resolve, 75));
  }
}

async function activateSettledFrame() {
  for (let attempt = 0; attempt < 2; attempt++) {
    await settleTitlebarLayout();
    const mode = await window.__TAURI__.core.invoke('activate_desktop_frame');
    if (!['custom', 'native', 'deferred'].includes(mode)) throw new Error('Invalid desktop frame state.');
    if (mode === 'custom' || mode === 'deferred' || attempt === 1) return mode;
    // A resize during activation can invalidate native Snap coordinates. The
    // backend restores a usable frame; retry once with its new client bounds.
  }
}

export function activateDesktopFrame() {
  if (!desktopAvailable()) return Promise.resolve('web');
  // Multiple mounted consumers must not race native activation/restoration.
  frameActivation ??= activateSettledFrame().finally(() => { frameActivation = undefined; });
  return frameActivation;
}

export async function showDesktopWindowMenu() {
  if (desktopAvailable()) await window.__TAURI__.core.invoke('show_desktop_window_menu');
}

export async function hideDesktopWindow() {
  if (!desktopAvailable()) throw new Error('Open the desktop app to control this session.');
  await window.__TAURI__.core.invoke('hide_desktop_window');
}

export async function quitDesktop() {
  if (!desktopAvailable()) throw new Error('Open the desktop app to control this session.');
  await window.__TAURI__.core.invoke('quit_desktop');
}

export async function handleDesktopTitlebarMouseDown(event) {
  if (!desktopAvailable() || event.button !== 0 || ![1, 2].includes(event.detail)) return;
  // Own this event before Tauri's document listener so errors can be shown and
  // only one native operation is started. Interactive controls are outside it.
  event.preventDefault();
  event.stopPropagation();
  const action = event.detail === 2 ? 'internal_toggle_maximize' : 'start_dragging';
  await window.__TAURI__.core.invoke(`plugin:window|${action}`);
}

export async function syncDesktopAppearance(theme, locale) {
  if (!['light', 'dark'].includes(theme) || !['en', 'zh-CN'].includes(locale)) throw new Error('Invalid desktop appearance.');
  if (!desktopAvailable()) return;
  const revision = ++desktopAppearanceRevision;
  await window.__TAURI__.core.invoke('set_desktop_appearance', { theme });
  if (revision !== desktopAppearanceRevision) return;
  // The pinned plugin owns caption controls. Translate only their labels; its
  // state/geometry/keyboard handling remains upstream. Observe maximize/restore.
  const translateControls = () => {
    const root = document.querySelector('[data-tauri-plugin-decoration-root]');
    if (!root) return;
    const labels = locale === 'zh-CN' ? {
      minimize: '最小化窗口', maximize: '最大化窗口', restore: '还原窗口', close: '收起到托盘',
    } : { minimize: 'Minimize window', maximize: 'Maximize window', restore: 'Restore window', close: 'Hide to tray' };
    root.querySelectorAll('[data-tauri-decoration-control]').forEach(button => {
      const control = button.getAttribute('data-tauri-decoration-control');
      const restoring = button.textContent === '\uE923' || button.getAttribute('data-tauri-decoration-icon') === 'restore';
      const label = labels[control === 'maximize' && restoring ? 'restore' : control];
      if (label && button.getAttribute('aria-label') !== label) {
        button.setAttribute('aria-label', label);
        button.setAttribute('title', label);
      }
    });
  };
  desktopControlObserver?.disconnect();
  desktopControlObserver = new MutationObserver(translateControls);
  desktopControlObserver.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-tauri-decoration-icon', 'aria-label'] });
  translateControls();
}
let desktopControlObserver;
let desktopAppearanceRevision = 0;
