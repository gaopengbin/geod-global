import React, { useEffect, useRef, useState } from 'react';
import { AudioLines, Pause, Play } from 'lucide-react';
import { Button, Input, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';

const clock=seconds=>`${Math.floor(seconds/60)}:${String(Math.floor(seconds%60)).padStart(2,'0')}`;
export default function AgentAudioPreview({ preview }) {
  const {t,number}=useI18n(),element=useRef(null);
  const [ready,setReady]=useState(false),[playing,setPlaying]=useState(false),[failed,setFailed]=useState(false);
  const [position,setPosition]=useState(0),[duration,setDuration]=useState(preview.audio.durationMs/1000);
  const live=useRef(false);
  useEffect(()=>{
    const audio=element.current;live.current=true;
    setReady(false);setFailed(false);setPlaying(false);setPosition(0);
    audio.src=preview.dataUrl;audio.load();
    return()=>{live.current=false;audio.pause();audio.removeAttribute('src');audio.load();};
  },[preview.dataUrl]);
  const play=async()=>{
    const audio=element.current;if(!ready || failed)return;
    if(!audio.paused){audio.pause();return;}
    try{await audio.play();}catch{if(live.current)setFailed(true);}
  };
  return <section className="agent-audio-preview" aria-label={t('Audio preview')}>
    <audio ref={element} preload="metadata" onLoadedMetadata={()=>{
      const value=element.current.duration;
      if(!Number.isFinite(value) || value<=0 || value>600){setFailed(true);return;}
      setDuration(value);setReady(true);
    }} onPlay={()=>setPlaying(true)} onPause={()=>setPlaying(false)} onEnded={()=>setPlaying(false)} onTimeUpdate={()=>setPosition(element.current.currentTime)} onError={()=>setFailed(true)}/>
    <div className="agent-audio-symbol"><AudioLines size={28} aria-hidden="true"/></div>
    <div className="agent-audio-controls">
      <Button size="icon" primary icon={playing?Pause:Play} aria-label={t(playing?'Pause audio':'Play audio')} tooltip={t(playing?'Pause audio':'Play audio')} disabled={!ready || failed} onClick={play}/>
      <div className="agent-audio-track"><Input type="range" min={0} max={duration} step={0.1} value={Math.min(position,duration)} aria-label={t('Audio position')} disabled={!ready || failed} onChange={event=>{
        const next=Number(event.target.value);element.current.currentTime=next;setPosition(next);
      }}/><div className="agent-audio-time"><span>{clock(position)}</span><span>{clock(duration)}</span></div></div>
    </div>
    {!ready && !failed && <p role="status" className="agent-loading"><Spinner size={16}/>{t('Loading audio…')}</p>}
    {failed && <p role="alert" className="agent-error">{t('This device could not play the audio preview. The saved original file is still available.')}</p>}
    <p className="agent-file-note">{preview.document.name.split('.').at(-1).toUpperCase()} · {number(preview.audio.sampleRate/1000)} kHz · {preview.audio.channels===1?t('Mono'):preview.audio.channels===2?t('Stereo'):t('{count} channels',{count:number(preview.audio.channels)})}</p>
  </section>;
}
