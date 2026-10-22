import GeoTIFF from 'ol/source/GeoTIFF.js';
import { retryPreviewRequest } from './preview-network.js';
import {diagnosticError,failureDiagnostics} from './usage-diagnostics.js';

function metadataError(cause) {
  const text=String(cause?.message || '');
  if (/^(Landsat|NAIP|Elevation) COG metadata does not match the selected (original bands|RGB \+ NIR source|original height grid)\.$/.test(text))return new Error(text);
  if (cause?.name==='TimeoutError')return new DOMException('Preview metadata request timed out.','TimeoutError');
  if (/invalid|unsupported|mismatch|not match/i.test(text))return new Error('COG metadata validation failed.');
  const status=text.match(/HTTP (\d{3})\b/);
  return new Error(status ? `Preview metadata request returned HTTP ${status[1]}.` : 'Preview network request failed.');
}

// Buffer each bounded range before returning it: failures while reading the
// body need the same retry policy as a failed connection. Never fall back to
// downloading a whole original when a source ignores Range.
export async function loadPreviewRange(url, headers, external, fetchImpl = fetch, lifetime) {
  const controller = new AbortController();
  const signals = [external, lifetime].filter(Boolean);
  const abort = event => controller.abort(event.target.reason);
  for (const signal of signals) {
    if (signal.aborted) controller.abort(signal.reason);
    else signal.addEventListener('abort', abort, {once:true});
  }
  const timer = setTimeout(() => controller.abort(new DOMException('Preview range request timed out.','TimeoutError')),30_000);
  let attempts=0;
  try {
    return await retryPreviewRequest(async () => {
      attempts++;
      controller.signal.throwIfAborted();
      const response = await fetchImpl(url,{headers,signal:controller.signal,credentials:'omit'});
      if (!response.ok || (new Headers(headers).has('Range') && response.status !== 206)) {
        await response.body?.cancel();
        const error = new Error(`Preview range request returned HTTP ${response.status}.`);
        error.retryable = [408,425,429].includes(response.status) || response.status >= 500;
        throw diagnosticError(error,{httpStatus:String(response.status),diagnosticOrigin:'explicit'});
      }
      const bytes = await response.arrayBuffer();
      const declared = response.headers.get('content-length');
      if (declared && !response.headers.get('content-encoding') && Number(declared) !== bytes.byteLength)
        throw new TypeError('Preview network response was incomplete.');
      return new Response(bytes,{status:response.status,statusText:response.statusText,headers:response.headers});
    },controller.signal);
  } catch(error) {throw diagnosticError(error,{retries:Math.max(0,attempts-1),diagnosticOrigin:'explicit'});} finally {
    clearTimeout(timer);
    for (const signal of signals) signal.removeEventListener('abort',abort);
  }
}

export class CogPreviewSource extends GeoTIFF {
  constructor(options) {
    const lifetime = new AbortController();
    super({...options,sourceOptions:{blockSize:65536,...options.sourceOptions},
      sources:options.sources.map(source=>({...source,
        loader:(url,headers,signal)=>loadPreviewRange(url,headers,signal,fetch,lifetime.signal)}))});
    this.previewLifetime = lifetime;
    // OL's initial transport catch only sets the state. All COG products must
    // also reject getView(), otherwise metadata remains pending until the UI's
    // inactivity timer, and the underlying network failure is lost.
    this.on('change',()=>{
      if(this.getState()==='error'){
        this.error_=diagnosticError(metadataError(this.getError()),{...failureDiagnostics(this.getError(),'preview_metadata'),stage:'preview_metadata',diagnosticOrigin:'explicit'});
        this.viewRejector(this.error_);
      }
    });
  }
  disposeInternal() {
    this.previewLifetime.abort(new DOMException('Preview source disposed.','AbortError'));
    super.disposeInternal();
  }
}
