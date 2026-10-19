import {DownloadReview,SelectionActions,SavedResultsReceipt} from './acquisition-ui.jsx';
import React,{useCallback,useEffect,useMemo,useRef,useState} from 'react';
import {Download,Layers,RefreshCw} from 'lucide-react';
import { InfoDetails,Button,Disclosure,Input,Select,Spinner,Surface} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {ExploreSourceFrame} from './explore-source-workspace.jsx';
import {ServiceExploreMap} from './service-explore-map.jsx';
import {genericMapMetadata} from './stac-map-model.js';
import {gebcoRequest,saveGebcoFile,validateGebcoRequest} from './gebco-client.js';
import {PublicDatasetExplorer} from './public-dataset-ui.jsx';
import './earth-engine.css';

export function GebcoExplorer(props) {
  const {t}=useI18n();
  const [product,setProduct]=useState('region'),[busy,setBusy]=useState(false);
  const mutation=useCallback(value=>{setBusy(Boolean(value));props.onMutationChange?.(value);},[props.onMutationChange]);
  if(props.libraryOnly)return <GebcoRegionExplorer {...props}/>;
  const header=<label className="stac-field"><span>{t('GEBCO product')}</span><Select aria-label={t('GEBCO product')} value={product} disabled={busy} onChange={e=>setProduct(e.target.value)}>
    <option value="region">{t('Regional elevation and water depth')}</option>
    <option value="tid">{t('Global source-type grid · original tiles')}</option>
  </Select></label>;
  return product==='tid'?<PublicDatasetExplorer {...props} datasetId="gebco-tid" autoSearch workflowHeader={header} onMutationChange={mutation}/>
    :<GebcoRegionExplorer {...props} workflowHeader={header} onMutationChange={mutation}/>;
}

function GebcoRegionExplorer({areaBounds:suppliedBounds,areaGeometry:suppliedGeometry,sidebarTarget,listHidden,onShowList,onRequestArea:suppliedRequestArea,onMutationChange,libraryOnly=false,workflowHeader}) {
  const areaBounds=libraryOnly?undefined:suppliedBounds,areaGeometry=libraryOnly?undefined:suppliedGeometry,onRequestArea=libraryOnly?undefined:suppliedRequestArea;
  const {t,number}=useI18n();
  const [name,setName]=useState('GEBCO 2026'),[catalog,setCatalog]=useState(null),[records,setRecords]=useState([]);
  const [busy,setBusy]=useState(false),[loading,setLoading]=useState(true),[error,setError]=useState('');
  const [chosen,setChosen]=useState([]),[entries,setEntries]=useState([]),[rowErrors,setRowErrors]=useState({});
  const [selected,setSelected]=useState(null),[raster,setRaster]=useState(null),[column,setColumn]=useState('0'),[row,setRow]=useState('0'),[sample,setSample]=useState(null);
  const [review,setReview]=useState(false);
  const alive=useRef(true),epoch=useRef(0),controller=useRef(null),locked=useRef(false);
  const boundsKey=areaBounds?.join(',')||'';
  useEffect(()=>{alive.current=true;const abort=new AbortController();Promise.all([libraryOnly?Promise.resolve(null):gebcoRequest('grids',{},abort.signal),gebcoRequest('list',{},abort.signal)]).then(([g,r])=>{if(alive.current){setCatalog(g);setRecords(r);}}).catch(e=>{if(alive.current&&!abort.signal.aborted)setError(e.message);}).finally(()=>{if(alive.current)setLoading(false);});return()=>{alive.current=false;epoch.current++;abort.abort();controller.current?.abort();};},[libraryOnly]);
  useEffect(()=>{epoch.current++;setReview(false);setChosen([]);setEntries([]);setRowErrors({});setSelected(null);setRaster(null);setSample(null);setError('');},[boundsKey,JSON.stringify(areaGeometry||null)]);
  useEffect(()=>{onMutationChange?.(busy);return()=>onMutationChange?.(false);},[busy,onMutationChange]);
  const visible=useMemo(()=>libraryOnly?records:records.filter(r=>r.bounds.join(',')===boundsKey),[records,boundsKey,libraryOnly]);
  const valid=useMemo(()=>{try{return validateGebcoRequest({name:name.trim(),bounds:areaBounds});}catch{return null;}},[name,boundsKey]);
  const run=async action=>{if(locked.current)return;locked.current=true;setBusy(true);setError('');const current=epoch.current;const abort=new AbortController();controller.current=abort;
    try{await action(current,abort.signal);}catch(e){if(alive.current&&current===epoch.current&&!abort.signal.aborted)setError(String(e?.message||e));}finally{locked.current=false;if(alive.current)setBusy(false);}};
  const current=seq=>alive.current&&seq===epoch.current;
  const create=()=>run(async(seq,signal)=>{const result=await gebcoRequest('create',valid,signal);if(current(seq)){setRecords(old=>[result.record,...old.filter(r=>r.id!==result.record.id)]);setSelected(result.record);setRaster(result.raster);setSample(null);}});
  const load=(ids=chosen)=>run(async(seq,signal)=>{
    if(new Set(entries.map(e=>e.key).concat(ids)).size>4)throw new Error('Load up to 4 numerical subset previews at a time.');
    const requested=ids.filter(id=>!entries.some(e=>e.key===id));
    const results=await Promise.allSettled(requested.map(async id=>{const record=visible.find(r=>r.id===id);if(!record)throw new Error('The selected subset belongs to a different region.');const data=await gebcoRequest('inspect',{id},signal);genericMapMetadata(data);if(data.sha256!==record.sha256)throw new Error('The GEBCO numerical grid or checksum is invalid.');return {key:id,title:record.name,kind:'map',visible:true,opacity:1,data:{imageUrl:data.previewDataUrl,asset:{bounds:data.bounds,crs:data.crs,imageExtent:data.bounds,source:{serviceTitle:'GEBCO 2026',attribution:record.attribution}}}};}));
    if(current(seq)){setEntries(old=>[...old,...results.filter(r=>r.status==='fulfilled').map(r=>r.value)]);setRowErrors(old=>{const next={...old};results.forEach((r,i)=>{if(r.status==='rejected')next[requested[i]]=String(r.reason?.message||r.reason);else delete next[requested[i]];});return next;});}
  });
  const inspect=record=>run(async(seq,signal)=>{const info=await gebcoRequest('inspect',{id:record.id},signal);if(info.sha256!==record.sha256)throw new Error('The GEBCO numerical grid or checksum is invalid.');if(current(seq)){setSelected(record);setRaster(info);setSample(null);setColumn('0');setRow('0');}});
  const pixel=()=>run(async(seq,signal)=>{const v=await gebcoRequest('pixel',{id:selected.id,column:Number(column),row:Number(row)},signal);if(v.sha256!==selected.sha256)throw new Error('The GEBCO numerical grid or checksum is invalid.');if(current(seq))setSample(v);});
  return <ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} title="GEBCO · Official regional elevation" areaBounds={areaBounds} areaGeometry={areaGeometry}
    actions={<SelectionActions count={chosen.length} onClear={()=>setChosen([])}
      preview={<Button variant="secondary" icon={Layers} disabled={busy||!chosen.length||new Set(entries.map(e=>e.key).concat(chosen)).size>4} onClick={()=>load()}>{t('Preview selected')}</Button>}
      download={!libraryOnly&&<Button primary icon={Download} disabled={busy||loading||!catalog||!valid} onClick={()=>{setSelected(null);setRaster(null);setReview(true);}}>{t('Save and download')}</Button>}
      details={<InfoDetails closeLabel={t('Close')} summary={t('Preview details')}><p>{t('Up to 4 preview layers. Clearing the map keeps your result selection.')}</p></InfoDetails>}/>}
    map={<ServiceExploreMap entries={entries} areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} onPatch={(key,patch)=>setEntries(old=>old.map(e=>e.key===key?{...e,...patch}:e))} onRemove={key=>setEntries(old=>old.filter(e=>e.key!==key))}/>}>
    {review&&<DownloadReview count={valid?1:0} onClose={()=>setReview(false)} onConfirm={create} busy={busy} error={error}
      receipt={selected&&raster?<SavedResultsReceipt entries={[{id:selected.id,title:selected.name}]} hrefFor={()=> '#My%20Data?view=gebco-regions'}/>:null}><strong>{name}</strong><p>{t('GEBCO 2026 · Bathymetry · numerical GeoTIFF')}</p><p>{areaBounds?.join(', ')}</p><p>{t('Official regional subset; unchanged downloaded numerical values and grid. First-band grayscale is for browsing.')}</p></DownloadReview>}
    <div className="gee-body">
      {workflowHeader}
      <p>{t('The official GEBCO service generates a numerical regional subset from its 2026 elevation grid. The whole original grid remains pending.')}</p>
      <p className="gee-note">{t('15 arc-second grid · elevations in meters · negative values are water depths. 2026 is the grid release, not a single observation date.')}</p>
      {!libraryOnly&&<div className="gee-fields"><label>{t('Subset name')}<Input maxLength={120} value={name} disabled={busy} onChange={e=>setName(e.target.value)}/></label>
        <small>{t('GEBCO 2026 · Bathymetry · numerical GeoTIFF')}</small>
        {!areaBounds&&<Button variant="secondary" disabled={busy} onClick={onRequestArea}>{t('Choose a region')}</Button>}
        <Button primary icon={Download} disabled={busy||loading||!catalog||!valid} onClick={()=>{setSelected(null);setRaster(null);setReview(true);}}>{t('Confirm download')}</Button>
        {busy&&<p role="status"><Spinner size={14}/>{t('The official service prepares the subset, then GeoD verifies the complete ZIP and numerical TIFF. This may take up to three minutes.')}</p>}
        {!valid&&areaBounds&&<p className="gee-note">{t('Enter a subset name and choose a sufficiently small region.')}</p>}
      </div>}
      {error&&<p className="gee-error" role="alert">{t(error)}</p>}
      <div className="gee-library-heading"><strong>{t(libraryOnly?'Saved GEBCO regions':'Saved numerical subsets for this region')}</strong><Button size="icon" variant="quiet" icon={RefreshCw} aria-label={t('Refresh')} disabled={busy||loading} onClick={()=>run(async(seq,signal)=>{const r=await gebcoRequest('list',{},signal);if(current(seq))setRecords(r);})}/></div>
      {loading?<Spinner/>:!visible.length?<p className="gee-note">{t(libraryOnly?'No verified GEBCO regions yet.':'No verified GEBCO subset for this region yet.')}</p>:<div className="gee-library">{visible.map(r=><article key={r.id}><div className="gee-record"><label><Input type="checkbox" aria-label={t('Select result {name}',{name:r.name})} checked={chosen.includes(r.id)} disabled={busy||chosen.length>=4&&!chosen.includes(r.id)} onChange={e=>setChosen(old=>e.target.checked?[...old,r.id]:old.filter(id=>id!==r.id))}/><span><strong>{r.name}</strong><small>{t('Official regional subset')} · {number(r.bytes/1024,{maximumFractionDigits:1})} KiB</small></span></label><Button size="xs" variant="quiet" disabled={busy} onClick={()=>inspect(r)}>{t('Details and pixels')}</Button></div>{rowErrors[r.id]&&<p className="gee-error" role="alert">{t(rowErrors[r.id])}<Button size="xs" disabled={busy} onClick={()=>load([r.id])}>{t('Retry preview')}</Button></p>}</article>)}</div>}
      {selected&&raster&&<Surface className="gee-result"><strong>{selected.name}</strong><p>{raster.width} × {raster.height} · EPSG:4326 · Int16</p><p>{t('Official regional subset; unchanged downloaded numerical values and grid. First-band grayscale is for browsing.')}</p>
        <div><Button variant="secondary" icon={Download} disabled={busy} onClick={()=>run(()=>saveGebcoFile(selected.id))}>{t('Save numerical GeoTIFF')}</Button> <Button variant="secondary" icon={Download} disabled={busy} onClick={()=>run(()=>saveGebcoFile(selected.id,true))}>{t('Save complete official ZIP')}</Button></div>
        <Disclosure summary={t('Read a numerical pixel')}><div className="gee-fields"><label>{t('Pixel column')}<Input type="number" min={0} max={raster.width-1} value={column} disabled={busy} onChange={e=>setColumn(e.target.value)}/></label><label>{t('Pixel row')}<Input type="number" min={0} max={raster.height-1} value={row} disabled={busy} onChange={e=>setRow(e.target.value)}/></label><Button variant="secondary" disabled={busy||column===''||row===''} onClick={pixel}>{t('Read original numerical value')}</Button>{sample&&<p role="status">{t('Elevation (m)')}: {sample.values[0]} · {t(sample.noData[0]?'NoData':'Valid raw value')}</p>}</div></Disclosure>
        <InfoDetails closeLabel={t('Close')} summary={t('Verification details')}><p className="gee-hash">GeoTIFF SHA-256: {selected.sha256}</p><p className="gee-hash">ZIP SHA-256: {selected.archiveSha256}</p><p>{selected.memberName}</p><p>{t('Official generation finished · complete ZIP CRC verified · numerical TIFF decoded and verified')}</p><p>{selected.attribution}</p>{selected.limitations.map(v=><p className="gee-note" key={v}>{t(v)}</p>)}</InfoDetails>
      </Surface>}
    </div>
  </ExploreSourceFrame>;
}
