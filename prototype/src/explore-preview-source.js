import { CogPreviewSource } from './cog-preview-source.js';
import { transformExtent } from 'ol/proj.js';
import { catalogItemPreview, catalogPreviewKind } from './catalog-preview.js';
import { createItemPreviewSource } from './item-preview-source.js';
import { elevationPreview, elevationPreviewURL, elevationColorStyle, validateElevationImages } from './elevation-preview.js';
import {modisPreviewOriginals,modisPreviewGrid,validateModisPreviewImages,modisPreviewColorStyle} from './modis-preview.js';
import {MODIS_CRS,MODIS_PROJECTION} from './modis.js';
import {assetReadURL,prepareAssetAccess} from './providers.js';
import proj4 from 'proj4';
import {register} from 'ol/proj/proj4.js';

class ModisPreviewSource extends CogPreviewSource {
  constructor(scene) {
    const hrefs=modisPreviewOriginals(scene);
    proj4.defs(MODIS_CRS,MODIS_PROJECTION);register(proj4);
    super({sources:hrefs.map(url=>({url,bands:[1],nodata:-28672})),projection:MODIS_CRS,normalize:false,interpolate:false,
      resolveURL:async(url,signal)=>{await prepareAssetAccess(hrefs,{signal});return assetReadURL(url);}});
    this.scene=scene;
  }
  async configure_(sources) {
    try{validateModisPreviewImages(sources,this.scene);await super.configure_(sources);}
    catch{this.error_=new Error('MODIS COG metadata does not match the selected original bands.');this.setState('error');this.viewRejector(this.error_);this.dispatchEvent('error');}
  }
  // The original has a user-defined MODIS sinusoidal GeoKey, not an EPSG code.
  async determineProjection(){return;}
}

class ElevationSource extends CogPreviewSource {
  constructor(scene,href) {
    super({sources:[{url:href,bands:[1]}],normalize:false,interpolate:false});
    this.scene = scene;
  }
  async configure_(sources) {
    try { this.pointTransform = validateElevationImages(sources,this.scene); await super.configure_(sources); }
    catch {
      this.error_ = new Error('Elevation COG metadata does not match the selected original height grid.');
      this.setState('error'); this.viewRejector(this.error_); this.dispatchEvent('error');
    }
  }
  determineTransformMatrix(sources) {
    super.determineTransformMatrix(sources);
    this.transformMatrix = this.pointTransform;
  }
}

export function createCatalogPreviewSource(scene,channel) {
  if(catalogPreviewKind(scene?.provider)==='reflectance'){
    if(channel!=='rgb')throw new Error('This scene has no verified MODIS reflectance preview source.');
    const source=new ModisPreviewSource(scene),t=modisPreviewGrid(scene);
    const bounds=[t[2],t[5]+2400*t[4],t[2]+2400*t[0],t[5]];
    return {source,extent:transformExtent(bounds,MODIS_CRS,'EPSG:3857',32),style:modisPreviewColorStyle(),getStyle:()=>modisPreviewColorStyle(source.hasAlpha),imageTiles:false,dispose:()=>source.dispose()};
  }
  if (catalogPreviewKind(scene?.provider) !== 'elevation') {
    return {...createItemPreviewSource(catalogItemPreview(scene,channel)),imageTiles:true};
  }
  if (channel !== 'height') throw new Error('This scene has no verified elevation preview source.');
  const preview = elevationPreview(scene), source = new ElevationSource(scene,elevationPreviewURL(scene));
  return {source,extent:transformExtent(preview.bounds,'EPSG:4326','EPSG:3857',16),
    style:elevationColorStyle(),getStyle:() => elevationColorStyle(source.hasAlpha),
    imageTiles:false,dispose:() => source.dispose()};
}
