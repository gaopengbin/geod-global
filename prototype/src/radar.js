export const RADAR_HOST = 'sentinel1euwestrtc.blob.core.windows.net';
export const RADAR_KEYS = ['vv', 'vh', 'hh', 'hv'];
export function radarAssetIdentity(href, key) {
  try {
    const u = new URL(href);
    if (u.protocol !== 'https:' || u.hostname !== RADAR_HOST || u.port && u.port !== '443' || u.username || u.password || u.search || u.hash || u.pathname.includes('%')) return null;
    const p = u.pathname.slice(1).split('/'), s = p[7]?.split('_');
    if (p.length !== 10 || p[0] !== 'sentinel1-grd-rtc' || p[1] !== 'GRD' || p[5] !== 'IW' || p[8] !== 'measurement'
      || s?.length !== 9 || !/^S1[ABCD]$/.test(s[0]) || s[1] !== 'IW' || s[2] !== 'GRDH' || !/^1S(DV|DH|SV|SH)$/.test(s[3])
      || !/^\d{6}$/.test(s[6]) || !/^[A-F\d]{6}$/.test(s[7]) || !/^[A-F\d]{4}$/.test(s[8])) return null;
    const date = v => /^\d{8}T\d{6}$/.test(v) ? `${v.slice(0,4)}-${v.slice(4,6)}-${v.slice(6,8)}T${v.slice(9,11)}:${v.slice(11,13)}:${v.slice(13,15)}Z` : '';
    const start = date(s[4]), end = date(s[5]);
    if (!Number.isFinite(Date.parse(start)) || !Number.isFinite(Date.parse(end)) || new Date(start).toISOString().replace('.000Z','Z') !== start
      || new Date(end).toISOString().replace('.000Z','Z') !== end || Date.parse(end) < Date.parse(start) || Date.parse(end)-Date.parse(start) > 300000
      || p[2] !== s[4].slice(0,4) || p[3] !== String(Number(s[4].slice(4,6))) || p[4] !== String(Number(s[4].slice(6,8))) || p[6] !== s[3].slice(2)) return null;
    const band = /^iw-(vv|vh|hh|hv)\.rtc\.tiff$/.exec(p[9])?.[1];
    if (!({DV:['vv','vh'], DH:['hh','hv'], SV:['vv'], SH:['hh']}[p[6]]).includes(band) || key && key !== band) return null;
    return { id: `${s.slice(0,s[0] === 'S1C' ? 7 : 8).join('_')}_rtc`, ids: [7,8].map(n => `${s.slice(0,n).join('_')}_rtc`), key: band, platform: `Sentinel-1${s[0].slice(2)}`, start, end, product: p[7] };
  } catch { return null; }
}
export function radarMatchesJob(job, data) {
  const i = radarAssetIdentity(job.href, job.assetKey), info = data?.radar;
  if (!i || data.sha256 !== job.sha256 || info?.polarization !== job.assetKey.toUpperCase() || !validRadarMetadata(data)) return false;
  if (job.kind === 'download') return i.ids.includes(job.itemId);
  if (job.kind !== 'raster_mosaic') return false;
  const plan = job.mosaicOutput, pins = job.mosaic?.sources, profile = plan?.radar;
  const count = plan?.width * plan?.height;
  return Boolean(job.itemId === `project:${job.mosaic?.projectId}` && job.mosaic?.assetKey === job.assetKey
    && Array.isArray(pins) && pins.length > 0 && pins.length <= 32 && pins.every(pin => typeof pin.jobId === 'string' && /^[a-f\d]{64}$/.test(pin.sha256))
    && profile?.product === info.product && profile.polarization === info.polarization && profile.quantity === 'gamma0' && profile.unit === 'linear'
    && plan.calibration == null && plan.elevation == null && plan.aerial == null && plan.sourceCount === pins.length
    && plan.width === data.width && plan.height === data.height && plan.bandCount === 1 && plan.crs === data.crs
    && plan.bounds?.length === 4 && plan.bounds.every((v,n) => v === data.bounds?.[n])
    && plan.pixelSize?.length === 2 && plan.pixelSize.every((v,n) => v === data.pixelSize[n])
    && Number.isSafeInteger(plan.coveredPixels) && plan.coveredPixels > 0 && plan.coveredPixels <= count
    && Number.isSafeInteger(plan.maskedPixels) && plan.maskedPixels >= 0 && plan.maskedPixels <= count);
}
export function validRadarMetadata(data) {
  const info = data?.radar;
  return Boolean(info?.product === 'sentinel-1-iw-rtc' && RADAR_KEYS.includes(info.polarization?.toLowerCase()) && info.quantity === 'gamma0' && info.unit === 'linear'
    && data.reflectance === undefined && data.elevation === undefined && data.aerial === undefined
    && data.dataType === 'Float32' && data.bandCount === 1 && data.nodata === -32768 && data.classes?.length === 0
    && /^EPSG:(326|327)(0[1-9]|[1-5]\d|60)$/.test(data.crs) && data.pixelSize?.length === 2 && data.pixelSize.every(v => v === 10)
    && info.displayUnit === 'dB' && info.displayRange?.length === 2 && info.displayRange.every(Number.isFinite) && info.displayRange[0] <= info.displayRange[1]
    && typeof info.overview === 'boolean' && info.sampleCount === data.previewWidth * data.previewHeight && Number.isSafeInteger(info.validSampleCount) && info.validSampleCount >= 0 && info.validSampleCount <= info.sampleCount);
}
export function validRadarPixel(pixel, metadata) {
  if (!metadata.radar || !Number.isFinite(pixel.value) || Math.fround(pixel.value) !== pixel.value || pixel.values !== undefined
    || pixel.isNoData !== (pixel.value === -32768) || pixel.value < 0 && !pixel.isNoData) return false;
  return pixel.value > 0 ? Number.isFinite(pixel.decibels) && Math.abs(pixel.decibels - 10 * Math.log10(pixel.value)) < 1e-10 : pixel.decibels === undefined;
}
