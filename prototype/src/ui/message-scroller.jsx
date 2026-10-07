import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowDown } from 'lucide-react';
import { Button } from './index.jsx';

// Follow actual content growth, including late attachment layout. The reader
// owns the scroll position as soon as they leave the live edge.
export function MessageScroller({ children, revision, conversationId, busy, viewportRef, followRef, className, label, jumpLabel }) {
  const content = useRef(null), frame = useRef(null);
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
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    if (!smooth || reduce) { viewport.scrollTop = viewport.scrollHeight; return; }
    const step = () => {
      if (!followRef.current || !viewport.isConnected) { frame.current = null; return; }
      const distance = viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop;
      if (distance <= 1) { viewport.scrollTop = viewport.scrollHeight; frame.current = null; return; }
      viewport.scrollTop += Math.max(1, distance * .35);
      frame.current = requestAnimationFrame(step);
    };
    frame.current = requestAnimationFrame(step);
  }, [viewportRef, followRef]);
  const leave = useCallback(() => { cancel(); setFollow(false); }, [cancel, setFollow]);
  useLayoutEffect(() => { cancel(); setFollow(true); follow(); }, [conversationId, cancel, setFollow, follow]);
  useLayoutEffect(() => { measure(); follow(busy); }, [revision, busy, follow, measure]);
  useEffect(() => {
    if (!content.current || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => { measure(); follow(busy); });
    observer.observe(content.current);
    return () => observer.disconnect();
  }, [busy, follow, measure]);
  useEffect(() => cancel, [cancel]);
  return <div className="bui-message-scroller">
    <div ref={viewportRef} className={className} role="region" aria-label={label} tabIndex={0}
      onScroll={event => {
        if (frame.current !== null) return;
        const viewport = event.currentTarget;
        measure();
        setFollow(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop < 48);
      }}
      onWheel={event => { if (event.deltaY < 0) leave(); }} onTouchStart={leave}
      onKeyDown={event => { if (['ArrowUp', 'PageUp', 'Home'].includes(event.key)) leave(); }}
      onClickCapture={event => { if (event.target.closest?.('[data-slot="disclosure-trigger"]')) leave(); }}>
      <div ref={content} role="log" aria-live="polite" aria-relevant="additions text" aria-busy={busy}>{children}</div>
    </div>
    {!following && canJump && <Button className="bui-message-jump" variant="secondary" size="icon" icon={ArrowDown} aria-label={jumpLabel} tooltip={jumpLabel} onClick={() => { setFollow(true); follow(true); }}/>}
  </div>;
}
