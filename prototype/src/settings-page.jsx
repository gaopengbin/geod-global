import React from 'react';
import { InfoDetails, Badge, PageHeader, Select, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { desktopAvailable } from './runtime-client.js';
import { ProxySettingsPanel } from './proxy-ui.jsx';
import { DiagnosticsPanel } from './diagnostics-ui.jsx';
import { DesktopSessionControl } from './desktop-session.jsx';
import { ProviderAccountsPanel } from './accounts-ui.jsx';
import './settings-page.css';
import { StacSourcesPanel } from './stac-ui.jsx';
import { WcsSourcesPanel } from './wcs-ui.jsx';
import { UpdatesPanel } from './distribution-ui.jsx';
import { UsageSettings } from './usage-ui.jsx';

export function SettingsPage({ theme, onThemeChange, onOpenRasterSources, onOpenCoverageSources, areaBounds }) {
  const { t, locale, setLocale } = useI18n();
  return <section className="settings-page" aria-label={t('Settings')}>
    <div className="bui-info-row"><PageHeader title={t('Settings')}/><InfoDetails closeLabel={t('Close')} contentClassName="settings-diagnostics" summary={t('Local diagnostics · advanced')}><DiagnosticsPanel/></InfoDetails></div>
    <div className="settings-grid">
      <Surface className="settings-panel">
        <h2>{t('Interface and window')}</h2>
        <div className="settings-fields">
          <label><strong>{t('Language')}</strong><Select aria-label={t('Interface language')} value={locale} onChange={event => setLocale(event.target.value)}><option value="en" lang="en">English</option><option value="zh-CN" lang="zh-CN">简体中文</option></Select></label>
          <label><strong>{t('Appearance')}</strong><Select aria-label={t('Appearance')} value={theme} onChange={event => onThemeChange(event.target.value)}><option value="light">{t('Light')}</option><option value="dark">{t('Dark')}</option></Select></label>
          <p className="settings-help">{t('Applies immediately and stays on this device.')}</p>
          {desktopAvailable() && <div className="settings-background">
            <div><strong>{t('Run in the background')}</strong><Badge>{t('Enabled')}</Badge><InfoDetails presentation="popover" closeLabel={t('Close')} summary={t('Run in the background')}><p>{t('Closing the window keeps downloads and processing running in the system tray. Click the tray icon to reopen; choose Quit to stop tasks and exit.')}</p></InfoDetails></div>
            <DesktopSessionControl />
          </div>}
        </div>
      </Surface>
      <ProxySettingsPanel/>
      <UsageSettings/>
    </div>
    <UpdatesPanel/>
    <ProviderAccountsPanel/>
    <StacSourcesPanel onOpen={onOpenRasterSources} bounds={areaBounds}/>
    <WcsSourcesPanel onOpen={onOpenCoverageSources} bounds={areaBounds}/>
  </section>;
}
