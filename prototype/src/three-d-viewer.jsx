import React,{useEffect,useRef,useState} from 'react';
import {RotateCcw,MousePointer2} from 'lucide-react';
import {CesiumWidget,Cesium3DTileset,Model,HeadingPitchRange,Math as CesiumMath,Matrix4,Color,BoundingSphere} from 'cesium';
import 'cesium/Build/Cesium/Widgets/widgets.css';
import {Button,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {threeDRequest,createThreeDScene} from './three-d-client.js';
import {OfflineThreeDResource} from './three-d-resource.js';
export default function ThreeDViewer({asset}){
  const{t,number}=useI18n(),host=useRef(null),reset=useRef(null);const[status,setStatus]=useState('loading'),[progress,setProgress]=useState([0,asset.resources.length]),[error,setError]=useState('');
  useEffect(()=>{const abort=new AbortController();let widget,scene,primitive,unsubscribe,readyTimer,readyListener,visibleListener;let triangles=0,failed=false;setStatus('loading');setError('');
    const fail=e=>{if(!abort.signal.aborted){failed=true;setError(String(e?.message||e));setStatus('error');if(widget)widget.useDefaultRenderLoop=false;}};
    (async()=>{const checked=await threeDRequest('inspect',{id:asset.id},abort.signal);if(checked.receiptSha256!==asset.receiptSha256)throw new Error('3D source receipt changed');scene=await createThreeDScene(checked,{signal:abort.signal,onProgress:(a,b)=>{if(!abort.signal.aborted)setProgress([a,b]);}});if(abort.signal.aborted){scene.dispose();return;}
      widget=new CesiumWidget(host.current,{baseLayer:false,globe:false,skyBox:false,skyAtmosphere:false,scene3DOnly:true,contextOptions:{webgl:{alpha:false}},requestRenderMode:false});widget.scene.backgroundColor=Color.fromCssColorString('#17202b');widget.scene.highDynamicRange=false;if(widget.scene.sun)widget.scene.sun.show=false;if(widget.scene.moon)widget.scene.moon.show=false;widget.scene.screenSpaceCameraController.minimumZoomDistance=0.1;widget.scene.screenSpaceCameraController.maximumZoomDistance=40000000;
      const renderError=widget.scene.renderError.addEventListener((_s,e)=>fail(e));
      const resource=new OfflineThreeDResource(scene.url,scene.resourceUrls);
      if(scene.kind==='tileset'){primitive=await Cesium3DTileset.fromUrl(resource,{maximumScreenSpaceError:8});if(abort.signal.aborted){primitive.destroy();return;}widget.scene.primitives.add(primitive);primitive.tileFailed.addEventListener(fail);visibleListener=primitive.tileVisible.addEventListener(tile=>{triangles=Math.max(triangles,tile.content.trianglesLength||0);});reset.current=()=>widget.zoomTo(primitive,new HeadingPitchRange(0,CesiumMath.toRadians(-35),0));await reset.current();}
      else if(['gltf','glb'].includes(scene.kind)){primitive=await Model.fromGltfAsync({url:resource,modelMatrix:Matrix4.IDENTITY,asynchronous:true});if(abort.signal.aborted){primitive.destroy();return;}widget.scene.primitives.add(primitive);primitive.errorEvent.addEventListener(fail);reset.current=()=>{if(primitive.ready){widget.camera.flyToBoundingSphere(BoundingSphere.clone(primitive.boundingSphere),{duration:0,offset:new HeadingPitchRange(0,CesiumMath.toRadians(-30),Math.max(primitive.boundingSphere.radius*3,1))});}};readyListener=primitive.readyEvent.addEventListener(()=>reset.current?.());}
      else{throw new Error('A standalone b3dm tile needs its tileset transform and bounds');}
      if(abort.signal.aborted)return;readyTimer=setTimeout(()=>fail(new Error('3D scene did not produce visible geometry')),30000);
      const remove=widget.scene.postRender.addEventListener(()=>{if(abort.signal.aborted||failed)return;const visible=scene.kind==='tileset'?primitive.tilesLoaded&&triangles>0:primitive.ready&&primitive.boundingSphere.radius>0;if(visible){clearTimeout(readyTimer);setStatus('ready');host.current?.setAttribute('data-rendered-triangles',String(triangles));remove();}});unsubscribe=()=>{renderError();remove();};
    })().catch(fail);
    return()=>{abort.abort();clearTimeout(readyTimer);readyListener?.();visibleListener?.();unsubscribe?.();reset.current=null;if(widget&&!widget.isDestroyed())widget.destroy();scene?.dispose();};
  },[asset.id,asset.receiptSha256]);
  return <div className="three-d-viewer"><div className="three-d-viewer-toolbar"><Button size="icon" aria-label={t('Reset 3D camera')} tooltip={t('Reset 3D camera')} disabled={status!=='ready'} onClick={()=>reset.current?.()}><RotateCcw size={16}/></Button><span><MousePointer2 size={14}/>{t('Drag to orbit · wheel to zoom')}</span><span className="three-d-render-state" data-state={status}>{status==='ready'?t('Offline scene ready'):status==='error'?t('Scene could not be displayed'):t('Loading 3D scene')}</span></div><div className="three-d-canvas" ref={host} aria-label={t('Offline 3D viewport')}/>{status==='loading'&&<div className="three-d-viewer-overlay" role="status"><Spinner/>{t('Preparing local 3D assets')} · {number(progress[0])} / {number(progress[1])}</div>}{error&&<div role="alert" className="three-d-viewer-overlay three-d-error">{t(error)}</div>}<div className="three-d-attribution"><span>{asset.rights.attribution}</span><span>{asset.rights.license}</span></div></div>;
}
