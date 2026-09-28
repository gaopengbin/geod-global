import React, { useEffect, useRef, useState } from 'react';
import { Package, Download, FolderOpen } from 'lucide-react';
import { desktopAvailable, formatBytes, runtimeRequest } from './runtime-client.js';
import { useI18n } from './i18n.jsx';
import { Button, Disclosure, Spinner, Surface } from './ui/index.jsx';

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
  return <div className="artifact-export"><Button disabled={busy} onClick={() => act('package')}>{busy ? <Spinner size={15}/> : <Package size={15}/>} {t(busy ? 'Preparing package…' : 'Prepare delivery package')}</Button>
    {result && <Surface as="section" className="artifact-package" aria-label={t('Verified delivery package')}><strong>{t('Verified delivery package')}</strong><p>{t('GeoTIFF, provenance, recipe and checksums are saved together.')} {formatBytes(result.bytes, locale)}</p><p className="mono runtime-wrap">{result.path}</p><Disclosure summary={t('Package checksum and contents')}><p className="mono runtime-wrap">SHA-256 · {result.sha256}</p><ul>{result.files.map(file => <li key={file} className="mono runtime-wrap">{file}</li>)}</ul></Disclosure><p>{t('The package includes your recipe name and spatial bounds. Review them before sharing.')}</p>
      {desktopAvailable() ? <Button disabled={busy} onClick={() => act('revealPackage')}><FolderOpen size={15}/>{t('Show package in folder')}</Button> : <Button asChild><a href={`http://127.0.0.1:4318/jobs/${encodeURIComponent(job.id)}/package`} download={result.filename}><Download size={15}/>{t('Download ZIP')}</a></Button>}
    </Surface>}
    {error && <Surface className="runtime-error" role="alert"><p>{t('The delivery package could not be prepared. Check the source files and retry.')}</p><Disclosure summary={t('Technical details')}><p className="runtime-wrap">{t(error)}</p></Disclosure></Surface>}
  </div>;
}
