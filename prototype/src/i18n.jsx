import React, { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { formatDate, formatNumber, initialLocale, saveLocale, SUPPORTED_LOCALES, translate } from './i18n-core.js';
import mainChinese from './locales/main.zh-CN.js';
import runtimeChinese from './locales/runtime.zh-CN.js';
import processingChinese from './locales/processing.zh-CN.js';
import artifactChinese from './locales/artifact.zh-CN.js';
import workspaceChinese from './locales/workspace.zh-CN.js';
import projectsChinese from './locales/projects.zh-CN.js';
import proxyChinese from './locales/proxy.zh-CN.js';
import accountsChinese from './locales/accounts.zh-CN.js';
import elevationChinese from './locales/elevation.zh-CN.js';
import modisChinese from './locales/modis.zh-CN.js';
import vegetationChinese from './locales/vegetation.zh-CN.js';
import vegetationQualityChinese from './locales/vegetation-quality.zh-CN.js';
import modisScienceChinese from './locales/modis-science.zh-CN.js';
import viirsChinese from './locales/viirs.zh-CN.js';
import localRgbChinese from './locales/local-rgb.zh-CN.js';
import landsatQualityChinese from './locales/landsat-quality.zh-CN.js';
import aerialChinese from './locales/aerial.zh-CN.js';
import radarChinese from './locales/radar.zh-CN.js';
import vectorChinese from './locales/vector.zh-CN.js';
import featuresChinese from './locales/features.zh-CN.js';
import wmsChinese from './locales/wms.zh-CN.js';
import stacChinese from './locales/stac.zh-CN.js';
import wcsChinese from './locales/wcs.zh-CN.js';
import tilesChinese from './locales/tiles.zh-CN.js';
import threeDChinese from './locales/three-d.zh-CN.js';
import { syncDesktopLocale } from './runtime-client.js';

const chinese = { ...threeDChinese, ...tilesChinese, ...wcsChinese, ...stacChinese, ...wmsChinese, ...featuresChinese, ...vectorChinese, ...radarChinese, ...mainChinese, ...runtimeChinese, ...processingChinese, ...artifactChinese, ...workspaceChinese, ...projectsChinese, ...proxyChinese, ...accountsChinese, ...elevationChinese, ...aerialChinese, ...localRgbChinese, ...modisChinese, ...viirsChinese, ...landsatQualityChinese, ...vegetationChinese, ...vegetationQualityChinese, ...modisScienceChinese };
const I18nContext = createContext(null);
const browserStorage = () => { try { return window.localStorage; } catch { return null; } };

export function I18nProvider({ children }) {
  const [locale, updateLocale] = useState(() => initialLocale(browserStorage(), navigator.languages || [navigator.language]));
  useEffect(() => {
    saveLocale(browserStorage(), locale);
    document.documentElement.lang = locale;
    document.title = locale === 'zh-CN' ? 'GeoD Global · 本地工作空间' : 'GeoD Global · Local workspace';
    syncDesktopLocale(locale).catch(error => console.warn('Could not update the desktop tray language:', error));
  }, [locale]);
  const value = useMemo(() => ({
    locale,
    setLocale(next) { if (SUPPORTED_LOCALES.includes(next)) updateLocale(next); },
    t: (message, variables) => translate(locale, message, variables, chinese),
    date: (value, options) => formatDate(locale, value, options),
    number: (value, options) => formatNumber(locale, value, options),
  }), [locale]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const value = useContext(I18nContext);
  if (!value) throw new Error('useI18n must be used inside I18nProvider');
  return value;
}
