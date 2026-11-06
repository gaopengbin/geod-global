import {desktopAvailable} from './runtime-client.js';
import {diagnosticError} from './usage-diagnostics.js';
import {usageRequest} from './usage-events.js';

export function validateProvenanceFile(value,id){
  if(!value||value.jobId!==id||value.filename!==`${id}.tif.provenance.json`
    ||!Number.isSafeInteger(value.bytes)||value.bytes<1||value.bytes>16*1024*1024
    ||!/^[a-f0-9]{64}$/.test(value.sha256||'')
    ||![value.sourceCount,value.capturedStacItems,value.missingStacItems].every(x=>Number.isSafeInteger(x)&&x>=0&&x<=256)
    ||value.capturedStacItems+value.missingStacItems!==value.sourceCount)throw new Error('The provenance service returned invalid metadata.');
  return value;
}
export async function prepareRasterProvenance(id,signal){
  return usageRequest('runtime','prepareProvenance',{id},()=>performPrepare(id,signal));
}
async function performPrepare(id,signal){
  if(!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(id))throw new Error('Invalid raster job identifier.');
  let value;
  try{
    if(desktopAvailable())value=await window.__TAURI__.core.invoke('prepare_raster_provenance',{id});
    else{
      const r=await fetch(`http://127.0.0.1:4318/jobs/${id}/provenance`,{method:'POST',headers:{'Content-Type':'application/json','X-GeoD-Client':'geod-global'},body:'{}',signal});
      const body=await r.json();if(!r.ok)throw new Error(body.error||'Provenance preparation failed.');value=body;
    }
    if(signal?.aborted)throw signal.reason;return validateProvenanceFile(value,id);
  }catch(error){throw diagnosticError(error,{stage:'export',diagnosticOrigin:'explicit'});}
}
export async function exportRasterProvenance(id){
  try{return await usageRequest('runtime','exportProvenance',{id},async()=>{
    const saved=await performExport(id);if(!saved)throw new DOMException('Raster export cancelled.','AbortError');return true;
  });}catch(error){if(error?.name==='AbortError')return false;throw diagnosticError(error,{stage:'export',diagnosticOrigin:'explicit'});}
}
async function performExport(id){
  if(!desktopAvailable())throw new Error('Open the desktop app to save the raster and provenance together.');
  const saved=await window.__TAURI__.core.invoke('export_raster_provenance',{id});
  if(typeof saved!=='boolean')throw new Error('The provenance service returned an invalid export result.');return saved;
}
