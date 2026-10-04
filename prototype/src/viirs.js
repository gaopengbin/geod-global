// NASA VIIRS 09A1 v002 originals contain science and QA layers in one HDF5.
export const VIIRS_HOST = 'data.lpdaac.earthdatacloud.nasa.gov';
export const VIIRS_CRS = 'VIIRS:Sinusoidal';
export const VIIRS_PIXEL = Math.PI * 6371007.181 / 18 / 1200;
export const VIIRS_PRODUCTS = Object.freeze({
  VJ209A1: { platform: 'NOAA-21', provider: 'nasa-viirs-noaa21' },
  VJ109A1: { platform: 'NOAA-20', provider: 'nasa-viirs-noaa20' },
  VNP09A1: { platform: 'Suomi-NPP', provider: 'nasa-viirs-npp' },
});
export function viirsIdentity(id) {
  const m = /^(VNP09A1|VJ109A1|VJ209A1)\.A(\d{4})(\d{3})\.h(\d{2})v(\d{2})\.002\.(\d{4})(\d{3})(\d{2})(\d{2})(\d{2})$/.exec(id);
  if (!m || +m[2] < 2012 || +m[2] > 9998 || +m[4] > 35 || +m[5] > 17 || +m[8] > 23 || +m[9] > 59 || +m[10] > 59) return null;
  const day = (year, doy) => { const d = new Date(Date.UTC(+year, 0, +doy)); return +doy >= 1 && +doy <= 366 && d.getUTCFullYear() === +year ? d : null; };
  const start = day(m[2], m[3]), production = day(m[6], m[7]);
  if (!start || !production || +m[3] % 8 !== 1 || production < start) return null;
  const end = new Date(Math.min(+start + 7 * 86400000, Date.UTC(+m[2], 11, 31)));
  return { ...VIIRS_PRODUCTS[m[1]], product: m[1], collection: `${m[1]}_002`, h: +m[4], v: +m[5],
    date: start.toISOString(), endDate: end.toISOString().replace('00:00:00.000Z', '23:59:59Z'), production: m.slice(6).join(''),
    href: `https://${VIIRS_HOST}/lp-prod-protected/${m[1]}.002/${id}/${id}.h5`,
    browse: `https://${VIIRS_HOST}/lp-prod-public/${m[1]}.002/${id}/BROWSE.${id}.1.jpg` };
}
export function viirsAssetIdentity(href) {
  try { const u = new URL(href), id = u.pathname.split('/').at(-1)?.replace(/\.h5$/, ''), p = viirsIdentity(id); return p && href === p.href ? { ...p, id } : null; } catch { return null; }
}

// This summary is native validation evidence, not a map preview or QA mask.
export function verifiedViirsScience(job) {
  const s = job?.viirsScience, p = viirsAssetIdentity(job?.href), digest = value => /^[a-f0-9]{64}$/.test(value || '');
  if (!s || !p || p.id !== job.itemId || job.kind !== 'download' || job.assetKey !== 'viirs' || job.status !== 'succeeded'
    || s.schemaVersion !== 'geod-viirs-science/v1' || s.itemId !== job.itemId || !digest(job.sha256) || s.sourceSha256 !== job.sha256
    || s.startDate !== p.date.slice(0,10) || s.endDate !== p.endDate.slice(0,10) || s.qualityMaskApplied !== false
    || s.width !== 1200 || s.height !== 1200 || s.crs !== 'VIIRS:Sinusoidal' || !/^[A-Za-z0-9_]{1,80}$/.test(s.gridName)) return null;
  const size = Math.PI * 6371007.181 / 18;
  const bounds = [(p.h-18)*size,(8-p.v)*size,(p.h-17)*size,(9-p.v)*size];
  if (!Array.isArray(s.bounds) || s.bounds.length !== 4 || s.bounds.some((v,i) => !Number.isFinite(v) || Math.abs(v-bounds[i]) > 0.02)
    || !Array.isArray(s.pixelSize) || s.pixelSize.length !== 2 || s.pixelSize.some(v => !Number.isFinite(v) || Math.abs(v-size/1200) > 1e-5)
    || !Array.isArray(s.bands) || s.bands.length !== 3) return null;
  for (const [index,b] of s.bands.entries()) {
    const [band,name] = [['red','SurfReflect_M5'],['green','SurfReflect_M4'],['blue','SurfReflect_M3']][index];
    if (b.band !== band || b.dataset !== `/HDFEOS/GRIDS/${s.gridName}/Data Fields/${name}` || b.dataType !== 'Int16'
      || b.scale !== 0.0001 || b.offset !== 0 || b.nodata !== -28672 || b.validRange?.join(',') !== '-100,16000'
      || b.sampleCount !== 1440000 || !digest(b.samplesSha256)
      || !Number.isSafeInteger(b.noDataCount) || b.noDataCount < 0 || b.noDataCount > b.sampleCount
      || !Number.isSafeInteger(b.outsideValidRangeCount) || b.outsideValidRangeCount < 0 || b.outsideValidRangeCount > b.sampleCount-b.noDataCount
      || (b.noDataCount === b.sampleCount ? b.minimum !== null || b.maximum !== null
        : !Number.isInteger(b.minimum) || !Number.isInteger(b.maximum) || b.minimum < -32768 || b.maximum > 32767
          || b.minimum > b.maximum || b.minimum === -28672 || b.maximum === -28672)) return null;
  }
  return s;
}

// Prepared bands pin the checked original HDF5, not a public browse JPEG.
export function viirsPreparationScience(job) {
  const spec = job?.viirsPrepare;
  if (!spec || job.kind !== 'raster_prepare' || !['red','green','blue'].includes(job.assetKey)
    || job.safe || job.safeOutput || job.viirsScience || job.parentId !== spec.sourceJobId
    || !/^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/.test(spec.sourceJobId || '')) return null;
  return verifiedViirsScience({ ...job, kind: 'download', assetKey: 'viirs', status: 'succeeded',
    sha256: spec.sourceSha256, viirsScience: spec.science });
}
