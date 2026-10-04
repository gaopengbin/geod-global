import React, { useContext, useEffect, useState } from 'react';
import { CheckCircle2, RefreshCw } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { runtimeRequest } from './runtime-client.js';
import { Badge, Button, Input, Select, Spinner, Surface } from './ui/index.jsx';
import './proxy.css';

const initial = { mode: 'system', url: '' };
const normalize = value => ({ mode: value.mode || 'system', url: value.url || '' });

export function ProxySettingsPanel() {
  const { health } = useContext(RuntimeContext);
  const { t, number } = useI18n();
  const [saved, setSaved] = useState(initial);
  const [draft, setDraft] = useState(initial);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState('');
  const [message, setMessage] = useState(null);
  const connected = Boolean(health);

  useEffect(() => {
    if (!connected) { setLoading(false); return; }
    let current = true;
    setLoading(true);
    runtimeRequest('proxy').then(value => {
      if (current) { const next = normalize(value); setSaved(next); setDraft(next); setMessage(null); }
    }).catch(error => { if (current) setMessage({ type: 'error', text: error.message }); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [connected]);

  const payload = { mode: draft.mode, url: draft.mode === 'custom' ? draft.url.trim() : null };
  const dirty = draft.mode !== saved.mode || (draft.mode === 'custom' && draft.url.trim() !== saved.url);
  const run = async operation => {
    setBusy(operation); setMessage(null);
    try {
      const result = await runtimeRequest(operation, payload);
      if (operation === 'saveProxy') {
        const next = normalize(result);
        setSaved(next); setDraft(next);
        setMessage({ type: 'success', text: t('Proxy settings saved. New downloads use this route.') });
      } else {
        setMessage({ type: 'success', text: t('Connection succeeded in {ms} ms.', { ms: number(result.elapsedMs) }) });
      }
    } catch (error) {
      setMessage({ type: 'error', text: operation === 'testProxy' ? t('Connection test failed. Check your network or proxy settings.') : error.message });
    } finally {
      setBusy('');
    }
  };

  return <Surface as="section" className="proxy-settings" aria-label={t('Source download proxy')}>
    <div className="proxy-heading"><div className="proxy-title"><h2>{t('Source download proxy')}</h2>{connected && !loading && <Badge>{t(saved.mode === 'system' ? 'Follow system' : saved.mode === 'direct' ? 'Direct' : 'Custom proxy')}</Badge>}</div><p>{t('Choose how GeoD connects to original files and account services.')}</p></div>
    {!connected ? <p className="proxy-note">{t('Connect the local task service to change proxy settings.')}</p> : loading ? <p className="proxy-note" role="status"><Spinner size={15}/>{t('Loading proxy settings…')}</p> : <>
      <div className="proxy-fields">
        <label>{t('Connection mode')}<Select aria-label={t('Connection mode')} value={draft.mode} onChange={event => { setDraft(previous => ({ ...previous, mode: event.target.value })); setMessage(null); }}>
          <option value="system">{t('Follow system')}</option>
          <option value="direct">{t('Direct')}</option>
          <option value="custom">{t('Custom proxy')}</option>
        </Select><small>{t(draft.mode === 'system' ? 'Use the operating system proxy and proxy environment variables.' : draft.mode === 'direct' ? 'Connect to the source without a proxy.' : 'Route source downloads through the address below.')}</small></label>
        {draft.mode === 'custom' && <label>{t('Proxy address')}<Input value={draft.url} onChange={event => { setDraft(previous => ({ ...previous, url: event.target.value })); setMessage(null); }} placeholder="http://127.0.0.1:10808" spellCheck={false} autoComplete="off"/><small>{t('Use an HTTP(S) or SOCKS5 address with a port; credentials are not stored.')}</small></label>}
      </div>
      <div className="proxy-actions"><Button onClick={() => run('testProxy')} disabled={Boolean(busy)}>{busy === 'testProxy' ? <Spinner size={15}/> : <RefreshCw size={15}/>} {t('Test connection')}</Button><Button variant="primary" onClick={() => run('saveProxy')} disabled={Boolean(busy) || !dirty}>{busy === 'saveProxy' && <Spinner size={15}/>} {t('Save proxy settings')}</Button></div>
      {message && <p className={`proxy-message proxy-message-${message.type}`} role="status">{message.type === 'success' && <CheckCircle2 size={16}/>}<span>{message.type === 'error' ? t(message.text) : message.text}</span></p>}
      <p className="proxy-note">{t('This setting applies to new source-file downloads. Active transfers keep their current connection; catalog and map requests follow the browser network settings.')}</p>
    </>}
  </Surface>;
}
