// MOD/MYD09A1 v061: a period composite, never a single-scene acquisition.
export const MODIS_HOST = 'modiseuwest.blob.core.windows.net';
export const MODIS_CRS = 'MODIS:Sinusoidal';
export const MODIS_PIXEL = 463.312716527778;
export const MODIS_PROJECTION = '+proj=sinu +lon_0=0 +R=6371007.181 +units=m +no_defs';
export const MODIS_QUALITY_KEYS = ['modis_qc', 'modis_state'];
export const MODIS_ASSET_NAMES = Object.freeze({ red: 'sur_refl_b01', green: 'sur_refl_b04', blue: 'sur_refl_b03', modis_qc: 'sur_refl_qc_500m', modis_state: 'sur_refl_state_500m' });
export function modisIdentity(id) {
  const m = /^(MOD09A1|MYD09A1)\.A(\d{4})(\d{3})\.h(\d{2})v(\d{2})\.061\.(\d{4})(\d{3})(\d{2})(\d{2})(\d{2})$/.exec(id);
  if (!m || +m[2] < 2000 || +m[2] > 9998 || +m[4] > 35 || +m[5] > 17 || +m[8] > 23 || +m[9] > 59 || +m[10] > 59) return null;
  const day = (year, doy) => {
    if (+doy < 1 || +doy > 366) return null;
    const date = new Date(Date.UTC(+year, 0, +doy));
    return date.getUTCFullYear() === +year ? date : null;
  };
  const start = day(m[2], m[3]), production = day(m[6], m[7]);
  if (!start || !production || +m[3] % 8 !== 1 || production < start) return null;
  const end = new Date(Math.min(+start + 7 * 86400000, Date.UTC(+m[2], 11, 31)));
  return { product: m[1], platform: m[1] === 'MOD09A1' ? 'Terra' : 'Aqua', h: +m[4], v: +m[5],
    date: start.toISOString(), endDate: end.toISOString().replace('00:00:00.000Z', '23:59:59Z'),
    directory: `/modis-061-cogs/${m[1]}/${m[4]}/${m[5]}/${m[2]}${m[3]}/` };
}
export function modisAssetIdentity(href, key) {
  try {
    const url = new URL(href), band = MODIS_ASSET_NAMES[key];
    if (url.protocol !== 'https:' || url.hostname !== MODIS_HOST || url.port || url.username || url.password || url.search || url.hash || !band) return null;
    const filename = url.pathname.split('/').at(-1), suffix = `_${band}.tif`;
    if (!filename.endsWith(suffix)) return null;
    const id = filename.slice(0, -suffix.length), identity = modisIdentity(id);
    return identity && url.pathname === identity.directory + filename ? { ...identity, id } : null;
  } catch { return null; }
}
export function modisPeriodLabel(scene, date, locale) {
  const info = modisIdentity(scene?.itemId || scene?.id);
  if (info && locale) return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' }).formatRange(new Date(info.date), new Date(info.endDate));
  return info ? `${date(info.date)} – ${date(info.endDate)}` : date(scene?.date);
}
