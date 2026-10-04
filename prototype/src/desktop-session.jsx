import React, { useEffect, useState } from 'react';
import { Power, PanelRightClose, RotateCw } from 'lucide-react';
import { Button, Modal, Spinner } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { desktopAvailable, runtimeRequest } from './runtime-client.js';
import { hideDesktopWindow, quitDesktop } from './desktop-frame.js';
import './desktop-session.css';

export function DesktopSessionControl() {
  const { t, number } = useI18n();
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  const [count, setCount] = useState(null);
  const [error, setError] = useState('');
  const [pending, setPending] = useState('');
  useEffect(() => {
    if (!open) return;
    const abort = new AbortController();
    setCount(null);
    setError('');
    runtimeRequest('list', undefined, abort.signal).then(jobs => {
      if (!Array.isArray(jobs) || jobs.some(job => !['queued', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.status))) {
        throw new Error('Could not read task status. Try again.');
      }
      if (!abort.signal.aborted) setCount(jobs.filter(job => ['queued', 'running'].includes(job.status)).length);
    }).catch(failure => { if (!abort.signal.aborted) setError(String(failure?.message || failure)); });
    return () => abort.abort();
  }, [open, revision]);
  if (!desktopAvailable()) return null;
  const act = async action => {
    setPending(action);
    setError('');
    try {
      if (action === 'hide') {
        await hideDesktopWindow();
        setOpen(false);
        setPending('');
      } else {
        await quitDesktop();
        // Keep the busy state until native worker cleanup closes this window.
      }
    } catch (failure) {
      setError(String(failure?.message || failure));
      setPending('');
    }
  };
  return <>
    <Button variant="secondary" icon={Power} onClick={() => setOpen(true)}>{t('Exit app')}</Button>
    {open && <Modal title={t('Exit GeoD Global?')} closeLabel={t('Close')} onClose={() => setOpen(false)} closeDisabled={Boolean(pending)}
      description={t('Closing the window keeps tasks running. Exiting stops unfinished tasks; you can retry them after reopening. Projects and completed files are kept.')}>
      <div className="desktop-session-summary" role="status" aria-live="polite">
        {pending === 'quit' ? <><Spinner />{t('Saving task state and exiting…')}</>
          : count === null && !error ? <><Spinner />{t('Checking active tasks…')}</>
            : count !== null ? <>{t('Running or queued tasks: {count}', { count: number(count) })}</> : null}
      </div>
      {error && <div className="desktop-session-error" role="alert"><p>{t('Could not read or control the desktop session. Try again.')}</p><small>{error}</small>
        {count === null && <Button variant="secondary" icon={RotateCw} onClick={() => setRevision(value => value + 1)}>{t('Retry')}</Button>}
      </div>}
      <div className="desktop-session-actions">
        <Button variant="secondary" icon={PanelRightClose} disabled={Boolean(pending)} onClick={() => act('hide')}>{t('Keep running in tray')}</Button>
        <Button variant="primary" icon={Power} disabled={count === null || Boolean(pending)} onClick={() => act('quit')}>{t('Exit and stop tasks')}</Button>
      </div>
    </Modal>}
  </>;
}
