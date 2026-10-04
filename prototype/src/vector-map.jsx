import React,{useEffect,useRef,useState} from 'react';
import Map from 'ol/Map.js';import View from 'ol/View.js';import GeoJSON from 'ol/format/GeoJSON.js';
import {vectorFeatures} from './vector-map-model.js';
import VectorLayer from 'ol/layer/Vector.js';import VectorSource from 'ol/source/Vector.js';
import {Style,Fill,Stroke,Circle as CircleStyle} from 'ol/style.js';
import {asArray} from 'ol/color.js';
import {createEmpty,extend} from 'ol/extent.js';
import {FolderOpen,Maximize,Download,FileDown,Shapes} from 'lucide-react';
import {Button,Spinner,EmptyState,ResizableGroup,ResizablePanel,ResizeHandle,Disclosure,Select} from './ui/index.jsx';
import {vectorRequest,exportVector} from './vector-client.js';import {useI18n} from './i18n.jsx';
import {FeatureProvenanceDetails,OsmProvenanceDetails} from './features-ui.jsx';
import {GeoPackageDetails,GeoPackageAccuracyNotice} from './geopackage-ui.jsx';
import {ShapefileDetails,ShapefileNotice,vectorErrorText} from './shapefile-ui.jsx';
import {LocalOsmDetails} from './osm-local-ui.jsx';
import 'ol/ol.css';import './vector.css';
function mapStyles(target) {
  const theme=getComputedStyle(target),token=name=>theme.getPropertyValue(name).trim();
  const feature=(color,width,radius,alpha)=>new Style({stroke:new Stroke({color,width}),fill:new Fill({color:[...asArray(color).slice(0,3),alpha]}),image:new CircleStyle({radius,fill:new Fill({color}),stroke:new Stroke({color:token('--surface'),width:1})})});
  return {data:feature(token('--accent'),2,5,0.18),focus:feature(token('--orange'),3,7,0.28),
    land:new Style({fill:new Fill({color:token('--canvas')}),stroke:new Stroke({color:token('--line-strong'),width:1})})};
}
function visibleExtent(source,table) {
  const extent=createEmpty();
  for(const feature of source.getFeatures())if(!table||feature.get('geodSourceLayer')===table) {
    const bounds=feature.getGeometry()?.getExtent();if(bounds?.every(Number.isFinite))extend(extent,bounds);
  }
  return extent;
}
export function VectorWorkspace({id}) {
  const {t,number}=useI18n();const [data,setData]=useState(null),[error,setError]=useState(''),[selected,setSelected]=useState(null),[selectedLayer,setSelectedLayer]=useState('');
  const target=useRef(null),map=useRef(null),layer=useRef(null),filter=useRef(''),selection=useRef(null);
  useEffect(()=>{const abort=new AbortController();setError('');setData(null);setSelected(null);setSelectedLayer('');filter.current='';selection.current=null;
    vectorRequest('inspect',{id},abort.signal).then(setData).catch(e=>{if(!abort.signal.aborted)setError(e.message);});return()=>abort.abort();},[id]);
  useEffect(()=>{
    if(!data||!target.current)return;
    const features=vectorFeatures(data.geojson,{localOsm:Boolean(data.asset.localOsm)});
    const source=new VectorSource({features,wrapX:false});let styles=mapStyles(target.current);
    const land=new VectorLayer({source:new VectorSource({url:'./basemaps/natural-earth-50m-land.geojson',format:new GeoJSON(),wrapX:false}),style:styles.land});
    const vectors=new VectorLayer({source,style:feature=>!filter.current||feature.get('geodSourceLayer')===filter.current?feature===selection.current?styles.focus:styles.data:null});layer.current=vectors;
    const instance=new Map({target:target.current,layers:[land,vectors],view:new View({projection:'EPSG:4326',center:[0,0],zoom:2}),controls:[]});map.current=instance;
    const fit=()=>{const extent=visibleExtent(source,filter.current);if(extent.every(Number.isFinite))instance.getView().fit(extent,{padding:[48,48,48,48],maxZoom:18,duration:250});};
    const resize=new ResizeObserver(()=>{instance.updateSize();});resize.observe(target.current);fit();
    instance.on('singleclick',event=>{
      let feature=null;instance.forEachFeatureAtPixel(event.pixel,f=>{feature=f;return true;},{layerFilter:l=>l===vectors,hitTolerance:5});
      selection.current=feature;vectors.changed();
      if(feature){setSelected({id:feature.get('geodOriginalId'),properties:feature.get('geodOriginalProperties')||{},sourceLayer:feature.get('geodSourceLayer'),measures:feature.get('geodOriginalMeasures')});}else{setSelected(null);}
    });
    const theme=new MutationObserver(()=>{styles=mapStyles(target.current);land.setStyle(styles.land);vectors.changed();});
    theme.observe(document.documentElement,{attributes:true,attributeFilter:['data-theme','class','style']});
    return()=>{theme.disconnect();resize.disconnect();instance.setTarget(undefined);map.current=null;layer.current=null;};
  },[data]);
  useEffect(()=>{filter.current=selectedLayer;selection.current=null;layer.current?.changed();setSelected(null);},[selectedLayer]);
  const fit=()=>{const source=layer.current?.getSource();if(!source)return;const extent=visibleExtent(source,filter.current);if(extent.every(Number.isFinite))map.current?.getView().fit(extent,{padding:[48,48,48,48],maxZoom:18,duration:250});};
  const asset=data?.asset;const properties=selected?.properties;
  const attributes=Object.entries(properties||{}).flatMap(([key,value])=>
    (asset?.format==='overpass-json'||asset?.localOsm)&&['tags','osm_metadata'].includes(key)&&value&&typeof value==='object'
      ? Object.entries(value).map(([tag,text])=>[`${key==='tags'?'tags':'metadata'}.${tag}`,text]) : [[key,value]]);
  return <main className="vector-workspace" aria-label={t('Vector workspace')}>
    <ResizableGroup orientation="horizontal" storageKey="vector-workspace" panelIds={['vector-details','vector-map']}><ResizablePanel id="vector-details" defaultSize="300px" minSize="240px" maxSize="480px">
      <aside className="vector-inspector">
        <div className="vector-inspector-heading"><Shapes size={20}/><h2>{asset?.name||t('Vector workspace')}</h2></div>
        <Button asChild variant="secondary" size="sm"><a href="#My%20Data?view=vectors"><FolderOpen size={16}/>{t('Vector files')}</a></Button>
        {asset&&<><p className="vector-source-note">{t('{count} features',{count:number(asset.featureCount)})} · WGS84</p>
          <p className="vector-source-note">{t('Click a feature on the map to inspect its original attributes.')}</p>
          <GeoPackageAccuracyNotice source={asset.geoPackage}/>
          <ShapefileNotice source={asset.shapefile}/>
          {asset.localOsm&&<label className="vector-layer-selector">{t('OSM object type')}<Select aria-label={t('OSM object type')} value={selectedLayer} onChange={e=>setSelectedLayer(e.target.value)}><option value="">{t('All objects')}</option>{Object.entries(asset.localOsm.objectCounts).filter(([,n])=>n>0).map(([kind,n])=><option key={kind} value={kind}>{t(kind==='node'?'Nodes':kind==='way'?'Ways':'Relations')} · {number(n)}</option>)}</Select></label>}
          {asset.geoPackage&&<label className="vector-layer-selector">{t('GeoPackage layer')}<Select aria-label={t('GeoPackage layer')} value={selectedLayer} onChange={e=>setSelectedLayer(e.target.value)}><option value="">{t('All layers')}</option>{asset.geoPackage.layers.map(l=><option value={l.table} key={l.table}>{l.identifier||l.table} · {number(l.featureCount)}</option>)}</Select></label>}
          {asset.shapefile&&<label className="vector-layer-selector">{t('Shapefile layer')}<Select aria-label={t('Shapefile layer')} value={selectedLayer} onChange={e=>setSelectedLayer(e.target.value)}><option value="">{t('All layers')}</option>{asset.shapefile.layers.map(l=><option value={l.table} key={l.table}>{l.table} · {number(l.featureCount)}</option>)}</Select></label>}
          <div className="vector-map-actions"><Button size="icon" variant="secondary" aria-label={t('Fit vector extent')} tooltip={t('Fit vector extent')} onClick={fit}><Maximize size={16}/></Button><Button size="icon" variant="secondary" aria-label={t('Export GeoJSON')} tooltip={t('Export GeoJSON')} onClick={()=>exportVector(id).catch(e=>setError(e.message))}><Download size={16}/></Button>{(asset.geoPackage||asset.shapefile||asset.localOsm)&&<Button size="icon" variant="secondary" aria-label={t(asset.localOsm?'Export original OSM file':asset.geoPackage?'Export original GeoPackage':'Export original Shapefile bundle')} tooltip={t(asset.localOsm?'Export original OSM file':asset.geoPackage?'Export original GeoPackage':'Export original Shapefile bundle')} onClick={()=>exportVector(id,true).catch(e=>setError(e.message))}><FileDown size={16}/></Button>}</div>
          <h3>{t('Feature attributes')}</h3>
          {selected?<dl className="vector-properties">{selected.sourceLayer&&<><dt>{t(asset.localOsm?'OSM object type':asset.shapefile?'Source layer':'Source table')}</dt><dd>{selected.sourceLayer}</dd></>}{selected.id!==undefined&&<><dt>{t('Feature ID')}</dt><dd>{String(selected.id)}</dd></>}{attributes.map(([key,value],index)=><React.Fragment key={index}><dt>{key}</dt><dd>{value!==null&&typeof value==='object'?JSON.stringify(value):String(value??'')}</dd></React.Fragment>)}{selected.measures!==undefined&&<><dt>{t('Original M values')}</dt><dd>{JSON.stringify(selected.measures)}</dd></>}</dl>:<p className="vector-source-note">{t('No feature selected')}</p>}
          {asset.attribution&&<p className="vector-attribution">{asset.attribution} · <a href={asset.licenseUrl} target="_blank" rel="noreferrer">ODbL</a></p>}
          {asset.remoteSource&&<Disclosure className="vector-attribution" summary={t('Query source details')}><dl className="vector-properties"><FeatureProvenanceDetails source={asset.remoteSource} geojsonSha256={asset.geojsonSha256} sourceSha256={asset.sourceSha256} sourceBytes={asset.bytes}/></dl></Disclosure>}
          {asset.osmSource&&<Disclosure className="vector-attribution" summary={t('Query source details')}><dl className="vector-properties"><OsmProvenanceDetails source={asset.osmSource} geojsonSha256={asset.geojsonSha256}/></dl></Disclosure>}
          {asset.geoPackage&&<Disclosure className="vector-attribution" summary={t('Vector source details')}><dl className="vector-properties"><GeoPackageDetails source={asset.geoPackage}/></dl></Disclosure>}
          {asset.shapefile&&<Disclosure className="vector-attribution" summary={t('Vector source details')}><dl className="vector-properties"><ShapefileDetails source={asset.shapefile}/></dl></Disclosure>}
          {asset.localOsm&&<Disclosure className="vector-attribution" summary={t('Vector source details')}><dl className="vector-properties"><LocalOsmDetails source={asset.localOsm}/></dl></Disclosure>}
        </>}
        {error&&<p className="vector-error" role="alert">{vectorErrorText(error,t)}</p>}
      </aside>
    </ResizablePanel><ResizeHandle label={t('Resize vector details')}/><ResizablePanel id="vector-map" minSize="360px">
      <div className="vector-map" ref={target}/>
      {!data&&!error&&<div className="vector-map-state" role="status"><Spinner/>{t('Opening vector file')}</div>}
      {data&&!data.geojson.features.some(f=>f.geodDeleted!==true&&f.geometry)&&<div className="vector-map-state"><EmptyState icon={Shapes} title={t('This file has no drawable geometry')}/></div>}
    </ResizablePanel></ResizableGroup>
  </main>;
}
