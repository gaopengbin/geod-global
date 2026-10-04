import React, { useContext, useEffect, useRef, useState } from 'react';
import { ExternalLink, KeyRound, LogOut, RefreshCw, ShieldCheck } from 'lucide-react';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { desktopAvailable, runtimeRequest } from './runtime-client.js';
import { Badge, Button, Input, Modal, Spinner, Surface } from './ui/index.jsx';
import './accounts.css';

const providers = [
  { id: 'nasa-earthdata', name: 'NASA Earthdata', scope: 'HLS L30 / SRTMGL1 v003 / VIIRS 09A1 v002', website: 'https://urs.earthdata.nasa.gov/profile', method: 'Earthdata user token' },
  { id: 'copernicus', name: 'Copernicus Data Space', scope: 'Sentinel-2', website: 'https://dataspace.copernicus.eu/', method: 'Copernicus account' },
];
const labels = { 'not-connected': 'Not authorized', saved: 'Saved · check access', connected: 'Authorization verified', expired: 'Authorization expired', 'storage-error': 'Credential storage unavailable', unsupported: 'Secure storage unsupported' };
const safeErrors = new Set([
  'Account service could not be reached. Check the network or source download proxy and retry.',
  'Authorization was rejected. Check the credentials, token expiry or two-step verification code.',
  'Secure credential storage is unavailable. Authorization was not saved. Check Windows Credential Manager and retry.',
  'Connect this data source in Settings before downloading protected files.',
  'Enter an Earthdata user token without spaces. Create it on the official Earthdata Login website.',
  'Enter a Copernicus username and password; the optional two-step code must contain six digits.',
  'Too many authorization attempts. Wait a moment before retrying.',
  'Account service returned an invalid response. Retry later.',
  'The Earthdata token is invalid or expired. Create a new user token on the official website.',
]);
const emptyDraft = () => ({ username: '', password: '', token: '', totp: '' });
function issue(error, t) {
  return { text: t(safeErrors.has(error?.message) ? error.message : error?.name === 'TimeoutError' ? 'Authorization timed out. Check the network and retry.' : 'Could not complete authorization. Retry or reconnect this account.'),
    reference: `AUTH-${Date.now().toString(36).toUpperCase()}` };
}
function accountTarget() {
  const id = new URLSearchParams(location.hash.split('?')[1] || '').get('account');
  return providers.find(provider => provider.id === id) || null;
}

export function ProviderAccountsPanel() {
  const { t, date } = useI18n();
  const { health } = useContext(RuntimeContext) || {};
  const native = desktopAvailable();
  const [accounts, setAccounts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [busy, setBusy] = useState('');
  const [target, setTarget] = useState(() => native ? accountTarget() : null);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    const controller = new AbortController();
    if (!health) { setLoading(false); return () => controller.abort(); }
    setLoading(true);
    runtimeRequest('accounts', null, controller.signal).then(value => {
      if (!controller.signal.aborted) { setAccounts(value); setError(null); }
    }).catch(failure => { if (!controller.signal.aborted) setError(issue(failure, t)); })
      .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [Boolean(health)]);
  const update = value => setAccounts(previous => [...previous.filter(account => account.provider !== value.provider), value]);
  const run = async (operation, provider) => {
    if (busy) return;
    setBusy(provider); setError(null);
    try { const value = await runtimeRequest(operation, { provider }); if (alive.current) update(value); }
    catch (failure) {
      if (alive.current) {
        setError(issue(failure, t));
        const states = await runtimeRequest('accounts').catch(() => null);
        if (states && alive.current) setAccounts(states);
      }
    } finally { if (alive.current) setBusy(''); }
  };
  return <Surface as="section" className="provider-accounts" aria-label={t('Data source accounts')}>
    <div className="accounts-heading"><div><h2>{t('Data source accounts')}</h2><p>{t('Authorize access to protected original products. Public catalogs remain available without signing in.')}</p></div><ShieldCheck size={20} aria-hidden="true"/></div>
    {!native && <p className="accounts-note">{t('Open the desktop app to manage data source authorization.')}</p>}
    <p className="accounts-note">{t('Account setup is available. Protected original downloads are pending real-account verification and are unavailable in this candidate.')}</p>
    {loading && <p className="accounts-note" role="status"><Spinner size={15}/>{t('Loading account status…')}</p>}
    <div className="accounts-grid">{providers.map(provider => {
      const account = accounts.find(value => value.provider === provider.id);
      const state = account?.status || 'not-connected';
      const canAct = native && Boolean(health) && !loading && !busy && state !== 'unsupported';
      return <article className="account-card" key={provider.id}>
        <div className="account-card-heading"><KeyRound size={18} aria-hidden="true"/><div><h3>{provider.name}</h3><span>{provider.scope}</span></div></div>
        <div className="account-state"><Badge tone={state === 'connected' ? 'accent' : state === 'expired' || state === 'storage-error' ? 'red' : 'neutral'}>{t(labels[state] || 'Not authorized')}</Badge>{account?.expiresAt && <span>{t('Expires {date}', { date: date(account.expiresAt) })}</span>}</div>
        <div className="account-actions"><Button size="sm" disabled={!canAct} onClick={() => { setError(null); setTarget(provider); }}><KeyRound size={15}/>{t(state === 'not-connected' ? 'Connect account' : 'Reconnect')}</Button>
          {account && state !== 'not-connected' && <><Button size="icon" aria-label={t('Check {provider} access', { provider: provider.name })} tooltip={t('Check access')} disabled={!canAct} onClick={() => run('verifyAccount', provider.id)}>{busy === provider.id ? <Spinner size={15}/> : <RefreshCw size={15}/>}</Button><Button size="icon" aria-label={t('Forget {provider} authorization', { provider: provider.name })} tooltip={t('Forget authorization on this device')} disabled={!canAct} onClick={() => run('disconnectAccount', provider.id)}><LogOut size={15}/></Button></>}
          <Button asChild size="icon" aria-label={t('Open {provider} website', { provider: provider.name })} tooltip={t('Official website')}><a href={provider.website} target="_blank" rel="noreferrer"><ExternalLink size={15}/></a></Button>
        </div>
      </article>;
    })}</div>
    <p className="accounts-note"><ShieldCheck size={14}/>{t('Tokens are saved in Windows Credential Manager. Passwords and two-step codes are never saved.')}</p>
    {error && <p className="account-error" role="alert">{error.text}<small>{t('Reference: {id}', { id: error.reference })}</small></p>}
    {target && <AccountConnectDialog provider={target} onClose={() => setTarget(null)} onConnected={value => { update(value); setError(null); setTarget(null); }}/>} 
  </Surface>;
}

function AccountConnectDialog({ provider, onClose, onConnected }) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(emptyDraft);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const submitting = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const nasa = provider.id === 'nasa-earthdata';
  const set = key => event => { setDraft(previous => ({ ...previous, [key]: event.target.value })); setError(null); };
  const submit = async event => {
    event.preventDefault();
    if (submitting.current) return;
    if ((nasa && (!draft.token.trim() || /\s/.test(draft.token.trim()))) || (!nasa && (!draft.username.trim() || !draft.password || (draft.totp && !/^\d{6}$/.test(draft.totp))))) {
      setError(issue(new Error(nasa ? 'Enter an Earthdata user token without spaces. Create it on the official Earthdata Login website.' : 'Enter a Copernicus username and password; the optional two-step code must contain six digits.'), t));
      return;
    }
    submitting.current = true; setBusy(true); setError(null);
    const payload = nasa ? { provider: provider.id, token: draft.token.trim() } : { provider: provider.id, username: draft.username.trim(), password: draft.password, totp: draft.totp };
    // Keep only the non-sensitive username if a retry is needed.
    setDraft(previous => ({ ...emptyDraft(), username: previous.username }));
    try { const value = await runtimeRequest('connectAccount', payload); if (alive.current) onConnected(value); }
    catch (failure) { if (alive.current) setError(issue(failure, t)); }
    finally { payload.password = ''; payload.token = ''; payload.totp = ''; submitting.current = false; if (alive.current) setBusy(false); }
  };
  return <Modal title={t('Authorize {provider}', { provider: provider.name })} description={t(nasa ? 'Create a user token on Earthdata Login, then verify it here.' : 'Sign in with your Copernicus account. Add the current six-digit code if two-step verification is enabled.')} className="account-dialog" closeLabel={t('Close')} closeDisabled={busy} onClose={onClose}>
    <form className="account-form" onSubmit={submit}>
      <Button asChild size="sm" className="account-official"><a href={provider.website} target="_blank" rel="noreferrer"><ExternalLink size={15}/>{t(nasa ? 'Manage Earthdata tokens' : 'Register or manage Copernicus account')}</a></Button>
      {nasa ? <label>{t('Earthdata user token')}<Input type="password" value={draft.token} onChange={set('token')} autoComplete="off" spellCheck={false} maxLength={8000} disabled={busy} autoFocus required/></label> : <>
        <label>{t('Username or email')}<Input value={draft.username} onChange={set('username')} autoComplete="username" maxLength={254} disabled={busy} autoFocus required/></label>
        <label>{t('Password')}<Input type="password" value={draft.password} onChange={set('password')} autoComplete="off" maxLength={1024} disabled={busy} required/></label>
        <label>{t('Two-step code · optional')}<Input type="password" inputMode="numeric" value={draft.totp} onChange={set('totp')} autoComplete="off" maxLength={6} disabled={busy}/></label>
      </>}
      <p className="accounts-note">{t(nasa ? 'Verification checks token validity. Product access may also require accepting the provider’s terms on Earthdata Login.' : 'Your password is used once to obtain authorization. Only the refresh token is saved on this device.')}</p>
      {error && <p className="account-error" role="alert">{error.text}<small>{t('Reference: {id}', { id: error.reference })}</small></p>}
      <div className="account-form-actions"><Button disabled={busy} onClick={onClose}>{t('Cancel')}</Button><Button variant="primary" type="submit" disabled={busy}>{busy ? <Spinner size={15}/> : <ShieldCheck size={15}/>} {t(busy ? 'Verifying authorization…' : 'Verify and save')}</Button></div>
    </form>
  </Modal>;
}
