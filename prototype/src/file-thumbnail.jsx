import React, { useEffect, useRef, useState } from 'react';
import { ImageOff, RefreshCw } from 'lucide-react';
import { Button, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { cachedFileThumbnail, fileThumbnailKey, loadFileThumbnail } from './file-thumbnail.js';

export function FileThumbnail({ job }) {
  const { t } = useI18n();
  const container = useRef(null);
  const [visible, setVisible] = useState(false);
  const [preview, setPreview] = useState(() => cachedFileThumbnail(job));
  const [error, setError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const key = fileThumbnailKey(job);
  const label = t(job.assetKey === 'scl' ? 'Local SCL file preview' : 'Local true-color file preview');
  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined') { setVisible(true); return; }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: '160px' });
    if (container.current) observer.observe(container.current);
    return () => observer.disconnect();
  }, [key]);
  useEffect(() => {
    let active = true;
    setPreview(cachedFileThumbnail(job)); setError(false);
    if (visible) loadFileThumbnail(job).then(data => { if (active) setPreview(data); }, () => { if (active) setError(true); });
    return () => { active = false; };
  }, [key, visible, attempt]);
  return <div ref={container} className="runtime-file-thumbnail" title={error ? t('File preview unavailable') : label}>
    {preview && !error ? <img src={preview.dataUrl} alt={`${label} · ${job.title || job.itemId}`} width={preview.width} height={preview.height} onError={() => setError(true)}/> : error ? <><ImageOff size={20} aria-hidden="true"/><Button size="icon" variant="quiet" aria-label={t('Retry file preview')} onClick={() => setAttempt(value => value + 1)}><RefreshCw size={16}/></Button></> : <span role="status" aria-label={t('Loading file preview')}><Spinner size={20}/></span>}
  </div>;
}
