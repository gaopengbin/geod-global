export const LOCALE_STORAGE_KEY = 'geod-global-locale';
export const SUPPORTED_LOCALES = ['en', 'zh-CN'];

// A language preference affects presentation only, never catalog/recipe identifiers.
export function supportedLocale(value) {
  if (typeof value !== 'string') return null;
  const tag = value.toLowerCase().replaceAll('_', '-');
  if (tag === 'en' || tag.startsWith('en-')) return 'en';
  if (['zh', 'zh-cn', 'zh-sg', 'zh-hans'].includes(tag) || tag.startsWith('zh-hans-')) return 'zh-CN';
  return null;
}

export function initialLocale(storage, languages = []) {
  try {
    const saved = supportedLocale(storage?.getItem(LOCALE_STORAGE_KEY));
    if (saved) return saved;
  } catch { /* Private browser storage can be unavailable; switching still works. */ }
  for (const language of languages) {
    const locale = supportedLocale(language);
    if (locale) return locale;
  }
  return 'en';
}

export function saveLocale(storage, locale) {
  if (!SUPPORTED_LOCALES.includes(locale)) return false;
  try { storage?.setItem(LOCALE_STORAGE_KEY, locale); return Boolean(storage); }
  catch { return false; }
}

export function translate(locale, message, variables = {}, dictionary = {}) {
  if (typeof message !== 'string') return message;
  const template = locale === 'zh-CN' && Object.hasOwn(dictionary, message) ? dictionary[message] : message;
  return template.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (token, name) => Object.hasOwn(variables, name) ? String(variables[name]) : token);
}

export function formatDate(locale, value, options = {}) {
  const parsed = value instanceof Date ? value : new Date(value);
  if (value == null || value === '' || !Number.isFinite(parsed.getTime())) return '—';
  return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC', ...options }).format(parsed);
}

export function formatNumber(locale, value, options = {}) {
  if (value == null || value === '' || !Number.isFinite(Number(value))) return '—';
  return new Intl.NumberFormat(locale, options).format(Number(value));
}
