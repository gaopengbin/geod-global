import { isSupportedAsset } from './providers.js';

// STAC previews are not original products. Resolve the official OData identity
// on demand and keep only the stable URL in the project snapshot.
export async function prepareOriginalScenes(scenes, requester, signal) {
  const pending = scenes.filter(scene => scene.provider === 'copernicus' && !isSupportedAsset(scene.assets?.product?.href, 'product'));
  if (!pending.length) return scenes;
  const result = await requester('resolveProducts', { itemIds: pending.map(scene => scene.id) }, signal);
  if (signal?.aborted) throw signal.reason || new DOMException('Request cancelled', 'AbortError');
  const ids = new Set(pending.map(scene => scene.id));
  if (!Array.isArray(result) || result.length !== pending.length || new Set(result.map(asset => asset.itemId)).size !== pending.length
    || result.some(asset => !ids.has(asset.itemId) || !isSupportedAsset(asset.href, 'product') || asset.mediaType !== 'application/zip'
      || !Number.isSafeInteger(asset.bytes) || asset.bytes < 1 || asset.bytes > 4 * 1024 ** 3
      || Object.keys(asset).some(key => !['itemId', 'href', 'mediaType', 'bytes'].includes(key)))) {
    throw new Error('The source returned invalid original product metadata.');
  }
  const products = new Map(result.map(asset => [asset.itemId, asset]));
  return scenes.map(scene => {
    const product = products.get(scene.id);
    return product ? { ...scene, assets: { ...scene.assets, product: { href: product.href, type: product.mediaType, bytes: product.bytes } } } : scene;
  });
}
