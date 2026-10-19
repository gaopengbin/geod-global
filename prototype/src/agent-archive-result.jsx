import React,{useEffect,useState} from 'react';
import {FileArchive,FileText,FolderOpen,RefreshCw,ShieldCheck} from 'lucide-react';
import { InfoDetails,Button,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {formatBytes,runtimeRequest} from './runtime-client.js';
import {inspectGSCloudOriginal} from './gscloud-client.js';

export default function AgentArchiveResult({id}) {
  const {t,locale}=useI18n();
  const [data,setData]=useState(null),[error,setError]=useState(''),[revision,setRevision]=useState(0),[busy,setBusy]=useState(false);
  useEffect(()=>{let alive=true;setData(null);setError('');inspectGSCloudOriginal(id).then(value=>{if(alive)setData(value);}).catch(error=>{if(alive)setError(String(error.message||error));});return()=>{alive=false;};},[id,revision]);
  const reveal=async()=>{setBusy(true);setError('');try{await runtimeRequest('reveal',{id});}catch(e){setError(String(e.message||e));}finally{setBusy(false);}};
  return <section className="agent-archive-result" aria-label={t('Verified original archive')}>
    {!data&&!error&&<p role="status"><Spinner size={16}/> {t('Verifying original archive…')}</p>}
    {data&&<>
      <div className="agent-result-title"><FileArchive size={22}/><h2>{data.itemId}.zip</h2><ShieldCheck size={18}/></div>
      <p>{t('Geospatial Data Cloud / 地理空间数据云')} · {formatBytes(data.bytes,locale)} · {t('Original ZIP')}</p>
      <p>{t('Whole original ZIP, selected product and tile, member format signatures, bytes and ZIP CRC verified. Archive pixels are not decoded by GeoD.')}</p>
      <h3>{t('Archive files')}</h3>
      <ul className="agent-archive-members">{data.members.map(member=><li key={member.name}><FileText size={16}/><span>{member.name}</span><small>{formatBytes(member.bytes,locale)}</small></li>)}</ul>
      <InfoDetails closeLabel={t('Close')} summary={t('Original ZIP verification')}><p className="agent-archive-hash">SHA-256: {data.sha256}</p><p>{t('Product')} {data.productId} · {t('CRC verified')}</p></InfoDetails>
      <p>{t('Archive map preview is unavailable. Open the downloaded folder to use the original files.')}</p>
      <Button variant="secondary" icon={FolderOpen} disabled={busy} onClick={reveal}>{t('Open folder')}</Button>
    </>}
    {error&&<><p role="alert" className="agent-error">{t(error)}</p><Button variant="secondary" icon={RefreshCw} onClick={()=>setRevision(value=>value+1)}>{t('Retry verification')}</Button></>}
  </section>;
}
