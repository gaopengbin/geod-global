import React,{useEffect,useRef,useState} from 'react';
import {Download,FileJson} from 'lucide-react';
import {Button,InfoDetails,Modal,Spinner} from './ui/index.jsx';
import {desktopAvailable} from './runtime-client.js';
import {prepareRasterProvenance,exportRasterProvenance} from './provenance-client.js';
import {useI18n} from './i18n.jsx';

export function RasterProvenanceButton({job}){
  const {t}=useI18n(),[result,setResult]=useState(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[saved,setSaved]=useState(false),request=useRef(null);
  useEffect(()=>()=>request.current?.abort(),[]);
  async function prepare(){
    if(busy)return;const controller=new AbortController();request.current=controller;setBusy(true);setError('');setSaved(false);
    try{const value=await prepareRasterProvenance(job.id,controller.signal);if(!controller.signal.aborted)setResult(value);}
    catch(e){if(!controller.signal.aborted)setError(e.message||String(e));}
    finally{if(!controller.signal.aborted)setBusy(false);}
  }
  async function save(){
    if(busy)return;setBusy(true);setError('');
    try{setSaved(await exportRasterProvenance(job.id));}catch(e){setError(e.message||String(e));}finally{setBusy(false);}
  }
  return <><Button size="sm" disabled={busy} onClick={prepare}>{busy?<Spinner size={15}/>:<FileJson size={15}/>} {t('Save raster with provenance')}</Button>
    {(result||error)&&<Modal title={t('Raster and provenance')} closeLabel={t('Close')} closeDisabled={busy} onClose={()=>{setResult(null);setError('');setSaved(false);}}>
      {result&&<><p>{t('The GeoTIFF and a matching JSON sidecar keep source identifiers, original catalog context, checksums and the actual processing steps together.')}</p>
        <p>{t('{captured} of {count} source assets have original STAC context.',{captured:result.capturedStacItems,count:result.sourceCount})}</p>
        {result.missingStacItems>0&&<><p role="status">{t('Some original catalog records could not be captured. The JSON marks them as unavailable; it retains known source identities and processing records.')}</p><Button size="sm" variant="secondary" disabled={busy} onClick={prepare}>{t('Retry catalog context')}</Button></>}
        <InfoDetails summary={t('What the provenance file contains')} closeLabel={t('Close')} presentation="popover"><p>{t('Recorded processing parameters describe the operations that actually ran. No resampling or reprojection is claimed when the original grid was retained.')}</p><p>{t('Catalog metadata fetched during export is identified separately from a saved download snapshot. Temporary credentials and local file paths are excluded.')}</p><p>{t('The JSON includes spatial bounds. Review them before sharing.')}</p></InfoDetails>
        {desktopAvailable()?<Button disabled={busy} onClick={save}>{busy?<Spinner size={15}/>:<Download size={15}/>} {t('Save GeoTIFF + JSON')}</Button>:<Button asChild><a href={`http://127.0.0.1:4318/jobs/${job.id}/provenance`} download={result.filename}><Download size={15}/>{t('Download provenance JSON')}</a></Button>}
        {saved&&<p role="status">{t('The raster and its provenance sidecar were saved together.')}</p>}
      </>}
      {error&&<><p role="alert">{t('The raster and provenance could not be saved. Check the files and destination, then retry.')}</p><InfoDetails summary={t('Technical details')} closeLabel={t('Close')} presentation="popover"><p>{t(error)}</p></InfoDetails></>}
    </Modal>}
  </>;
}
