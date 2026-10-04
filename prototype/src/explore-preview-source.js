import GeoTIFF from 'ol/source/GeoTIFF.js';
import { transformExtent } from 'ol/proj.js';
import { catalogItemPreview, catalogPreviewKind } from './catalog-preview.js';
import { createItemPreviewSource } from './item-preview-source.js';
import { elevationPreview, elevationPreviewURL, elevationColorStyle, validateElevationImages } from './elevation-preview.js';

class ElevationSource extends GeoTIFF {
  constructor(scene,href) {
    super({sources:[{url:href,bands:[1]}],normalize:false,interpolate:false});
    this.scene = scene;
    this.on('change',() => {
      if (this.getState() === 'error') this.viewRejector(new Error('Elevation COG metadata does not match the selected original height grid.'));
    });
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
  if (catalogPreviewKind(scene?.provider) !== 'elevation') {
    return {...createItemPreviewSource(catalogItemPreview(scene,channel)),imageTiles:true};
  }
  if (channel !== 'height') throw new Error('This scene has no verified elevation preview source.');
  const preview = elevationPreview(scene), source = new ElevationSource(scene,elevationPreviewURL(scene));
  return {source,extent:transformExtent(preview.bounds,'EPSG:4326','EPSG:3857',16),
    style:elevationColorStyle(),getStyle:() => elevationColorStyle(source.hasAlpha),
    imageTiles:false,dispose:() => source.dispose()};
}
