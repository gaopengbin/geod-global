import React, { useEffect, useRef, useState } from 'react';
import { FileJson, RefreshCw } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { runtimeRequest } from './runtime-client.js';

export function DiagnosticsPanel() {
  const { t } = useI18n();
  const [report, setReport] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(null);
  useEffect(() => () => request.current?.abort(), []);
  const read = async () => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true); setError('');
    try {
      const result = await runtimeRequest('diagnostics', {}, controller.signal);
      if (!controller.signal.aborted) setReport(result);
    }
    catch (e) { if (!controller.signal.aborted) setError(e.message); }
    finally { if (!controller.signal.aborted) setBusy(false); }
  };
  return <section className="runtime-section"><h2>{t('Local diagnostics')}</h2><p>{t('Preview a support report with versions, capabilities and task counts. No paths, coordinates, source URLs or personal names are included. Nothing is uploaded.')}</p><button className="button" disabled={busy} onClick={read}>{report ? <RefreshCw size={15}/> : <FileJson size={15}/>} {t(busy ? 'Reading diagnostics…' : 'Generate support report')}</button>{error && <p className="runtime-error" role="alert">{t('Local task service is offline')}</p>}{report && <label className="runtime-field">{t('Support report JSON')}<textarea className="processing-json-input mono" rows={16} readOnly value={JSON.stringify(report,null,2)} spellCheck={false}/></label>}</section>;
}
