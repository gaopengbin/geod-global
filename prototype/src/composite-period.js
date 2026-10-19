import { modisIdentity } from './modis.js';
import { vegetationIdentity } from './vegetation.js';
import { viirsIdentity } from './viirs.js';
import {numCell} from './providers.js';
export const compositeIdentity = id => vegetationIdentity(id) || modisIdentity(id) || viirsIdentity(id);
export function compositePeriodLabel(scene, date, locale) {
  if(numCell(scene?.itemId || scene?.id))return `${date('2000-02-11T00:00:00Z')} – ${date('2000-02-21T23:59:59Z')}`;
  const info = compositeIdentity(scene?.itemId || scene?.id);
  if (info && locale) return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' }).formatRange(new Date(info.date), new Date(info.endDate));
  return info ? `${date(info.date)} – ${date(info.endDate)}` : date(scene?.date);
}
