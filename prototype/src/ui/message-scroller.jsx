import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowDown } from 'lucide-react';
import { Button } from './index.jsx';
import { useReducedMotion } from 'motion/react';

// Follow actual content growth, including late attachment layout. The reader
// owns the scroll position as soon as they leave the live edge.
// Ported follow mechanics from GeoD Agent MessageScroller, ecd93b7f (MIT).
// Global uses its existing styled viewport and back-to-latest control.
export function MessageScroller({ children, revision, conversationId, followKey, busy, viewportRef, followRef, className, label, jumpLabel }) {
  const content = useRef(null), frame = useRef(null);
  const programmatic=useRef(false),readerScrolled=useRef(false),userScrollUntil=useRef(0),scrollTimer=useRef(null),previousBusy=useRef(busy),reduce=useReducedMotion();
  const [following, setFollowing] = useState(true);
  const [canJump, setCanJump] = useState(false);
  const measure = useCallback(() => {
    const viewport = viewportRef.current;
    if (viewport) setCanJump(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop >= 48);
  }, [viewportRef]);
  const setFollow = useCallback(value => {
    followRef.current = value;
    setFollowing(value);
  }, [followRef]);
  const cancel = useCallback(() => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
  }, []);
  const follow = useCallback((smooth = false) => {
    const viewport = viewportRef.current;
    if (!viewport || !followRef.current) return;
    setFollowing(true);
    if (frame.current !== null) return;
    programmatic.current=true;clearTimeout(scrollTimer.current);
    if (!smooth || reduce) { viewport.scrollTop = viewport.scrollHeight; scrollTimer.current=setTimeout(()=>{programmatic.current=false;},80); return; }
    const step = () => {
      if (!followRef.current || !viewport.isConnected) { frame.current = null; return; }
      const distance = viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop;
      if (distance <= 1) { viewport.scrollTop = viewport.scrollHeight; frame.current = null; scrollTimer.current=setTimeout(()=>{programmatic.current=false;},80); return; }
      viewport.scrollTop += Math.max(1, distance * .26);
      frame.current = requestAnimationFrame(step);
    };
    frame.current = requestAnimationFrame(step);
  }, [viewportRef, followRef,reduce]);
  const leave = useCallback((readerInput=true) => { cancel();programmatic.current=false;clearTimeout(scrollTimer.current);if(readerInput){readerScrolled.current=true;userScrollUntil.current=performance.now()+1200;}setFollow(false); }, [cancel, setFollow]);
  useLayoutEffect(() => { cancel();readerScrolled.current=false;setFollow(true);follow(); }, [conversationId,followKey,cancel,setFollow,follow]);
  useLayoutEffect(()=>{const completed=previousBusy.current&&!busy;previousBusy.current=busy;if(completed&&!readerScrolled.current){setFollow(true);follow(true);}},[busy,setFollow,follow]);
  useLayoutEffect(() => { measure(); follow(busy); }, [revision, busy, follow, measure]);
  useEffect(() => {
    if (!content.current || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => { measure(); follow(busy); });
    observer.observe(content.current);
    return () => observer.disconnect();
  }, [busy, follow, measure]);
  useEffect(() => ()=>{cancel();clearTimeout(scrollTimer.current);}, [cancel]);
  return <div className="bui-message-scroller">
    <div ref={viewportRef} className={className} role="region" aria-label={label} tabIndex={0}
      onScroll={event => {
        if (programmatic.current) return;
        const viewport = event.currentTarget;
        measure();
        const distance=viewport.scrollHeight-viewport.clientHeight-viewport.scrollTop;
        if(distance<=56){readerScrolled.current=false;setFollow(true);}else if(performance.now()<userScrollUntil.current){readerScrolled.current=true;setFollow(false);}
      }}
      onWheel={event => { if (event.deltaY < 0&&!event.target.closest?.('.agent-work-code-area')) leave(); }} onTouchStart={()=>leave()}
      onPointerDown={event=>{if(event.target===event.currentTarget){userScrollUntil.current=performance.now()+1200;cancel();programmatic.current=false;}}}
      onKeyDown={event => { if (['ArrowUp', 'PageUp', 'Home'].includes(event.key)) leave(); }}
      onClickCapture={event => { if (event.target.closest?.('button[aria-expanded],summary')) leave(false); }}>
      <div ref={content} role="log" aria-live="polite" aria-relevant="additions text" aria-busy={busy}>{children}</div>
    </div>
    {!following && canJump && <Button className="bui-message-jump" variant="secondary" size="icon" icon={ArrowDown} aria-label={jumpLabel} tooltip={jumpLabel} onClick={() => { setFollow(true); follow(true); }}/>}
  </div>;
}
