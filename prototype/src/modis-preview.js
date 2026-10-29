import {modisReflectancePreview} from './catalog-preview.js';
import {MODIS_PIXEL,modisIdentity} from './modis.js';

// Preview reads only bounded original COG ranges. Neither the signed access URL
// nor the display stretch changes the stored source asset or original DN.
export function modisPreviewOriginals(scene) {
  modisReflectancePreview(scene);
  modisPreviewGrid(scene);
  return ['red','green','blue'].map(key=>scene.assets[key].href);
}
export function modisPreviewGrid(scene) {
  return sinusoidalPreviewGrid(scene,modisIdentity(scene.id),2400,MODIS_PIXEL);
}
export function sinusoidalPreviewGrid(scene,identity,width,pixelSize) {
  const expected=identity&&[pixelSize,0,(identity.h-18)*width*pixelSize,0,-pixelSize,(9-identity.v)*width*pixelSize];
  const t=scene.grid?.transform??expected,shape=scene.grid?.shape??[width,width];
  if(!identity||shape?.join(',')!==`${width},${width}`||t?.length!==6||!t.every(Number.isFinite)
    ||t[1]!==0||t[3]!==0||Math.abs(t[0]-pixelSize)>1e-6||Math.abs(t[4]+pixelSize)>1e-6
    ||Math.abs(t[2]-expected[2])>0.01||Math.abs(t[5]-expected[5])>0.01)
    throw new Error('MODIS COG metadata does not match the selected original bands.');
  return t;
}

export function validateModisPreviewImages(sources,scene) {
  modisPreviewOriginals(scene);
  validateSinusoidalPreviewImages(sources,modisPreviewGrid(scene),{count:3,width:2400,nodata:-28672});
}
export function validateSinusoidalPreviewImages(sources,t,{count,width,nodata}) {
  if(sources.length!==count)throw new Error('MODIS COG metadata does not match the selected original bands.');
  for(const images of sources){
    const image=[...images].sort((a,b)=>b.getWidth()-a.getWidth())[0];
    const keys=image?.getGeoKeys(),tag=name=>image?.fileDirectory.getValue(name);
    if(!image||image.getWidth()!==width||image.getHeight()!==width||image.getSamplesPerPixel()!==1
      ||Array.from(tag('BitsPerSample')||[]).join(',')!=='16'||Array.from(tag('SampleFormat')||[]).join(',')!=='2'
      ||image.getGDALNoData()!==nodata||keys?.GTModelTypeGeoKey!==1||keys.GTRasterTypeGeoKey!==1
      ||keys.ProjCoordTransGeoKey!==24||keys.ProjLinearUnitsGeoKey!==9001||keys.GeogAngularUnitsGeoKey!==9102
      ||keys.GeogSemiMajorAxisGeoKey!==6371007.181||keys.GeogSemiMinorAxisGeoKey!==6371007.181
      ||keys.ProjCenterLongGeoKey!==0||keys.ProjFalseEastingGeoKey!==0||keys.ProjFalseNorthingGeoKey!==0
      ||tag('ModelTransformation')||tag('ExtraSamples')?.length||(tag('PhotometricInterpretation')??1)!==1
      ||(tag('Orientation')??1)!==1||(tag('PlanarConfiguration')??1)!==1)
      throw new Error('MODIS COG metadata does not match the selected original bands.');
    const origin=image.getOrigin(),resolution=image.getResolution();
    if(Math.abs(origin[0]-t[2])>1e-5||Math.abs(origin[1]-t[5])>1e-5
      ||Math.abs(resolution[0]-t[0])>1e-6||Math.abs(resolution[1]-t[4])>1e-6)
      throw new Error('MODIS COG metadata does not match the selected original bands.');
  }
}

export function modisPreviewColorStyle(hasAlpha=false) {
  const channel=band=>['*',255,['^',['clamp',['/',['band',band],3000],0,1],1/2.2]];
  return {color:['color',channel(1),channel(2),channel(3),hasAlpha?['/',['band',4],255]:['case',['all',...([1,2,3].map(band=>['!=',['band',band],-28672]))],1,0]]};
}
