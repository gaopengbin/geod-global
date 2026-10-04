import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {decodeLandsatQuality,LANDSAT_QUALITY_DEFINITION,LANDSAT_FILL_RULE,LANDSAT_RADSAT_RULE,LANDSAT_DERIVED_COVERAGE,LANDSAT_MOSAIC_POLICY,landsatQualityPalette,landsatQualityClass,landsatQualityProcessingProfile,validLandsatQualityMetadata,landsatQualityMatchesJob,validLandsatQualityPixel} from './landsat-quality.js';
const item=JSON.parse(readFileSync(new URL('../public/samples/landsat-quality-response.json',import.meta.url))).features[0];
const ids=['c45d6717-4ca6-439b-9027-4915f65f8798','93c495ab-4bcd-4f65-85c6-66e0fc9d0589','ca77729e-af5a-4d9d-9ab4-d8b1238f883d','471011eb-fb3c-4fc5-87ca-ff6c8c8c10d0'];
function processed(key) {
  const raw=key==='qa_pixel'?[64,65,0x8040,1]:[0,65535,32768,0],covered=[true,false,true,false];
  const definitions=decodeLandsatQuality(key,0).fields;
  const data={width:2,height:2,bandCount:1,dataType:'UInt16',nodata:null,crs:'EPSG:32610',bounds:[500000,4199940,500060,4200000],pixelSize:[30,30],classes:landsatQualityPalette(key).map(([label,color],value)=>({value,label,color,count:raw.filter((v,i)=>covered[i]&&landsatQualityClass(key,v)===value).length})),quality:{product:'landsat-c2-l2',band:key,layer:key.toUpperCase(),pixelInterpretation:'PixelIsArea',displayField:key==='qa_pixel'?'Pixel quality flags':'Saturation and terrain flags',sampleCount:4,validSampleCount:2,countsFullResolution:true,definition:LANDSAT_QUALITY_DEFINITION,flags:{sourceNoData:null,fillRule:key==='qa_pixel'?LANDSAT_FILL_RULE:LANDSAT_RADSAT_RULE,coverage:LANDSAT_DERIVED_COVERAGE,coverageMask:{kind:'internal-1bit',coveredPixels:2,uncoveredPixels:2},fields:definitions.map(({name,startBit,endBit})=>({name,startBit,endBit,counts:Array.from({length:2**(endBit-startBit+1)},(_,v)=>raw.filter(x=>(x>>>startBit & (2**(endBit-startBit+1)-1))===v).length)}))}}};
  const pin=jobId=>({jobId,sha256:'a'.repeat(64)});
  const job={id:ids[0],kind:'raster_mosaic',status:'succeeded',itemId:`project:${ids[1]}`,assetKey:key,href:item.assets[key].href,mosaic:{projectId:ids[1],assetKey:key,sources:[pin(ids[2])],...(key==='qa_radsat'?{coverageSources:[pin(ids[3])]}:{})},mosaicOutput:{width:2,height:2,bandCount:1,crs:data.crs,bounds:data.bounds,pixelSize:data.pixelSize,sourceCount:1,coveredPixels:2,maskedPixels:1,overlapPolicy:LANDSAT_MOSAIC_POLICY,landsatQuality:landsatQualityProcessingProfile(key)}};
  return {raw,covered,data,job};
}
test('derived quality metadata binds flags, grid, independent coverage and all pinned sources',()=>{
  for(const key of ['qa_pixel','qa_radsat']) {
    const {data,job}=processed(key);assert(validLandsatQualityMetadata(data));assert(landsatQualityMatchesJob(job,data));
    for(const mutate of [v=>v.mosaicOutput.landsatQuality.bits=8,v=>v.mosaicOutput.landsatQuality.extra='invalid',v=>v.mosaicOutput.coveredPixels=4,v=>v.mosaicOutput.bounds=[0,0,1,1],v=>v.mosaicOutput.quality={},v=>v.mosaic.sources[0].sha256='missing',v=>v.mosaic.projectId='not-uuid']) {
      const wrong=structuredClone(job);mutate(wrong);assert(!landsatQualityMatchesJob(wrong,data));
    }
    for(const mutate of [v=>delete v.quality.flags.coverageMask,v=>v.quality.flags.coverageMask.coveredPixels=3,v=>v.quality.flags.sourceNoData=0,v=>v.quality.pixelInterpretation='PixelIsPoint',v=>v.quality.flags.fields[0].counts[0]+=1]) {
      const wrong=structuredClone(data);mutate(wrong);assert(!validLandsatQualityMetadata(wrong));
    }
    assert(!landsatQualityMatchesJob({...job,kind:'download',itemId:item.id},data));
  }
});
test('saturation results require separate complete matching QA_PIXEL pins',()=>{
  const {job,data}=processed('qa_radsat');
  for(const mutate of [v=>delete v.mosaic.coverageSources,v=>v.mosaic.coverageSources=[],v=>v.mosaic.coverageSources[0].jobId=v.mosaic.sources[0].jobId,v=>v.mosaic.coverageSources[0].sha256='A'.repeat(64)]) {
    const wrong=structuredClone(job);mutate(wrong);assert(!landsatQualityMatchesJob(wrong,data));
  }
  const pixel=processed('qa_pixel');pixel.job.mosaic.coverageSources=job.mosaic.coverageSources;
  assert(!landsatQualityMatchesJob(pixel.job,pixel.data));
});
test('zero RADSAT and unused high bits retain raw values while uncovered cells stay transparent',()=>{
  for(const key of ['qa_pixel','qa_radsat']) {
    const {raw,covered,data}=processed(key);
    raw.forEach((value,i)=>{
      const [label,color]=landsatQualityPalette(key)[landsatQualityClass(key,value)];
      const pixel={value,isNoData:!covered[i],label:covered[i]?label:'Outside source coverage',color:covered[i]?color:'#000000',quality:{...decodeLandsatQuality(key,value),covered:covered[i]}};
      assert(validLandsatQualityPixel(pixel,data));
      assert(!validLandsatQualityPixel({...pixel,isNoData:covered[i]},data));
      const missing=structuredClone(pixel);delete missing.quality.covered;assert(!validLandsatQualityPixel(missing,data));
      assert(!validLandsatQualityPixel(pixel,{quality:{band:key}}));
    });
  }
});
