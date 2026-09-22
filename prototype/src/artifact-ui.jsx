import React, { useEffect, useRef, useState } from 'react';
import { Package, Download, FolderOpen, LoaderCircle } from 'lucide-react';
import { desktopAvailable, formatBytes, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';

export function ArtifactPackageButton({ job }) {
  const { t, locale } = useI18n();
  const [result, setResult] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(null);
  useEffect(() => () => request.current?.abort(), []);
  const act = async operation => {
    request.current?.abort();
    const controller = new AbortController(); request.current = controller;
    setBusy(true); setError('');
    try {
      const output = await runtimeRequest(operation, { id: job.id }, controller.signal);
      if (!controller.signal.aborted && operation === 'package') setResult(output);
    } catch (e) { if (!controller.signal.aborted) setError(e.message); }
    finally { if (!controller.signal.aborted) setBusy(false); }
  };
  return <div className="artifact-export"><button className="button" disabled={busy} onClick={() => act('package')}>{busy ? <LoaderCircle size={15} className="runtime-spinner"/> : <Package size={15}/>} {t(busy ? 'Preparing package…' : 'Prepare delivery package')}</button>
    {result && <section className="artifact-package" aria-label={t('Verified delivery package')}><strong>{t('Verified delivery package')}</strong><p>{t('GeoTIFF, provenance, recipe and checksums are saved together.')} {formatBytes(result.bytes, locale)}</p><p className="mono runtime-wrap">{result.path}</p><details><summary>{t('Package checksum and contents')}</summary><p className="mono runtime-wrap">SHA-256 · {result.sha256}</p><ul>{result.files.map(file => <li key={file} className="mono runtime-wrap">{file}</li>)}</ul></details><p>{t('The package includes your recipe name and spatial bounds. Review them before sharing.')}</p>
      {desktopAvailable() ? <button className="button" disabled={busy} onClick={() => act('revealPackage')}><FolderOpen size={15}/>{t('Show package in folder')}</button> : <a className="button" href={`http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/package`} download={result.filename}><Download size={15}/>{t('Download ZIP')}</a>}
    </section>}
    {error && <div className="runtime-error" role="alert"><p>{t('The delivery package could not be prepared. Check the source files and retry.')}</p><details><summary>{t('Technical details')}</summary><p className="runtime-wrap">{t(error)}</p></details></div>}
  </div>;
}
