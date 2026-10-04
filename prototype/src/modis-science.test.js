import test from 'node:test';
import assert from 'node:assert/strict';
import { MODIS_SCIENCE, MODIS_SCIENCE_KEYS, MODIS_SCIENCE_COLORS, MODIS_SCIENCE_DEFINITION } from './modis-science-layers.js';
import { scienceCalendarDate, scienceMatchesJob, validSciencePixel } from './modis-science.js';
import { normalizeScene } from './catalog.js';
import { projectRequest } from './projects-client.js';
import { projectCatalogScenes, mergeProjectCatalog } from './project-explore.js';
import { vegetationIdentity, VEGETATION_PIXEL } from './vegetation.js';
import { validateRasterInspection } from './runtime-client.js';
import { verifiedMapMetadata } from './workspace-map-geometry.js';

// Reviewed protocol fixtures, not download or scientific processing evidence.
const id = 'MOD13Q1.A2025177.h08v05.061.2025195142416';
const href = key => `https://modiseuwest.blob.core.windows.net/modis-061-cogs/MOD13Q1/08/05/2025177/${id}_${MODIS_SCIENCE[key].asset}.tif`;
function item() {
  const p = vegetationIdentity(id);
  return { id, collection:'modis-13Q1-061', bbox:[-130.54,30,-103.92,40], properties:{start_datetime:p.date,end_datetime:p.endDate,platform:'terra','modis:horizontal-tile':8,'modis:vertical-tile':5},
    assets:{...Object.fromEntries(['ndvi','evi'].map(key=>[`250m_16_days_${key.toUpperCase()}`,{href:href('vi_red').replace(MODIS_SCIENCE.vi_red.asset,`250m_16_days_${key.toUpperCase()}`),type:'image/tiff; application=geotiff','raster:bands':[{data_type:'int16',scale:.0001,spatial_resolution:250,unit:key.toUpperCase()}]}])),
      ...Object.fromEntries(MODIS_SCIENCE_KEYS.map(key=>{const l=MODIS_SCIENCE[key];return [l.asset,{href:href(key),type:'image/tiff; application=geotiff','raster:bands':[{data_type:l.dataType,scale:l.scale,nodata:l.nodata,spatial_resolution:250,...(l.catalogUnit?{unit:key==='vi_doy'?'Julian Day':l.catalogUnit}:{})}]}];}))}};
}
function metadata(key) {
  const l=MODIS_SCIENCE[key],size=VEGETATION_PIXEL*4800;
  const labels=key==='vi_quality'?['Produced · good','Produced · check flags','Likely cloudy','Not produced · other']:['Good','Marginal','Snow or ice','Cloudy'];
  return {sha256:'a'.repeat(64),width:4800,height:4800,bandCount:1,dataType:{int8:'Int8',int16:'Int16',uint16:'UInt16'}[l.dataType],crs:'MODIS:Sinusoidal',nodata:l.nodata,
    bounds:[-10*size,3*size,-9*size,4*size],pixelSize:[VEGETATION_PIXEL,VEGETATION_PIXEL],previewWidth:160,previewHeight:160,previewDataUrl:'data:image/png;base64,AAAA',
    classes:['rank','flags'].includes(l.kind)?labels.map((label,value)=>({label,value,color:MODIS_SCIENCE_COLORS[value],count:value===0?20000:0})):[],
    science:{product:'modis-13q1-v061',band:key,layer:l.asset,kind:l.kind,unit:l.unit,scale:l.scale,offset:0,validRange:l.validRange,displayRange:l.validRange,
      palette:'modis13-science-v1',sampleCount:25600,validSampleCount:20000,outOfRangeSampleCount:0,countsFullResolution:false,pixelInterpretation:'PixelIsArea',definition:MODIS_SCIENCE_DEFINITION,...(key==='vi_doy'?{calendarYear:2025}:{})}};
}
test('All twelve layers preserve source identity and ancillary calibration when returning to Explore',()=>{
  const scene=normalizeScene(item(),'planetary-vegetation');
  const project=projectRequest({name:'science',bounds:[-122.55,37.68,-122.32,37.84],scenes:[scene]});
  assert.equal(Object.keys(project.scenes[0].assets).length,12);
  const restored=projectCatalogScenes(project)[0];
  for(const key of MODIS_SCIENCE_KEYS){assert.equal(restored.assets[key].href,scene.assets[key].href);assert.deepEqual(restored.assets[key].rasterBand,scene.assets[key].rasterBand);assert.equal(project.scenes[0].assets[key].rasterBand.dataType,MODIS_SCIENCE[key].dataType);}
  const current={...scene,grid:{stale:true},assets:{...scene.assets,vi_red:{...scene.assets.vi_red,href:href('vi_nir')}}};
  assert.deepEqual(mergeProjectCatalog({scenes:[current]},[scene]).scenes[0].grid,scene.grid);
});
test('Catalogue rejects divisor scales, wrong signedness, units, fill, and substituted channels',()=>{
  for(const [key,field,value] of [['vi_red','scale',10000],['vi_view_zenith','scale',100],['vi_relative_azimuth','scale',.1],['vi_quality','data_type','int16'],['vi_reliability','data_type','uint8'],['vi_doy','unit','Julian date'],['vi_relative_azimuth','nodata',-18000]]){
    const data=item();data.assets[MODIS_SCIENCE[key].asset]['raster:bands'][0][field]=value;assert.throws(()=>normalizeScene(data,'planetary-vegetation'));
  }
  const changed=item();changed.assets[MODIS_SCIENCE.vi_red.asset].href=href('vi_nir');assert.throws(()=>normalizeScene(changed,'planetary-vegetation'));
});
for(const key of MODIS_SCIENCE_KEYS)test(`${key} metadata binds actual sample type and separate sampled statistics`,()=>{
  const data=metadata(key),job={kind:'download',status:'succeeded',sha256:data.sha256,itemId:id,href:href(key),assetKey:key};
  assert.equal(validateRasterInspection(data),data);assert.equal(verifiedMapMetadata(job,data),data);
  for(const mutate of [d=>d.science.scale=100,d=>d.science.countsFullResolution=true,d=>d.science.validSampleCount=25601,d=>d.science.layer='NDVI',d=>d.science.unit='wrong',d=>d.reflectance={},d=>d.dataType='UInt8',d=>d.nodata=0,d=>d.science.calendarYear=2024]){
    const bad=structuredClone(data);mutate(bad);assert(!scienceMatchesJob(job,bad));
  }
});
test('Signed Int8 reliability retains fill -1 and rejects unsigned reinterpretation',()=>{
  const data=metadata('vi_reliability');
  for(const value of [-128,-1,0,1,2,3,127]){
    const p={value,isNoData:value===-1,science:{withinRange:value>=0&&value<=3}};assert(validSciencePixel(p,data));
    assert(!validSciencePixel({...p,value:255},data));assert(!validSciencePixel({...p,indexValue:0},data));
  }
});
test('Angle fill within the nominal range remains NoData and divisor scales cannot pass',()=>{
  const data=metadata('vi_relative_azimuth');
  for(const value of [-32768,-18000,-4000,-1,0,18000,32767]){
    const fill=value===-4000,p={value,isNoData:fill,science:{withinRange:!fill&&value>=-18000&&value<=18000,...(!fill?{convertedValue:value*.01}:{})}};
    assert(validSciencePixel(p,data));if(!fill)assert(!validSciencePixel({...p,science:{...p.science,convertedValue:value/10}},data)||value===0);
  }
});
test('Observation dates use the source year, leap days and fill independently of composite start',()=>{
  assert.equal(scienceCalendarDate(2024,60),'2024-02-29');assert.equal(scienceCalendarDate(2024,366),'2024-12-31');assert.equal(scienceCalendarDate(2025,366),undefined);
  const data=metadata('vi_doy');
  for(const value of [-1,0,177,185,365,366,367]){const date=scienceCalendarDate(2025,value),p={value,isNoData:value===-1,science:{withinRange:Boolean(date),...(date?{date}:{})}};assert(validSciencePixel(p,data));}
  assert(!validSciencePixel({value:185,isNoData:false,science:{withinRange:true,date:'2025-06-26'}},data));
});
