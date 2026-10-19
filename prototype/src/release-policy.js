import packageInfo from '../../package.json' with { type: 'json' };
import { providerById, providerForAssets, PROVIDERS } from './providers.js';

export const RELEASE_VERSION = packageInfo.version;
export const PROTECTED_ORIGINAL_NOTICE = 'Original downloads for this source are pending real-account verification and are unavailable in this candidate. Catalog search and account setup remain available.';
// Actual authenticated originals, independent numerical comparison and normal
// reopen evidence: docs/nasa-original-acceptance-20261015.md. Account and
// product entitlement checks remain native and apply to every new transfer.
export const VERIFIED_ACCOUNT_SOURCES = new Set(['nasa-earthdata','nasa-srtm','srtm-num','nasa-viirs-npp','nasa-viirs-noaa20','nasa-viirs-noaa21']);
export const protectedOriginalsDeferred = provider => PROVIDERS.some(source => source.id === provider && source.account && !VERIFIED_ACCOUNT_SOURCES.has(source.id));
export const projectOriginalsDeferred = project => Boolean(project.scenes?.some(scene => protectedOriginalsDeferred(providerForAssets(scene.assets))));
// Account-dialog tests never release a provider; only an accepted original
// workflow enters the set above. This does not establish every user's access.
export function originalsReleased(provider) {
  const source = typeof provider === 'string' ? providerById(provider) : provider;
  return Boolean(source?.download && (!source.account || VERIFIED_ACCOUNT_SOURCES.has(source.id)));
}
