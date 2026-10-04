import { MODIS_CRS, MODIS_PIXEL, MODIS_ASSET_NAMES, MODIS_QUALITY_KEYS, modisAssetIdentity } from './modis.js';
import { LANDSAT_QUALITY_KEYS, validLandsatQualityMetadata, landsatQualityMatchesJob, decodeLandsatQuality, validLandsatQualityPixel, landsatQualityPalette } from './landsat-quality.js';
export const QUALITY_KEYS = [...MODIS_QUALITY_KEYS,...LANDSAT_QUALITY_KEYS];

export const QUALITY_DEFINITION = 'https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf';
const types = { modis_qc:{ bits:32,type:'UInt32',fill:4294967295,display:'MODLAND quality' }, modis_state:{ bits:16,type:'UInt16',fill:65535,display:'Cloud state' } };
export const qualityPalette = key => LANDSAT_QUALITY_KEYS.includes(key) ? landsatQualityPalette(key) : key === 'modis_qc'
  ? [['Ideal MODLAND quality','#2563eb'],['Reduced MODLAND quality','#eab308'],['Not produced: cloud effects','#ef4444'],['Not produced: other reasons','#6b7280']]
  : [['Clear cloud flag','#2563eb'],['Cloudy cloud flag','#e2e8f0'],['Mixed cloud flag','#eab308'],['Cloud flag unset; product assumes clear','#6b7280']];
export function validQualityMetadata(data) {
  if (LANDSAT_QUALITY_KEYS.includes(data?.quality?.band)) return validLandsatQualityMetadata(data);
  const q = data?.quality, profile = types[q?.band];
  const palette = profile && qualityPalette(q.band);
  return Boolean(profile && q.product === 'modis-09a1-v061' && q.layer === MODIS_ASSET_NAMES[q.band]
    && q.definition === QUALITY_DEFINITION && q.displayField === profile.display && q.pixelInterpretation === 'PixelIsArea'
    && q.countsFullResolution === true && Number.isSafeInteger(q.sampleCount) && q.sampleCount === data.width * data.height && Number.isSafeInteger(q.validSampleCount)
    && q.validSampleCount >= 0 && q.validSampleCount <= q.sampleCount
    && Number.isSafeInteger(data.width) && data.width > 0 && Number.isSafeInteger(data.height) && data.height > 0
    && data.crs === MODIS_CRS && data.bandCount === 1
    && data.dataType === profile.type && data.nodata === profile.fill
    && Array.isArray(data.pixelSize) && data.pixelSize.length === 2 && data.pixelSize.every(v => Math.abs(v - MODIS_PIXEL) < 1e-6)
    && Array.isArray(data.classes) && data.classes.length === 4
    && data.classes.every((c,i) => c.value === i && c.label === palette[i][0] && c.color === palette[i][1] && Number.isSafeInteger(c.count) && c.count >= 0)
    && data.classes.reduce((sum,c) => sum + c.count,0) === q.validSampleCount);
}
export function qualityMatchesJob(job, data) {
  if (LANDSAT_QUALITY_KEYS.includes(job.assetKey)) return landsatQualityMatchesJob(job,data);
  if (!validQualityMetadata(data) || !MODIS_QUALITY_KEYS.includes(job.assetKey) || data.quality.band !== job.assetKey) return false;
  const identity = modisAssetIdentity(job.href,job.assetKey);
  if (!identity) return false;
  if (job.kind === 'download') return identity.id === job.itemId && data.width === 2400 && data.height === 2400;
  const plan = job.mosaicOutput, p = plan?.quality, expected = types[job.assetKey];
  return job.kind === 'raster_mosaic' && job.itemId === `project:${job.mosaic?.projectId}`
    && job.mosaic?.assetKey === job.assetKey && Array.isArray(job.mosaic?.sources) && job.mosaic.sources.length > 0
    && p?.product === data.quality.product && p.band === job.assetKey && p.layer === data.quality.layer
    && p.bits === expected.bits && p.nodata === expected.fill && p.definition === QUALITY_DEFINITION && p.pixelInterpretation === 'PixelIsArea'
    && plan.calibration === undefined && plan.elevation === undefined && plan.aerial === undefined && plan.radar === undefined
    && plan.width === data.width && plan.height === data.height && plan.bandCount === 1 && plan.crs === MODIS_CRS
    && plan.sourceCount === job.mosaic.sources.length && plan.coveredPixels === data.quality.validSampleCount
    && Number.isSafeInteger(plan.maskedPixels) && plan.maskedPixels >= 0 && plan.maskedPixels <= data.width * data.height
    && Array.isArray(plan.bounds) && plan.bounds.length === 4 && plan.bounds.every((v,i) => v === data.bounds?.[i])
    && Array.isArray(plan.pixelSize) && plan.pixelSize.length === 2 && plan.pixelSize.every((v,i) => v === data.pixelSize[i]);
}
const field = (raw,name,startBit,endBit,labels) => {
  const value = Math.floor(raw / 2**startBit) % 2**(endBit-startBit+1), label = labels[value];
  return {name,startBit,endBit,value,label:label || 'Undocumented quality code',defined:Boolean(label)};
};
export function decodeQuality(key, raw) {
  if (LANDSAT_QUALITY_KEYS.includes(key)) return decodeLandsatQuality(key,raw);
  const profile = types[key];
  if (!profile || !Number.isSafeInteger(raw) || raw < 0 || raw > profile.fill) throw new Error('Invalid unsigned MODIS quality value.');
  const fields = [], yesNo = ['No','Yes'];
  if (raw !== profile.fill) {
    if (key === 'modis_qc') {
      fields.push(field(raw,'MODLAND quality',0,1,['Ideal','Reduced quality','Cloud prevented production','Other production failure']));
      const labels = ['Best quality','','','','','','','Detector noise','Detector interpolated','Solar zenith at least 86°','Solar zenith from 85° to 86°','Missing input','Atmospheric input substituted','Correction clipped','Faulty Level-1 input','Ocean or cloud: not processed'];
      for (let b=0;b<7;b++) fields.push(field(raw,`Band ${b+1} quality`,2+b*4,5+b*4,labels));
      fields.push(field(raw,'Atmospheric correction',30,30,yesNo),field(raw,'Adjacency correction',31,31,yesNo));
    } else {
      fields.push(field(raw,'Cloud state',0,1,['Clear','Cloudy','Mixed','Unset; product assumes clear']),
        field(raw,'Cloud shadow',2,2,yesNo), field(raw,'Land / water',3,5,['Shallow sea','Land','Coast or lake shore','Shallow inland water','Temporary water','Deep inland water','Shelf or moderate ocean','Deep ocean']),
        field(raw,'Aerosol correction uncertainty',6,7,['Climatology used','Low','Medium','High']),field(raw,'Cirrus',8,9,['None','Low','Medium','High']));
      for (const [bit,name] of [[10,'Internal cloud'],[11,'Internal fire'],[12,'MOD35 snow / ice'],[13,'Adjacent to cloud'],[14,'Salt pan'],[15,'Internal snow']]) fields.push(field(raw,name,bit,bit,yesNo));
    }
  }
  return {layer:MODIS_ASSET_NAMES[key],binary:raw.toString(2).padStart(profile.bits,'0'),hex:`0x${raw.toString(16).toUpperCase().padStart(profile.bits/4,'0')}`,fields};
}
export function validQualityPixel(result, data) {
  if (LANDSAT_QUALITY_KEYS.includes(data?.quality?.band)) return validLandsatQualityPixel(result,data);
  const key = data.quality?.band, profile = types[key];
  if (!profile || !Number.isSafeInteger(result.value) || result.value < 0 || result.value > profile.fill
    || result.isNoData !== (result.value === profile.fill) || result.values !== undefined || result.reflectance !== undefined
    || result.nearInfrared !== undefined || result.decibels !== undefined) return false;
  const expected = decodeQuality(key,result.value), actual = result.quality;
  if (!actual || actual.layer !== expected.layer || actual.binary !== expected.binary || actual.hex !== expected.hex
    || !Array.isArray(actual.fields) || actual.fields.length !== expected.fields.length
    || actual.fields.some((f,i) => Object.keys(expected.fields[i]).some(k => f[k] !== expected.fields[i][k]))) return false;
  const [label,color] = qualityPalette(key)[result.value % 4];
  return result.label === (result.isNoData ? 'NoData' : label) && result.color === (result.isNoData ? '#000000' : color);
}
