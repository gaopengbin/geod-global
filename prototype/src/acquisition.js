// These are public distribution labels, never a user, account, URL or device identity.
export function acquisitionLabel(value) {
  return value?.schemaVersion===1&&['zhihu','bilibili'].includes(value.source)&&value.campaign==='landsat_batch_20261023'
    &&Object.keys(value).every(key=>['schemaVersion','source','campaign'].includes(key))
    ?{acquisitionSource:value.source,acquisitionCampaign:value.campaign}:null;
}

export async function initializeAcquisition(client,invoke=globalThis.window?.__TAURI__?.core?.invoke) {
  if(typeof invoke!=='function'||!client.enabled())return;
  let timer;
  try {
    const value=await Promise.race([invoke('acquisition_context'),new Promise(resolve=>{timer=setTimeout(()=>resolve(null),500);})]);
    client.setAcquisition(value);
  }catch{/* Acquisition never blocks the application. */}
  finally{clearTimeout(timer);}
}
