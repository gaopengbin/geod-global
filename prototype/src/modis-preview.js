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
  const identity=modisIdentity(scene.id);
  const expected=identity&&[MODIS_PIXEL,0,(identity.h-18)*2400*MODIS_PIXEL,0,-MODIS_PIXEL,(9-identity.v)*2400*MODIS_PIXEL];
  const t=scene.grid?.transform??expected,shape=scene.grid?.shape??[2400,2400];
  if(!identity||shape?.join(',')!=='2400,2400'||t?.length!==6||!t.every(Number.isFinite)
    ||t[1]!==0||t[3]!==0||Math.abs(t[0]-MODIS_PIXEL)>1e-6||Math.abs(t[4]+MODIS_PIXEL)>1e-6
    ||Math.abs(t[2]-(identity.h-18)*2400*MODIS_PIXEL)>0.01||Math.abs(t[5]-(9-identity.v)*2400*MODIS_PIXEL)>0.01)
    throw new Error('MODIS COG metadata does not match the selected original bands.');
  return t;
}

export function validateModisPreviewImages(sources,scene) {
  modisPreviewOriginals(scene);
  if(sources.length!==3)throw new Error('MODIS COG metadata does not match the selected original bands.');
  const t=modisPreviewGrid(scene);
  for(const images of sources){
    const image=[...images].sort((a,b)=>b.getWidth()-a.getWidth())[0];
    const keys=image?.getGeoKeys(),tag=name=>image?.fileDirectory.getValue(name);
    if(!image||image.getWidth()!==2400||image.getHeight()!==2400||image.getSamplesPerPixel()!==1
      ||Array.from(tag('BitsPerSample')||[]).join(',')!=='16'||Array.from(tag('SampleFormat')||[]).join(',')!=='2'
      ||image.getGDALNoData()!==-28672||keys?.GTModelTypeGeoKey!==1||keys.GTRasterTypeGeoKey!==1
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
