import packageInfo from '../../package.json' with { type: 'json' };
import { providerById, providerForAssets, PROVIDERS } from './providers.js';

export const RELEASE_VERSION = packageInfo.version;
export const PROTECTED_ORIGINAL_NOTICE = 'Original downloads for this source are pending real-account verification and are unavailable in this candidate. Catalog search and account setup remain available.';
export const protectedOriginalsDeferred = provider => PROVIDERS.some(source => source.id === provider && source.account);
export const projectOriginalsDeferred = project => Boolean(project.scenes?.some(scene => protectedOriginalsDeferred(providerForAssets(scene.assets))));
// Preserve the adapters and protocol tests for the next stage. The release UI
// must not turn account-dialog tests into an original-download support claim.
export function originalsReleased(provider) {
  const source = typeof provider === 'string' ? providerById(provider) : provider;
  return Boolean(source?.download && !source.account);
}
