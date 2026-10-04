import { vegetationPreview } from './vegetation-preview.js';
import { createItemPreviewSource } from './item-preview-source.js';

export function createVegetationPreviewSource(scene, index) {
  return createItemPreviewSource(vegetationPreview(scene,index));
}
