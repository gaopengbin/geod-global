import {SelectionActions} from './acquisition-ui.jsx';
import React, {useEffect,useRef,useState} from 'react';
import {Download,ExternalLink,Info,Layers,RefreshCw,Trash2} from 'lucide-react';
import { InfoDetails,Button,Input,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {ExploreSourceFrame} from './explore-source-workspace.jsx';
import {ServiceExploreMap} from './service-explore-map.jsx';
import {validQueryBounds} from './features-client.js';
import {fetchWaybackCatalog,fetchWaybackPointMetadata,waybackReference,waybackOfficialUrl,WAYBACK_EXPORT_NOTE,WAYBACK_TERMS_URL} from './wayback-client.js';
import './features.css';
import './service-explore.css';

export function downloadWaybackReference(reference) {
  const blob=new Blob([JSON.stringify(reference,null,2)],{type:'application/json'}),url=URL.createObjectURL(blob),link=document.createElement('a');
  link.href=url;link.download=`geod-${reference.version.id}-reference.json`;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}

export function WaybackExplorer({areaBounds,areaGeometry,sidebarTarget,listHidden,onShowList,onRequestArea,onMutationChange}) {
  const {t,number}=useI18n();
  const [catalog,setCatalog]=useState(null),[loading,setLoading]=useState(true),[error,setError]=useState(''),[search,setSearch]=useState(''),[checked,setChecked]=useState([]),[entries,setEntries]=useState([]),[metadata,setMetadata]=useState({}),[metadataBusy,setMetadataBusy]=useState(false),[revision,setRevision]=useState(0);
  const requests=useRef(null),metadataSequence=useRef(0),mutation=useRef(onMutationChange);mutation.current=onMutationChange;
  const boundsKey=areaBounds?.join(',')||'',geometryKey=JSON.stringify(areaGeometry||null),regionValid=validQueryBounds(areaBounds);
  useEffect(()=>{const abort=new AbortController();setLoading(true);setError('');fetchWaybackCatalog(abort.signal).then(value=>{if(!abort.signal.aborted){setCatalog(value);setChecked([]);setEntries([]);setMetadata({});}}).catch(reason=>{if(!abort.signal.aborted){setCatalog(null);setError(reason.message);}}).finally(()=>{if(!abort.signal.aborted)setLoading(false);});return()=>abort.abort();},[revision]);
  useEffect(()=>{metadataSequence.current++;requests.current?.abort();setMetadataBusy(false);setEntries([]);setMetadata({});setError('');mutation.current?.(false);return()=>{metadataSequence.current++;requests.current?.abort();mutation.current?.(false);};},[boundsKey,geometryKey]);
  const versions=catalog?.versions||[],filtered=versions.filter(version=>`${version.title} ${version.layerIdentifier}`.toLowerCase().includes(search.trim().toLowerCase())),shown=filtered.slice(0,100);
  const toggle=id=>setChecked(old=>old.includes(id)?old.filter(value=>value!==id):old.length<4?[...old,id]:old);
  const selected=versions.filter(version=>checked.includes(version.id));
  const preview=()=>{metadataSequence.current++;requests.current?.abort();setMetadataBusy(false);setMetadata({});setError('');mutation.current?.(false);setEntries(selected.map(version=>({key:version.id,title:version.title,kind:'wayback',data:{version},visible:true,opacity:1})));};
  const inspectPoint=async(point,zoom,visibleVersions)=>{
    if(zoom<10||zoom>23){setError(t('Zoom to level 10 or closer to inspect imagery acquisition metadata.'));return;}
    const token=++metadataSequence.current;requests.current?.abort();const abort=new AbortController();requests.current=abort;setMetadataBusy(true);setError('');setMetadata({});mutation.current?.(true);
    try{
      const rows=await Promise.all(visibleVersions.map(async version=>({version,result:await fetchWaybackPointMetadata(version,point,zoom,abort.signal)})));
      if(!abort.signal.aborted&&metadataSequence.current===token)setMetadata(Object.fromEntries(rows.map(row=>[row.version.id,row.result])));
    }catch(reason){if(!abort.signal.aborted&&metadataSequence.current===token)setError(reason.message);}finally{if(!abort.signal.aborted&&metadataSequence.current===token){setMetadataBusy(false);mutation.current?.(false);}}
  };
  const save=version=>{try{downloadWaybackReference(waybackReference(version,areaBounds,catalog,metadata[version.id]||null));}catch(reason){setError(reason.message);}};
  const actions=<SelectionActions count={selected.length} onClear={()=>setChecked([])}
    preview={<Button primary icon={Layers} disabled={loading||!regionValid||!selected.length} onClick={preview}>{t('Preview selected')}</Button>}
    details={<InfoDetails closeLabel={t('Close')} summary={t('Preview details')}><p>{t('Online rendered basemap. Publication dates identify archive versions, not imagery acquisition dates.')}</p></InfoDetails>}>
    {entries.length>0&&<Button variant="quiet" size="xs" disabled={metadataBusy} onClick={()=>setEntries([])}>{t('Clear map previews')}</Button>}
  </SelectionActions>;

  return <ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} title="Wayback" areaBounds={areaBounds} areaGeometry={areaGeometry} actions={actions} map={<ServiceExploreMap entries={entries} areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} onInspectWaybackPoint={inspectPoint} onPatch={(key,patch)=>setEntries(old=>old.map(entry=>entry.key===key?{...entry,...patch}:entry))} onRemove={key=>setEntries(old=>old.filter(entry=>entry.key!==key))}/> }>
    <div className="service-explore-controls">
      <div className="feature-service-row"><strong>{t('Wayback archive versions')}</strong><Button size="icon" variant="quiet" aria-label={t('Refresh Wayback directory')} disabled={loading||metadataBusy} onClick={()=>setRevision(value=>value+1)}><RefreshCw size={16}/></Button></div>
      <InfoDetails closeLabel={t('Close')} summary={t('Source details')}><p className="feature-help">{t('Esri World Imagery Wayback · rendered basemap archive')}</p><p className="feature-help">{t(WAYBACK_EXPORT_NOTE)}</p><a href={WAYBACK_TERMS_URL}>{t('Publisher terms of use')}</a></InfoDetails>
      {!regionValid&&<p className="feature-help"><Info size={14}/>{t('Select a query region on Explore first.')}</p>}
      {loading?<div className="feature-loading" role="status"><Spinner/>{t('Reading Wayback version directory')}</div>:catalog&&<>
        <Input type="search" aria-label={t('Filter results')} placeholder={t('Filter results')} value={search} onChange={event=>setSearch(event.target.value)}/>
        <div className="service-result-heading"><span>{t('{count} results',{count:number(filtered.length)})}</span><Button variant="quiet" size="sm" disabled={!shown.length} onClick={()=>setChecked(shown.slice(0,4).map(version=>version.id))}>{t('Select visible results')}</Button></div>
        <div className="service-result-list" aria-label={t('Search results')}>{shown.map(version=>{
          const point=metadata[version.id];
          return <article className="service-result" key={version.id} data-selected={checked.includes(version.id)}><label><Input type="checkbox" aria-label={t('Select result {name}',{name:version.title})} checked={checked.includes(version.id)} disabled={!checked.includes(version.id)&&checked.length>=4} onChange={()=>toggle(version.id)}/><span><strong>{version.title}</strong></span></label>
            <small className="service-result-meta" title={t('Rendered imagery · acquisition metadata varies by point and zoom')}>{t('Basemap publication date')} · {version.publicationDate}</small>
            {point&&<InfoDetails closeLabel={t('Close')} icon={Info} summary={t('Point imagery metadata')}><p className="feature-help">{t('Queried point')} · {point.point.map(value=>value.toFixed(5)).join(', ')} · {t('Level')} {point.zoom}</p>{point.records.length?point.records.map((record,index)=><dl className="feature-help" key={index}><dt>{t('Acquisition date')}</dt><dd>{record.acquisitionDate?.slice(0,10)||t('Not provided by publisher')}</dd><dt>{t('Imagery source')}</dt><dd>{[record.provider,record.source].filter(Boolean).join(' · ')||t('Not provided by publisher')}</dd><dt>{t('Resolution / accuracy (m)')}</dt><dd>{String(record.resolutionMeters??'—')} / {String(record.accuracyMeters??'—')}</dd></dl>):<p className="feature-help">{t('No imagery acquisition record was returned for this point and level.')}</p>}</InfoDetails>}
            <div className="service-result-actions"><Button size="icon" variant="quiet" icon={Download} aria-label={t('Save reference {name}',{name:version.title})} tooltip={t('Save version reference')} disabled={!regionValid||metadataBusy} onClick={()=>save(version)}/><Button asChild size="icon" variant="quiet" tooltip={t('Publisher item')}><a href={version.itemUrl} aria-label={t('Publisher item')}><ExternalLink size={14}/></a></Button>{regionValid&&<Button asChild size="icon" variant="quiet" tooltip={t('Authorized export in ArcGIS')}><a href={waybackOfficialUrl(version,areaBounds)} aria-label={t('Authorized export in ArcGIS')}><ExternalLink size={14}/></a></Button>}</div>
          </article>;
        })}</div>
        {filtered.length>100&&<p className="feature-help">{t('Showing the first 100 versions. Filter by publication year or date to find older versions.')}</p>}
        <p className="feature-help">{t('Preview up to 4 versions. Click the map at level 10 or closer to read source acquisition metadata.')}</p>
        <p className="feature-help">{t('Saved references contain the region, version directory and any queried metadata; they contain no imagery. Offline tile export is handled by the official ArcGIS application.')}</p>
      </>}
      {metadataBusy&&<p role="status" className="feature-help"><Spinner size={14}/>{t('Reading point imagery metadata')}</p>}
      {error&&<p role="alert" className="feature-error">{t(error)}</p>}
    </div>
  </ExploreSourceFrame>;
}
