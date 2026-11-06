// Small bounded retries for public display requests. Callers classify HTTP
// errors; cancellation and source/authentication validation are never retried.
import {diagnosticError} from './usage-diagnostics.js';

// A browser policy rejection has no HTTP response and otherwise looks exactly
// like an offline TypeError. Keep only the category, never the blocked URL/SAS.
export async function fetchSourceResponse(url, options, fetcher=fetch) {
  const target=globalThis.document;
  let blocked=false;
  const violation=event=>{
    if(event.effectiveDirective!=='connect-src'||event.disposition==='report')return;
    try{if(new URL(event.blockedURI).origin===new URL(url).origin)blocked=true;}catch{/* Not a URL-bound policy event. */}
  };
  target?.addEventListener('securitypolicyviolation',violation);
  try{return await fetcher(url,options);}
  catch(error){
    // Chromium may dispatch the policy event just after rejecting fetch.
    if(target&&error instanceof TypeError)await new Promise(resolve=>setTimeout(resolve,0));
    if(blocked){
      const denied=diagnosticError(new Error('Source request blocked by the desktop security policy.'),{reason:'permission_denied',diagnosticOrigin:'explicit'});
      denied.retryable=false;throw denied;
    }
    throw error;
  }finally{target?.removeEventListener('securitypolicyviolation',violation);}
}

function retryDelay(milliseconds, signal) {
  return new Promise((resolve,reject) => {
    const cancelled = () => { clearTimeout(timer); reject(signal.reason || new DOMException('Preview request cancelled.','AbortError')); };
    const timer = setTimeout(() => { signal?.removeEventListener('abort',cancelled); resolve(); },milliseconds);
    if (signal?.aborted) cancelled();
    else signal?.addEventListener('abort',cancelled,{once:true});
  });
}

export async function retryPreviewRequest(operation,signal) {
  for(let attempt=0;attempt<3;attempt++) {
    try {return await operation();}
    catch(error) {
      if(signal?.aborted)throw signal.reason || new DOMException('Preview request cancelled.','AbortError');
      if(error.name==='AbortError'||error.name==='TimeoutError'&&error.retryable!==true||error.retryable===false||attempt===2)throw error;
      await retryDelay(attempt===0?250:750,signal);
    }
  }
}
