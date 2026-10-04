// Small bounded retries for public display requests. Callers classify HTTP
// errors; cancellation and source/authentication validation are never retried.
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
      if(['AbortError','TimeoutError'].includes(error.name)||error.retryable===false||attempt===2)throw error;
      await retryDelay(attempt===0?250:750,signal);
    }
  }
}
