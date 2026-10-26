import GeoTIFF from 'ol/source/GeoTIFF.js';
import { retryPreviewRequest } from './preview-network.js';
import {diagnosticError,failureDiagnostics} from './usage-diagnostics.js';

function metadataError(cause,phase='metadata') {
  const text=String(cause?.message || '');
  if (/^(Landsat|NAIP|Elevation) COG metadata does not match the selected (original bands|RGB \+ NIR source|original height grid)\.$/.test(text))return new Error(text);
  if (cause?.name==='TimeoutError')return new DOMException(`Preview ${phase} request timed out.`,'TimeoutError');
  if (/invalid|unsupported|mismatch|not match/i.test(text))return new Error('COG metadata validation failed.');
  const status=text.match(/HTTP (\d{3})\b/);
  return new Error(status ? `Preview ${phase} request returned HTTP ${status[1]}.` : 'Preview network request failed.');
}

// Buffer each bounded range before returning it: failures while reading the
// body need the same retry policy as a failed connection. Never fall back to
// downloading a whole original when a source ignores Range.
export async function loadPreviewRange(url, headers, external, fetchImpl = fetch, lifetime, deadlines={totalMs:42000,attemptMs:30000}) {
  const controller = new AbortController();
  const signals = [external, lifetime].filter(Boolean);
  const abort = event => controller.abort(event.target.reason);
  for (const signal of signals) {
    if (signal.aborted) controller.abort(signal.reason);
    else signal.addEventListener('abort', abort, {once:true});
  }
  const timer = setTimeout(() => controller.abort(new DOMException('Preview range request timed out.','TimeoutError')),deadlines.totalMs);
  let attempts=0;
  try {
    return await retryPreviewRequest(async () => {
      attempts++;
      controller.signal.throwIfAborted();
      const attempt=new AbortController(),cancel=()=>attempt.abort(controller.signal.reason);
      controller.signal.addEventListener('abort',cancel,{once:true});
      const attemptTimer=setTimeout(()=>attempt.abort(new DOMException('Preview range attempt timed out.','TimeoutError')),deadlines.attemptMs);
      try {
      const response = await fetchImpl(url,{headers,signal:attempt.signal,credentials:'omit'});
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
      const requested=new Headers(headers).get('Range')?.match(/^bytes=(\d+)-(\d+)$/);
      // Some public buckets do not CORS-expose Content-Range. Validate it when
      // visible; a hidden header must not reject an otherwise valid 206 read.
      const contentRange=response.headers.get('content-range');
      if(requested&&contentRange!==null){
        const range=contentRange.match(/^bytes (\d+)-(\d+)\/(\d+)$/);
        const start=Number(requested[1]),end=Number(requested[2]);
        if(!range||![start,end,...range.slice(1).map(Number)].every(Number.isSafeInteger)
          ||Number(range[1])!==start||Number(range[2])!==Math.min(end,Number(range[3])-1)
          ||Number(range[2])<start||bytes.byteLength!==Number(range[2])-start+1){
          const error=new Error('Preview response has an invalid Content-Range.');error.retryable=false;throw error;
        }
      }
      return new Response(bytes,{status:response.status,statusText:response.statusText,headers:response.headers});
      }catch(error){
        if(controller.signal.aborted)throw controller.signal.reason;
        if(attempt.signal.aborted){const timeout=attempt.signal.reason;timeout.retryable=true;throw timeout;}
        throw error;
      }finally{clearTimeout(attemptTimer);controller.signal.removeEventListener('abort',cancel);}
    },controller.signal);
  } catch(error) {throw diagnosticError(error,{retries:Math.max(0,attempts-1),diagnosticOrigin:'explicit'});} finally {
    clearTimeout(timer);
    for (const signal of signals) signal.removeEventListener('abort',abort);
  }
}

export class CogPreviewSource extends GeoTIFF {
  constructor(options) {
    const lifetime = new AbortController();
    // geotiff replaces rejected blocks with 'Request failed' and undefined
    // entries. Keep the final transport cause within this source's lifetime.
    const failures={last:null};
    super({...options,sourceOptions:{blockSize:65536,...options.sourceOptions},
      sources:options.sources.map(source=>({...source,
        loader:async(url,headers,signal)=>{
          try{return await loadPreviewRange(url,headers,signal,fetch,lifetime.signal);}
          catch(error){if(error.name!=='AbortError')failures.last=error;throw error;}
        }}))});
    this.previewLifetime = lifetime;
    // OL's initial transport catch only sets the state. All COG products must
    // also reject getView(), otherwise metadata remains pending until the UI's
    // inactivity timer, and the underlying network failure is lost.
    this.on('change',()=>{
      if(this.getState()==='error'){
        const cause=failures.last||this.getError();
        this.error_=diagnosticError(metadataError(cause),{...failureDiagnostics(cause,'preview_metadata'),stage:'preview_metadata',diagnosticOrigin:'explicit'});
        this.viewRejector(this.error_);
      }
    });
    this.on('tileloaderror',event=>{
      const cause=failures.last||event.tile?.getError?.();
      if(event.tile)event.tile.previewFailure=diagnosticError(metadataError(cause,'tile'),{
        ...failureDiagnostics(cause,'preview_tile'),stage:'preview_tile',diagnosticOrigin:'explicit'});
    });
  }
  disposeInternal() {
    this.cancelPreviewRequests();
    super.disposeInternal();
  }
  cancelPreviewRequests() {
    this.previewLifetime.abort(new DOMException('Preview source stopped.','AbortError'));
  }
}
