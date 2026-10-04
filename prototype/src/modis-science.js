import { MODIS_CRS, MODIS_PIXEL } from './modis.js';
import { vegetationAssetIdentity, VEGETATION_PRODUCT } from './vegetation.js';
import { MODIS_SCIENCE, MODIS_SCIENCE_COLORS, MODIS_SCIENCE_DEFINITION, MODIS_SCIENCE_PALETTE } from './modis-science-layers.js';

export function scienceCalendarDate(year, day) {
  if (!Number.isInteger(year) || year < 2000 || year > 9998 || !Number.isInteger(day) || day < 1 || day > 366) return undefined;
  const date = new Date(Date.UTC(year,0,day));
  return date.getUTCFullYear() === year ? date.toISOString().slice(0,10) : undefined;
}
export function validScienceMetadata(data) {
  const info = data?.science, layer = MODIS_SCIENCE[info?.band];
  if (!layer || info.product !== VEGETATION_PRODUCT || info.layer !== layer.asset || info.kind !== layer.kind || info.unit !== layer.unit
    || ['vegetation','reflectance','quality','elevation','aerial','radar','composite'].some(key=>data[key] !== undefined)
    || data.bandCount !== 1 || data.dataType !== {int8:'Int8',int16:'Int16',uint16:'UInt16'}[layer.dataType]
    || data.nodata !== layer.nodata || data.crs !== MODIS_CRS || info.scale !== layer.scale || info.offset !== 0
    || info.validRange?.join(',') !== layer.validRange.join(',') || info.displayRange?.join(',') !== layer.validRange.join(',')
    || info.palette !== MODIS_SCIENCE_PALETTE || info.definition !== MODIS_SCIENCE_DEFINITION || info.pixelInterpretation !== 'PixelIsArea'
    || info.countsFullResolution !== false || data.pixelSize?.length !== 2 || data.pixelSize.some(v=>!Number.isFinite(v) || Math.abs(v-MODIS_PIXEL/2)>1e-6)
    || !Number.isSafeInteger(info.sampleCount) || info.sampleCount !== data.previewWidth * data.previewHeight
    || !Number.isSafeInteger(info.validSampleCount) || info.validSampleCount < 0 || info.validSampleCount > info.sampleCount
    || !Number.isSafeInteger(info.outOfRangeSampleCount) || info.outOfRangeSampleCount < 0 || info.outOfRangeSampleCount > info.validSampleCount
    || (info.kind === 'date' ? !scienceCalendarDate(info.calendarYear,1) : info.calendarYear !== undefined)) return false;
  if (!Array.isArray(data.classes)) return false;
  if (['flags','rank'].includes(info.kind)) {
    const labels = info.kind === 'flags' ? ['Produced · good','Produced · check flags','Likely cloudy','Not produced · other'] : ['Good','Marginal','Snow or ice','Cloudy'];
    return data.classes.length === 4 && data.classes.every((entry,i)=>entry.value === i && entry.color === MODIS_SCIENCE_COLORS[i] && entry.label === labels[i] && Number.isSafeInteger(entry.count) && entry.count >= 0)
      && data.classes.reduce((s,c)=>s+c.count,0) === info.validSampleCount - (info.kind === 'rank' ? info.outOfRangeSampleCount : 0);
  }
  return data.classes.length === 0;
}
export function scienceMatchesJob(job, data) {
  const source = vegetationAssetIdentity(job?.href,job?.assetKey), layer = MODIS_SCIENCE[job?.assetKey];
  if (!source || !layer || !validScienceMetadata(data) || data.science.band !== job.assetKey
    || job.assetKey === 'vi_doy' && data.science.calendarYear !== new Date(source.date).getUTCFullYear()) return false;
  if (job.kind === 'download') {
    const size = MODIS_PIXEL * 2400, bounds = [(source.h-18)*size,(8-source.v)*size,(source.h-17)*size,(9-source.v)*size];
    return job.itemId === source.id && data.width === 4800 && data.height === 4800 && data.bounds?.length === 4 && data.bounds.every((v,i)=>Math.abs(v-bounds[i])<0.02);
  }
  const plan = job.mosaicOutput, p = plan?.calibration;
  return job.kind === 'raster_mosaic' && job.itemId === `project:${job.mosaic?.projectId}` && job.mosaic?.assetKey === job.assetKey && job.mosaic.sources?.length > 0
    && p?.product === VEGETATION_PRODUCT && p.scienceKey === job.assetKey && p.signed === (layer.dataType !== 'uint16') && (p.sampleBits ?? 16) === (layer.dataType === 'int8' ? 8 : 16)
    && p.scale === layer.scale && p.offset === 0 && p.nodata === layer.nodata && p.calendarYear === data.science.calendarYear
    && plan.bandCount === 1 && plan.crs === data.crs && plan.width === data.width && plan.height === data.height
    && plan.bounds?.length === 4 && plan.bounds.every((v,i)=>v === data.bounds[i]) && plan.pixelSize?.length === 2 && plan.pixelSize.every((v,i)=>v === data.pixelSize[i]);
}
const FLAG_FIELDS = [
  ['MODLAND quality',0,1,['Produced · good','Produced · check flags','Likely cloudy','Not produced · other']],
  ['VI usefulness',2,5,['Best','Lower','Reduced','','Reduced','','','','Reduced','Reduced','Reduced','','Lowest','Unusable quality','Faulty L1B','Unusable or not processed']],
  ['Aerosol quantity',6,7,['Climatology','Low','Intermediate','High']], ['Adjacent cloud',8,8,['No','Yes']], ['BRDF correction',9,9,['No','Yes']], ['Mixed clouds',10,10,['No','Yes']],
  ['Land / water',11,13,['Shallow ocean','Land','Coast or shoreline','Shallow inland water','Ephemeral water','Deep inland water','Continental ocean','Deep ocean']],
  ['Possible snow / ice',14,14,['No','Yes']], ['Possible shadow',15,15,['No','Yes']],
];
export function validSciencePixel(result,data) {
  const info = data?.science, layer = MODIS_SCIENCE[info?.band], p = result?.science, value = result?.value;
  if (!layer || !p || !Number.isInteger(value) || !({int8:v=>v>=-128&&v<=127,int16:v=>v>=-32768&&v<=32767,uint16:v=>v>=0&&v<=65535}[layer.dataType])(value)
    || ['reflectance','indexValue','values','nearInfrared','decibels','quality'].some(key=>result[key] !== undefined) || result.isNoData !== (value === layer.nodata)) return false;
  const date = layer.kind === 'date' && !result.isNoData ? scienceCalendarDate(info.calendarYear,value) : undefined;
  const inRange = !result.isNoData && value >= layer.validRange[0] && value <= layer.validRange[1] && (layer.kind !== 'date' || Boolean(date));
  if (p.withinRange !== inRange || p.date !== date) return false;
  if (!result.isNoData && ['angle','reflectance'].includes(layer.kind)) {
    if (!Number.isFinite(p.convertedValue) || Math.abs(p.convertedValue-value*layer.scale)>1e-12) return false;
  } else if (p.convertedValue !== undefined) return false;
  if (layer.kind === 'flags' && !result.isNoData) {
    const f = p.flags;
    return f?.layer === layer.asset && f.hex === `0x${value.toString(16).toUpperCase().padStart(4,'0')}` && f.binary === value.toString(2).padStart(16,'0') && f.covered === undefined
      && f.fields?.length === 9 && f.fields.every((item,i)=>{
        const [name,start,end,labels] = FLAG_FIELDS[i], code = value >> start & (2**(end-start+1)-1), label = labels[code];
        return item.name === name && item.startBit === start && item.endBit === end && item.value === code && item.defined === Boolean(label) && item.label === (label || 'Unspecified code');
      });
  }
  return p.flags === undefined;
}
