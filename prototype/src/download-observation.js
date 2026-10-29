// In-memory UI observations only. Rates measure changes between successful
// native polls; queue age and unknown source sizes never imply network speed.
export function observeDownloadProgress(previous,jobs,at=Date.now()) {
  const next=new Map();
  for(const job of jobs){
    if((job.kind??'download')!=='download'||!['queued','running'].includes(job.status))continue;
    const bytes=Math.max(0,job.bytesDownloaded||0),old=previous.get(job.id);
    const continuing=job.status==='running'&&old?.status==='running'&&bytes>=old.bytes;
    const advanced=continuing&&bytes>old.bytes;
    const elapsed=old?at-old.at:0;
    const lastProgressAt=advanced||!continuing?at:old.lastProgressAt;
    const idleMs=job.status==='running'?Math.max(0,at-lastProgressAt):0;
    const rate=advanced&&elapsed>0?(bytes-old.bytes)*1000/elapsed:continuing&&idleMs<5000?old.rate:null;
    next.set(job.id,{status:job.status,bytes,at,lastProgressAt,idleMs,rate:job.status==='running'?rate:null});
  }
  return next;
}
export function downloadQueueCounts(jobs=[]) {
  const rows=[...new Map(jobs.filter(job=>(job.kind??'download')==='download').map(job=>[job.id,job])).values()];
  return {running:rows.filter(job=>job.status==='running').length,queued:rows.filter(job=>job.status==='queued').length};
}
export function downloadStatusLabel(job) {
  if((job.kind??'download')!=='download')return null;
  if(job.status==='queued')return 'Waiting for download slot';
  if(job.status!=='running')return null;
  if(job.totalBytes>0&&job.bytesDownloaded>=job.totalBytes)return 'Verifying download';
  if(job.bytesDownloaded>0)return 'Downloading';
  return job.totalBytes>0?'Waiting for source data':'Connecting to source';
}
