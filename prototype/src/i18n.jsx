import React, { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { formatDate, formatNumber, initialLocale, saveLocale, SUPPORTED_LOCALES, translate } from './i18n-core.js';
import mainChinese from './locales/main.zh-CN.js';
import runtimeChinese from './locales/runtime.zh-CN.js';
import processingChinese from './locales/processing.zh-CN.js';
import artifactChinese from './locales/artifact.zh-CN.js';
import workspaceChinese from './locales/workspace.zh-CN.js';
import projectsChinese from './locales/projects.zh-CN.js';

const chinese = { ...mainChinese, ...runtimeChinese, ...processingChinese, ...artifactChinese, ...workspaceChinese, ...projectsChinese };
const I18nContext = createContext(null);
const browserStorage = () => { try { return window.localStorage; } catch { return null; } };

export function I18nProvider({ children }) {
  const [locale, updateLocale] = useState(() => initialLocale(browserStorage(), navigator.languages || [navigator.language]));
  useEffect(() => {
    saveLocale(browserStorage(), locale);
    document.documentElement.lang = locale;
    document.title = locale === 'zh-CN' ? 'GeoD Global · 本地工作空间' : 'GeoD Global · Local workspace';
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
