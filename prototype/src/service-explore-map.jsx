import React, {useEffect, useRef, useState} from 'react';
import OLMap from 'ol/Map.js';
import View from 'ol/View.js';
import ImageLayer from 'ol/layer/Image.js';
import TileLayer from 'ol/layer/Tile.js';
import ImageStatic from 'ol/source/ImageStatic.js';
import XYZ from 'ol/source/XYZ.js';
import VectorLayer from 'ol/layer/Vector.js';
import VectorTileLayer from 'ol/layer/VectorTile.js';
import VectorSource from 'ol/source/Vector.js';
import VectorTileSource from 'ol/source/VectorTile.js';
import GeoJSON from 'ol/format/GeoJSON.js';
import MVT from 'ol/format/MVT.js';
import TileState from 'ol/TileState.js';
import {createXYZ} from 'ol/tilegrid.js';
import {transformExtent, toLonLat} from 'ol/proj.js';
import {fromExtent} from 'ol/geom/Polygon.js';
import Feature from 'ol/Feature.js';
import {Style, Fill, Stroke, Circle as CircleStyle} from 'ol/style.js';
import {Eye, EyeOff, Layers, MapPin, Maximize, Minus, PanelLeftOpen, Plus, RefreshCw, Search, Trash2, X} from 'lucide-react';
import {Button, EmptyState, Input, Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {tileRequest} from './tiles-client.js';
import {vectorFeatures} from './vector-map-model.js';
import {validQueryBounds} from './features-client.js';
import {parseWaybackCatalog,WAYBACK_ATTRIBUTION} from './wayback-client.js';
import 'ol/ol.css';
import './service-explore.css';

const layerColors=['#2675dc','#0c977f','#a765cd','#d58a29'];
function featureStyle(color) {
  return new Style({stroke:new Stroke({color,width:2}),fill:new Fill({color:color+'30'}),image:new CircleStyle({radius:5,fill:new Fill({color}),stroke:new Stroke({color:'#fff',width:1})})});
}

const drawableExtent=extent=>Array.isArray(extent)&&extent.length===4&&extent.every(Number.isFinite)&&extent[0]<=extent[2]&&extent[1]<=extent[3];
function loadedExtent(records) {
  let combined=null;
  for(const record of records.values()) {
    if(!record.layer.getVisible())continue;
    let extent=null;
    try {
      if(record.kind==='vector')extent=record.source.getExtent();
      else if(record.kind==='map')extent=transformExtent(record.source.getImageExtent(),record.source.getProjection(),'EPSG:3857');
      // Package coverage describes the downloaded tiles. A source header may
      // describe the entire remote archive, so it must not set this view.
      else if(validQueryBounds(record.data.asset.tileCoverageBounds))extent=transformExtent(record.data.asset.tileCoverageBounds,'EPSG:4326','EPSG:3857');
    }catch{/* Unsupported or empty geometry never falls back to world bounds. */}
    if(!drawableExtent(extent))continue;
    combined=combined?[Math.min(combined[0],extent[0]),Math.min(combined[1],extent[1]),Math.max(combined[2],extent[2]),Math.max(combined[3],extent[3])]:[...extent];
  }
  return combined;
}

// An entry owns its request controller. Removing or changing a source cannot
// attach a late tile response to a different map entry.
export function ServiceExploreMap({entries=[],areaBounds,areaGeometry,onPatch,onRemove,listHidden,onShowList,onRequestArea,onInspectWaybackPoint}) {
  const {t}=useI18n();
  const target=useRef(null),map=useRef(null),records=useRef(new Map()),aoi=useRef(null),callbacks=useRef({onPatch,onRemove}),tilePackages=useRef(new Map()),fittedLoadedData=useRef(false);
  const [selected,setSelected]=useState(null),[failures,setFailures]=useState({}),[statuses,setStatuses]=useState({}),[retries,setRetries]=useState({});
  callbacks.current={onPatch,onRemove,onInspectWaybackPoint};
  useEffect(()=>{
    if(!target.current)return;
    const theme=getComputedStyle(target.current);
    const land=new VectorLayer({source:new VectorSource({url:'./basemaps/natural-earth-50m-land.geojson',format:new GeoJSON(),wrapX:false}),style:new Style({fill:new Fill({color:theme.getPropertyValue('--canvas').trim()||'#eef0ed'}),stroke:new Stroke({color:theme.getPropertyValue('--line-strong').trim()||'#b9c0c0',width:1})})});
    const region=new VectorLayer({source:new VectorSource(),style:new Style({stroke:new Stroke({color:'#2675dc',width:2,lineDash:[7,5]}),fill:new Fill({color:'#2675dc10'})})});
    const instance=new OLMap({target:target.current,layers:[land,region],view:new View({projection:'EPSG:3857',center:[0,0],zoom:2}),controls:[]});map.current=instance;aoi.current=region;
    const resize=new ResizeObserver(()=>instance.updateSize());resize.observe(target.current);
    const themeChanges=new MutationObserver(()=>{const next=getComputedStyle(target.current);land.setStyle(new Style({fill:new Fill({color:next.getPropertyValue('--canvas').trim()||'#eef0ed'}),stroke:new Stroke({color:next.getPropertyValue('--line-strong').trim()||'#b9c0c0',width:1})}));});themeChanges.observe(document.documentElement,{attributes:true,attributeFilter:['data-theme','class','style']});
    instance.on('singleclick',event=>{
      let hit=null;
      instance.forEachFeatureAtPixel(event.pixel,(feature,layer)=>{
        const record=[...records.current.values()].find(item=>item.layer===layer);
        if(!record)return;
        hit={key:record.key,data:record.data,title:record.title,id:feature.get('geodOriginalId')??feature.getId(),properties:feature.get('geodOriginalProperties')||Object.fromEntries(Object.entries(feature.getProperties()).filter(([key])=>key!=='geometry'))};return true;
      },{hitTolerance:5});setSelected(hit);
      if(!hit&&event.coordinate&&callbacks.current.onInspectWaybackPoint){const previews=[...records.current.values()].filter(record=>record.kind==='wayback'&&record.layer.getVisible());if(previews.length)callbacks.current.onInspectWaybackPoint(toLonLat(event.coordinate),Math.floor(instance.getView().getZoom()),previews.map(record=>record.data.version));}
    });
    return()=>{resize.disconnect();themeChanges.disconnect();for(const record of records.current.values()){record.abort.abort();record.source.clear?.();}records.current.clear();for(const cached of tilePackages.current.values())cached.abort.abort();tilePackages.current.clear();fittedLoadedData.current=false;instance.setTarget(undefined);map.current=null;aoi.current=null;};
  },[]);
  const boundsKey=areaBounds?.join(',')||'';
  useEffect(()=>{
    const instance=map.current,source=aoi.current?.getSource();if(!instance||!source)return;
    source.clear();setSelected(null);
    const geometry=areaGeometry?.geometry||areaGeometry;
    try {if(geometry?.type)source.addFeature(new GeoJSON().readFeature({type:'Feature',geometry,properties:{}},{dataProjection:'EPSG:4326',featureProjection:'EPSG:3857'}));else if(validQueryBounds(areaBounds))source.addFeature(new Feature(fromExtent(transformExtent(areaBounds,'EPSG:4326','EPSG:3857'))));}catch{/* Invalid supplied geometry never replaces valid shared bounds. */}
    if(validQueryBounds(areaBounds))instance.getView().fit(transformExtent(areaBounds,'EPSG:4326','EPSG:3857'),{padding:[48,48,48,48],maxZoom:14});
  },[boundsKey,areaGeometry]);
  useEffect(()=>{
    const instance=map.current;if(!instance)return;
    const keys=new Set(entries.map(entry=>entry.key));
    setSelected(current=>current&&!entries.some(entry=>entry.key===current.key&&entry.data===current.data&&entry.visible!==false)?null:current);
    const packageIds=new Set(entries.filter(entry=>entry.kind==='tiles').map(entry=>entry.data.asset.id));
    for(const[id,cached]of tilePackages.current){if(!packageIds.has(id)){cached.abort.abort();tilePackages.current.delete(id);}}
    for(const[key,record]of records.current){if(!keys.has(key)){record.abort.abort();instance.removeLayer(record.layer);record.source.clear?.();records.current.delete(key);}}
    setFailures(old=>Object.fromEntries(Object.entries(old).filter(([key])=>keys.has(key))));
    setStatuses(old=>Object.fromEntries(Object.entries(old).filter(([key])=>keys.has(key))));
    entries.forEach((entry,index)=>{
      let record=records.current.get(entry.key);
      if(record&&(record.data!==entry.data||record.retry!==(retries[entry.key]||0))){record.abort.abort();instance.removeLayer(record.layer);record.source.clear?.();records.current.delete(entry.key);record=null;}
      if(!record){
        const abort=new AbortController();let source,layer;
        const fail=error=>{if(!abort.signal.aborted)setFailures(old=>({...old,[entry.key]:error.message}));};
        const status=value=>{if(!abort.signal.aborted)setStatuses(old=>old[entry.key]===value?old:{...old,[entry.key]:value});};
        setFailures(old=>{if(!old[entry.key])return old;const next={...old};delete next[entry.key];return next;});
        if(entry.kind==='map'){
          const asset=entry.data.asset;source=new ImageStatic({url:entry.data.imageUrl,projection:asset.crs,imageExtent:asset.imageExtent||asset.bounds});layer=new ImageLayer({source});
          status('loading');source.on('imageloadstart',()=>status('loading'));source.on('imageloadend',()=>status('ready'));source.on('imageloaderror',()=>{status('error');fail(new Error('Map preview unavailable'));});
        }else if(entry.kind==='wayback'){
          const version=entry.data.version;
          try{
            const canonical=parseWaybackCatalog({[version.releaseNumber]:version.publisherRecord})[0];
            if(canonical.id!==version.id||canonical.tileTemplate!==version.tileTemplate)throw new Error('Wayback preview version identity changed');
            // Online rendered basemap only. These browser tile requests never
            // enter the native map-image save or offline export path.
            source=new XYZ({url:canonical.tileTemplate,minZoom:0,maxZoom:23,tileSize:256,wrapX:false,attributions:WAYBACK_ATTRIBUTION});layer=new TileLayer({source});
            let activeTiles=0;source.on('tileloadstart',()=>{activeTiles++;status('loading');});
            source.on('tileloadend',()=>{activeTiles=Math.max(0,activeTiles-1);status(activeTiles?'loading':'ready');});
            source.on('tileloaderror',()=>{if(abort.signal.aborted)return;activeTiles=Math.max(0,activeTiles-1);status('error');fail(new Error('Wayback online preview unavailable'));});
          }catch(error){fail(error);return;}
        }else if(entry.kind==='vector'){
          const features=vectorFeatures(entry.data.geojson,{localOsm:Boolean(entry.data.asset.localOsm)});features.forEach(feature=>feature.getGeometry()?.transform('EPSG:4326','EPSG:3857'));
          source=new VectorSource({features,wrapX:false});layer=new VectorLayer({source,style:featureStyle(layerColors[index%layerColors.length])});
          status(features.some(feature=>feature.getGeometry())?'ready':'empty');
        }else{
          const asset=entry.data.asset,format=new MVT();
          let cached=tilePackages.current.get(asset.id);
          if(!cached){cached={abort:new AbortController(),requests:new Map()};tilePackages.current.set(asset.id,cached);}
          source=new VectorTileSource({format,tileGrid:createXYZ({minZoom:asset.minZoom,maxZoom:asset.maxZoom}),wrapX:false,url:'local://{z}/{x}/{y}',tileLoadFunction:tile=>tile.setLoader((extent,resolution,projection)=>{
            if(abort.signal.aborted)return;const[z,x,y]=tile.getTileCoord();
            const coordinate=`${z}/${x}/${y}`;
            if(!cached.requests.has(coordinate)){
              const request=tileRequest('tile',{id:asset.id,z,x,y},cached.abort.signal).then(async result=>{
                if(result.contentType!==undefined)throw new Error('Expected a local vector tile');
                let bytes=null;
                if(result.dataBase64!==null){bytes=Uint8Array.from(atob(result.dataBase64),character=>character.charCodeAt(0));const hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(value=>value.toString(16).padStart(2,'0')).join('');if(hash!==result.sha256)throw new Error('Local tile checksum changed');}
                return bytes;
              }).catch(error=>{cached.requests.delete(coordinate);throw error;});
              cached.requests.set(coordinate,request);
            }
            cached.requests.get(coordinate).then(bytes=>{
              if(abort.signal.aborted)return;
              const features=bytes?format.readFeatures(bytes.buffer,{extent,featureProjection:projection}):[];
              if(!abort.signal.aborted)tile.setFeatures(features);
            }).catch(error=>{if(!abort.signal.aborted){fail(error);tile.setState(TileState.ERROR);}});
          })});
          let activeTiles=0;
          source.on('tileloadstart',()=>{activeTiles++;status('loading');});
          const finished=()=>{activeTiles=Math.max(0,activeTiles-1);status(activeTiles?'loading':'ready');};
          source.on('tileloadend',finished);source.on('tileloaderror',()=>{if(abort.signal.aborted)return;finished();setFailures(old=>old[entry.key]?old:{...old,[entry.key]:'Tile preview unavailable'});});
          const style=featureStyle(layerColors[index%layerColors.length]);layer=new VectorTileLayer({source,style:feature=>!entry.layerName||feature.get('layer')===entry.layerName?style:null});
        }
        layer.set('exploreTitle',entry.title);record={key:entry.key,kind:entry.kind,source,layer,abort,title:entry.title,data:entry.data,retry:retries[entry.key]||0};records.current.set(entry.key,record);instance.addLayer(layer);
      }
      record.layer.setVisible(entry.visible!==false);record.layer.setOpacity(entry.opacity??1);record.layer.setZIndex(index+1);
    });
    aoi.current?.setZIndex(entries.length+1);
    if(!entries.length)fittedLoadedData.current=false;
    else if(!fittedLoadedData.current&&!validQueryBounds(areaBounds)) {
      const extent=loadedExtent(records.current);
      if(extent){instance.getView().fit(extent,{padding:[48,48,48,48],maxZoom:14});fittedLoadedData.current=true;}
    }
  },[entries,retries,boundsKey]);
  const fit=()=>{const extent=validQueryBounds(areaBounds)?transformExtent(areaBounds,'EPSG:4326','EPSG:3857'):loadedExtent(records.current);if(extent)map.current?.getView().fit(extent,{padding:[48,48,48,48],maxZoom:18,duration:200});};
  const zoom=delta=>{const view=map.current?.getView();if(view)view.animate({zoom:view.getZoom()+delta,duration:150});};
  return <div className="service-explore-map" aria-label={t('Explore map')}>
    <div className="service-map-canvas" ref={target}/>
    {!entries.length && !validQueryBounds(areaBounds) && <div className="explore-map-empty"><EmptyState icon={Search} title={t('Choose your next observation')}>{t('Select results to load them on the map.')}</EmptyState></div>}
    <div className="service-map-tools">{listHidden&&onShowList&&<Button size="icon" variant="secondary" aria-label={t('Show results')} onClick={onShowList}><PanelLeftOpen size={16}/></Button>}{onRequestArea&&<Button size="icon" variant="secondary" aria-label={t('Change search area')} onClick={onRequestArea}><MapPin size={16}/></Button>}<Button size="icon" variant="secondary" aria-label={t('Zoom in')} onClick={()=>zoom(1)}><Plus size={16}/></Button><Button size="icon" variant="secondary" aria-label={t('Zoom out')} onClick={()=>zoom(-1)}><Minus size={16}/></Button><Button size="icon" variant="secondary" aria-label={t(validQueryBounds(areaBounds)?'Fit search area':'Fit loaded layers')} onClick={fit}><Maximize size={16}/></Button>{entries.length>0&&<Button size="icon" variant="secondary" aria-label={t('Clear map previews')} onClick={()=>{setSelected(null);entries.forEach(entry=>callbacks.current.onRemove?.(entry.key));}}><Trash2 size={16}/></Button>}</div>
    {entries.length>0&&<section className="service-map-layers" aria-label={t('Map layers')}><strong><Layers size={15}/>{t('Map layers')} · {entries.length}</strong>{entries.map(entry=><div className="service-map-layer" key={entry.key}><div><span title={entry.title}>{entry.title}</span><Button size="icon" variant="quiet" aria-label={t(entry.visible!==false?'Hide layer {name}':'Show layer {name}',{name:entry.title})} onClick={()=>callbacks.current.onPatch?.(entry.key,{visible:entry.visible===false})}>{entry.visible!==false?<Eye size={14}/>:<EyeOff size={14}/>}</Button><Button size="icon" variant="quiet" aria-label={t('Remove layer {name}',{name:entry.title})} onClick={()=>callbacks.current.onRemove?.(entry.key)}><Trash2 size={14}/></Button></div><Input type="range" min={0} max={100} step={5} aria-label={t('Layer opacity {name}',{name:entry.title})} value={Math.round((entry.opacity??1)*100)} onChange={event=>callbacks.current.onPatch?.(entry.key,{opacity:Number(event.target.value)/100})}/>{entry.visible!==false&&statuses[entry.key]==='loading'&&!failures[entry.key]&&<small role="status"><Spinner size={12}/>{t('Loading map preview')}</small>}{statuses[entry.key]==='empty'&&<small>{t('This file has no drawable geometry')}</small>}{failures[entry.key]&&<small role="alert">{t(failures[entry.key])}<Button size="icon" variant="quiet" aria-label={t('Retry layer {name}',{name:entry.title})} onClick={()=>setRetries(old=>({...old,[entry.key]:(old[entry.key]||0)+1}))}><RefreshCw size={14}/></Button></small>}</div>)}</section>}
    {selected&&<section className="service-feature-details"><div><strong>{selected.title}</strong><Button size="icon" variant="quiet" aria-label={t('Close')} onClick={()=>setSelected(null)}><X size={14}/></Button></div><dl>{selected.id!==undefined&&<><dt>{t('Feature ID')}</dt><dd>{String(selected.id)}</dd></>}{Object.entries(selected.properties||{}).map(([key,value])=><React.Fragment key={key}><dt>{key}</dt><dd>{value!==null&&typeof value==='object'?JSON.stringify(value):String(value??'')}</dd></React.Fragment>)}</dl></section>}
    <div className="service-map-attribution">{[...new Set(entries.map(entry=>entry.kind==='wayback'?WAYBACK_ATTRIBUTION:entry.kind==='map'?entry.data.asset.source.attribution||entry.data.asset.source.serviceTitle:entry.kind==='vector'?entry.data.asset.attribution||entry.data.asset.remoteSource?.serviceName:entry.data.asset.source.name).filter(Boolean))].join(' · ')||'Natural Earth'}</div>
  </div>;
}
