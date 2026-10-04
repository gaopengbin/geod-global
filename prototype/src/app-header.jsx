import React, { useEffect, useState } from 'react';
import { PanelLeftClose, PanelLeftOpen } from 'lucide-react';
import { Button } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { desktopAvailable } from './runtime-client.js';
import { activateDesktopFrame, syncDesktopAppearance, showDesktopWindowMenu, handleDesktopTitlebarMouseDown } from './desktop-frame.js';
import './app-header.css';

export function GeoDBrand({ compact = false }) {
  const { t } = useI18n();
  return <a className="geod-brand" href="#Explore" aria-label={t('GeoD home')}>
    <img src="./brand/geod-symbol.png" width="32" height="32" alt="" draggable="false" />
    {!compact && <span>GeoD <span className="geod-brand-edition">Global</span></span>}
  </a>;
}

export function AppHeader({ theme, collapsed, onToggleNavigation, leading, context, actions }) {
  const { t, locale } = useI18n();
  const [mode, setMode] = useState(desktopAvailable() ? 'preparing' : 'web');
  const [error, setError] = useState('');
  useEffect(() => {
    if (!desktopAvailable()) return;
    let active = true;
    let frameMode = 'preparing';
    let inFlight = false;
    let wakePending = false;
    let retryTimer;
    const activate = async () => {
      if (!active || inFlight || frameMode === 'custom') return;
      inFlight = true;
      try {
        const next = await activateDesktopFrame();
        if (active) { frameMode = next; setMode(next === 'deferred' ? 'preparing' : next); setError(''); }
      } catch (failure) {
        if (active) { frameMode = 'native'; setMode('native'); setError(String(failure)); }
      } finally {
        inFlight = false;
        if (active && wakePending && frameMode !== 'custom') schedule();
        wakePending = false;
      }
    };
    const schedule = () => {
      window.clearTimeout(retryTimer);
      retryTimer = window.setTimeout(activate, 160);
    };
    const recover = event => {
      if (frameMode === 'custom') return;
      // Native fallback itself changes the viewport. Only deferred activation
      // retries on resize, so a persistent failure cannot create a retry loop.
      if (event.type === 'resize' && frameMode !== 'deferred') return;
      if (event.type === 'visibilitychange' && document.visibilityState !== 'visible') return;
      if (inFlight) wakePending = true;
      else schedule();
    };
    window.addEventListener('focus', recover);
    window.addEventListener('resize', recover);
    document.addEventListener('visibilitychange', recover);
    activate();
    return () => {
      active = false;
      window.clearTimeout(retryTimer);
      window.removeEventListener('focus', recover);
      window.removeEventListener('resize', recover);
      document.removeEventListener('visibilitychange', recover);
    };
  }, []);
  useEffect(() => {
    if (!desktopAvailable()) return;
    syncDesktopAppearance(theme, locale).catch(failure => setError(String(failure)));
  }, [theme, locale]);
  useEffect(() => {
    const focus = () => document.documentElement.setAttribute('data-window-focused', String(document.hasFocus()));
    focus();
    window.addEventListener('focus', focus);
    window.addEventListener('blur', focus);
    return () => { window.removeEventListener('focus', focus); window.removeEventListener('blur', focus); };
  }, []);
  const drag = mode === 'custom' ? { 'data-tauri-drag-region': true } : {};
  return <header className="app-header" data-titlebar-mode={mode}>
    <div className="app-header-identity">
      <Button variant="quiet" size="icon" icon={collapsed ? PanelLeftOpen : PanelLeftClose}
        aria-label={t(collapsed ? 'Expand navigation' : 'Collapse navigation')}
        aria-expanded={!collapsed} aria-controls="primary-navigation" onClick={onToggleNavigation} />
      <GeoDBrand />
    </div>
    {leading}
    <div className="app-header-drag" {...drag} onMouseDown={event => {
      if (mode === 'custom') handleDesktopTitlebarMouseDown(event).catch(failure => setError(String(failure)));
    }} onContextMenu={event => {
      if (!desktopAvailable()) return;
      event.preventDefault();
      showDesktopWindowMenu().catch(failure => setError(String(failure)));
    }}>
      <div className="app-header-context" {...drag}>{context}</div>
      <div className="app-header-spacer" {...drag} />
    </div>
    <div className="app-header-actions">{actions}</div>
    {error && <span className="app-header-error" role="status" title={error}>{t('Window control unavailable')}</span>}
  </header>;
}
