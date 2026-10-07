import React, { useEffect, useRef, useState } from 'react';
import { Package, Download, FolderOpen } from 'lucide-react';
import { desktopAvailable, formatBytes, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Button, Disclosure, Modal, Spinner, Surface } from './ui/index.jsx';

export function ArtifactPackageButton({ job, compact = false }) {
  const { t, locale } = useI18n();
  const [result, setResult] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(null);
  useEffect(() => () => request.current?.abort(), []);
  const act = async operation => {
    if (request.current && !request.current.signal.aborted) return;
    const controller = new AbortController(); request.current = controller;
    setBusy(true); setError(''); if (operation === 'package') setResult(null);
    try {
      const output = await runtimeRequest(operation, { id: job.id }, controller.signal);
      if (!controller.signal.aborted && operation === 'package') setResult(output);
    } catch (e) { if (!controller.signal.aborted) setError(e.message); }
    finally { if (request.current === controller) request.current = null; if (!controller.signal.aborted) setBusy(false); }
  };
  const contents = result && <><p>{t(job.kind === 'raster_rgb' ? 'GeoTIFF, preview, source provenance and checksums are saved together.' : 'GeoTIFF, provenance, recipe and checksums are saved together.')} {formatBytes(result.bytes, locale)}</p><p className="mono runtime-wrap">{compact ? result.filename : result.path}</p><Disclosure summary={t('Package checksum and contents')}><p className="mono runtime-wrap">SHA-256 · {result.sha256}</p>{compact && <p className="mono runtime-wrap">{result.path}</p>}<ul>{result.files.map(file => <li key={file} className="mono runtime-wrap">{file}</li>)}</ul></Disclosure><p>{t(job.kind === 'raster_rgb' ? 'The package includes source attribution and processing bounds.' : 'The package includes your recipe name and spatial bounds. Review them before sharing.')}</p>
      {desktopAvailable() ? <Button disabled={busy} onClick={() => act('revealPackage')}><FolderOpen size={15}/>{t('Show package in folder')}</Button> : <Button asChild><a href={`http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/package`} download={result.filename}><Download size={15}/>{t('Download ZIP')}</a></Button>}
    </>;
  const label = t(busy ? 'Preparing package…' : 'Prepare delivery package');
  const failure = <><p role="alert">{t('The delivery package could not be prepared. Check the source files and retry.')}</p><Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(error)}</p></Disclosure></>;
  return <div className={`artifact-export${compact ? ' artifact-export-compact' : ''}`}><Button disabled={busy} size={compact ? 'icon' : undefined} variant="secondary" {...(compact ? {'aria-label':label,tooltip:label} : {})} onClick={() => act('package')}>{busy ? <Spinner size={15}/> : <Package size={15}/>} {!compact && label}</Button>
    {result && !error && (compact ? <Modal title={t('Verified delivery package')} closeLabel={t('Close')} onClose={() => setResult(null)} closeDisabled={busy} className="artifact-package-dialog">{contents}</Modal> : <Surface as="section" className="artifact-package" aria-label={t('Verified delivery package')}><strong>{t('Verified delivery package')}</strong>{contents}</Surface>)}
    {error && (compact ? <Modal title={t('Delivery package unavailable')} closeLabel={t('Close')} onClose={() => {setError('');setResult(null);}} closeDisabled={busy}>{failure}</Modal> : <Surface className="runtime-error">{failure}</Surface>)}
  </div>;
}
