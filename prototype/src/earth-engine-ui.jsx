import {DownloadReview,SelectionActions,SavedResultsReceipt} from './acquisition-ui.jsx';
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Copy, Download, ExternalLink, RefreshCw, Layers } from 'lucide-react';
import { InfoDetails, Button, Disclosure, Input, Modal, Select, Spinner, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { earthEngineProject, earthEngineRequest, earthEngineScript, landsatEarthEngineId, openEarthEnginePage, saveEarthEngineFile } from './earth-engine-client.js';
import './earth-engine.css';
import { ExploreSourceFrame } from './explore-source-workspace.jsx';
import { ServiceExploreMap } from './service-explore-map.jsx';
import { transformExtent } from 'ol/proj.js';
import { register } from 'ol/proj/proj4.js';
import proj4 from 'proj4';
import { genericMapMetadata, genericProjectionDefinition } from './stac-map-model.js';

export function EarthEngineDialog(props) { return <EarthEngineWorkflow {...props}/>; }
export function EarthEngineExplorer(props) { return <EarthEngineWorkflow {...props} inline/>; }

function EarthEngineWorkflow({ areaBounds, areaGeometry, itemId, onClose, inline = false, sidebarTarget, listHidden, onShowList, onRequestArea, onMutationChange }) {
  const {t, number} = useI18n();
  const [name,setName]=useState('Landsat · GEE'), [url,setUrl]=useState('');
  const [records,setRecords]=useState([]), [selected,setSelected]=useState(null), [raster,setRaster]=useState(null);
  const [busy,setBusy]=useState(false), [loading,setLoading]=useState(true), [error,setError]=useState(''), [copied,setCopied]=useState(false);
  const [imageId,setImageId]=useState(landsatEarthEngineId(itemId)), [band,setBand]=useState('SR_B4');
  const [region,setRegion]=useState((areaBounds || []).join(', '));
  const [chosen,setChosen]=useState([]), [entries,setEntries]=useState([]), [rowErrors,setRowErrors]=useState({});
  const [review,setReview]=useState(false);
  const lifetime=useRef(0), mounted=useRef(true);
  const tag=useRef(`geod-export-${crypto.randomUUID()}`);
  const script=useMemo(()=>{try{return {text:earthEngineScript({imageId,band,bounds:region.split(',').map(v=>v.trim()?Number(v):NaN),tag:tag.current})};}catch(e){return {error:e.message};}},[imageId,band,region]);
  useEffect(()=>{let active=true;earthEngineRequest('list').then(value=>{if(active)setRecords(value);}).catch(e=>{if(active)setError(e.message);}).finally(()=>{if(active)setLoading(false);});return()=>{active=false;};},[]);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;lifetime.current+=1;};},[]);
  useEffect(()=>{if(inline){lifetime.current+=1;setReview(false);setRegion((areaBounds||[]).join(', '));setChosen([]);setEntries([]);setRowErrors({});setError('');setSelected(null);setRaster(null);}},[inline,areaBounds?.join(','),JSON.stringify(areaGeometry || null)]);
  useEffect(()=>{onMutationChange?.(busy);return()=>onMutationChange?.(false);},[busy,onMutationChange]);
  const run=async action=>{if(busy)return;const sequence=lifetime.current;setBusy(true);setError('');try{await action();}catch(e){if(mounted.current&&sequence===lifetime.current)setError(String(e?.message || e));}finally{if(mounted.current)setBusy(false);}};
  const loadSelected=(ids=chosen)=>run(async()=>{
    const sequence=lifetime.current;
    if(new Set(entries.map(entry=>entry.key).concat(ids)).size>4)throw new Error('Load up to 4 original raster previews at a time.');
    const requested=ids.filter(id=>!entries.some(entry=>entry.key===id));
    const values=await Promise.allSettled(requested.map(async id=>{
      const record=records.find(item=>item.id===id), data=await earthEngineRequest('inspect',{id});
      genericMapMetadata(data);
      const definition=genericProjectionDefinition(data.crs);
      if(definition){proj4.defs(data.crs,definition);register(proj4);}
      const bounds=transformExtent(data.bounds,data.crs,'EPSG:4326',16);
      return {key:id,title:record.name,kind:'map',visible:true,opacity:1,data:{imageUrl:data.previewDataUrl,asset:{bounds,crs:data.crs,imageExtent:data.bounds,source:{serviceTitle:'Google Earth Engine · '+record.projectId}}}};
    }));
    if(mounted.current&&sequence===lifetime.current){
      const successful=values.filter(value=>value.status==='fulfilled').map(value=>value.value);
      setEntries(previous=>[...previous,...successful.filter(entry=>!previous.some(value=>value.key===entry.key))]);
      setRowErrors(previous=>{const next={...previous};values.forEach((value,index)=>{const id=requested[index];if(value.status==='rejected')next[id]=String(value.reason?.message||value.reason);else delete next[id];});return next;});
    }
  });
  const download=()=>run(async()=>{
    const request={name:name.trim(),url:url.trim()};
    setUrl(''); // Signed URLs stay only in the in-flight request, never in storage.
    const value=await earthEngineRequest('download',request);
    setRecords(prev=>[value.export,...prev]);setSelected(value.export);setRaster(value.raster);
  });
  const project=earthEngineProject(url) || selected?.projectId;
  const reviewDialog=review&&<DownloadReview count={1} onClose={()=>setReview(false)} onConfirm={download} busy={busy} error={error}
    receipt={selected&&raster?<SavedResultsReceipt entries={[{id:selected.id,title:selected.name}]} onOpen={entry=>{setReview(false);if(inline)loadSelected([entry.id]);}}/>:null}><strong>{name}</strong><p>{project}</p><p>{t('Generated subsets, up to 32 MiB. This entry does not automatically search GEE or download complete original scenes.')}</p><InfoDetails closeLabel={t('Close')} summary={t('GEE compute usage')}><p>{t('EECU usage: not measured. Download duration is not compute usage. Check Completed EECU-seconds in Cloud Monitoring.')}</p></InfoDetails></DownloadReview>;
  const body=<div className="gee-body">
      <p>{t('Download a GeoTIFF exported by your own GEE project. Google authorization stays in your browser.')}</p>
      <p className="gee-note">{t('Generated subsets, up to 32 MiB. This entry does not automatically search GEE or download complete original scenes.')}</p>
      <Disclosure summary={t('Prepare a Landsat export link')}>
        <div className="gee-fields">
          <label>{t('GEE image ID')}<Input value={imageId} onChange={e=>{setImageId(e.target.value);setCopied(false);}} placeholder="LANDSAT/LC09/C02/T1_L2/LC09_123032_20260929"/></label>
          <label>{t('Band')}<Select value={band} onChange={e=>{setBand(e.target.value);setCopied(false);}}>{['SR_B4','SR_B3','SR_B2','SR_B5','QA_PIXEL','QA_RADSAT'].map(v=><option key={v} value={v}>{v}</option>)}</Select></label>
          {!inline&&<label>{t('WGS84 bounds: west, south, east, north')}<Input value={region} onChange={e=>{setRegion(e.target.value);setCopied(false);}}/></label>}
          {script.error ? <p className="gee-note">{t(script.error)}</p> : <><pre className="gee-script">{script.text}</pre><Button variant="secondary" disabled={busy} onClick={()=>run(async()=>{await navigator.clipboard.writeText(script.text);setCopied(true);})}><Copy size={15}/>{t(copied?'Copied':'Copy export script')}</Button></>}
          <Button variant="secondary" disabled={busy} onClick={()=>run(()=>openEarthEnginePage('https://code.earthengine.google.com/'))}><ExternalLink size={15}/>{t('Open Earth Engine editor')}</Button>
          <small>{t('Run the script in your Google project, then paste its GeoTIFF link below. The script adds a workload tag for usage tracking.')}</small>
        </div>
      </Disclosure>
      <div className="gee-fields">
        <label>{t('Export name')}<Input value={name} maxLength={120} disabled={busy} onChange={e=>setName(e.target.value)}/></label>
        <label>{t('GEE download link')}<Input type="password" autoComplete="off" spellCheck={false} value={url} disabled={busy} onChange={e=>setUrl(e.target.value)} placeholder="https://earthengine.googleapis.com/…:getPixels"/></label>
        {project&&<small>{t('Quota project')}: <strong>{project}</strong></small>}
        <Button primary disabled={busy||!name.trim()||!earthEngineProject(url)} onClick={()=>{setSelected(null);setRaster(null);setReview(true);}}>{busy?<Spinner size={16}/>:<Download size={16}/>} {t(busy?'Downloading or verifying…':'Download and verify')}</Button>
        {busy&&<p role="status">{t('Wait for transfer and TIFF verification to finish. The download has a three-minute timeout.')}</p>}
        {error&&<p role="alert" className="gee-error">{t(error)}</p>}
      </div>
      <InfoDetails closeLabel={t('Close')} summary={t('GEE compute usage')}><Surface className="gee-quota"><p>{t('EECU usage: not measured. Download duration is not compute usage. Check Completed EECU-seconds in Cloud Monitoring.')}</p>
        <p>{t('Uses the project in the export link. Official hosted GEE / Pro service is not enabled.')}</p>
        {project&&<Button variant="secondary" disabled={busy} onClick={()=>run(()=>openEarthEnginePage(`https://console.cloud.google.com/monitoring/metrics-explorer?project=${encodeURIComponent(project)}`))}><ExternalLink size={15}/>{t('View project usage')}</Button>}
      </Surface></InfoDetails>
      <div className="gee-library-heading"><strong>{t('Saved GEE exports')}</strong><Button variant="quiet" size="icon" aria-label={t('Refresh')} disabled={busy||loading} onClick={()=>run(async()=>setRecords(await earthEngineRequest('list')))}><RefreshCw size={16}/></Button></div>
      {loading?<Spinner/>:!records.length?<p className="gee-note">{t('No verified GEE exports yet.')}</p>:<div className="gee-library">{records.map(record=>inline?<article key={record.id}><div className="gee-record"><label><Input type="checkbox" aria-label={t('Select result {name}',{name:record.name})} checked={chosen.includes(record.id)} disabled={busy||chosen.length>=4&&!chosen.includes(record.id)} onChange={e=>setChosen(previous=>e.target.checked?[...previous,record.id]:previous.filter(id=>id!==record.id))}/><span><strong>{record.name}</strong><small>{record.projectId} · {number(record.bytes/1048576,{maximumFractionDigits:2})} MiB</small></span></label><Button variant="quiet" size="icon" icon={Download} aria-label={t('Export result {name}',{name:record.name})} disabled={busy} onClick={()=>run(()=>saveEarthEngineFile(record.id))}/></div>{rowErrors[record.id]&&<div className="gee-error"><p role="alert">{t(rowErrors[record.id])}</p><Button size="xs" disabled={busy} onClick={()=>loadSelected([record.id])}>{t('Retry preview')}</Button></div>}</article>:<Button variant="quiet" key={record.id} className="gee-record" disabled={busy} onClick={()=>run(async()=>{const info=await earthEngineRequest('inspect',{id:record.id});setSelected(record);setRaster(info);})}><span><strong>{record.name}</strong><small>{record.projectId} · {number(record.bytes/1048576,{maximumFractionDigits:2})} MiB</small></span><span>{t('Preview')}</span></Button>)}</div>}
      {selected&&raster&&<Surface className="gee-result">
        <strong>{selected.name}</strong>
        {raster.previewDataUrl&&<img src={raster.previewDataUrl} alt={t('GEE export first-band grayscale preview')}/>}
        <p>{raster.width} × {raster.height} · {raster.crs} · {raster.bands.map(b=>b.dataType).join(' / ')}</p>
        <p>{t('First-band display only. File coordinates and raw sample values are retained.')}</p>
        <small>{t('Download duration')}: {number(selected.downloadSeconds,{maximumFractionDigits:2})} s · {t('EECU usage: not measured')}</small>
        <Button variant="secondary" disabled={busy} onClick={()=>run(()=>saveEarthEngineFile(selected.id))}><Download size={16}/>{t('Save GeoTIFF as…')}</Button>
        <InfoDetails closeLabel={t('Close')} summary={t('Verification details')}><p className="gee-hash">SHA-256: {selected.sha256}</p><p>{t('The file was downloaded from GEE and fully decoded. The link alone does not verify its dataset identity or native-grid alignment.')}</p>{raster.limitations.map(v=><p key={v} className="gee-note">{t(v)}</p>)}</InfoDetails>
      </Surface>}
    </div>;
  return inline?<ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} title="Google Earth Engine exports"
    actions={<SelectionActions count={chosen.length} onClear={()=>setChosen([])} preview={<Button variant="secondary" icon={Layers} disabled={busy||!chosen.length||new Set(entries.map(entry=>entry.key).concat(chosen)).size>4} onClick={()=>loadSelected()}>{t('Preview selected')}</Button>}
      details={<InfoDetails closeLabel={t('Close')} summary={t('Preview details')}><p>{t('Up to 4 preview layers. Clearing the map keeps your result selection.')}</p></InfoDetails>}/>}
    map={<ServiceExploreMap entries={entries} areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} onPatch={(key,patch)=>setEntries(previous=>previous.map(entry=>entry.key===key?{...entry,...patch}:entry))} onRemove={key=>setEntries(previous=>previous.filter(entry=>entry.key!==key))}/>}>{body}{reviewDialog}</ExploreSourceFrame>
    :<Modal title={t('Google Earth Engine exports')} className="gee-dialog" onClose={()=>{if(!busy)onClose();}} closeLabel={t('Close')}>{body}{reviewDialog}</Modal>;
}
