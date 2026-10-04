import { LANDSAT_BANDS, LANDSAT_HOST, NASA_HOST, isSupportedAsset } from './providers.js';

import { MODIS_HOST, MODIS_CRS, MODIS_PIXEL, modisAssetIdentity } from './modis.js';
import { VIIRS_CRS, VIIRS_PIXEL, viirsPreparationScience } from './viirs.js';
import { QUALITY_DEFINITION } from './quality.js';
import { LANDSAT_QUALITY_DEFINITION, landsatQualityIdentity } from './landsat-quality.js';

const profiles = {
  'viirs-09a1-v002': { dataType: 'Int16', nodata: -28672, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
  'modis-09a1-v061': { dataType: 'Int16', nodata: -28672, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
  'landsat-c2-l2': { dataType: 'UInt16', nodata: 0, scale: 0.0000275, offset: -0.2, min: 0, max: 65535 },
  'hls-l30-v2': { dataType: 'Int16', nodata: -9999, scale: 0.0001, offset: 0, min: -32768, max: 32767 },
};
const hash = value => /^[a-f\d]{64}$/i.test(value || '');
export const rgbQualityPolicyLabel = policy => ({
  clear:'Exclude cloud and shadow flags', clear_best:'Clear pixels with best RGB quality',
  cloud_free:'Exclude cloud, shadow and RGB saturation', cloud_free_conservative:'Conservative cloud-free flags',
})[policy] || 'Quality screening';
function identity(job, jobs = []) {
  if (job.kind === 'raster_mosaic' && job.status === 'succeeded' && hash(job.sha256) && LANDSAT_BANDS.includes(job.assetKey)) {
    const p = job.mosaicOutput?.calibration, g = job.mosaicOutput, spec = job.mosaic;
    if (!profiles[p?.product] || !spec?.projectId || !spec.sources?.length || g.bandCount !== 1 || !finite(g.bounds,4) || !finite(g.pixelSize,2)) return null;
    const inputs = spec.sources.map(pin => jobs.find(source => source.id === pin.jobId && source.sha256 === pin.sha256));
    const identities = inputs.map(source => source && identity(source));
    if (identities.some((value, i) => !value || value.product !== p.product || inputs[i].assetKey !== job.assetKey)) return null;
    return { product:p.product, derived:true, directory:JSON.stringify([spec.projectId,g.width,g.height,g.crs,g.bounds,g.pixelSize,inputs.map((source,i)=>[source.itemId,identities[i].directory])]) };
  }
  if (job.kind === 'raster_prepare' && job.status === 'succeeded' && hash(job.sha256) && viirsPreparationScience(job)) {
    return { product: 'viirs-09a1-v002', directory: `${job.href}#${job.viirsPrepare.sourceJobId}:${job.viirsPrepare.sourceSha256}` };
  }
  if (job.kind !== 'download' || job.status !== 'succeeded' || !hash(job.sha256)
    || !LANDSAT_BANDS.includes(job.assetKey) || !isSupportedAsset(job.href, job.assetKey)) return null;
  const url = new URL(job.href);
  const product = url.hostname === MODIS_HOST ? 'modis-09a1-v061' : url.hostname === LANDSAT_HOST ? 'landsat-c2-l2' : url.hostname === NASA_HOST ? 'hls-l30-v2' : null;
  if (!product) return null;
  const parts = url.pathname.split('/').at(-2)?.split('_');
  if (product === 'modis-09a1-v061' ? modisAssetIdentity(job.href, job.assetKey)?.id !== job.itemId : product === 'hls-l30-v2' ? url.pathname.split('/')[3] !== job.itemId
    : parts?.length !== 7 || [parts[0], parts[1], parts[2], parts[3], parts[5], parts[6]].join('_') !== job.itemId) return null;
  return { product, directory: new URL('.', url).href };
}

// One complete triplet per exact scene + original processing directory. A
// newer completed repeat replaces the same channel, without mixing versions.
export function localRgbGroups(jobs) {
  const groups = new Map();
  for (const job of [...jobs].sort((a, b) => (b.updatedAt || '').localeCompare(a.updatedAt || ''))) {
    const info = identity(job, jobs);
    if (!info) continue;
    const key = JSON.stringify([job.itemId, info.product, info.directory]);
    const group = groups.get(key) || { ...info, channels: {} };
    group.channels[job.assetKey] ||= job;
    groups.set(key, group);
  }
  return [...groups.values()].filter(group => LANDSAT_BANDS.every(key => group.channels[key])).map(group => {
    const sourceJobs = LANDSAT_BANDS.map(key => group.channels[key]);
    return { ...sourceJobs[0], id: `rgb:${sourceJobs.map(job => job.id).join(':')}`, assetKey: 'rgb',
      kind: 'local_composite', sourceJobs, product: group.product, derived:Boolean(group.derived), outputPath: undefined, href: undefined, sha256: undefined };
  });
}

// Matched project outputs pin a common area and ordered original scene list.
// Multi-scene screening reopens those originals and selects all five layers
// together; independently mosaicked pixel values are never used as a mask.
export function modisRgbQualityJobs(group, jobs) {
  if (group?.product !== 'modis-09a1-v061' || group.sourceJobs?.length !== 3) return [];
  const rgb = group.sourceJobs[0];
  const original = job => {
    if (job.kind !== 'download' || job.status !== 'succeeded' || !hash(job.sha256)) return null;
    const id = modisAssetIdentity(job.href, job.assetKey);
    return id?.id === job.itemId ? JSON.stringify([job.itemId,new URL('.',job.href).href]) : null;
  };
  const key = job => {
    if (job.kind === 'download') return original(job);
    const plan=job.mosaicOutput, pins=job.mosaic?.sources;
    if (job.kind !== 'raster_mosaic' || job.status !== 'succeeded' || !hash(job.sha256) || !pins?.length || pins.length>32 || new Set(pins.map(p=>p.jobId)).size!==pins.length || !plan) return null;
    const inputs=pins.map(pin=>jobs.find(j=>j.id===pin.jobId && j.sha256===pin.sha256 && j.assetKey===job.assetKey));
    const sources=inputs.map(parent=>parent && original(parent));
    return sources.every(Boolean) && new Set(sources).size===sources.length ? JSON.stringify([job.mosaic.projectId,plan.width,plan.height,plan.crs,plan.bounds,plan.pixelSize,sources]) : null;
  };
  const expected=key(rgb);
  if (!expected || group.sourceJobs.some(j=>j.kind!==rgb.kind || key(j)!==expected)) return [];
  const matches=['modis_qc','modis_state'].map(band=>[...jobs].sort((a,b)=>(b.updatedAt||'').localeCompare(a.updatedAt||''))
    .find(j=>j.assetKey===band && j.kind===rgb.kind && key(j)===expected
      && (j.kind==='download' || j.mosaicOutput?.quality?.product==='modis-09a1-v061')));
  return matches.every(Boolean) ? matches : [];
}

function landsatPinIdentity(source) {
  const band=source.assetKey || source.band, href=source.href;
  try {
    const job={...source,assetKey:band,status:'succeeded',kind:'download'};
    const id=['qa_pixel','qa_radsat'].includes(band) ? landsatQualityIdentity(href,band)?.id
      : identity(job)?.product==='landsat-c2-l2' ? job.itemId : null;
    return id && id===source.itemId ? JSON.stringify([id,new URL('.',href).href]) : null;
  } catch { return null; }
}
export function landsatRgbQualityJobs(group, jobs) {
  if (group?.product!=='landsat-c2-l2' || group.sourceJobs?.length!==3) return [];
  const rgb=group.sourceJobs[0];
  const key=job=>{
    if (job.status!=='succeeded' || !hash(job.sha256)) return null;
    if (job.kind==='download') return landsatPinIdentity(job);
    const pins=job.mosaic?.sources, g=job.mosaicOutput;
    if (job.kind!=='raster_mosaic' || !pins?.length || pins.length>32 || new Set(pins.map(p=>p.jobId)).size!==pins.length || !g) return null;
    const originals=pins.map(pin=>{
      const source=jobs.find(j=>j.id===pin.jobId && j.sha256===pin.sha256 && j.assetKey===job.assetKey
        && j.kind==='download' && j.status==='succeeded');
      return source && landsatPinIdentity(source);
    });
    return originals.every(Boolean) && new Set(originals).size===originals.length
      ? JSON.stringify([job.mosaic.projectId,g.width,g.height,g.crs,g.bounds,g.pixelSize,originals]) : null;
  };
  const expected=key(rgb);
  if (!expected || group.sourceJobs.some(j=>j.kind!==rgb.kind || key(j)!==expected)) return [];
  const matches=['qa_pixel','qa_radsat'].map(band=>[...jobs].sort((a,b)=>(b.updatedAt||'').localeCompare(a.updatedAt||''))
    .find(j=>j.assetKey===band && j.kind===rgb.kind && key(j)===expected
      && (j.kind==='download' || j.mosaicOutput?.landsatQuality?.product==='landsat-c2-l2')));
  return matches.every(Boolean) ? matches : [];
}
export function rgbQualityJobs(group,jobs) {
  return group?.product==='landsat-c2-l2' ? landsatRgbQualityJobs(group,jobs) : modisRgbQualityJobs(group,jobs);
}
function validMaskCounts(job,result,pixels) {
  return Number.isSafeInteger(pixels) && pixels>0
    && ['examinedPixels','rejectedPixels','inputCommonValidPixels','removedValidPixels'].every(k=>Number.isSafeInteger(result?.[k]) && result[k]>=0 && result[k]<=pixels)
    && result.examinedPixels===pixels && result.removedValidPixels<=result.rejectedPixels
    && result.removedValidPixels<=result.inputCommonValidPixels
    && result.inputCommonValidPixels-result.removedValidPixels===job.rgbOutput.commonValidPixels;
}
function validLandsatRgbMask(job,mask,result,pixels) {
  const sources=job.rgbSpec.sources, parents=[...sources,...mask.sources];
  const coupled=mask.coupled, multi=Boolean(coupled);
  if (mask.schemaVersion!==(multi?'geod-landsat-rgb-mask/v2':'geod-landsat-rgb-mask/v1') || !multi && result.coupled
    || !['cloud_free','cloud_free_conservative'].includes(mask.policy) || typeof mask.excludeSnow!=='boolean'
    || mask.definition!==LANDSAT_QUALITY_DEFINITION || !validMaskCounts(job,result,pixels)
    || new Set(parents.map(s=>s.jobId)).size!==5) return false;
  const pair=source=>{
    if (!hash(source.sha256) || !Number.isSafeInteger(source.bytes) || source.bytes<=0 || source.bytes>536870912) return null;
    if (source.kind==='download') return landsatPinIdentity(source);
    const p=source.provenance;
    if (source.kind!=='raster_mosaic' || !p?.sources?.length || p.sources.length>32 || !multi && p.sources.length!==1 || !p.project?.id) return null;
    const original=p.sources.map(pin=>hash(pin.sha256) && landsatPinIdentity({...pin,band:source.band}));
    return original.every(Boolean) && new Set(original).size===original.length
      ? JSON.stringify([p.project.id,p.project.bounds,p.project.geometry,original,p.sources.map(pin=>pin.acquiredAt)]) : null;
  };
  const expected=pair(sources[0]);
  if (!expected || !parents.every((s,i)=>s.kind===sources[0].kind
    && s.band===['red','green','blue','qa_pixel','qa_radsat'][i] && pair(s)===expected)) return false;
  if (!multi) return true;
  const grid=job.rgbSpec.grid, counts=result.coupled, used=new Set();let previous='';
  return Boolean(coupled.selection==='newest complete qualified RGB scene wins; acquisition date then item ID break ties'
    && grid.pixelInterpretation==='PixelIsArea' && Array.isArray(coupled.scenes) && coupled.scenes.length>=2 && coupled.scenes.length<=32
    && JSON.stringify(coupled.geometry)===JSON.stringify(parents[0].provenance?.project?.geometry)
    && coupled.scenes.every((scene,index)=>{
      const g=scene.grid,id=scene.sources?.[0]?.itemId,date=id?.split('_')[3],priority=date+':'+id;
      if (!g || !Number.isSafeInteger(g.width) || g.width<1 || g.width>20000 || !Number.isSafeInteger(g.height) || g.height<1 || g.height>20000
        || g.crs!==grid.crs || !['PixelIsArea','PixelIsPoint'].includes(g.pixelInterpretation) || !finite(g.bounds,4) || !finite(g.pixelSize,2)
        || g.pixelSize.some(v=>v!==30) || [0,1].some(i=>Math.abs((g.bounds[i+2]-g.bounds[i])/[g.width,g.height][i]-30)>1e-7)
        || !/^\d{8}$/.test(date) || priority<=previous || !Array.isArray(scene.sources) || scene.sources.length!==5) return false;
      previous=priority;
      const offsets=[(g.bounds[0]-grid.bounds[0])/30,(grid.bounds[3]-g.bounds[3])/30];
      return offsets.every(v=>Number.isFinite(v) && Math.abs(v-Math.round(v))<1e-6 && Math.abs(v)<=100000)
        && scene.sources.every((s,c)=>{
          const pins=parents[c].provenance?.sources,pin=pins?.[index],identity=landsatPinIdentity(s);
          if (parents[c].kind!=='raster_mosaic' || pins?.length!==coupled.scenes.length || s.kind!=='download' || s.provenance
            || s.band!==['red','green','blue','qa_pixel','qa_radsat'][c] || !hash(s.sha256) || !identity
            || identity!==landsatPinIdentity(scene.sources[0]) || used.has(s.jobId)
            || !Number.isSafeInteger(s.bytes) || s.bytes<=0 || s.bytes>536870912
            || s.jobId!==pin?.jobId || s.sha256!==pin.sha256 || s.href!==pin.href || s.itemId!==pin.itemId) return false;
          used.add(s.jobId);return true;
        });
    })
    && Array.isArray(counts?.sceneValidPixels) && counts.sceneValidPixels.length===coupled.scenes.length
    && counts.sceneValidPixels.every(n=>Number.isSafeInteger(n)&&n>=0&&n<=pixels)
    && counts.sceneValidPixels.reduce((a,b)=>a+b,0)===job.rgbOutput.commonValidPixels
    && Number.isSafeInteger(counts.fallbackPixels) && counts.fallbackPixels>=0 && counts.fallbackPixels<=job.rgbOutput.commonValidPixels
    && result.rejectedPixels===pixels-job.rgbOutput.commonValidPixels
    && Array.isArray(job.rgbOutput.channelValidPixels) && job.rgbOutput.channelValidPixels.length===3
    && job.rgbOutput.channelValidPixels.every(n=>n===job.rgbOutput.commonValidPixels));
}

export function validRgbQualityMask(job) {
  const mask=job.rgbSpec?.qualityMask, result=job.rgbOutput?.qualityMask;
  if (!mask) return !result;
  if (!Array.isArray(job.rgbSpec.sources) || job.rgbSpec.sources.length!==3
    || !Array.isArray(mask.sources) || mask.sources.length!==2 || !result) return false;
  const pixels=job.rgbSpec.grid?.width*job.rgbSpec.grid?.height;
  if (job.rgbSpec.profile?.product==='landsat-c2-l2') return validLandsatRgbMask(job,mask,result,pixels);
  const coupled=mask.coupled;
  const coupledResult=result?.coupled;
  const parents=[...job.rgbSpec.sources,...(mask.sources||[])];
  const coherent=mask.schemaVersion==='geod-modis-rgb-mask/v1' ? !coupled && !coupledResult
    : mask.schemaVersion==='geod-modis-rgb-mask/v2' && coupled?.selection==='newest complete qualified RGB scene wins; composite start then item ID break ties'
      && Array.isArray(coupled.scenes) && coupled.scenes.length>=2 && coupled.scenes.length<=32
      && JSON.stringify(coupled.geometry)===JSON.stringify(parents[0]?.provenance?.project?.geometry)
      && coupled.scenes.every((scene,index)=>scene.grid?.width===2400 && scene.grid?.height===2400 && scene.grid?.crs===MODIS_CRS
        && Array.isArray(scene.sources) && scene.sources.length===5 && scene.sources.every((source,channel)=>{
          const pin=parents[channel]?.provenance?.sources?.[index];
          return parents[channel]?.kind==='raster_mosaic' && parents[channel].provenance.sources.length===coupled.scenes.length
            && source.kind==='download' && source.band===['red','green','blue','modis_qc','modis_state'][channel] && hash(source.sha256)
            && modisAssetIdentity(source.href,source.band)?.id===source.itemId && source.itemId===scene.sources[0].itemId
            && source.jobId===pin?.jobId && source.sha256===pin?.sha256 && source.href===pin?.href && source.itemId===pin?.itemId;
        }))
      && Array.isArray(coupledResult?.sceneValidPixels) && coupledResult.sceneValidPixels.length===coupled.scenes.length
      && coupledResult.sceneValidPixels.every(n=>Number.isSafeInteger(n) && n>=0 && n<=pixels)
      && coupledResult.sceneValidPixels.reduce((a,b)=>a+b,0)===job.rgbOutput.commonValidPixels
      && Number.isSafeInteger(coupledResult.fallbackPixels) && coupledResult.fallbackPixels>=0 && coupledResult.fallbackPixels<=job.rgbOutput.commonValidPixels
      && result.rejectedPixels===pixels-job.rgbOutput.commonValidPixels
      && Array.isArray(job.rgbOutput.channelValidPixels) && job.rgbOutput.channelValidPixels.length===3
      && job.rgbOutput.channelValidPixels.every(n=>n===job.rgbOutput.commonValidPixels);
  return Boolean(coherent && job.rgbSpec.profile?.product==='modis-09a1-v061'
    && ['clear','clear_best'].includes(mask.policy) && typeof mask.excludeSnow==='boolean' && mask.definition===QUALITY_DEFINITION
    && Array.isArray(mask.sources) && mask.sources.length===2 && new Set(mask.sources.map(s=>s.jobId)).size===2
    && mask.sources.every((s,i)=>s.band===['modis_qc','modis_state'][i] && hash(s.sha256) && modisAssetIdentity(s.href,s.band)
      && !job.rgbSpec.sources.some(b=>b.jobId===s.jobId))
    && validMaskCounts(job,result,pixels));
}

const finite = (array, length) => Array.isArray(array) && array.length === length && array.every(Number.isFinite);
export function validateCompositeInspection(data) {
  const info = data?.composite, profile = profiles[info?.product];
  if (!profile || data.bandCount !== 3 || data.dataType !== profile.dataType || data.nodata !== profile.nodata
    || !['PixelIsPoint', 'PixelIsArea'].includes(data.pixelInterpretation)
    || info.derived !== undefined && typeof info.derived !== 'boolean'
    || (info.product === 'viirs-09a1-v002' ? data.crs !== VIIRS_CRS || !info.derived && (data.width !== 1200 || data.height !== 1200) || data.pixelInterpretation !== 'PixelIsArea' : info.product === 'modis-09a1-v061' ? data.crs !== MODIS_CRS || !info.derived && (data.width !== 2400 || data.height !== 2400) || data.pixelInterpretation !== 'PixelIsArea' : !/^EPSG:(326|327)(0[1-9]|[1-5]\d|60)$/.test(data.crs))
    || !Number.isSafeInteger(data.width) || data.width < 1 || data.width > 20000
    || !Number.isSafeInteger(data.height) || data.height < 1 || data.height > 20000
    || !finite(data.bounds, 4) || data.bounds[0] >= data.bounds[2] || data.bounds[1] >= data.bounds[3]
    || !finite(data.pixelSize, 2) || data.pixelSize.some(value => Math.abs(value - (info.product === 'viirs-09a1-v002' ? VIIRS_PIXEL : info.product === 'modis-09a1-v061' ? MODIS_PIXEL : 30)) > 1e-6)
    || [0, 1].some(index => Math.abs((data.bounds[index + 2] - data.bounds[index]) / [data.width, data.height][index] - data.pixelSize[index]) > 1e-7)
    || !Number.isSafeInteger(data.previewWidth) || data.previewWidth < 1 || data.previewWidth > Math.min(data.width, 768)
    || !Number.isSafeInteger(data.previewHeight) || data.previewHeight < 1 || data.previewHeight > Math.min(data.height, 768)
    || typeof data.previewDataUrl !== 'string' || data.previewDataUrl.length > 4000000
    || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(data.previewDataUrl)
    || info.scale !== profile.scale || info.offset !== profile.offset
    || !Array.isArray(info.sources) || info.sources.length !== 3
    || info.sources.some((source, index) => source.band !== LANDSAT_BANDS[index] || typeof source.jobId !== 'string' || !source.jobId || !hash(source.sha256))
    || new Set(info.sources.map(source => source.jobId)).size !== 3
    || info.sampleCount !== data.previewWidth * data.previewHeight
    || !Number.isSafeInteger(info.validSampleCount) || info.validSampleCount < 0 || info.validSampleCount > info.sampleCount
    || !Array.isArray(info.displayRanges) || info.displayRanges.length !== 3
    || info.displayRanges.some(range => !finite(range, 2) || range.some(value => !Number.isInteger(value) || value < profile.min || value > profile.max)
      || range[0] > range[1] || info.validSampleCount === 0 && range.some(value => value !== 0)))
    throw new Error('The local RGB service returned invalid grid, calibration or source pins.');
  return data;
}

export function verifiedCompositeMetadata(job, data) {
  validateCompositeInspection(data);
  if (job?.kind === 'raster_rgb') {
    const spec=job.rgbSpec, g=spec?.grid, p=spec?.profile;
    if (job.status !== 'succeeded' || spec?.schemaVersion !== 'geod-scientific-rgb/v1' || data.artifact?.jobId !== job.id || data.artifact?.sha256 !== job.sha256 || !hash(job.sha256)
      || data.composite.derived !== true || data.composite.product !== p?.product || data.composite.scale !== p.scale || data.composite.offset !== p.offset || data.nodata !== p.nodata
      || !g || ['width','height','crs','pixelInterpretation'].some(key=>data[key] !== g[key]) || ['bounds','pixelSize'].some(key=>JSON.stringify(data[key]) !== JSON.stringify(g[key]))
      || !Array.isArray(spec.sources) || spec.sources.length !== 3 || data.composite.sources.some((s,i)=>s.jobId !== spec.sources[i].jobId || s.sha256 !== spec.sources[i].sha256 || s.band !== spec.sources[i].band)
      || !validRgbQualityMask(job))
      throw new Error('The scientific RGB response does not match its committed file and source specification.');
    return data;
  }
  if (job?.derived) {
    const sources=job.sourceJobs, grid=sources?.[0]?.mosaicOutput;
    if (!sources || sources.length !== 3 || !grid || data.composite.derived !== true || data.composite.product !== job.product
      || data.composite.sources.some((s,i)=>s.jobId!==sources[i].id || s.sha256!==sources[i].sha256)
      || ['width','height','crs'].some(key=>data[key]!==grid[key]) || ['bounds','pixelSize'].some(key=>JSON.stringify(data[key])!==JSON.stringify(grid[key])))
      throw new Error('The processed RGB response does not match its pinned band grids.');
    return data;
  }
  const expected = localRgbGroups(job?.sourceJobs || []).find(group => group.id === job.id);
  if (!expected || data.composite.product !== expected.product
    || data.artifact || data.composite.derived === true
    || data.composite.sources.some((source, index) => source.jobId !== expected.sourceJobs[index].id || source.sha256 !== expected.sourceJobs[index].sha256))
    throw new Error('The local RGB response does not match its completed source bands.');
  return data;
}

export function verifyCompositePixel(result, job, data, coordinate) {
  verifiedCompositeMetadata(job, data);
  const profile = profiles[data.composite.product];
  const [x, y] = coordinate;
  const pixel = [Math.floor((x - data.bounds[0]) / data.pixelSize[0]), Math.floor((data.bounds[3] - y) / data.pixelSize[1])];
  if (!result || !finite(coordinate, 2) || x < data.bounds[0] || x >= data.bounds[2] || y <= data.bounds[1] || y > data.bounds[3]
    || (job.kind === 'raster_rgb' ? result.artifact?.jobId !== job.id || result.artifact?.sha256 !== job.sha256 : Boolean(result.artifact))
    || !Array.isArray(result.sources) || result.sources.length !== 3
    || result.sources.some((source, index) => source.jobId !== data.composite.sources[index].jobId || source.sha256 !== data.composite.sources[index].sha256 || source.band !== LANDSAT_BANDS[index])
    || result.crs !== data.crs || !finite(result.pixel, 2) || result.pixel.some((value, index) => value !== pixel[index])
    || !finite(result.coordinate, 2) || result.coordinate.some((value, index) => Math.abs(value - coordinate[index]) > 1e-7)
    || !finite(result.center, 2) || result.center.some((value, index) => Math.abs(value - [data.bounds[0] + (pixel[0] + .5) * data.pixelSize[0], data.bounds[3] - (pixel[1] + .5) * data.pixelSize[1]][index]) > 1e-7)
    || !finite(result.values, 3) || result.values.some(value => !Number.isInteger(value) || value < profile.min || value > profile.max)
    || !Array.isArray(result.channelNoData) || result.channelNoData.length !== 3
    || !Array.isArray(result.reflectances) || result.reflectances.length !== 3
    || result.channelNoData.some((flag, index) => flag !== (result.values[index] === profile.nodata))
    || result.reflectances.some((value, index) => result.channelNoData[index] ? value !== null : !Number.isFinite(value) || Math.abs(value - (result.values[index] * profile.scale + profile.offset)) > 1e-12)
    || result.isNoData !== result.channelNoData.some(Boolean))
    throw new Error('The local RGB pixel response does not match its original bands and coordinate.');
  return result;
}
