import React, {useEffect, useMemo, useRef, useState} from 'react';
import {Download, FolderOpen, Info, Layers, Plug, Plus, RefreshCw, Trash2} from 'lucide-react';
import { InfoDetails,Button, Disclosure, Input, Select, Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {ExploreSourceFrame} from './explore-source-workspace.jsx';
import {ServiceExploreMap} from './service-explore-map.jsx';
import {DownloadReview,SelectionActions,SavedResultsReceipt} from './acquisition-ui.jsx';
import {mapRequest, mapSize, exportMapImage} from './wms-client.js';
import {mapProtocol} from './arcgis-map-client.js';
import {defaultTileConfig, xyzMatrix, xyzPlan, gibsXyzExample, dlrTmsExample} from './xyz-client.js';
import {wmtsPlan} from './wmts-client.js';
import {featureRequest, validQueryBounds, validOverpassBounds} from './features-client.js';
import {vectorRequest, exportVector} from './vector-client.js';
import {tileRequest, tileSelection, exportTilePackage, pmtilesExample} from './tiles-client.js';
import './features.css';
import './service-explore.css';

const featureProtocol=source=>source.geoboundaries?'geoBoundaries':source.arcgis?'ArcGIS':source.overpass?'Overpass':source.wfs?'WFS2':'OGC';
const titleFor=(kind,t)=>t(kind==='map'?'Map services':kind==='feature'?'Vector data':'Offline tiles');
const sourceKeyFor=kind=>kind==='map'?'services':kind==='feature'?'list':'sources';
const requestFor=kind=>kind==='map'?mapRequest:kind==='feature'?featureRequest:tileRequest;

// Uses the same declared service grid as the existing explicit download form.
// Choosing a tile level does not invent sensor resolution or resample pixels.
export function exploreMapImageRequest(service,layer,bounds,geometry,options={}) {
  let size=mapSize(bounds,1024,service.maxWidth,service.maxHeight),tileOptions={};
  if(!size)throw new Error('Select a query region on Explore first.');
  if(service.xyz||service.wmts){
    const candidates=[];
    if(service.xyz){for(let level=service.xyz.grid.minZoom;level<=service.xyz.grid.maxZoom;level++){try{const matrix=xyzMatrix(service.xyz,level);candidates.push({set:'WebMercator',matrix,plan:xyzPlan(bounds,service.xyz,level)});}catch{/* Other declared levels may cover the area. */}}}
    else for(const link of layer.wmts.links){const set=service.wmts.matrixSets.find(value=>value.id===link.matrixSet);if(!set)continue;for(const matrix of set.matrices){if(link.limits?.length&&!link.limits.some(limit=>limit.matrix===matrix.id))continue;try{candidates.push({set:set.id,matrix,plan:wmtsPlan(bounds,set.crs,matrix,link.limits?.filter(limit=>limit.matrix===matrix.id)||[])});}catch{/* Ignore unsupported extent at this level. */}}}
    const fitting=candidates.filter(value=>Math.max(value.plan.width,value.plan.height)<=1024).sort((a,b)=>Math.max(b.plan.width,b.plan.height)-Math.max(a.plan.width,a.plan.height));
    const chosen=fitting[0]||candidates.sort((a,b)=>Math.max(a.plan.width,a.plan.height)-Math.max(b.plan.width,b.plan.height))[0];
    if(!chosen)throw new Error('Area exceeds this tile grid.');
    size=[chosen.plan.width,chosen.plan.height];tileOptions={tileMatrixSet:chosen.set,tileMatrix:chosen.matrix.id};
  }
  const defaultTime=layer.time?.default==='current'?new Date().toISOString().slice(0,10):layer.time?.default||'';
  const time=options.time??defaultTime;
  if(layer.time&&!time)throw new Error('Choose a service time.');
  return {serviceId:service.id,layerName:layer.name,style:options.style??layer.wmts?.defaultStyle??'',time:layer.time?time:null,bounds,width:size[0],height:size[1],areaGeometry:geometry?.geometry||geometry||null,...tileOptions};
}

function connectionDefaults(kind,protocol,preset) {
  if(kind==='tiles')return {...pmtilesExample};
  if(kind==='feature'&&protocol==='geoBoundaries')return {name:'geoBoundaries · CHN',url:'https://www.geoboundaries.org/api/current/gbOpen/CHN/ALL/'};
  if(kind==='feature')return protocol==='ArcGIS'?{name:'Esri · Earthquakes since 1970',url:'https://sampleserver6.arcgisonline.com/arcgis/rest/services/Earthquakes_Since1970/FeatureServer'}:protocol==='OGC'?{name:'pygeoapi · Natural Earth',url:'https://demo.pygeoapi.io/stable'}:{name:'',url:''};
  if(protocol==='XYZ'||protocol==='TMS')return protocol==='TMS'?dlrTmsExample:gibsXyzExample;
  if(protocol==='ArcGIS')return {name:'USGS NAIP',url:'https://imagery.nationalmap.gov/arcgis/rest/services/USGSNAIPImagery/ImageServer'};
  return preset==='gibs'?{name:'NASA GIBS',url:protocol==='WMTS'?'https://gibs.earthdata.nasa.gov/wmts/epsg4326/best/wmts.cgi':'https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi'}:{name:'',url:''};
}

function ServiceExplorer({kind,areaBounds,areaGeometry,currentProject,sidebarTarget,listHidden,onShowList,onRequestArea,onMutationChange,initialProtocol,initialPreset}) {
  const {t,number}=useI18n(),request=requestFor(kind);
  const initial=initialProtocol||(kind==='map'?'WMS':kind==='feature'?'OGC':'PMTiles');
  const defaults=connectionDefaults(kind,initial,initialPreset);
  const [sources,setSources]=useState([]),[sourceId,setSourceId]=useState(''),[loading,setLoading]=useState(true),[adding,setAdding]=useState(false),[protocol,setProtocol]=useState(initial),[name,setName]=useState(defaults.name),[url,setUrl]=useState(defaults.url),[tileConfig,setTileConfig]=useState(defaults.tileConfig||defaultTileConfig);
  const [boundaryCountry,setBoundaryCountry]=useState('CHN');
  const [review,setReview]=useState(false);
  const [search,setSearch]=useState(''),[checked,setChecked]=useState([]),[entries,setEntries]=useState([]),[savedResults,setSavedResults]=useState({}),[options,setOptions]=useState({}),[busy,setBusy]=useState(''),[error,setError]=useState(''),[rowErrors,setRowErrors]=useState({}),[minZoom,setMinZoom]=useState(0),[maxZoom,setMaxZoom]=useState(0),[revision,setRevision]=useState(0);
  const pending=useRef(null),sequence=useRef(0),mounted=useRef(true),tileCache=useRef(null),mutation=useRef(onMutationChange);mutation.current=onMutationChange;
  const boundsKey=areaBounds?.join(',')||'',geometryKey=JSON.stringify(areaGeometry||null);
  const source=sources.find(value=>value.id===sourceId);
  const sourceIdentity=JSON.stringify(source||null);
  const visibleSources=sources.filter(value=>!initialProtocol||kind==='tiles'||(kind==='map'?mapProtocol(value):featureProtocol(value))===initialProtocol);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;sequence.current++;pending.current?.abort();mutation.current?.(false);};},[]);
  useEffect(()=>{
    const abort=new AbortController();setLoading(true);setError('');
    request(sourceKeyFor(kind),{},abort.signal).then(list=>{
      if(abort.signal.aborted)return;setSources(list);
      const available=list.filter(value=>!initialProtocol||kind==='tiles'||(kind==='map'?mapProtocol(value):featureProtocol(value))===initialProtocol);
      const selected=initialPreset==='gibs'?available.find(value=>value.name.toLowerCase().includes('gibs'))||available[0]:available[0];
      setSourceId(selected?.id||'');setAdding(!selected);
    }).catch(reason=>{if(!abort.signal.aborted){setError(reason.message);setAdding(true);}}).finally(()=>{if(!abort.signal.aborted)setLoading(false);});
    return()=>abort.abort();
  },[kind,initialProtocol,initialPreset,revision]);
  useEffect(()=>{
    sequence.current++;pending.current?.abort();pending.current=null;tileCache.current=null;setChecked([]);setEntries([]);setSavedResults({});setRowErrors({});setError('');setBusy('');setOptions({});setReview(false);mutation.current?.(false);
  },[sourceId,sourceIdentity,boundsKey,geometryKey]);
  const sourceHeader=source?.header;
  useEffect(()=>{if(sourceHeader){setMinZoom(sourceHeader.minZoom);setMaxZoom(Math.min(sourceHeader.maxZoom,sourceHeader.minZoom+4));}},[sourceHeader]);
  useEffect(()=>{tileCache.current=null;sequence.current++;pending.current?.abort();setBusy('');mutation.current?.(false);setEntries([]);setSavedResults({});setRowErrors({});},[minZoom,maxZoom]);
  const items=useMemo(()=>kind==='map'?(source?.layers||[]).map(layer=>({...layer,key:layer.name,title:layer.title.replaceAll('_',' ')})):kind==='feature'?(source?.collections||[]).map(collection=>({...collection,key:collection.id,title:source?.overpass?t(collection.title):collection.title})):(source?.metadata?.vector_layers||[]).map(layer=>({...layer,key:layer.id,title:layer.id})),[kind,source,t]);
  const filtered=items.filter(item=>`${item.title} ${item.key}`.toLowerCase().includes(search.trim().toLowerCase())),shown=filtered.slice(0,100);
  const selectedItems=items.filter(item=>checked.includes(item.key));
  const regionValid=source?.geoboundaries?true:source?.overpass?validOverpassBounds(areaBounds):validQueryBounds(areaBounds);
  let tilePlan=null,tileError='';if(kind==='tiles'&&source&&validQueryBounds(areaBounds)){try{tilePlan=tileSelection(areaBounds,minZoom,maxZoom);}catch(reason){tileError=reason.message;}}
  const toggle=key=>setChecked(old=>old.includes(key)?old.filter(value=>value!==key):old.length<4?[...old,key]:old);
  const run=async(label,action)=>{
    const token=++sequence.current;pending.current?.abort();const abort=new AbortController();pending.current=abort;setBusy(label);setError('');mutation.current?.(true);
    const current=()=>mounted.current&&!abort.signal.aborted&&sequence.current===token;
    try{await action(abort.signal,current);}catch(reason){if(current()&&reason.name!=='AbortError')setError(reason.message);}finally{if(current()){setBusy('');mutation.current?.(false);}}
  };
  const connect=()=>run('connect',async(signal,current)=>{
    const saved=await request('connect',{name:name.trim(),url:url.trim(),...(kind==='map'?{protocol,...(['XYZ','TMS'].includes(protocol)?{tileConfig}:{})}:kind==='feature'&&protocol!=='OGC'?{protocol}:{})},signal);
    if(current()){setSources(old=>[...old.filter(value=>value.id!==saved.id),saved]);setSourceId(saved.id);setAdding(false);setSearch('');}
  });
  const addEntries=()=>run('load',async(signal,current)=>{
    let packageData=tileCache.current;
    if(kind==='tiles'&&!packageData){const asset=await tileRequest('extract',{sourceId,bounds:areaBounds,minZoom,maxZoom},signal);if(!current())return;packageData=await tileRequest('inspect',{id:asset.id},signal);if(!current())return;tileCache.current=packageData;}
    for(const item of selectedItems){
      if(!current())return;const key=`${sourceId}:${item.key}`;if(entries.some(entry=>entry.key===key))continue;
      try{
        let data=savedResults[item.key]?.data;
        if(!data&&kind==='map'){const asset=await mapRequest('get',exploreMapImageRequest(source,item,areaBounds,areaGeometry,options[item.key]),signal);if(!current())return;data=await mapRequest('inspect',{id:asset.id},signal);}
        else if(!data&&kind==='feature'){const asset=await featureRequest('query',{serviceId:sourceId,collectionId:item.id,bounds:source.geoboundaries&&!validQueryBounds(areaBounds)?[-180,-90,180,90]:areaBounds,areaGeometry:areaGeometry?.geometry||areaGeometry||null,...(source.wfs?{responseFormat:item.wfs.defaultFormat}:{})},signal);if(!current())return;data=await vectorRequest('inspect',{id:asset.id},signal);}
        else if(!data)data=packageData;
        if(current()){const entry={key,title:item.title,kind:kind==='feature'?'vector':kind==='tiles'?'tiles':'map',data,visible:true,opacity:1,...(kind==='tiles'?{layerName:item.id}:{})};setEntries(old=>old.some(value=>value.key===key)?old:[...old,entry]);setSavedResults(old=>({...old,[item.key]:entry}));setRowErrors(old=>{const next={...old};delete next[item.key];return next;});}
      }catch(reason){if(current()&&reason.name!=='AbortError')setRowErrors(old=>({...old,[item.key]:reason.message}));}
    }
  });
  const exportEntry=entry=>run('export',()=>kind==='map'?exportMapImage(entry.data.asset.id):kind==='feature'?exportVector(entry.data.asset.id):exportTilePackage(entry.data.asset.id));
  const title=titleFor(kind,t),loadNote=kind==='map'?'Loading saves rendered map images locally. Display colors are not original scientific bands.':source?.geoboundaries?'Loading saves the complete published country administrative level. The map area is reference only; boundaries are not clipped or filtered.':kind==='feature'?'Loading queries and saves complete matching features locally. Full geometry is retained without clipping.':'Loading extracts and saves whole intersecting vector tiles locally. Tile geometry may be simplified and is not clipped.';
  const workspaceHref=entry=>`#Workspace?${kind==='map'?'map':kind==='feature'?'vector':'tiles'}=${entry.data.asset.id}`;
  const patchOptions=(key,patch)=>{setOptions(old=>({...old,[key]:{...old[key],...patch}}));setEntries(old=>old.filter(entry=>entry.key!==`${sourceId}:${key}`));setSavedResults(old=>{const next={...old};delete next[key];return next;});};
  const layerCount=new Set([...entries.map(entry=>entry.key),...selectedItems.map(item=>`${sourceId}:${item.key}`)]).size;
  const selectedSaved=selectedItems.map(item=>savedResults[item.key]).filter(Boolean);
  const actions=!loading&&!adding&&source?<SelectionActions count={selectedItems.length} onClear={()=>setChecked([])}
    preview={<Button variant="secondary" icon={Layers} disabled={Boolean(busy)||!selectedSaved.length||layerCount>4} onClick={()=>setEntries(old=>[...old.filter(entry=>!selectedSaved.some(saved=>saved.key===entry.key)),...selectedSaved])}>{t('Preview selected')}</Button>}
    download={<Button variant="primary" icon={Download} disabled={Boolean(busy)||!selectedItems.length||!regionValid||kind==='tiles'&&!tilePlan||layerCount>4} onClick={()=>setReview(true)}>{t('Save and download')}</Button>}
    details={<InfoDetails closeLabel={t('Close')} summary={t('Preview details')}><p className="feature-help">{t(loadNote)}</p><p className="feature-help">{t('Up to 4 preview layers. Clearing the map keeps your result selection.')}{tilePlan&&<> · {t('{count} candidate tiles',{count:number(tilePlan.length)})}</>}</p></InfoDetails>}>
    {entries.length>0&&<Button variant="quiet" size="xs" icon={Trash2} disabled={Boolean(busy)} onClick={()=>setEntries([])}>{t('Clear map previews')}</Button>}
    <SavedResultsReceipt entries={selectedSaved} hrefFor={workspaceHref}/>
  </SelectionActions>:null;
  const controls=<div className="service-explore-controls">
    {loading?<div className="feature-loading" role="status"><Spinner/>{t('Reading service metadata')}</div>:<>
      {visibleSources.length>0&&<div className="feature-service-row"><label className="feature-field"><span>{title}</span><Select aria-label={t('Connected source')} value={sourceId} disabled={Boolean(busy)} onChange={event=>{setSourceId(event.target.value);setAdding(false);setSearch('');}}>{visibleSources.map(value=><option key={value.id} value={value.id}>{value.name}</option>)}</Select></label><Button size="icon" variant="quiet" aria-label={t('Add source connection')} onClick={()=>setAdding(value=>!value)} disabled={Boolean(busy)}><Plus size={16}/></Button><Button size="icon" variant="quiet" aria-label={t('Refresh source list')} onClick={()=>setRevision(value=>value+1)} disabled={Boolean(busy)}><RefreshCw size={16}/></Button></div>}
      {adding&&<section className="feature-connect">
        {!initialProtocol&&kind!=='tiles'&&<label className="feature-field"><span>{t('Service protocol')}</span><Select aria-label={t('Service protocol')} value={protocol} disabled={Boolean(busy)} onChange={event=>{setProtocol(event.target.value);const next=connectionDefaults(kind,event.target.value,initialPreset);setName(next.name);setUrl(next.url);setBoundaryCountry('CHN');setTileConfig(next.tileConfig||defaultTileConfig);}}>{(kind==='map'?['WMS','WMTS','XYZ','TMS','ArcGIS']:['OGC','ArcGIS','Overpass','WFS2','geoBoundaries']).map(value=><option value={value} key={value}>{value}</option>)}</Select></label>}
        <label className="feature-field"><span>{t('Connection name')}</span><Input aria-label={t('Connection name')} value={name} maxLength={80} disabled={Boolean(busy)} onChange={event=>setName(event.target.value)}/></label>
        {protocol==='geoBoundaries'?<><label className="feature-field"><span>{t('Country or territory code (three letters)')}</span><Input aria-label={t('Boundary country code')} value={boundaryCountry} maxLength={3} disabled={Boolean(busy)} placeholder="CHN / USA / DEU" onChange={event=>{const country=event.target.value.toUpperCase().replace(/[^A-Z]/g,'');setBoundaryCountry(country);setUrl(`https://www.geoboundaries.org/api/current/gbOpen/${country}/ALL/`);setName(`geoBoundaries · ${country}`);}}/></label><p className="feature-help">{t('gbOpen publishes national administrative levels with CC BY 4.0 attribution. These are research boundaries, not certified legal borders.')}</p></>:<label className="feature-field"><span>{t(kind==='tiles'?'PMTiles archive URL':['XYZ','TMS'].includes(protocol)?'Tile URL template':'Service URL')}</span><Input aria-label={t(kind==='tiles'?'PMTiles archive URL':['XYZ','TMS'].includes(protocol)?'Tile URL template':'Service URL')} value={url} maxLength={2048} disabled={Boolean(busy)} onChange={event=>setUrl(event.target.value)} placeholder="https://…"/></label>}
        <div className="feature-connect-actions"><Button primary icon={Plug} disabled={Boolean(busy)||!name.trim()||!url.trim()||protocol==='geoBoundaries'&&(!/^[A-Z]{3}$/.test(boundaryCountry)||boundaryCountry==='ALL')} onClick={connect}>{t('Connect service')}</Button>{source&&<Button variant="quiet" onClick={()=>setAdding(false)} disabled={Boolean(busy)}>{t('Cancel')}</Button>}</div>
        {protocol==='Overpass'&&<p className="feature-help">{t('Use an Overpass endpoint you operate or are authorized to use with this application. No public endpoint is selected automatically.')}</p>}
      </section>}
      {!adding&&source&&<>
        <InfoDetails closeLabel={t('Close')} summary={t('Source details')}><p className="feature-help">{source.url}<br/>{kind==='map'?mapProtocol(source):kind==='feature'?featureProtocol(source):'PMTiles'}<br/>{source.accessConstraints||t('Check source metadata and license before reuse.')}</p></InfoDetails>
        {kind==='tiles'&&sourceHeader&&<div className="tiles-options">{[['Minimum level',minZoom,setMinZoom],['Maximum level',maxZoom,setMaxZoom]].map(([label,value,setter])=><label className="feature-field" key={label}><span>{t(label)}</span><Select aria-label={t(label)} value={String(value)} disabled={Boolean(busy)} onChange={event=>setter(Number(event.target.value))}>{Array.from({length:sourceHeader.maxZoom-sourceHeader.minZoom+1},(_,index)=>sourceHeader.minZoom+index).map(level=><option key={level} value={String(level)}>{level}</option>)}</Select></label>)}</div>}
        <Input type="search" aria-label={t('Filter results')} placeholder={t('Filter results')} value={search} onChange={event=>setSearch(event.target.value)}/>
        <div className="service-result-heading"><span>{t('{count} results',{count:number(filtered.length)})}</span><Button variant="quiet" size="sm" disabled={Boolean(busy)||!shown.length} onClick={()=>setChecked(shown.slice(0,4).map(item=>item.key))}>{t('Select visible results')}</Button></div>
        <div className="service-result-list" aria-label={t('Search results')}>{shown.map(item=>{
          const entry=savedResults[item.key],itemOptions=options[item.key]||{};
          const metadata=[item.geoboundaries?`${item.geoboundaries.yearRepresented} · ${t('{count} administrative units',{count:number(item.geoboundaries.unitCount)})}`:`${t(kind==='map'?'Rendered map layer':kind==='feature'?'Complete feature query':'Vector tile layer')} · ${item.key}`,entry&&t('Saved locally'),entry&&kind==='feature'&&!item.geoboundaries&&t('{count} features',{count:number(entry.data.asset.featureCount)})].filter(Boolean).join(' · ');
          return <article className="service-result" key={item.key} data-selected={checked.includes(item.key)} data-boundary={Boolean(item.geoboundaries)}><label><Input type="checkbox" aria-label={t('Select result {name}',{name:item.title})} checked={checked.includes(item.key)} disabled={Boolean(busy)||!checked.includes(item.key)&&checked.length>=4} onChange={()=>toggle(item.key)}/><span title={item.title}>{item.title}</span></label><small className="service-result-meta" title={metadata}>{metadata}</small>
            {item.geoboundaries&&<InfoDetails closeLabel={t('Close')} icon={Info} summary={t('Boundary source and license')}><p className="feature-help">{t('Complete country boundary file')}<br/>{item.geoboundaries.countryName} · {item.geoboundaries.adminLevel}<br/>{item.geoboundaries.originalSource}<br/>{item.geoboundaries.originalLicense}<br/>{t('Release')} · {item.geoboundaries.releaseRef}<br/>{item.licenseLinks.join(' · ')}</p></InfoDetails>}
            {entry&&<div className="service-result-actions"><Button size="icon" variant="quiet" aria-label={t('Export result {name}',{name:item.title})} tooltip={t('Export result {name}',{name:item.title})} disabled={Boolean(busy)} onClick={()=>exportEntry(entry)}><Download size={14}/></Button><Button asChild size="icon" variant="quiet" tooltip={t('Open in workspace')}><a href={workspaceHref(entry)} aria-label={t('Open in workspace')}><FolderOpen size={14}/></a></Button></div>}
            {checked.includes(item.key)&&kind==='map'&&(item.time||item.styles?.length>0)&&<div className="service-result-options">{item.time&&<label className="feature-field"><span>{t('Service time (UTC)')}</span><Input aria-label={t('Service time for {name}',{name:item.title})} value={itemOptions.time??(item.time.default==='current'?new Date().toISOString().slice(0,10):item.time.default||'')} disabled={Boolean(busy)} onChange={event=>patchOptions(item.key,{time:event.target.value})}/></label>}{item.styles?.length>0&&<label className="feature-field"><span>{t('Map style')}</span><Select aria-label={t('Map style for {name}',{name:item.title})} value={itemOptions.style??item.wmts?.defaultStyle??''} disabled={Boolean(busy)} onChange={event=>patchOptions(item.key,{style:event.target.value})}>{!source.wmts&&<option value="">{t('Default style')}</option>}{item.styles.map(style=><option key={style} value={style}>{style}</option>)}</Select></label>}</div>}
            {rowErrors[item.key]&&<small role="alert">{t(rowErrors[item.key])}</small>}
          </article>;
        })}</div>
        {!shown.length&&<p className="feature-help">{t('No matching results')}</p>}{filtered.length>100&&<p className="feature-help">{t('Showing 100 results. Refine the filter to see other layers.')}</p>}
        {!regionValid&&source.overpass&&validQueryBounds(areaBounds)&&<p role="alert" className="vector-error">{t('OSM query region is too large. Choose at most 100 km² and 1° per side.')}</p>}{tileError&&<p role="alert" className="vector-error">{t(tileError)}</p>}
      </>}
    </>}
    {busy&&<div role="status" className="feature-loading"><Spinner/>{t(busy==='connect'?'Reading service metadata':busy==='export'?'Exporting saved data':'Retrieving and verifying selected data')}</div>}{error&&<p role="alert" className="vector-error">{t(error)}</p>}
  </div>;
  return <ExploreSourceFrame sidebarTarget={sidebarTarget} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} title={title} areaBounds={areaBounds} areaGeometry={areaGeometry} actions={actions} map={<ServiceExploreMap entries={entries} areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea} onPatch={(key,patch)=>setEntries(old=>old.map(entry=>entry.key===key?{...entry,...patch}:entry))} onRemove={key=>setEntries(old=>old.filter(entry=>entry.key!==key))}/>}>
    {controls}
    {review&&<DownloadReview count={selectedItems.length} onClose={()=>setReview(false)} onConfirm={addEntries} busy={busy==='load'} error={error||selectedItems.map(item=>rowErrors[item.key]).filter(Boolean).join(' · ')} receipt={selectedSaved.length===selectedItems.length&&selectedSaved.length>0?<SavedResultsReceipt entries={selectedSaved} hrefFor={workspaceHref}/>:null}>
      <p>{t(kind==='map'?'Rendered map layer':kind==='feature'?'Complete feature query':'Vector tile layer')}</p>
      <ul className="download-review-files">{selectedItems.map(item=><li key={item.key}>{item.title}</li>)}</ul>
      <p className="stac-help">{t(loadNote)}</p>
      <InfoDetails closeLabel={t('Close')} summary={t('Files and area')}><p>{areaBounds?.join(', ')||t('Whole country · no clipping')}</p><p>{source?.url}</p></InfoDetails>
    </DownloadReview>}
  </ExploreSourceFrame>;
}

export function MapServiceExplorer(props){return <ServiceExplorer {...props} kind="map"/>;}
export function FeatureServiceExplorer(props){return <ServiceExplorer {...props} kind="feature"/>;}
export function TileSourceExplorer(props){return <ServiceExplorer {...props} kind="tiles"/>;}
