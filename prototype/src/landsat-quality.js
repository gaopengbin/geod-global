import { isSupportedAsset, LANDSAT_HOST } from './providers.js';
export const LANDSAT_QUALITY_KEYS = ['qa_pixel','qa_radsat'];
export const LANDSAT_QUALITY_DEFINITION = 'https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands';
export const LANDSAT_FILL_RULE = 'QA_PIXEL bit 0 marks fill; all other unsigned values are retained';
export const LANDSAT_RADSAT_RULE = 'QA_RADSAT has no fill flag; zero means no saturation or terrain occlusion flags';
export const LANDSAT_PIXEL_COVERAGE = 'Image coverage follows QA_PIXEL bit 0, independently of the TIFF NoData tag';
export const LANDSAT_RADSAT_COVERAGE = 'QA_RADSAT alone cannot determine image coverage; use the matching QA_PIXEL fill flag';
export const LANDSAT_DERIVED_COVERAGE = 'Coverage follows the independent internal mask, derived from matching QA_PIXEL bit 0 and the project geometry';
export const LANDSAT_MOSAIC_POLICY = 'newest scene with QA_PIXEL bit 0 unset wins; complete UInt16 flags retained; filled newer scenes do not erase covered older samples; independent internal mask; no quality ranking or bit merging';
export const landsatQualityProcessingProfile = key => ({schemaVersion:'geod-landsat-quality-mosaic/v1',product:'landsat-c2-l2',band:key,bits:16,definition:LANDSAT_QUALITY_DEFINITION,coverage:LANDSAT_DERIVED_COVERAGE});
export function landsatQualityIdentity(href,key) {
  if (!LANDSAT_QUALITY_KEYS.includes(key) || !isSupportedAsset(href,key)) return null;
  const p = new URL(href).pathname.split('/'), product = p.at(-2), parts = product.split('_');
  if (p.length !== 10 || !/^(LC08|LC09)_(L2SP|L2SR)_\d{6}_\d{8}_\d{8}_02_(T1|T2|RT)$/.test(product)
    || p[5] !== parts[3].slice(0,4) || p[6] !== parts[2].slice(0,3) || p[7] !== parts[2].slice(3)
    || p.at(-1) !== `${product}_${key.toUpperCase()}.TIF`) return null;
  return {id:[...parts.slice(0,4),...parts.slice(5)].join('_'),product,host:LANDSAT_HOST};
}
const noSat = ['Not saturated','Saturated'], unused = ['Unset','Unused bit set'];
const fields = {
  qa_pixel:[
    ['Fill',0,0,['Image data','Fill data']],['Dilated cloud',1,1,['Not dilated or no cloud','Dilated cloud']],
    ['High-confidence cirrus',2,2,['Cirrus confidence not high','High-confidence cirrus']],
    ['High-confidence cloud',3,3,['Cloud confidence not high','High-confidence cloud']],
    ['High-confidence cloud shadow',4,4,['Shadow confidence not high','High-confidence cloud shadow']],
    ['High-confidence snow / ice',5,5,['Snow confidence not high','High-confidence snow / ice']],
    ['Clear cloud flag',6,6,['Clear flag unset','Cloud and dilated-cloud flags unset']],['Water',7,7,['Land or cloud','Water']],
    ['Cloud confidence',8,9,['Confidence unset','Low','Medium','High']],
    ['Cloud shadow confidence',10,11,['Confidence unset','Low','Reserved code','High']],
    ['Snow / ice confidence',12,13,['Confidence unset','Low','Reserved code','High']],
    ['Cirrus confidence',14,15,['Confidence unset','Low','Reserved code','High']]],
  qa_radsat:[...[1,2,3,4,5,6,7].map((b,i)=>[`Band ${b} saturation`,i,i,noSat]),
    ['Unused bit 7',7,7,unused],['Band 9 saturation',8,8,noSat],['Unused bit 9',9,9,unused],
    ['Unused bit 10',10,10,unused],['Terrain occlusion',11,11,['Not terrain-occluded','Terrain occlusion']],
    ['Unused bits 12–15',12,15,['Unset']]],
};
export function decodeLandsatQuality(key,raw) {
  if (!fields[key] || !Number.isSafeInteger(raw) || raw<0 || raw>65535) throw new Error('Invalid unsigned Landsat quality value.');
  return {layer:key.toUpperCase(),binary:raw.toString(2).padStart(16,'0'),hex:`0x${raw.toString(16).toUpperCase().padStart(4,'0')}`,
    fields:fields[key].map(([name,startBit,endBit,labels])=>{
      const value = raw>>>startBit & (2**(endBit-startBit+1)-1), label=labels[value] || 'Unused bits set';
      return {name,startBit,endBit,value,label,defined:labels[value] !== undefined && label !== 'Reserved code' && !(name.startsWith('Unused') && value!==0)};
    })};
}
export const landsatQualityPalette = key => key==='qa_pixel'
  ? [['Clear flag unset','#6b7280'],['Clear cloud flag','#2563eb'],['Dilated cloud','#eab308'],['High-confidence cloud','#e2e8f0'],['High-confidence cirrus','#7dd3fc'],['High-confidence cloud shadow','#643200'],['High-confidence snow / ice','#ff96ff'],['Water','#00a0be']]
  : [['No saturation or terrain occlusion flags','#2563eb'],['RGB band saturation','#ef4444'],['Other band saturation','#eab308'],['Terrain occlusion','#643200'],['Unused bits set','#6b7280']];
export function landsatQualityClass(key,v) {
  if (key==='qa_pixel') {for(const [mask,index] of [[8,3],[2,2],[4,4],[16,5],[32,6],[128,7],[64,1]]) if(v&mask) return index;}
  else {if(v&0xf680) return 4;if(v&2048) return 3;if(v&14) return 1;if(v&0x0171) return 2;}
  return 0;
}
export function validLandsatQualityMetadata(data) {
  const q=data?.quality, flags=q?.flags, key=q?.band, definitions=fields[key], mask=flags?.coverageMask, derived=mask!==undefined;
  if (!definitions || q.product!=='landsat-c2-l2' || q.layer!==key.toUpperCase() || q.definition!==LANDSAT_QUALITY_DEFINITION
    || !['PixelIsArea','PixelIsPoint'].includes(q.pixelInterpretation) || q.displayField!==(key==='qa_pixel'?'Pixel quality flags':'Saturation and terrain flags')
    || q.countsFullResolution!==true || !Number.isSafeInteger(data.width) || data.width<1 || (!derived && data.width>20000)
    || !Number.isSafeInteger(data.height) || data.height<1 || (!derived && data.height>20000)
    || !Number.isSafeInteger(q.sampleCount) || q.sampleCount!==data.width*data.height
    || !Number.isSafeInteger(q.validSampleCount) || q.validSampleCount<0 || q.validSampleCount>q.sampleCount
    || !/^EPSG:(326|327)(0[1-9]|[1-5][0-9]|60)$/.test(data.crs) || data.bandCount!==1 || data.dataType!=='UInt16' || data.nodata!==null
    || !Array.isArray(data.pixelSize) || data.pixelSize.length!==2 || data.pixelSize.some(v=>!Number.isFinite(v)||Math.abs(v-30)>1e-6)
    || data.reflectance!==undefined || data.elevation!==undefined || data.aerial!==undefined || data.radar!==undefined
    || !flags || ![null,key==='qa_pixel'?1:0].includes(flags.sourceNoData)
    || flags.fillRule!==(key==='qa_pixel'?LANDSAT_FILL_RULE:LANDSAT_RADSAT_RULE)
    || flags.coverage!==(derived?LANDSAT_DERIVED_COVERAGE:key==='qa_pixel'?LANDSAT_PIXEL_COVERAGE:LANDSAT_RADSAT_COVERAGE)
    || !Array.isArray(flags.fields) || flags.fields.length!==definitions.length) return false;
  if(derived && (!mask || mask.kind!=='internal-1bit' || q.pixelInterpretation!=='PixelIsArea' || flags.sourceNoData!==null
    || !Number.isSafeInteger(mask.coveredPixels) || mask.coveredPixels!==q.validSampleCount
    || !Number.isSafeInteger(mask.uncoveredPixels) || mask.uncoveredPixels<0 || mask.coveredPixels+mask.uncoveredPixels!==q.sampleCount)) return false;
  if(flags.fields.some((f,i)=>f.name!==definitions[i][0] || f.startBit!==definitions[i][1] || f.endBit!==definitions[i][2]
    || !Array.isArray(f.counts) || f.counts.length!==2**(f.endBit-f.startBit+1)
    || f.counts.some(v=>!Number.isSafeInteger(v)||v<0) || f.counts.reduce((a,b)=>a+b,0)!==q.sampleCount)) return false;
  if (key==='qa_pixel' && q.validSampleCount!==flags.fields[0].counts[0] || key==='qa_radsat' && !derived && q.validSampleCount!==q.sampleCount) return false;
  const palette=landsatQualityPalette(key);
  return Array.isArray(data.classes) && data.classes.length===palette.length
    && data.classes.every((c,i)=>c.value===i && c.label===palette[i][0] && c.color===palette[i][1] && Number.isSafeInteger(c.count) && c.count>=0)
    && data.classes.reduce((a,c)=>a+c.count,0)===q.validSampleCount;
}
export function landsatQualityMatchesJob(job,data) {
  if(!validLandsatQualityMetadata(data) || data.quality.band!==job.assetKey || job.rgbSpec) return false;
  const identity=landsatQualityIdentity(job.href,job.assetKey), derived=data.quality.flags.coverageMask!==undefined;
  if(!identity) return false;
  if(job.kind==='download') return !derived && identity.id===job.itemId && !job.mosaic && !job.mosaicOutput;
  const spec=job.mosaic,plan=job.mosaicOutput,expected=landsatQualityProcessingProfile(job.assetKey),profile=plan?.landsatQuality;
  const uuid=v=>typeof v==='string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
  const pinned=list=>Array.isArray(list) && list.length<=32 && list.every(pin=>uuid(pin?.jobId) && /^[0-9a-f]{64}$/.test(pin.sha256));
  const coverage=spec?.coverageSources || [];
  if(job.kind!=='raster_mosaic' || !derived || !uuid(job.id) || !uuid(spec?.projectId) || job.itemId!==`project:${spec.projectId}`
    || spec.assetKey!==job.assetKey || !pinned(spec.sources) || !spec.sources.length || !pinned(coverage)
    || coverage.length!==(job.assetKey==='qa_radsat'?spec.sources.length:0)
    || new Set([...spec.sources,...coverage].map(pin=>pin.jobId)).size!==spec.sources.length+coverage.length
    || !profile || Object.keys(profile).length!==Object.keys(expected).length || Object.keys(expected).some(k=>profile[k]!==expected[k])
    || ['calibration','elevation','aerial','radar','quality'].some(k=>plan[k]!==undefined)
    || plan.width!==data.width || plan.height!==data.height || plan.bandCount!==1 || plan.crs!==data.crs
    || plan.sourceCount!==spec.sources.length || plan.coveredPixels!==data.quality.validSampleCount
    || !Number.isSafeInteger(plan.maskedPixels) || plan.maskedPixels<0 || plan.maskedPixels+plan.coveredPixels>data.quality.sampleCount
    || plan.overlapPolicy!==LANDSAT_MOSAIC_POLICY
    || !Array.isArray(plan.bounds) || plan.bounds.length!==4 || plan.bounds.some((v,i)=>!Number.isFinite(v)||v!==data.bounds?.[i])
    || !Array.isArray(plan.pixelSize) || plan.pixelSize.length!==2 || plan.pixelSize.some((v,i)=>v!==data.pixelSize[i])) return false;
  return true;
}
export function validLandsatQualityPixel(result,data) {
  const key=data.quality?.band, derived=data.quality?.flags?.coverageMask!==undefined, covered=result.quality?.covered;
  if (!LANDSAT_QUALITY_KEYS.includes(key) || !Number.isSafeInteger(result.value) || result.value<0 || result.value>65535
    || (derived ? typeof covered!=='boolean' || result.isNoData===covered || key==='qa_pixel' && covered===Boolean(result.value&1)
      : covered!==undefined || result.isNoData!==(key==='qa_pixel' && Boolean(result.value&1)))
    || typeof result.isNoData!=='boolean' || result.values!==undefined || result.reflectance!==undefined
    || result.nearInfrared!==undefined || result.decibels!==undefined) return false;
  const expected=decodeLandsatQuality(key,result.value),actual=result.quality;
  if(!actual || actual.layer!==expected.layer || actual.binary!==expected.binary || actual.hex!==expected.hex
    || !Array.isArray(actual.fields) || actual.fields.length!==expected.fields.length
    || actual.fields.some((f,i)=>Object.keys(expected.fields[i]).some(k=>f[k]!==expected.fields[i][k]))) return false;
  const [label,color]=landsatQualityPalette(key)[landsatQualityClass(key,result.value)];
  return result.label===(result.isNoData?derived?'Outside source coverage':'Fill data':label) && result.color===(result.isNoData?'#000000':color);
}
