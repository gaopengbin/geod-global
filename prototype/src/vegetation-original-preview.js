import {vegetationPreview,VEGETATION_PREVIEW_COLORS} from './vegetation-preview.js';
import {vegetationIdentity,VEGETATION_PIXEL} from './vegetation.js';
import {sinusoidalPreviewGrid,validateSinusoidalPreviewImages} from './modis-preview.js';

export function vegetationPreviewOriginals(scene,index) {
  const preview=vegetationPreview(scene,index);
  vegetationPreviewGrid(scene);
  return preview.hrefs;
}
export function vegetationPreviewGrid(scene) {
  return sinusoidalPreviewGrid(scene,vegetationIdentity(scene.id),4800,VEGETATION_PIXEL);
}
export function validateVegetationPreviewImages(sources,scene,index) {
  vegetationPreviewOriginals(scene,index);
  validateSinusoidalPreviewImages(sources,vegetationPreviewGrid(scene),{count:1,width:4800,nodata:-3000});
}
export function vegetationOriginalColorStyle(hasAlpha=false) {
  const colors=['interpolate',['linear'],['clamp',['band',1],-2000,10000],
    ...VEGETATION_PREVIEW_COLORS.flatMap((color,index)=>[-2000+1200*index,color])];
  return {color:hasAlpha?['case',['==',['band',2],0],['color',0,0,0,0],colors]:colors};
}
