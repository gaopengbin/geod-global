import React, { useEffect, useRef, useState } from 'react';
import { Pause, Play, Volume2, VolumeX } from 'lucide-react';
import { Button, Input, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
const clock=seconds=>`${Math.floor(seconds/60)}:${String(Math.floor(seconds%60)).padStart(2,'0')}`;
export default function AgentVideoPreview({preview}){
  const {t}=useI18n(),element=useRef(null),live=useRef(false);
  const [ready,setReady]=useState(false),[playing,setPlaying]=useState(false),[failed,setFailed]=useState(false),[muted,setMuted]=useState(false);
  const [position,setPosition]=useState(0),[duration,setDuration]=useState(preview.video.durationMs/1000);
  const [size,setSize]=useState([preview.video.width,preview.video.height]);
  useEffect(()=>{
    const video=element.current;live.current=true;
    setReady(false);setPlaying(false);setFailed(false);setMuted(false);setPosition(0);
    // loadeddata can precede the compositor's first frame. Wait for the frame
    // before removing the loading state on current WebView/Chromium versions.
    const frame=video.requestVideoFrameCallback?.(()=>{if(live.current && metadata())setReady(true);});
    video.src=preview.dataUrl;video.load();
    return()=>{live.current=false;if(frame!==undefined)video.cancelVideoFrameCallback?.(frame);video.pause();video.removeAttribute('src');video.load();};
  },[preview.dataUrl]);
  const metadata=()=>{
    const video=element.current;
    if(!Number.isFinite(video.duration) || video.duration<=0 || video.duration>600
      || ![video.videoWidth,video.videoHeight].every(side=>side>0 && side<=4096)
      || !(video.videoWidth===preview.video.width && video.videoHeight===preview.video.height
        || video.videoWidth===preview.video.height && video.videoHeight===preview.video.width)){
      video.pause();setFailed(true);return false;
    }
    setSize([video.videoWidth,video.videoHeight]);setDuration(video.duration);
    if(video.readyState>=2 && !video.requestVideoFrameCallback)setReady(true);
    return true;
  };
  const play=async()=>{
    const video=element.current;if(!ready || failed)return;
    if(!video.paused){video.pause();return;}
    try{await video.play();}catch{if(live.current)setFailed(true);}
  };
  return <section className="agent-video-preview" aria-label={t('Video preview')}>
    <div className="agent-video-screen" style={{aspectRatio:`${size[0]}/${size[1]}`}}>
      <video ref={element} playsInline preload="metadata" onLoadedMetadata={metadata} onLoadedData={metadata} onPlay={()=>setPlaying(true)} onPause={()=>setPlaying(false)} onEnded={()=>setPlaying(false)} onVolumeChange={()=>setMuted(element.current.muted)} onTimeUpdate={()=>setPosition(element.current.currentTime)} onError={()=>setFailed(true)}/>
      {!ready && !failed && <p className="agent-video-loading" role="status"><Spinner size={20}/>{t('Loading video…')}</p>}
    </div>
    <div className="agent-audio-controls"><Button size="icon" primary icon={playing?Pause:Play} aria-label={t(playing?'Pause video':'Play video')} tooltip={t(playing?'Pause video':'Play video')} disabled={!ready || failed} onClick={play}/>
      <div className="agent-audio-track"><Input type="range" min={0} max={duration} step={0.1} value={Math.min(position,duration)} aria-label={t('Video position')} disabled={!ready || failed} onChange={event=>{const next=Number(event.target.value);element.current.currentTime=next;setPosition(next);}}/><div className="agent-audio-time"><span>{clock(position)}</span><span>{clock(duration)}</span></div></div>
      {preview.video.hasAudio && <Button size="icon" variant="quiet" icon={muted?VolumeX:Volume2} aria-label={t(muted?'Unmute video':'Mute video')} tooltip={t(muted?'Unmute video':'Mute video')} disabled={!ready || failed} onClick={()=>{element.current.muted=!element.current.muted;setMuted(element.current.muted);}}/>}
    </div>
    {failed && <p role="alert" className="agent-error">{t('This device could not play the video preview. The saved original file is still available.')}</p>}
    <p className="agent-file-note">{preview.video.codec} · {size[0]} × {size[1]}{!preview.video.hasAudio && ` · ${t('No audio track')}`}</p>
  </section>;
}
