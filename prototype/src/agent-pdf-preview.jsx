import React,{useEffect,useRef,useState} from 'react';
import {ChevronLeft,ChevronRight} from 'lucide-react';
import {Button,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {getDocument,GlobalWorkerOptions} from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
GlobalWorkerOptions.workerSrc=workerUrl;

export default function AgentPdfPreview({preview}){
  const {t}=useI18n(),canvas=useRef(null),[pdf,setPdf]=useState(null),[page,setPage]=useState(1),[busy,setBusy]=useState(true),[failed,setFailed]=useState(false);
  useEffect(()=>{
    let current=true;setPdf(null);setPage(1);setBusy(true);setFailed(false);
    const bytes=Uint8Array.from(atob(preview.dataUrl.split(',')[1]),character=>character.charCodeAt(0));
    const base=new URL('pdfjs/',location.href).href;
    const task=getDocument({data:bytes,cMapUrl:base+'cmaps/',standardFontDataUrl:base+'standard_fonts/',wasmUrl:base+'wasm/',iccUrl:base+'iccs/',
      disableAutoFetch:true,disableStream:true,enableXfa:false,isEvalSupported:false,maxImageSize:16*1024*1024,canvasMaxAreaInBytes:32*1024*1024});
    task.promise.then(document=>{if(document.numPages!==preview.document.pages)throw Error('PDF page count changed.');if(current)setPdf(document);}).catch(()=>{if(current){setFailed(true);setBusy(false);}});
    return()=>{current=false;task.destroy().catch(()=>{});};
  },[preview]);
  useEffect(()=>{
    if(!pdf || !canvas.current)return;
    let current=true,render;
    setBusy(true);setFailed(false);
    pdf.getPage(page).then(document=>{
      if(!current)return;const original=document.getViewport({scale:1});
      const scale=Math.min(760/original.width,1100/original.height,2),viewport=document.getViewport({scale});
      if(!Number.isFinite(viewport.width) || !Number.isFinite(viewport.height) || viewport.width<=0 || viewport.height<=0)throw Error('Invalid PDF page.');
      const target=canvas.current;target.width=Math.ceil(viewport.width);target.height=Math.ceil(viewport.height);
      render=document.render({canvas:target,viewport,annotationMode:0});return render.promise;
    }).then(()=>{if(current)setBusy(false);}).catch(()=>{if(current){setFailed(true);setBusy(false);}});
    return()=>{current=false;render?.cancel();};
  },[pdf,page]);
  return <div className="agent-pdf-preview"><div className="agent-pdf-toolbar"><Button size="icon" variant="quiet" icon={ChevronLeft} aria-label={t('Previous page')} onClick={()=>setPage(value=>value-1)} disabled={busy || page<=1}/><span>{t('Page {page} of {count}',{page,count:preview.document.pages})}</span><Button size="icon" variant="quiet" icon={ChevronRight} aria-label={t('Next page')} onClick={()=>setPage(value=>value+1)} disabled={busy || page>=preview.document.pages}/></div>
    {busy && <p className="agent-loading" role="status"><Spinner size={16}/>{t('Rendering PDF…')}</p>}{failed && <p className="agent-error" role="alert">{t('PDF could not be previewed.')}</p>}<canvas ref={canvas} aria-label={t('PDF page {page}',{page})} hidden={failed} data-rendered={!busy && !failed}/>
  </div>;
}
