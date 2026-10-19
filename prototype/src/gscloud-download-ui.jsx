import React,{useEffect,useRef,useState} from 'react';
import {Download} from 'lucide-react';
import { InfoDetails,Button,Spinner} from './ui/index.jsx';
import {DeliveryReceipt,DownloadReview} from './acquisition-ui.jsx';
import {useI18n} from './i18n.jsx';
import {desktopAvailable,runtimeRequest} from './runtime-client.js';
import {downloadGSCloudOriginal,GSCloudOriginalProducts,validateGSCloudOriginalJob} from './gscloud-client.js';

export function GSCloudOriginalDownloads({productId='305',items,selectedIds,account,onBusyChange}) {
  const {t,number}=useI18n();const [jobs,setJobs]=useState([]),[busy,setBusy]=useState(''),[error,setError]=useState('');
  const [review,setReview]=useState(false),[submittedIds,setSubmittedIds]=useState([]);
  const alive=useRef(true),locked=useRef(false),revision=useRef(0);const native=desktopAvailable();
  const product=GSCloudOriginalProducts[productId];
  const refresh=async()=>{
    const epoch=++revision.current;const all=await runtimeRequest('list');
    const rows=all.filter(j=>j.assetKey===product?.key).map(validateGSCloudOriginalJob);
    if(alive.current&&epoch===revision.current)setJobs(rows);
    return rows;
  };
  useEffect(()=>{alive.current=true;setJobs([]);setError('');let timer;const poll=async()=>{let rows=[];try{rows=await refresh();}catch(e){if(alive.current)setError(e.message);}if(alive.current)timer=setTimeout(poll,rows.some(j=>['queued','running'].includes(j.status))?1500:8000);};if(native)poll();return()=>{alive.current=false;revision.current++;clearTimeout(timer);};},[native,productId]);
  useEffect(()=>{onBusyChange?.(Boolean(busy));return()=>onBusyChange?.(false);},[busy,onBusyChange]);
  const run=async(id,fn)=>{if(locked.current)return;locked.current=true;setBusy(id);setError('');try{await fn();await refresh();}catch(e){if(alive.current)setError(e.message);}finally{locked.current=false;if(alive.current)setBusy('');}};
  const selected=items.filter(i=>selectedIds.includes(i.id));
  const currentIds=submittedIds.filter(id=>jobs.some(job=>job.id===id&&selectedIds.includes(job.itemId)));
  const download=()=>run('create',async()=>{
    if(!product||!selected.length||selected.length!==selectedIds.length||selected.length>4||selected.some(i=>i.presence!=='available'||i.productId&&i.productId!==productId))throw Error('Select up to 4 available originals of the current product. Reservation is not submitted automatically.');
    for(const item of selected){
      // Keep successful submissions if a later item fails. Retry only missing
      // items so a partial submission cannot create a duplicate download.
      if(jobs.some(job=>submittedIds.includes(job.id)&&job.itemId===item.id&&['queued','running','succeeded'].includes(job.status)))continue;
      const job=await downloadGSCloudOriginal({productId,itemId:item.id});
      if(alive.current){setSubmittedIds(old=>[...new Set([...old,job.id])]);setJobs(old=>[...old.filter(value=>value.id!==job.id),job]);}
    }
  });
  return <section aria-label={t('GSCloud original downloads')}>
    <Button variant="primary" icon={Download} disabled={!native||Boolean(busy)||!selected.length||selected.length>4||selected.some(item=>item.presence!=='available')||!account||account.status==='disconnected'} onClick={()=>setReview(true)}>{t('Save and download')}</Button>
    {currentIds.length>0&&<DeliveryReceipt jobIds={currentIds} jobs={jobs} submitted compact/>}
    {review&&<DownloadReview count={selected.length} onClose={()=>setReview(false)} onConfirm={download} busy={Boolean(busy)} error={error} receipt={selected.length>0&&selected.every(item=>jobs.some(job=>submittedIds.includes(job.id)&&job.itemId===item.id))?<DeliveryReceipt jobIds={currentIds} jobs={jobs} submitted/>:null}>
      <p>{t('Complete original ZIP · {format}. All archive members are retained and checked. Open the downloaded folder to use them; archive raster preview is unavailable. Retry restarts the whole file.',{format:product?.format||''})}</p>
      <ul className="download-review-files">{selected.map(item=><li key={item.id}>{item.id}{item.bytes?` · ${number(item.bytes/1048576,{maximumFractionDigits:1})} MiB`:''}</li>)}</ul>
      <InfoDetails closeLabel={t('Close')} summary={t('Original ZIP verification')}><p>{t('Whole original ZIP, selected product and tile, member format signatures, bytes and ZIP CRC verified. Archive pixels are not decoded by GeoD.')}</p></InfoDetails>
    </DownloadReview>}
    {error&&<p role="alert" className="stac-error">{t(error)}</p>}
  </section>;
}
