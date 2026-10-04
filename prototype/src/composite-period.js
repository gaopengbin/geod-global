import { modisIdentity } from './modis.js';
import { vegetationIdentity } from './vegetation.js';
import { viirsIdentity } from './viirs.js';
export const compositeIdentity = id => vegetationIdentity(id) || modisIdentity(id) || viirsIdentity(id);
export function compositePeriodLabel(scene, date, locale) {
  const info = compositeIdentity(scene?.itemId || scene?.id);
  if (info && locale) return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' }).formatRange(new Date(info.date), new Date(info.endDate));
  return info ? `${date(info.date)} – ${date(info.endDate)}` : date(scene?.date);
}
