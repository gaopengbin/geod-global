import {DownloadReview,SelectionActions,SavedResultsReceipt} from './acquisition-ui.jsx';
import React,{useEffect,useRef,useState} from 'react';
import {Download,ExternalLink,Info,Layers,RefreshCw,FolderOpen} from 'lucide-react';
import { InfoDetails,Button,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {ExploreSourceFrame} from './explore-source-workspace.jsx';
import {ServiceExploreMap} from './service-explore-map.jsx';
import {validQueryBounds} from './features-client.js';
import {vectorRequest} from './vector-client.js';
import {overtureRequest,exportOvertureOriginal,OVERTURE_PACKAGE_ID,OVERTURE_SELECTION} from './overture-client.js';
import './features.css';import './service-explore.css';
import './overture-explore.css';

export function OvertureExplorer(props) {
  const {t,number}=useI18n();const [result,setResult]=useState(null),[asset,setAsset]=useState(null),[entries,setEntries]=useState([]),[rows,setRows]=useState(null),[busy,setBusy]=useState(''),[error,setError]=useState('');
  const [review,setReview]=useState(false);
  const request=useRef(null),generation=useRef(0),mutation=useRef(props.onMutationChange);mutation.current=props.onMutationChange;
  const work=async(label,fn)=>{request.current?.abort();const abort=new AbortController();request.current=abort;const token=++generation.current;setBusy(label);setError('');mutation.current?.(['download','export'].includes(label));try{await fn(abort.signal,()=>!abort.signal.aborted&&token===generation.current);}catch(e){if(!abort.signal.aborted&&token===generation.current)setError(e instanceof Error?e.message:String(e));}finally{if(!abort.signal.aborted&&token===generation.current){setBusy('');mutation.current?.(false);}}};
  const search=()=>work('search',async(signal,current)=>{setResult(null);setAsset(null);setEntries([]);setRows(null);const found=await overtureRequest('search',{datasetId:'overture-maps',bounds:props.areaBounds},signal);if(!current())return;setResult(found);const saved=await vectorRequest('list',{},signal);if(current())setAsset(saved.find(a=>a.geoParquet?.packageId===OVERTURE_PACKAGE_ID&&a.geoParquet.requestedBounds.every((n,i)=>n===props.areaBounds[i]))||null);});
  useEffect(()=>{generation.current++;request.current?.abort();setReview(false);setEntries([]);setAsset(null);setRows(null);setResult(null);setError('');mutation.current?.(false);if(validQueryBounds(props.areaBounds))search();return()=>{generation.current++;request.current?.abort();mutation.current?.(false);};},[props.areaBounds?.join(',')]);
  const show=async(saved,signal,current)=>{const inspected=await vectorRequest('inspect',{id:saved.id},signal);if(current()){setAsset(inspected.asset);setEntries([{key:saved.id,title:t('Overture bathymetry · area preview'),kind:'vector',data:inspected,visible:true,opacity:1}]);setRows(null);}};
  const download=()=>work('download',async(signal,current)=>{const saved=await overtureRequest('download',{datasetId:'overture-maps',packageId:OVERTURE_PACKAGE_ID,bounds:props.areaBounds},signal);await show(saved,signal,current);});
  const propertyPage=()=>work('rows',async(signal,current)=>{const inspected=await vectorRequest('inspect',{id:asset.id},signal);const first=inspected.geojson.features[0]?.geodOriginalRow??0;const page=await overtureRequest('rows',{id:asset.id,offset:first,limit:5},signal);if(current())setRows(page);});
  return <ExploreSourceFrame {...props} title="Overture Maps" actions={<SelectionActions count={result?1:0}
    preview={<Button variant="secondary" icon={Layers} disabled={!!busy||!asset} onClick={()=>work('reopen',(signal,current)=>show(asset,signal,current))}>{t('Preview selected')}</Button>}
    download={<Button primary icon={Download} disabled={!!busy||!result} onClick={()=>setReview(true)}>{t('Save and download')}</Button>}
    details={<InfoDetails closeLabel={t('Close')} summary={t('Files and area')}><p>{t('The complete GeoParquet original stays separate from the selected area preview.')}</p></InfoDetails>}>
    {asset&&<SavedResultsReceipt entries={[{id:asset.id,title:asset.name}]} hrefFor={entry=>'#Workspace?vector='+entry.id}/>}
  </SelectionActions>} map={<ServiceExploreMap {...props} entries={entries} onPatch={(key,patch)=>setEntries(old=>old.map(e=>e.key===key?{...e,...patch}:e))} onRemove={key=>setEntries(old=>old.filter(e=>e.key!==key))}/>}> 
    {review&&<DownloadReview count={result?1:0} onClose={()=>setReview(false)} onConfirm={download} busy={busy==='download'} error={error}
      receipt={asset&&entries.length?<SavedResultsReceipt entries={[{id:asset.id,title:asset.name}]} hrefFor={entry=>'#Workspace?vector='+entry.id}/>:null}><p>{t('Overture · base bathymetry')}</p><p>{t('The complete GeoParquet original stays separate from the selected area preview.')}</p></DownloadReview>}
    <div className="service-explore-controls"><div className="feature-service-row"><strong>{t('Overture · base bathymetry')}</strong><Button size="icon" variant="quiet" aria-label={t('Refresh official packages')} disabled={!!busy||!validQueryBounds(props.areaBounds)} onClick={search}><RefreshCw size={16}/></Button></div>
      {!validQueryBounds(props.areaBounds)&&<p className="feature-help">{t('Select a query region on Explore first.')}</p>}
      {busy&&<p className="feature-loading" role="status"><Spinner size={15}/>{t(busy==='download'?'Downloading original and reading selected features…':busy==='export'?'Exporting complete GeoParquet original…':busy==='rows'?'Reading original properties…':'Reading reviewed publisher packages…')}</p>}
      {result&&<article className="overture-original-result"><strong>{t('Complete GeoParquet original')} · 2026-09-23.1</strong><small className="service-result-meta">52.02 MB · {number(407277)} {t('Original rows')} · WKB</small>
        <p className="feature-help">{t('Underwater polygons from ETOPO/GLOBathy. Depth is below surface in metres; this is a thematic representation, not a new seabed survey.')}</p>
        {asset&&<><p className="feature-help" data-testid="overture-preview-count">{t('Area preview')} · {number(asset.featureCount)} {t('Features')} · {number(asset.bytes)} {t('bytes')}</p><div className="overture-original-actions"><Button icon={FolderOpen} disabled={!!busy} onClick={()=>work('reopen',(signal,current)=>show(asset,signal,current))}>{t('Reopen saved preview')}</Button><Button icon={Download} disabled={!!busy} onClick={()=>work('export',()=>exportOvertureOriginal(asset))}>{t('Export complete original')}</Button><Button icon={Info} disabled={!!busy} onClick={propertyPage}>{t('Read original properties')}</Button></div></>}
        <InfoDetails closeLabel={t('Close')} icon={Info} summary={t('Source details')}><p className="feature-help">{t(OVERTURE_SELECTION)}</p><p className="feature-help">{t('Original container: up to 512 MiB. Each decoded row group: up to 32 MiB. Area preview: up to 20 MiB, 50,000 features and 500,000 coordinates. Larger previews are refused.')}</p><p className="feature-help">{t('Only this pinned base/bathymetry part is connected. Buildings, places and transport remain outside this entry.')}</p><a href="https://docs.overturemaps.org/attribution/" target="_blank" rel="noreferrer">{t('Publisher terms of use')}<ExternalLink size={13}/></a>
          {asset&&<><p className="feature-help" style={{overflowWrap:'anywhere'}}>{t('Complete original SHA-256')} · {asset.geoParquet.originalSha256}</p><p className="feature-help" style={{overflowWrap:'anywhere'}}>{t('Area preview SHA-256')} · {asset.sourceSha256}</p></>}
        </InfoDetails>
      </article>}
      {rows&&<InfoDetails closeLabel={t('Close')} summary={t('Original properties')}><p className="feature-help">{t('Original row offset')} · {number(rows.offset)} / {number(rows.originalRows)}</p>{rows.rows.map(row=><article className="overture-original-row" key={row.originalRow}><strong>{number(row.originalRow)} · {row.properties.id}</strong><pre className="feature-help">{JSON.stringify(row.properties,null,2)}</pre></article>)}{rows.nextOffset!==null&&<Button disabled={!!busy} onClick={()=>work('rows',async(signal,current)=>{const next=await overtureRequest('rows',{id:asset.id,offset:rows.nextOffset,limit:5},signal);if(current())setRows(next);})}>{t('Next original rows')}</Button>}</InfoDetails>}
      <p className="feature-help">{t('Saved previews reopen in My Data → Vector files. Export complete original preserves the Parquet bytes; export GeoJSON is the area preview.')}</p>
      {error&&<p className="feature-error" role="alert">{t(error)}</p>}
    </div>
  </ExploreSourceFrame>;
}
