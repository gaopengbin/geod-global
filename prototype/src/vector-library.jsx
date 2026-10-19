import React,{useEffect,useRef,useState} from 'react';
import {FolderOpen,FilePlus,Shapes,Layers,Download,FileDown,Trash2,RefreshCw,Info,Plug} from 'lucide-react';
import { InfoDetails,Button,Input,Select,EmptyState,Spinner} from './ui/index.jsx';
import {desktopAvailable} from './runtime-client.js';
import {vectorRequest,importVectorFile,exportVector} from './vector-client.js';
import {useI18n} from './i18n.jsx';
import {FeatureServiceDialog,FeatureProvenanceDetails,OsmProvenanceDetails} from './features-ui.jsx';
import {GeoPackageDetails} from './geopackage-ui.jsx';
import {ShapefileDetails,vectorErrorText} from './shapefile-ui.jsx';
import {LocalOsmDetails} from './osm-local-ui.jsx';
import {vectorLicenseLabel} from './vector-license.js';
import './vector.css';
function RetainedPublisherDetails({file,t,number}) {
  const parquet=file.geoParquet,receipt=file.officialPackage,packageInfo=receipt?.package;
  if(parquet)return <><dt>{t('Original GeoParquet rows')}</dt><dd>{number(parquet.originalRows)}</dd><dt>{t('Area preview features')}</dt><dd>{number(file.featureCount)}</dd><dt>{t('Original source bytes')}</dt><dd>{number(parquet.originalBytes)} B</dd><dt>{t('Original source SHA-256')}</dt><dd className="mono">{parquet.originalSha256}</dd><dt>{t('Area preview bytes')}</dt><dd>{number(file.bytes)} B</dd><dt>{t('Area preview SHA-256')}</dt><dd className="mono">{file.sourceSha256}</dd><dt>{t('Original publisher URL')}</dt><dd>{parquet.originalUrl}</dd><dt>{t('Selected area')}</dt><dd>{parquet.requestedBounds.join(', ')}</dd><dt>{t('Selection scope')}</dt><dd>{t('The workspace geometry is an area preview of complete features. The full GeoParquet original is retained separately; preview export does not contain every original row.')}</dd></>;
  if(packageInfo)return <><dt>{t('Original source bytes')}</dt><dd>{number(file.bytes)} B</dd><dt>{t('Converted GeoJSON SHA-256')}</dt><dd className="mono">{file.geojsonSha256}</dd><dt>{t('Original publisher URL')}</dt><dd>{packageInfo.url}</dd><dt>{t('Publisher release')}</dt><dd>{packageInfo.release}</dd><dt>{t('Selected area')}</dt><dd>{receipt.requestedBounds.join(', ')}</dd><dt>{t('Selection scope')}</dt><dd>{t('The complete publisher package is retained. The query area selects a package; geometry is not clipped or simplified.')}</dd><dt>{t('Publisher warnings')}</dt><dd>{packageInfo.warnings.map(w=><p key={w}>{t(w)}</p>)}</dd></>;
  return null;
}
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
      <InfoDetails closeLabel={t('Close')} summary={t('Vector files')}><p className="vector-source-note">{t(native?'Open GeoJSON, OSM (XML / PBF / JSON), GeoPackage or Shapefile (SHP / ZIP), up to 20 MiB. Reference mode keeps the originals in place.':'Import GeoJSON, OSM (XML / PBF / JSON), GeoPackage or Shapefile ZIP, up to 20 MiB. A managed copy stays on this device.')}</p></InfoDetails>
      <span className="vector-count">{t('{count} vector files',{count:number(files.length)})}</span>
      <Input ref={input} type="file" accept=".geojson,.json,.gpkg,.zip,.osm,.xml,.pbf" hidden onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(file)run(()=>importVectorFile(file));}}/>
    </div>
    {error&&<p role="alert" className="vector-error">{vectorErrorText(error,t)}</p>}
    {!files.length&&!error&&!busy&&!loading&&<EmptyState icon={Shapes} title={t('No vector files yet')} description={t('Open a local vector file to view its geometry and attributes.')}/>}
    <div className="vector-file-grid">{files.map(file=><article className="vector-file" key={file.id}>
      <Shapes size={22} aria-hidden="true"/>
      <div className="vector-file-copy"><strong title={file.name}>{file.name}</strong><p>{t(file.geoParquet?'{count} area preview features':'{count} features',{count:number(file.featureCount)})} · {file.format==='geoparquet-preview'?t('GeoParquet original + area preview'):file.format==='geojsonl-gzip'?'GeoJSONL gzip':file.format==='geopackage'?'GeoPackage':file.format==='shapefile'?'Shapefile':file.localOsm?`OSM ${file.localOsm.encoding.toUpperCase()}`:file.format==='overpass-json'?'OSM':file.format==='wfs-snapshot'?'WFS':'GeoJSON'} · {t(file.storageMode==='reference'?'Referenced file':'Managed copy')}</p>
        <div className="vector-file-actions">
          <Button asChild size="icon" variant="secondary" tooltip={t('Open in workspace')}><a href={`#Workspace?vector=${file.id}`} aria-label={t('Open in workspace')}><Layers size={16}/></a></Button>
          <Button size="icon" variant="secondary" aria-label={t(file.geoParquet?'Export area preview GeoJSON':'Export GeoJSON')} tooltip={t(file.geoParquet?'Export area preview GeoJSON':'Export GeoJSON')} disabled={busy} onClick={()=>run(()=>exportVector(file.id))}><Download size={16}/></Button>
          {file.geoParquet&&<Button size="icon" variant="secondary" aria-label={t('Export complete original GeoParquet')} tooltip={t('Export complete original GeoParquet')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.format==='geojsonl-gzip'&&<Button size="icon" variant="secondary" aria-label={t('Export original GeoJSONL gzip')} tooltip={t('Export original GeoJSONL gzip')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.geoPackage&&<Button size="icon" variant="secondary" aria-label={t('Export original GeoPackage')} tooltip={t('Export original GeoPackage')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.shapefile&&<Button size="icon" variant="secondary" aria-label={t('Export original Shapefile bundle')} tooltip={t('Export original Shapefile bundle')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          {file.localOsm&&<Button size="icon" variant="secondary" aria-label={t('Export original OSM file')} tooltip={t('Export original OSM file')} disabled={busy} onClick={()=>run(()=>exportVector(file.id,true))}><FileDown size={16}/></Button>}
          <Button size="icon" variant="quiet" aria-label={t('Remove registration')} tooltip={t('Remove registration')} disabled={busy} onClick={()=>run(()=>vectorRequest('forget',{id:file.id}))}><Trash2 size={16}/></Button>
          <InfoDetails closeLabel={t('Close')} icon={Info} summary={t('Vector source details')}>
            <dl className="vector-properties"><dt>{t('Coordinate system')}</dt><dd>{file.crs}</dd><dt>{t('Coordinates')}</dt><dd>{number(file.coordinateCount)}</dd>{!file.geoParquet&&!file.osmSource&&!file.remoteSource?.wfs&&<><dt>{t(file.officialPackage?'Original source SHA-256':'Source SHA-256')}</dt><dd className="mono">{file.sourceSha256}</dd></>}
              {file.dataTimestamp&&!file.osmSource&&!file.localOsm&&<><dt>{t('OSM dataset timestamp')}</dt><dd>{file.dataTimestamp}</dd></>}
              {!file.remoteSource&&<><dt>{t('License')}</dt><dd>{file.attribution?<><span>{file.attribution}</span> <a href={file.licenseUrl} target="_blank" rel="noreferrer">{t(vectorLicenseLabel(file))}</a></>:t('License not provided by this file')}</dd></>}
              <RetainedPublisherDetails file={file} t={t} number={number}/>
              <FeatureProvenanceDetails source={file.remoteSource} geojsonSha256={file.geojsonSha256} sourceSha256={file.sourceSha256} sourceBytes={file.bytes}/>
              <OsmProvenanceDetails source={file.osmSource} geojsonSha256={file.geojsonSha256}/>
              <GeoPackageDetails source={file.geoPackage}/>
              <ShapefileDetails source={file.shapefile}/>
              <LocalOsmDetails source={file.localOsm}/>
            </dl>
          </InfoDetails>
        </div>
      </div>
    </article>)}</div>
    {serviceOpen&&<FeatureServiceDialog areaBounds={areaBounds} areaPolygon={areaPolygon} onClose={()=>setServiceOpen(false)} onImported={()=>setRevision(n=>n+1)}/>}
  </section>;
}
