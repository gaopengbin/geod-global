import React,{useEffect,useRef,useState} from 'react';
import {FolderOpen,FilePlus,Shapes,Layers,Download,FileDown,Trash2,RefreshCw,Info,Plug} from 'lucide-react';
import {Button,Input,Select,EmptyState,Spinner,Disclosure} from './ui/index.jsx';
import {desktopAvailable} from './runtime-client.js';
import {vectorRequest,importVectorFile,exportVector} from './vector-client.js';
import {useI18n} from './i18n.jsx';
import {FeatureServiceDialog,FeatureProvenanceDetails,OsmProvenanceDetails} from './features-ui.jsx';
import {GeoPackageDetails} from './geopackage-ui.jsx';
import {ShapefileDetails,vectorErrorText} from './shapefile-ui.jsx';
import {LocalOsmDetails} from './osm-local-ui.jsx';
import './vector.css';
export function VectorLibrary({areaBounds,areaPolygon}) {
  const {t,number}=useI18n();const [files,setFiles]=useState([]),[loading,setLoading]=useState(true),[busy,setBusy]=useState(false),[error,setError]=useState(''),[revision,setRevision]=useState(0),[mode,setMode]=useState('reference');
  const input=useRef(null);const native=desktopAvailable();
  const [serviceOpen,setServiceOpen]=useState(false);
  useEffect(()=>{
    const abort=new AbortController();setError('');setLoading(true);
    vectorRequest('list',{},abort.signal).then(value=>{if(!abort.signal.aborted)setFiles(value);}).catch(e=>{if(!abort.signal.aborted)setError(e.message);}).finally(()=>{if(!abort.signal.aborted)setLoading(false);});
    return ()=>abort.abort();
  },[revision]);
  const run=async action=>{setBusy(true);setError('');try{await action();setRevision(n=>n+1);}catch(e){setError(e.message);}finally{setBusy(false);}};
  const open=()=>native?run(()=>vectorRequest('open',{managed:mode==='managed'})):input.current?.click();
  return <section className="vector-library" aria-label={t('Vector files')}>
    <div className="vector-toolbar">
      <Button primary icon={native?FolderOpen:FilePlus} disabled={busy} onClick={open}>{t(native?'Open vector file':'Import a vector copy')}</Button>
      <Button icon={Plug} variant="secondary" disabled={busy} onClick={()=>setServiceOpen(true)}>{t('Get vector data')}</Button>
      {native&&<Select aria-label={t('Vector file storage')} value={mode} onChange={e=>setMode(e.target.value)}><option value="reference">{t('Reference original file')}</option><option value="managed">{t('Copy into GeoD storage')}</option></Select>}
      <Button size="icon" variant="secondary" aria-label={t('Refresh vector files')} tooltip={t('Refresh vector files')} disabled={busy||loading} onClick={()=>setRevision(n=>n+1)}><RefreshCw size={16}/></Button>
      {(busy||loading)&&<Spinner aria-label={t(busy?'Opening vector file':'Loading vector files')}/>}
      <span className="vector-count">{t('{count} vector files',{count:number(files.length)})}</span>
      <Input ref={input} type="file" accept=".geojson,.json,.gpkg,.zip,.osm,.xml,.pbf" hidden onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(file)run(()=>importVectorFile(file));}}/>
    </div>
    <p className="vector-source-note">{t(native?'Open GeoJSON, OSM (XML / PBF / JSON), GeoPackage or Shapefile (SHP / ZIP), up to 20 MiB. Reference mode keeps the originals in place.':'Import GeoJSON, OSM (XML / PBF / JSON), GeoPackage or Shapefile ZIP, up to 20 MiB. A managed copy stays on this device.')}</p>
    {error&&<p role="alert" className="vector-error">{vectorErrorText(error,t)}</p>}
    {!files.length&&!error&&!busy&&!loading&&<EmptyState icon={Shapes} title={t('No vector files yet')} description={t('Open a local vector file to view its geometry and attributes.')}/>}
    <div className="vector-file-grid">{files.map(file=><article className="vector-file" key={file.id}>
      <Shapes size={22} aria-hidden="true"/>
      <div className="vector-file-copy"><strong title={file.name}>{file.name}</strong><p>{t('{count} features',{count:number(file.featureCount)})} · {file.format==='geopackage'?'GeoPackage':file.format==='shapefile'?'Shapefile':file.localOsm?`OSM ${file.localOsm.encoding.toUpperCase()}`:file.format==='overpass-json'?'OSM':file.format==='wfs-snapshot'?'WFS':'GeoJSON'} · {t(file.storageMode==='reference'?'Referenced file':'Managed copy')}</p>
        <div className="vector-file-actions">
          <Button asChild size="icon" variant="secondary" tooltip={t('Open in workspace')}><a href={`#Workspace?vector=${file.id}`} aria-label={t('Open in workspace')}><Layers size={16}/></a></Button>
          <Button size="icon" variant="secondary" aria-label={t('Export GeoJSON')} tooltip={t('Export GeoJSON')} disabled={busy} onClick={()=>run(()=>exportVector(file.id))}><Download size={16}/></Button>
          {file.geoPackage&&<Button size="icon" variant="secondary" aria-label={t('Export original GeoPackage')} tooltip={t('Export original GeoPackage')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.shapefile&&<Button size="icon" variant="secondary" aria-label={t('Export original Shapefile bundle')} tooltip={t('Export original Shapefile bundle')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.localOsm&&<Button size="icon" variant="secondary" aria-label={t('Export original OSM file')} tooltip={t('Export original OSM file')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          <Button size="icon" variant="quiet" aria-label={t('Remove registration')} tooltip={t('Remove registration')} disabled={busy} onClick={()=>run(()=>vectorRequest('forget',{id:file.id}))}><Trash2 size={16}/></Button>
          <Disclosure icon={Info} summary={t('Vector source details')}>
            <dl className="vector-properties"><dt>{t('Coordinate system')}</dt><dd>{file.crs}</dd><dt>{t('Coordinates')}</dt><dd>{number(file.coordinateCount)}</dd>{!file.osmSource&&!file.remoteSource?.wfs&&<><dt>{t('Source SHA-256')}</dt><dd className="mono">{file.sourceSha256}</dd></>}
              {file.dataTimestamp&&!file.osmSource&&!file.localOsm&&<><dt>{t('OSM dataset timestamp')}</dt><dd>{file.dataTimestamp}</dd></>}
              {!file.remoteSource&&<><dt>{t('License')}</dt><dd>{file.attribution?<><span>{file.attribution}</span> <a href={file.licenseUrl} target="_blank" rel="noreferrer">ODbL</a></>:t('License not provided by this file')}</dd></>}
              <FeatureProvenanceDetails source={file.remoteSource} geojsonSha256={file.geojsonSha256} sourceSha256={file.sourceSha256} sourceBytes={file.bytes}/>
              <OsmProvenanceDetails source={file.osmSource} geojsonSha256={file.geojsonSha256}/>
              <GeoPackageDetails source={file.geoPackage}/>
              <ShapefileDetails source={file.shapefile}/>
              <LocalOsmDetails source={file.localOsm}/>
            </dl>
          </Disclosure>
        </div>
      </div>
    </article>)}</div>
    {serviceOpen&&<FeatureServiceDialog areaBounds={areaBounds} areaPolygon={areaPolygon} onClose={()=>setServiceOpen(false)} onImported={()=>setRevision(n=>n+1)}/>}
  </section>;
}
