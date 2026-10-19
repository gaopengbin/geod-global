import React from 'react';
import {ArrowLeft,LogOut} from 'lucide-react';
import {Badge,Button,PageHeader,Surface} from './ui/index.jsx';
import {AccountAvatar,useActiveIdentity} from './identity-ui.jsx';
import {MembershipPanel} from './official-ui.jsx';
import {useI18n} from './i18n.jsx';

export function AccountPage(){
 const {t}=useI18n(),{snapshot,loading,error,run}=useActiveIdentity(),user=snapshot.user;
 return <section className="account-page" aria-label={t('Your account')}>
  <PageHeader title={t('Your account')} description={t('Manage your Global account, membership and usage.')} actions={<Button asChild icon={ArrowLeft}><a href="#Home">{t('Back to workspace')}</a></Button>}/>
  {user?<><Surface className="account-profile"><AccountAvatar user={user}/><div><strong>{user.name}</strong><p>{user.email}</p><Badge tone="accent">{t('Signed in')}</Badge></div><Button icon={LogOut} disabled={loading} onClick={()=>run('logout')}>{t(loading?'Signing out…':'Sign out of this device')}</Button></Surface><MembershipPanel/></>:<Surface className="official-signin"><p>{t('Connect your own model. No GeoD Global account is required.')}</p><Button asChild variant="primary"><a href="#Home">{t('Continue to workspace')}</a></Button></Surface>}
  {error&&<p className="official-error" role="alert">{t(error)}</p>}
 </section>;
}
