import React, { useContext, useEffect, useRef, useState } from 'react';
import { Image, ImageOff, RefreshCw } from 'lucide-react';
import { Button, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { cachedFileThumbnail, fileThumbnailKey, loadFileThumbnail } from './file-thumbnail.js';
import { RuntimeContext } from './runtime-context.js';

export function FileThumbnail({ job }) {
  const { t } = useI18n();
  const { health, checking } = useContext(RuntimeContext);
  const container = useRef(null);
  const [visible, setVisible] = useState(false);
  const [preview, setPreview] = useState(() => cachedFileThumbnail(job));
  const [error, setError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const key = fileThumbnailKey(job);
  const connected = useRef(Boolean(health));
  connected.current = Boolean(health);
  const label = t(['ndvi','evi'].includes(job.assetKey) ? 'Verified local vegetation index preview' : ['modis_qc','modis_state','qa_pixel','qa_radsat'].includes(job.assetKey) ? 'Local quality flags preview' : job.assetKey === 'wcs_coverage' ? 'Local coverage subset grayscale preview' : job.assetKey === 'stac_asset' ? 'Local original raster grayscale preview' : ['vv','vh','hh','hv'].includes(job.assetKey) ? 'Verified local radar preview in dB' : job.assetKey === 'aerial' ? 'Local aerial RGB preview' : ['elevation','srtm'].includes(job.assetKey) ? 'Verified local elevation tile in grayscale' : job.assetKey === 'scl' ? 'Local SCL file preview' : ['visual','reflectance_rgb'].includes(job.assetKey) ? 'Local true-color file preview' : 'Local reflectance band preview');
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
    if (visible && health) loadFileThumbnail(job, undefined, () => connected.current).then(data => { if (active) setPreview(data); }, () => { if (active) setError(true); });
    return () => { active = false; };
  }, [key, visible, attempt, Boolean(health)]);
  return <div ref={container} className="runtime-file-thumbnail" title={error ? t('File preview unavailable') : label}>
    {preview && !error ? <img src={preview.dataUrl} alt={`${label} · ${job.title || job.itemId}`} width={preview.width} height={preview.height} onError={() => setError(true)}/> : !health && !checking ? <ImageOff size={20} aria-label={t('Preview paused while the task service is offline')}/> : error ? <><ImageOff size={20} aria-hidden="true"/><Button size="icon" variant="quiet" aria-label={t('Retry file preview')} onClick={() => setAttempt(value => value + 1)}><RefreshCw size={16}/></Button></> : visible ? <span role="status" aria-label={t('Loading file preview')}><Spinner size={20}/></span> : <Image size={20} aria-hidden="true"/>}
  </div>;
}
