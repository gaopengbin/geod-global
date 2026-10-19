import React,{useEffect,useMemo,useRef,useState} from 'react';
import {ExternalLink,Info,Layers,Search,Trash2} from 'lucide-react';
import { InfoDetails,Button,Disclosure,Input,Select,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {ExploreSourceFrame} from './explore-source-workspace.jsx';
import {PublicDatasetExploreMap} from './public-dataset-explore-map.jsx';
import {gscloudAuth,gscloudRequest,GSCloudOriginalProducts,validateGSCloudSearch} from './gscloud-client.js';
import {GSCloudAccountDialog} from './gscloud-account.jsx';
import {GSCloudOriginalDownloads} from './gscloud-download-ui.jsx';
import {SelectionActions} from './acquisition-ui.jsx';
import {desktopAvailable} from './runtime-client.js';
import './service-explore.css';
import './gscloud.css';

const DEFAULT_PRODUCT='2c79f880a0304edebac6c48592d8e088';
export function GSCloudExplorer({areaBounds,areaGeometry,sidebarTarget,listHidden,onShowList,onRequestArea,onMutationChange,onContextChange}) {
  const {t,date,number}=useI18n();
  const [catalog,setCatalog]=useState(null),[product,setProduct]=useState(DEFAULT_PRODUCT);
  const [start,setStart]=useState('1980-01-01'),[end,setEnd]=useState(()=>new Date().toISOString().slice(0,10));
  const [presence,setPresence]=useState('all'),[cloud,setCloud]=useState(''),[filter,setFilter]=useState('');
  const [page,setPage]=useState(null),[items,setItems]=useState([]),[chosen,setChosen]=useState([]),[loaded,setLoaded]=useState([]),[focused,setFocused]=useState(null);
  const [busy,setBusy]=useState('directory'),[error,setError]=useState(''),[rowErrors,setRowErrors]=useState({});
  const [accountOpen,setAccountOpen]=useState(false),[account,setAccount]=useState(null);
  const [downloadBusy,setDownloadBusy]=useState(false);
  const generation=useRef(0),controller=useRef(null),locked=useRef(false);
  const boundsKey=areaBounds?.join(',')||'';
  const query=useMemo(()=>{try{return validateGSCloudSearch({productId:product,bounds:areaBounds,start,end,cloudMax:cloud===''?null:Number(cloud),presence,page:1,pageSize:20});}catch{return null;}},[product,boundsKey,start,end,cloud,presence]);
  const queryKey=JSON.stringify(query);
  const groups=useMemo(()=>[...new Set(catalog?.products.map(p=>p.seriesId)||[])],[catalog]);
  useEffect(()=>{const abort=new AbortController();gscloudRequest('products',{},abort.signal).then(setCatalog).catch(e=>{if(!abort.signal.aborted)setError(e.message);}).finally(()=>{if(!abort.signal.aborted)setBusy('');});return()=>abort.abort();},[]);
  useEffect(()=>{onMutationChange?.(Boolean(busy)||downloadBusy);return()=>onMutationChange?.(false);},[busy,downloadBusy,onMutationChange]);
  useEffect(()=>()=>{generation.current++;controller.current?.abort();},[]);
  useEffect(()=>{if(desktopAvailable())gscloudAuth('status').then(setAccount).catch(()=>{});},[]);
  const search=async(next=false)=>{
    if(!query||locked.current)return;locked.current=true;const epoch=generation.current,abort=new AbortController();controller.current=abort;setBusy('search');setError('');
    const request={...query,page:next?page.request.page+1:1};
    try{
      const result=await gscloudRequest('search',request,abort.signal);
      if(epoch!==generation.current||abort.signal.aborted)return;
      setPage(result);setItems(old=>next?[...new Map([...old,...result.items].map(v=>[v.id,v])).values()]:result.items);
      if(!next){setChosen([]);setLoaded([]);setFocused(null);setRowErrors({});}
    }catch(e){if(epoch===generation.current&&!abort.signal.aborted)setError(e.message);}
    finally{if(epoch===generation.current){locked.current=false;setBusy('');}}
  };
  useEffect(()=>{
    generation.current++;controller.current?.abort();locked.current=false;setItems([]);setPage(null);setChosen([]);setLoaded([]);setFocused(null);setRowErrors({});setError('');
    if(catalog)setBusy('');
    if(!catalog||!query)return;
    const timer=setTimeout(()=>search(false),300);return()=>clearTimeout(timer);
  },[queryKey,catalog]);
  const load=async()=>{
    if(locked.current||!chosen.length)return;locked.current=true;const epoch=generation.current,abort=new AbortController();controller.current=abort;setBusy('footprints');setError('');
    try{
      const results=[];
      // Bounded batches: no automatic detail request for every catalogue hit.
      for(let offset=0;offset<chosen.length;offset+=3){
        if(abort.signal.aborted)break;
        results.push(...await Promise.all(chosen.slice(offset,offset+3).map(async id=>{
          try{const result=await gscloudRequest('detail',{productId:product,itemId:id},abort.signal);if(!result.item.geometry)throw new Error('This item has no published footprint.');return {id,item:result.item};}
          catch(e){return {id,error:e.message};}
        })));
      }
      if(epoch!==generation.current||abort.signal.aborted)return;
      setLoaded(results.filter(r=>r.item).map(r=>r.item));setRowErrors(Object.fromEntries(results.filter(r=>r.error).map(r=>[r.id,r.error])));
      setItems(old=>old.map(v=>results.find(r=>r.id===v.id)?.item||v));
    }finally{if(epoch===generation.current){locked.current=false;setBusy('');}}
  };
  const visible=items.filter(v=>!filter||v.id.toLowerCase().includes(filter.toLowerCase()));
  const toggle=id=>setChosen(old=>old.includes(id)?old.filter(v=>v!==id):[...old,id]);
  const selectVisible=()=>setChosen(old=>visible.every(v=>old.includes(v.id))?old.filter(id=>!visible.some(v=>v.id===id)):[...new Set([...old,...visible.map(v=>v.id)])].slice(0,20));
  const selectedProduct=catalog?.products.find(v=>v.id===product);
  useEffect(()=>{
    const ids=new Set(items.map(v=>v.id)),selected=chosen.filter(id=>ids.has(id)),shown=loaded.filter(v=>ids.has(v.id));
    onContextChange?.(query?{request:query,catalog:{state:busy==='search'?'loading':error?'error':page?'ready':'idle',totalCount:items.length,filteredCount:visible.length,
      selectedCount:selected.length,loadedCount:shown.length,visibleLoadedCount:shown.length,selectedItemIds:selected,selectionTruncated:false,visibleItemIds:shown.slice(0,16).map(v=>v.id),focusedItemId:focused,
      complete:Boolean(page&&!page.hasMore&&items.length>=page.total),pages:page?.request.page||0,retrievedAt:page?.retrievedAt||null,includesSavedProjectScenes:false,activeDate:null,textFilter:filter.slice(0,200),sort:'date',dateCounts:[],dateCountsTruncated:false}}:null);
  },[onContextChange,queryKey,items,chosen,loaded,focused,filter,busy,error,page]);
  return <ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} title="Geospatial Data Cloud" areaBounds={areaBounds} areaGeometry={areaGeometry} onShowList={onShowList} onRequestArea={onRequestArea}
    map={<PublicDatasetExploreMap items={loaded} selectedIds={chosen} focusedId={focused} onFocusItem={setFocused} areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} areaDisabled={Boolean(busy)}/>}
    actions={<><SelectionActions count={chosen.length} onClear={()=>setChosen([])}
      preview={<Button variant="secondary" icon={Layers} disabled={Boolean(busy)||!chosen.length} onClick={load}>{t('Preview selected')}</Button>}
      download={GSCloudOriginalProducts[product]?<GSCloudOriginalDownloads key={product} productId={product} items={items} selectedIds={chosen} account={account} onBusyChange={setDownloadBusy}/>:<Button variant="primary" disabled>{t('Save and download')}</Button>}
      details={<InfoDetails closeLabel={t('Close')} summary={t('Preview details')}><p>{t('GSCloud footprints are metadata, not an imagery preview or download.')}</p><p>{t('Public metadata is searchable here. Desktop original ZIP downloads support SRTM90, slope, TPI and ASTER GDEM V1/V2/V3 with your own GSCloud account. Other products and reservations require separate delivery verification.')}</p></InfoDetails>}>
      {desktopAvailable()&&account?.status!=='connected'&&GSCloudOriginalProducts[product]&&<Button size="sm" onClick={()=>setAccountOpen(true)}>{t('Connect GSCloud account')}</Button>}
      {loaded.length>0&&<Button variant="quiet" size="xs" icon={Trash2} disabled={Boolean(busy)} onClick={()=>setLoaded([])}>{t('Clear map previews')}</Button>}
    </SelectionActions></>}>
    <div className="service-explore-controls gscloud-controls">
      <label className="stac-field"><span>{t('GSCloud product')}</span><Select aria-label={t('GSCloud product')} value={product} disabled={!catalog||Boolean(busy)||downloadBusy} onChange={e=>setProduct(e.target.value)}>
        {groups.map(group=><optgroup label={group} key={group}>{catalog.products.filter(p=>p.seriesId===group).map(p=><option key={p.id} value={p.id}>{p.title}</option>)}</optgroup>)}
      </Select></label>
      <div className="gscloud-dates"><label className="stac-field"><span>{t('Start date')}</span><Input type="date" aria-label={t('GSCloud start date')} value={start} onChange={e=>setStart(e.target.value)}/></label><label className="stac-field"><span>{t('End date')}</span><Input type="date" aria-label={t('GSCloud end date')} value={end} onChange={e=>setEnd(e.target.value)}/></label></div>
      <div className="gscloud-dates"><label className="stac-field"><span>{t('Data presence')}</span><Select aria-label={t('GSCloud data presence')} value={presence} onChange={e=>setPresence(e.target.value)}><option value="all">{t('All')}</option><option value="has">{t('File listed')}</option><option value="none">{t('Reservation required')}</option></Select></label><label className="stac-field"><span>{t('Cloud max (%)')}</span><Input type="number" min="0" max="100" placeholder={t('Any')} aria-label={t('GSCloud cloud max')} value={cloud} onChange={e=>setCloud(e.target.value)}/></label></div>
      <Button icon={Search} disabled={Boolean(busy)||!query||!catalog} onClick={()=>search(false)}>{t('Search')}</Button>
      {!query&&<p className="stac-help">{t('Choose a valid area and date range to search GSCloud.')}</p>}
      <InfoDetails closeLabel={t('Close')} summary={t('Source and download access')}><p className="stac-help">{t('Public metadata is searchable here. Desktop original ZIP downloads support SRTM90, slope, TPI and ASTER GDEM V1/V2/V3 with your own GSCloud account. Other products and reservations require separate delivery verification.')}</p>
        {desktopAvailable()?<><p className="stac-help">{t(account?.status==='connected'?'GSCloud signed in · download access depends on the selected product':account?.status==='saved'?'GSCloud session saved · verification required':'GSCloud not connected')}</p><Button size="sm" onClick={()=>setAccountOpen(true)}>{t('Connect GSCloud account')}</Button>{account&&account.status!=='disconnected'&&<Button size="sm" onClick={()=>gscloudAuth('logout').then(setAccount).catch(e=>setError(e.message))}>{t('Disconnect account')}</Button>}</>:<p className="stac-help">{t('Connect GSCloud accounts in the desktop app. Public search remains available here.')}</p>}
        <a href="https://www.gscloud.cn/accounts/login_user" target="_blank" rel="noreferrer">{t('Sign in on GSCloud')} <ExternalLink size={12}/></a></InfoDetails>
    </div>
    <Input type="search" aria-label={t('Filter GSCloud results')} placeholder={t('Filter results')} value={filter} onChange={e=>setFilter(e.target.value)}/>
    <div className="service-result-heading"><span>{page?t('{loaded} of {total} results',{loaded:items.length,total:number(page.total)}):t('Search results')}</span><Button variant="quiet" size="xs" disabled={Boolean(busy)||!visible.length} onClick={selectVisible}>{t('Select current results')}</Button></div>
    <div className="service-result-list" aria-label={t('GSCloud results')}>{visible.map(v=><div key={v.id} className="service-result gscloud-result" data-selected={chosen.includes(v.id)}>
      <label><Input type="checkbox" checked={chosen.includes(v.id)} disabled={Boolean(busy)||!chosen.includes(v.id)&&chosen.length>=20} aria-label={t('Select {name}',{name:v.id})} onChange={()=>toggle(v.id)}/><span title={v.id}>{v.id}</span></label>
      <small className="service-result-meta">{v.datetime?date(v.datetime):t('Observation date unavailable')} · {v.cloudCover==null?t('Cloud unknown'):`${v.cloudCover}%`} · {t(v.presence==='available'?'File listed · sign-in required':v.presence==='reservation'?'Reservation required':'Availability unknown')}{loaded.some(i=>i.id===v.id)?` · ${t('Footprint shown')}`:''}</small>
      <InfoDetails closeLabel={t('Close')} icon={Info} summary={t('Item details')}><p>{selectedProduct?.title}</p><p>{v.id}</p><p>{v.bytes?t('Published file size: {size} MiB',{size:number(v.bytes/1048576,{maximumFractionDigits:1})}):t('File size not published')}</p><p>{t(GSCloudOriginalProducts[product]?'Select an available tile and use Download selected originals. Searching or showing footprints does not start a download.':'Original delivery requires separate product verification. No file has been queued.')}</p>{v.bbox&&<p>{v.bbox.join(', ')}</p>}</InfoDetails>
      {rowErrors[v.id]&&<small role="alert">{t(rowErrors[v.id])}</small>}
    </div>)}</div>
    {page&&!items.length&&!busy&&<p className="stac-help">{t('No product items match this search.')}</p>}
    {page?.hasMore&&<Button disabled={Boolean(busy)||items.length>=200} onClick={()=>search(true)}>{t('Load more items')}</Button>}
    {items.length>=200&&page?.hasMore&&<p className="stac-help">{t('Narrow the area or dates to see further GSCloud results.')}</p>}
    {page&&!page.hasMore&&items.length<page.total&&<p className="stac-help" role="status">{t('GSCloud pages changed while loading. These results are incomplete; search again or narrow the date range.')}</p>}
    {busy&&<p className="stac-status" role="status"><Spinner size={14}/>{t(busy==='directory'?'Reading GSCloud products…':busy==='search'?'Searching GSCloud…':'Reading published footprints…')}</p>}
    {error&&<p className="stac-error" role="alert">{t(error)} <Button size="xs" disabled={Boolean(busy)} onClick={()=>catalog?search(false):gscloudRequest('products').then(setCatalog).catch(e=>setError(e.message))}>{t('Retry')}</Button></p>}
    {accountOpen&&<GSCloudAccountDialog onClose={()=>setAccountOpen(false)} onConnected={setAccount}/>}
  </ExploreSourceFrame>;
}
