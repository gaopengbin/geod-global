import React,{useEffect,useRef,useState} from 'react';
import {Button,Modal,Spinner} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {gscloudAuth} from './gscloud-client.js';

export function GSCloudAccountDialog({onClose,onConnected}) {
  const {t}=useI18n(),alive=useRef(true),started=useRef(false);
  const [opened,setOpened]=useState(false),[busy,setBusy]=useState(''),[error,setError]=useState('');
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;if(started.current)gscloudAuth('cancel').catch(()=>{});};},[]);
  const run=async operation=>{
    if(busy)return;setBusy(operation);setError('');
    try{
      const status=await gscloudAuth(operation);
      if(operation==='begin'){
        started.current=true;
        if(alive.current)setOpened(true);else gscloudAuth('cancel').catch(()=>{});
      }else if(alive.current){onConnected?.(status);onClose();}
    }catch(e){if(alive.current)setError(e.message);}
    finally{if(alive.current)setBusy('');}
  };
  return <Modal title={t('Connect GSCloud account')} closeLabel={t('Close')} closeDisabled={Boolean(busy)} onClose={()=>{if(!busy)onClose();}} className="gscloud-account">
    <p className="stac-help">{t('Sign in directly on the official GSCloud website in a separate software window. Use the same account and sign-in method as the website; GeoD does not collect your password or captcha.')}</p>
    <ol className="stac-help"><li>{t('Open the official sign-in window and complete website sign-in.')}</li><li>{t('After the website shows your account, return here and verify the connection.')}</li></ol>
    {opened&&<p role="status" className="stac-help">{t('Waiting for website sign-in. Only a verified session is saved in Windows Credential Manager; original delivery remains a separate check.')}</p>}
    {error&&<p className="stac-error" role="alert">{t(error)}</p>}
    <div className="gscloud-account-actions">
      <Button variant={opened?'secondary':'primary'} disabled={Boolean(busy)} onClick={()=>run('begin')}>{busy==='begin'&&<Spinner size={14}/>} {t(opened?'Show official sign-in window':'Open official sign-in window')}</Button>
      {opened&&<Button variant="primary" disabled={Boolean(busy)} onClick={()=>run('finish')}>{busy==='finish'&&<Spinner size={14}/>} {t('Verify website sign-in')}</Button>}
    </div>
  </Modal>;
}
