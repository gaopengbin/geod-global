import React,{useEffect,useRef,useState} from 'react';
import OLMap from 'ol/Map.js';import View from 'ol/View.js';import ImageLayer from 'ol/layer/Image.js';import ImageStatic from 'ol/source/ImageStatic.js';import VectorLayer from 'ol/layer/Vector.js';import VectorSource from 'ol/source/Vector.js';import GeoJSON from 'ol/format/GeoJSON.js';import Feature from 'ol/Feature.js';import {fromExtent} from 'ol/geom/Polygon.js';import {Style,Fill,Stroke} from 'ol/style.js';import {transform,transformExtent} from 'ol/proj.js';import {register} from 'ol/proj/proj4.js';import proj4 from 'proj4';
import {Eye,EyeOff,Layers,Maximize,Minus,Plus,PanelLeftOpen,MapPin,Trash2} from 'lucide-react';
import {Button,Input} from './ui/index.jsx';import {useI18n} from './i18n.jsx';import {validQueryBounds} from './features-client.js';import {genericMapMetadata,genericCoordinateToPixel,genericProjectionDefinition,genericGeographicBounds} from './stac-map-model.js';
import 'ol/ol.css';import './service-explore.css';
import {SpectralBandSelect} from './enmap-ui.jsx';

export function NativeRasterExploreMap({entries=[],areaBounds,areaGeometry,listHidden,onShowList,onRequestArea,onPatch,onRemove,onPixel,onDisplayBand,bandBusy}) {
  const {t}=useI18n();const target=useRef(null),map=useRef(null),callbacks=useRef({}),fitted=useRef('');const [error,setError]=useState('');callbacks.current={entries,onPixel,disabled:bandBusy};
  useEffect(()=>{
    if(!target.current)return;
    const theme=getComputedStyle(target.current),land=new VectorLayer({source:new VectorSource({url:'./basemaps/natural-earth-50m-land.geojson',format:new GeoJSON(),wrapX:false}),style:new Style({fill:new Fill({color:theme.getPropertyValue('--canvas').trim()}),stroke:new Stroke({color:theme.getPropertyValue('--line-strong').trim(),width:1})})});
    const instance=new OLMap({target:target.current,layers:[land],view:new View({projection:'EPSG:3857',center:[0,0],zoom:2}),controls:[]});map.current=instance;
    const resize=new ResizeObserver(()=>instance.updateSize());resize.observe(target.current);
    const themeChange=new MutationObserver(()=>{const theme=getComputedStyle(target.current);land.setStyle(new Style({fill:new Fill({color:theme.getPropertyValue('--canvas').trim()}),stroke:new Stroke({color:theme.getPropertyValue('--line-strong').trim(),width:1})}));});themeChange.observe(document.documentElement,{attributes:true,attributeFilter:['data-theme','class','style']});
    instance.on('singleclick',event=>{
      if(callbacks.current.disabled)return;
      for(const entry of [...callbacks.current.entries].reverse()) {
        if(entry.visible===false)continue;
        try{const coordinate=transform(event.coordinate,'EPSG:3857',entry.data.crs),pixel=genericCoordinateToPixel(coordinate,entry.data);if(pixel){callbacks.current.onPixel?.(entry,pixel);break;}}catch(reason){setError(reason.message);}
      }
    });
    return()=>{resize.disconnect();themeChange.disconnect();instance.setTarget(undefined);map.current=null;};
  },[]);
  useEffect(()=>{
    const instance=map.current;if(!instance)return;
    while(instance.getLayers().getLength()>1)instance.removeLayer(instance.getLayers().item(1));
    let combined=null;
    let problem='';
    for(const entry of entries) {
      try {
      genericMapMetadata(entry.data);const definition=genericProjectionDefinition(entry.data.crs);if(definition){proj4.defs(entry.data.crs,definition);register(proj4);}
      if(entry.data.previewDataUrl){const layer=new ImageLayer({source:new ImageStatic({url:entry.data.previewDataUrl,projection:entry.data.crs,imageExtent:entry.data.bounds,interpolate:false}),visible:entry.visible!==false,opacity:entry.opacity??1});instance.addLayer(layer);}
      const b=transformExtent(genericGeographicBounds(entry.data,transformExtent),'EPSG:4326','EPSG:3857',16);combined=combined?[Math.min(combined[0],b[0]),Math.min(combined[1],b[1]),Math.max(combined[2],b[2]),Math.max(combined[3],b[3])]:b;
      }catch(reason){problem=reason.message;}
    }
    if(validQueryBounds(areaBounds)){
      const geometry=areaGeometry?new GeoJSON().readGeometry(areaGeometry,{dataProjection:'EPSG:4326',featureProjection:'EPSG:3857'}):fromExtent(transformExtent(areaBounds,'EPSG:4326','EPSG:3857'));
      instance.addLayer(new VectorLayer({source:new VectorSource({features:[new Feature(geometry)]}),style:new Style({stroke:new Stroke({color:'#2675dc',width:2,lineDash:[7,5]})})}));
    }
    setError(problem);const identity=entries.map(e=>e.key+e.data.sha256).join(',')+'|'+areaBounds?.join(',');
    if(fitted.current!==identity){fitted.current=identity;if(combined)instance.getView().fit(combined,{padding:[48,48,48,48],maxZoom:20});else if(validQueryBounds(areaBounds))instance.getView().fit(transformExtent(areaBounds,'EPSG:4326','EPSG:3857'),{padding:[48,48,48,48],maxZoom:18});}
  },[entries,areaBounds?.join(','),areaGeometry]);
  const zoom=delta=>{const view=map.current?.getView();if(view)view.setZoom(view.getZoom()+delta);};
  const fit=()=>{if(entries.length){try{const e=entries.at(-1);map.current?.getView().fit(transformExtent(genericGeographicBounds(e.data,transformExtent),'EPSG:4326','EPSG:3857',16),{padding:[48,48,48,48],maxZoom:20});}catch(reason){setError(reason.message);}}};
  return <div className="service-explore-map" aria-label={t('Original raster map')}><div ref={target} className="service-map-canvas"/>
    <div className="service-map-tools">{listHidden&&<Button size="icon" variant="secondary" aria-label={t('Show results')} onClick={onShowList}><PanelLeftOpen size={16}/></Button>}{onRequestArea&&<Button size="icon" variant="secondary" aria-label={t('Change search area')} onClick={onRequestArea}><MapPin size={16}/></Button>}<Button size="icon" variant="secondary" aria-label={t('Zoom in')} onClick={()=>zoom(1)}><Plus size={16}/></Button><Button size="icon" variant="secondary" aria-label={t('Zoom out')} onClick={()=>zoom(-1)}><Minus size={16}/></Button><Button size="icon" variant="secondary" aria-label={t('Fit original raster')} disabled={!entries.length} onClick={fit}><Maximize size={16}/></Button></div>
    {entries.length>0&&<section className="service-map-layers" aria-label={t('Map layers')}><strong><Layers size={15}/>{t('Map layers')} · {entries.length}</strong>{entries.map(entry=><div className="service-map-layer" key={entry.key}><div><span title={entry.title}>{t(entry.title)}</span><Button size="icon" variant="quiet" aria-label={t(entry.visible===false?'Show layer {name}':'Hide layer {name}',{name:entry.title})} onClick={()=>onPatch?.(entry.key,{visible:entry.visible===false})}>{entry.visible===false?<EyeOff size={14}/>:<Eye size={14}/>}</Button><Button size="icon" variant="quiet" aria-label={t('Remove layer {name}',{name:entry.title})} onClick={()=>onRemove?.(entry.key)}><Trash2 size={14}/></Button></div><SpectralBandSelect data={entry.data} disabled={bandBusy} onChange={band=>onDisplayBand?.(entry,band)}/><Input type="range" min={0} max={100} step={5} aria-label={t('Layer opacity {name}',{name:entry.title})} value={Math.round((entry.opacity??1)*100)} onChange={event=>onPatch?.(entry.key,{opacity:Number(event.target.value)/100})}/>{!entry.data.previewDataUrl&&<small>{t('No display preview is available. File metadata remains available below.')}</small>}</div>)}</section>}
    {error&&<p role="alert" className="feature-error">{t('Map placement is unavailable for this file. Read metadata and raw pixels below.')} {t(error)}</p>}
    <div className="service-map-attribution">{t(entries.some(entry=>entry.data.spectralEncoding)?'EnMAP data ©DLR 2023 · Selected spectral band · display stretch only':entries.some(entry=>entry.data.categoricalEncoding)?'Natural Earth · Original class ColorMap · low-detail preview':'Natural Earth · Original file grid · first-band display')}</div>
  </div>;
}
