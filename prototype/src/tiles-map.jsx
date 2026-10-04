import React,{useEffect,useRef,useState} from 'react';
import Map from 'ol/Map.js';import View from 'ol/View.js';import MVT from 'ol/format/MVT.js';import GeoJSON from 'ol/format/GeoJSON.js';
import VectorTileLayer from 'ol/layer/VectorTile.js';import VectorTileSource from 'ol/source/VectorTile.js';import VectorLayer from 'ol/layer/Vector.js';import VectorSource from 'ol/source/Vector.js';import TileState from 'ol/TileState.js';
import TileLayer from 'ol/layer/Tile.js';import XYZ from 'ol/source/XYZ.js';
import {createXYZ} from 'ol/tilegrid.js';import {transformExtent} from 'ol/proj.js';import {Style,Fill,Stroke,Circle as CircleStyle} from 'ol/style.js';import {asArray} from 'ol/color.js';
import {FolderOpen,Maximize,Download,Map as MapIcon,Plus,Minus} from 'lucide-react';
import {Button,Spinner,ResizableGroup,ResizablePanel,ResizeHandle,Disclosure,Select} from './ui/index.jsx';
import {tileRequest,exportTilePackage,tileAttribution,tileFormat} from './tiles-client.js';import {TileSourceDetails} from './tiles-ui.jsx';import {useI18n} from './i18n.jsx';
import 'ol/ol.css';import './vector.css';import './tiles.css';
function stylesFor(target){
  const t=getComputedStyle(target),token=n=>t.getPropertyValue(n).trim(),color=token('--accent'),surface=token('--surface'),alpha=(c,a)=>[...asArray(c).slice(0,3),a];
  const polygon=(fill,stroke,width=1)=>new Style({fill:new Fill({color:fill}),stroke:new Stroke({color:stroke,width})});
  const generic=new Style({fill:new Fill({color:alpha(color,0.16)}),stroke:new Stroke({color,width:1}),image:new CircleStyle({radius:4,fill:new Fill({color}),stroke:new Stroke({color:surface,width:1})})});
  const styles={earth:polygon(token('--canvas'),token('--line')),landcover:polygon(token('--inset'),token('--line')),landuse:polygon(token('--inset'),token('--line')),water:polygon(alpha(color,0.12),alpha(color,0.5)),buildings:polygon(alpha(token('--ink-2'),0.18),alpha(token('--ink-2'),0.4)),roads:new Style({stroke:new Stroke({color,width:1.5})}),boundaries:new Style({stroke:new Stroke({color:token('--ink-2'),width:1,lineDash:[5,4]})})};
  ['earth','landcover','landuse','water','buildings','roads','boundaries'].forEach((name,index)=>styles[name].setZIndex(index));generic.setZIndex(7);
  return {data:feature=>styles[feature.get('layer')]||generic,land:polygon(token('--canvas'),token('--line-strong'))};
}
export function TileWorkspace({id}) {
  const {t,number}=useI18n(),[data,setData]=useState(null),[error,setError]=useState(''),[loading,setLoading]=useState(0),[painting,setPainting]=useState(true),[selected,setSelected]=useState(null),[selectedLayer,setSelectedLayer]=useState('all'),[level,setLevel]=useState(null);
  const target=useRef(null),map=useRef(null),layer=useRef(null),filter=useRef('all');
  useEffect(()=>{const a=new AbortController();setData(null);setError('');setSelected(null);setSelectedLayer('all');setLoading(0);setPainting(true);tileRequest('inspect',{id},a.signal).then(setData).catch(e=>{if(!a.signal.aborted)setError(e.message);});return()=>a.abort();},[id]);
  useEffect(()=>{filter.current=selectedLayer;layer.current?.changed();setSelected(null);},[selectedLayer]);
  useEffect(()=>{
    if(!data||!target.current)return;const a=new AbortController(),p=data.asset,format=new MVT(),tileGrid=createXYZ({minZoom:p.minZoom,maxZoom:p.maxZoom});let styles=stylesFor(target.current);
    const vectorSource=()=>new VectorTileSource({format,tileGrid,wrapX:false,url:'local://{z}/{x}/{y}',tileLoadFunction:(tile)=>{tile.setLoader((extent,resolution,projection)=>{
      if(a.signal.aborted)return;const [z,x,y]=tile.getTileCoord();setLoading(n=>n+1);
      tileRequest('tile',{id,z,x,y},a.signal).then(async result=>{
        if(a.signal.aborted)return;let features=[];
        if(result.contentType!==undefined)throw new Error('Expected a local vector tile');
        if(result.dataBase64!==null){const b=Uint8Array.from(atob(result.dataBase64),c=>c.charCodeAt(0)),hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',b))].map(n=>n.toString(16).padStart(2,'0')).join('');if(hash!==result.sha256)throw new Error('Local tile checksum changed');features=format.readFeatures(b.buffer,{extent,featureProjection:projection});}
        if(!a.signal.aborted)tile.setFeatures(features);
      }).catch(e=>{if(!a.signal.aborted){setError(e.message);tile.setState(TileState.ERROR);}}).finally(()=>{if(!a.signal.aborted)setLoading(n=>Math.max(0,n-1));});
    });}});
    const raster=tileFormat(p)!=='pbf';
    const source=raster?new XYZ({minZoom:p.minZoom,maxZoom:p.maxZoom,tileSize:p.source.mbtiles.tileSize,wrapX:false,url:'local://{z}/{x}/{y}',tileLoadFunction:tile=>{
      if(a.signal.aborted)return;const[z,x,y]=tile.getTileCoord();setLoading(n=>n+1);
      tileRequest('tile',{id,z,x,y},a.signal).then(async result=>{
        if(a.signal.aborted)return;
        if(result.dataBase64===null){tile.setState(TileState.EMPTY);return;}
        const expected=tileFormat(p)==='png'?'image/png':'image/jpeg';
        if(result.contentType!==expected)throw new Error('Local image tile format changed');
        const b=Uint8Array.from(atob(result.dataBase64),c=>c.charCodeAt(0)),hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',b))].map(n=>n.toString(16).padStart(2,'0')).join('');
        if(hash!==result.sha256)throw new Error('Local tile checksum changed');
        if(!a.signal.aborted)tile.getImage().src=`data:${expected};base64,${result.dataBase64}`;
      }).catch(e=>{if(!a.signal.aborted){setError(e.message);tile.setState(TileState.ERROR);}}).finally(()=>{if(!a.signal.aborted)setLoading(n=>Math.max(0,n-1));});
    }}):vectorSource();
    const vectors=raster?new TileLayer({source}):new VectorTileLayer({source,style:f=>filter.current==='all'||f.get('layer')===filter.current?styles.data(f):null});layer.current=vectors;
    const land=new VectorLayer({source:new VectorSource({url:'./basemaps/natural-earth-50m-land.geojson',format:new GeoJSON(),wrapX:false}),style:styles.land});
    const instance=new Map({target:target.current,layers:[land,vectors],view:new View({projection:'EPSG:3857',center:[0,0],zoom:p.minZoom,minZoom:p.minZoom,maxZoom:Math.min(24,p.maxZoom+4)}),controls:[]});map.current=instance;
    instance.on('loadstart',()=>{if(!a.signal.aborted)setPainting(true);});
    instance.on('rendercomplete',()=>{if(!a.signal.aborted)setPainting(false);});
    const resize=new ResizeObserver(()=>instance.updateSize());resize.observe(target.current);
    instance.getView().fit(transformExtent(p.requestedBounds,'EPSG:4326','EPSG:3857'),{padding:[48,48,48,48],maxZoom:p.maxZoom+2});
    const updateLevel=()=>setLevel(source.getTileGrid().getZForResolution(instance.getView().getResolution(),source.zDirection));updateLevel();instance.on('moveend',updateLevel);
    instance.on('singleclick',event=>{let f=null;instance.forEachFeatureAtPixel(event.pixel,feature=>{f=feature;return true;},{layerFilter:l=>l===vectors,hitTolerance:5});setSelected(f?{id:f.getId(),properties:Object.fromEntries(Object.entries(f.getProperties()).filter(([k])=>k!=='geometry'))}:null);});
    const theme=new MutationObserver(()=>{styles=stylesFor(target.current);land.setStyle(styles.land);vectors.changed();});theme.observe(document.documentElement,{attributes:true,attributeFilter:['data-theme','class','style']});
    return()=>{a.abort();theme.disconnect();resize.disconnect();instance.setTarget(undefined);source.clear();map.current=null;layer.current=null;};
  },[data,id]);
  const asset=data?.asset,raster=asset&&tileFormat(asset)!=='pbf',layerNames=asset?[...new Set(asset.tiles.flatMap(tile=>tile.layers.map(l=>l.name)))].sort():[];
  const fit=()=>asset&&map.current?.getView().fit(transformExtent(asset.requestedBounds,'EPSG:4326','EPSG:3857'),{padding:[48,48,48,48],maxZoom:asset.maxZoom+2,duration:250});
  const zoom=delta=>{const v=map.current?.getView();if(v)v.animate({zoom:v.getZoom()+delta,duration:200});};
  return <main className="vector-workspace tiles-workspace" aria-label={t('Offline tile workspace')}><ResizableGroup orientation="horizontal" storageKey="tile-workspace" panelIds={['tile-details','tile-map']}><ResizablePanel id="tile-details" defaultSize="300px" minSize="240px" maxSize="480px"><aside className="vector-inspector"><div className="vector-inspector-heading"><MapIcon size={20}/><h2>{asset?.name||t('Offline tiles')}</h2></div><Button asChild variant="secondary" size="sm"><a href="#My%20Data?view=tiles"><FolderOpen size={16}/>{t('Offline tiles')}</a></Button>{asset&&<><p className="vector-source-note">{t('{count} tiles',{count:number(asset.tiles.length)})} · {asset.minZoom}–{asset.maxZoom} · EPSG:3857</p>{!raster&&<><label className="feature-field"><span>{t('Tile layer')}</span><Select aria-label={t('Tile layer')} value={selectedLayer} onChange={e=>setSelectedLayer(e.target.value)}><option value="all">{t('All tile layers')}</option>{layerNames.map(n=><option key={n} value={n}>{n}</option>)}</Select></label><p className="vector-source-note">{t('Local preview styling. Geometry and attributes come from the saved tiles; external styles, fonts and sprites are not downloaded.')}</p><h3>{t('Feature attributes')}</h3>{selected?<dl className="vector-properties">{selected.id!==undefined&&<><dt>{t('Feature ID')}</dt><dd>{String(selected.id)}</dd></>}{Object.entries(selected.properties).map(([k,v])=><React.Fragment key={k}><dt>{k}</dt><dd>{v&&typeof v==='object'?JSON.stringify(v):String(v??'')}</dd></React.Fragment>)}</dl>:<p className="vector-source-note">{t('Click a tile feature to inspect its attributes.')}</p>}</>}{raster&&<p className="vector-source-note">{t('Rendered map tiles. Original image bytes stay in the saved database; no external map requests are made.')} {asset.source.mbtiles.tileSize} × {asset.source.mbtiles.tileSize} · {tileFormat(asset).toUpperCase()}</p>}<Disclosure className="vector-attribution" summary={t('Tile source details')}><TileSourceDetails asset={asset}/></Disclosure>{typeof asset.source.metadata.attribution==='string'&&<p className="vector-attribution">{tileAttribution(asset.source.metadata)}</p>}</>}{error&&<p className="vector-error" role="alert">{t(error)}</p>}</aside></ResizablePanel><ResizeHandle label={t('Resize tile details')}/><ResizablePanel id="tile-map" minSize="360px"><div className="vector-map" ref={target}/>{asset&&<div className="tiles-map-overlay"><Button size="icon" variant="quiet" aria-label={t('Zoom in')} tooltip={t('Zoom in')} onClick={()=>zoom(1)}><Plus size={16}/></Button><Button size="icon" variant="quiet" aria-label={t('Zoom out')} tooltip={t('Zoom out')} onClick={()=>zoom(-1)}><Minus size={16}/></Button><Button size="icon" variant="quiet" aria-label={t('Fit saved region')} tooltip={t('Fit saved region')} onClick={fit}><Maximize size={16}/></Button><Button size="icon" variant="quiet" aria-label={t('Export tile package')} tooltip={t('Export tile package')} onClick={()=>exportTilePackage(id).catch(e=>setError(e.message))}><Download size={16}/></Button>{(loading>0||painting)&&!error?<Spinner aria-label={t('Loading local tiles')}/>:<span>{t('Tile level')} {level}</span>}</div>}{!data&&!error&&<div className="vector-map-state" role="status"><Spinner/>{t('Opening offline tile package')}</div>}</ResizablePanel></ResizableGroup></main>;
}
