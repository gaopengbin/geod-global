import { demProduct, demCell, isSupportedAsset } from './providers.js';
import { validPreviewBounds } from './catalog-preview.js';

export const ELEVATION_PREVIEW_RANGE = [-100,1000];
export function elevationPreviewURL(scene) {
  elevationPreview(scene);
  const core=globalThis.window?.__TAURI__?.core;
  return typeof core?.convertFileSrc === 'function' ? core.convertFileSrc(scene.id,'geod-elevation')
    : `http://127.0.0.1:4318/preview/elevation/${encodeURIComponent(scene.id)}`;
}
export function elevationPreview(scene) {
  const product = demProduct(scene?.id), cell = demCell(scene?.id), href = scene?.assets?.elevation?.href;
  const [height,width] = scene?.grid?.shape || [], t = scene?.grid?.transform;
  if (!product || product.provider !== scene.provider || scene.collection !== `cop-dem-glo-${product.resolution}`
    || scene.crs !== 'EPSG:4326' || !isSupportedAsset(href,'elevation')
    || href !== `https://${product.host}/${scene.id}/${scene.id}.tif`
    || !product.widths.includes(width) || height !== product.height || !validPreviewBounds(scene.bbox)
    || !Array.isArray(t) || t.length !== 6 || !t.every(Number.isFinite) || t[1] !== 0 || t[3] !== 0
    || Math.abs(t[0]-1/width) > 1e-12 || Math.abs(t[4]+1/height) > 1e-12
    || Math.abs(t[2]-(cell[0]-t[0]/2)) > 1e-10 || Math.abs(t[5]-(cell[1]+1-t[4]/2)) > 1e-10) {
    throw new Error('This scene has no verified elevation preview source.');
  }
  return {href,bounds:[t[2],t[5]+height*t[4],t[2]+width*t[0],t[5]],channel:'height',itemId:scene.id};
}

// Verify the actual original TIFF before using its samples. PixelIsPoint
// tiepoints locate sample centres; adjust geometry by half a pixel only.
export function validateElevationImages(sources,scene) {
  elevationPreview(scene);
  const image = [...(sources?.[0] || [])].sort((a,b) => b.getWidth()-a.getWidth())[0];
  const keys = image?.getGeoKeys(), tag = name => image?.fileDirectory.getValue(name);
  const [height,width] = scene.grid.shape, t = scene.grid.transform;
  const nodata = image?.getGDALNoData();
  if (sources?.length !== 1 || !image || keys?.GTModelTypeGeoKey !== 2 || keys.GTRasterTypeGeoKey !== 2
    || keys.GeographicTypeGeoKey !== 4326 || keys.GeogAngularUnitsGeoKey !== 9102
    || image.getSamplesPerPixel() !== 1 || image.getWidth() !== width || image.getHeight() !== height
    || Array.from(tag('BitsPerSample') || []).join(',') !== '32' || Array.from(tag('SampleFormat') || []).join(',') !== '3'
    || tag('ExtraSamples')?.length || tag('ModelTransformation') || (tag('PhotometricInterpretation') ?? 1) !== 1
    || (tag('PlanarConfiguration') ?? 1) !== 1 || (tag('Orientation') ?? 1) !== 1
    || !(nodata === null || Number.isNaN(nodata) || Number.isFinite(nodata) && Math.fround(nodata) === nodata)) {
    throw new Error('Elevation COG metadata does not match the selected original height grid.');
  }
  const origin = image.getOrigin(), resolution = image.getResolution();
  if (Math.abs(resolution[0]-t[0]) > 1e-12 || Math.abs(resolution[1]-t[4]) > 1e-12
    || Math.abs(origin[0]-resolution[0]/2-t[2]) > 1e-10 || Math.abs(origin[1]-resolution[1]/2-t[5]) > 1e-10) {
    throw new Error('Elevation COG geometry differs from the selected Point grid.');
  }
  return [1,0,0,1,resolution[0]/2,resolution[1]/2];
}

export function elevationColorStyle(hasAlpha = false) {
  const raw = ['band',1], [min,max] = ELEVATION_PREVIEW_RANGE;
  const value = ['*',255,['clamp',['/', ['-',raw,min],max-min],0,1]];
  return {color:['color',value,value,value,['case',['==',raw,raw],hasAlpha ? ['band',2] : 1,0]]};
}
