import React,{lazy,Suspense,useEffect,useRef,useState} from 'react';
import {Box,Download,FolderOpen,Info,Plus,RefreshCw,Eye} from 'lucide-react';
import {Badge,Button,Disclosure,EmptyState,Input,Modal,SegmentedControl,Spinner,Surface} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
import {desktopAvailable} from './runtime-client.js';
import {threeDRequest,threeDExample,openThreeDArchive,exportThreeD} from './three-d-client.js';
import './three-d.css';
const Viewer=lazy(()=>import('./three-d-viewer.jsx'));
const size=n=>n<1048576?`${(n/1024).toFixed(1)} KiB`:`${(n/1048576).toFixed(1)} MiB`;
export function ThreeDSourceDialog({onClose,onSaved=()=>{}}){
  const {t,number}=useI18n();const[mode,setMode]=useState('remote'),[url,setUrl]=useState(''),[name,setName]=useState(''),[license,setLicense]=useState(''),[attribution,setAttribution]=useState(''),[licenseUrl,setLicenseUrl]=useState(''),[confirmed,setConfirmed]=useState(false),[discovery,setDiscovery]=useState(null),[busy,setBusy]=useState(''),[error,setError]=useState('');const file=useRef(null);
  const rights={license:license.trim(),attribution:attribution.trim(),licenseUrl:licenseUrl.trim()||null,permissionConfirmed:confirmed};const ready=name.trim()&&rights.license&&rights.attribution&&confirmed;
  const run=async(kind,action)=>{setBusy(kind);setError('');try{await action();}catch(e){setError(e.message);}finally{setBusy('');}};const saved=p=>{if(p){onSaved(p);onClose();}};
  const importFile=f=>run('import',async()=>saved(await openThreeDArchive(f,{name:name.trim(),rights})));
  return <Modal title={t('Add 3D assets')} description={t('Save a complete scene with its models and textures for offline use.')} className="three-d-source-dialog" onClose={onClose} closeDisabled={Boolean(busy)} closeLabel={t('Close')}>
    <SegmentedControl aria-label={t('3D import method')} value={mode} onValueChange={v=>{if(!busy){setMode(v);setError('');}}} items={[{value:'remote',label:t('Public source'),disabled:Boolean(busy)},{value:'local',label:t('Local scene'),disabled:Boolean(busy)}]}/>
    <div className="three-d-fields">
      {mode==='remote'&&<label><span>{t('3D scene URL')}</span><Input aria-label={t('3D scene URL')} disabled={Boolean(busy)} maxLength={2048} placeholder="https://…/tileset.json" value={url} onChange={e=>{setUrl(e.target.value);setDiscovery(null);}}/></label>}
      <label><span>{t('Asset name')}</span><Input aria-label={t('Asset name')} disabled={Boolean(busy)} maxLength={120} value={name} onChange={e=>setName(e.target.value)}/></label>
      <div className="three-d-field-pair"><label><span>{t('License or permission')}</span><Input aria-label={t('License or permission')} disabled={Boolean(busy)} maxLength={120} placeholder="CC0-1.0" value={license} onChange={e=>setLicense(e.target.value)}/></label><label><span>{t('Attribution')}</span><Input aria-label={t('Attribution')} disabled={Boolean(busy)} maxLength={1000} value={attribution} onChange={e=>setAttribution(e.target.value)}/></label></div>
      <Disclosure summary={t('License reference · optional')}><label><span>{t('License URL')}</span><Input aria-label={t('License URL')} disabled={Boolean(busy)} maxLength={2048} placeholder="https://…" value={licenseUrl} onChange={e=>setLicenseUrl(e.target.value)}/></label></Disclosure>
      <label className="three-d-permission"><Input type="checkbox" aria-label={t('I have permission to save and use these assets')} disabled={Boolean(busy)} checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/><span>{t('I have permission to save and use these assets')}</span></label>
    </div>
    {discovery&&mode==='remote'&&<Surface variant="inset" className="three-d-discovery"><Box size={20}/><div><strong>{t('Entry verified')}</strong><span>{t('{count} direct dependencies',{count:number(discovery.directDependencies)})} · {size(discovery.bytes)}</span></div></Surface>}
    {mode==='local'&&<p className="three-d-help">{t(desktopAvailable()?'Choose a tileset, glTF or GLB entry. Relative dependencies in its folder are copied together.':'In browser preview, import a GeoD 3D export ZIP. The desktop app also imports local tilesets and models.')}</p>}
    {error&&<p role="alert" className="three-d-error">{t(error)}</p>}{busy&&<div role="status" className="three-d-loading"><Spinner/>{t(busy==='discover'?'Inspecting 3D source':'Saving models and textures')}</div>}
    <div className="three-d-dialog-actions">
      {mode==='remote'?<>{discovery?<Button primary icon={Download} disabled={Boolean(busy)||!ready} onClick={()=>run('save',async()=>saved(await threeDRequest('save',{url:discovery.url,discoverySha256:discovery.sha256,name:name.trim(),rights})))}>{t('Save complete scene')}</Button>:<Button primary icon={Eye} disabled={Boolean(busy)||!url.trim()} onClick={()=>run('discover',async()=>setDiscovery(await threeDRequest('discover',{url:url.trim()})))}>{t('Inspect source')}</Button>}<Button variant="quiet" disabled={Boolean(busy)} onClick={()=>{setUrl(threeDExample.url);setName(threeDExample.name);setLicense(threeDExample.rights.license);setAttribution(threeDExample.rights.attribution);setLicenseUrl(threeDExample.rights.licenseUrl);setConfirmed(false);setDiscovery(null);}}>{t('Use public 3D demo')}</Button></>:<><Input ref={file} type="file" accept=".zip" className="three-d-file-input" aria-label={t('GeoD 3D export ZIP')} onChange={e=>{const f=e.target.files?.[0];e.target.value='';if(f)importFile(f);}}/><Button primary icon={FolderOpen} disabled={Boolean(busy)||!ready} onClick={()=>desktopAvailable()?importFile(null):file.current?.click()}>{t('Choose local scene')}</Button></>}
    </div>
    <Disclosure className="three-d-support" summary={t('Supported scenes and limits')}><p>{t('Explicit 3D Tiles 1.0 / 1.1, glTF 2.0, GLB and b3dm. Up to 128 MiB and 256 resources; each resource up to 32 MiB.')}</p><p>{t('Implicit tiling, point-cloud and instanced tile formats are not supported yet. Missing dependencies prevent saving.')}</p><p>{t('The whole source is saved. Region selection and geometry clipping are separate capabilities and are not applied here.')}</p><p>{t('The public demo contains generated sample buildings, not production geographic data.')}</p></Disclosure>
  </Modal>;
}
export function ThreeDSourceDetails({asset}){
  const{t,number}=useI18n();let upstream=asset.importedFrom;
  for(let depth=1;upstream?.previous&&depth<8;depth++)upstream=upstream.previous;
  return <dl className="three-d-details">
    <dt>{t('Source')}</dt><dd>{asset.source}</dd>
    <dt>{t('Scope')}</dt><dd>{t('Complete source scene')}</dd>
    <dt>{t('Resources')}</dt><dd>{number(asset.resources.length)} · {size(asset.bytes)}</dd>
    <dt>{t('License or permission')}</dt><dd>{asset.rights.license}</dd>
    <dt>{t('Attribution')}</dt><dd>{asset.rights.attribution}</dd>
    {asset.rights.licenseUrl&&<><dt>{t('License URL')}</dt><dd><a href={asset.rights.licenseUrl} target="_blank" rel="noreferrer">{asset.rights.licenseUrl}</a></dd></>}
    <dt>{t('Source receipt SHA-256')}</dt><dd className="mono">{asset.receiptSha256}</dd>
    {upstream&&<>
      <dt>{t('Original source')}</dt><dd>{upstream.source}</dd>
      <dt>{t('Original license or permission')}</dt><dd>{upstream.rights.license}</dd>
      <dt>{t('Original attribution')}</dt><dd>{upstream.rights.attribution}</dd>
      {upstream.rights.licenseUrl&&<><dt>{t('Original license URL')}</dt><dd><a href={upstream.rights.licenseUrl} target="_blank" rel="noreferrer">{upstream.rights.licenseUrl}</a></dd></>}
      <dt>{t('Original receipt SHA-256')}</dt><dd className="mono">{upstream.sourceReceiptSha256}</dd>
    </>}
  </dl>;
}
function ThreeDAssetCard({asset,onView,onExport,busy}){const{t,number}=useI18n(),[details,setDetails]=useState(null);return <Surface className="three-d-asset"><div className="three-d-asset-body"><div className="three-d-asset-icon"><Box size={26}/></div><div className="three-d-asset-content"><h3 title={asset.name}>{asset.name}</h3><div className="three-d-asset-meta"><span>{t('{count} resources',{count:number(asset.resources.length)})}</span><span>{size(asset.bytes)}</span><Badge title={asset.rights.license}>{asset.rights.license}</Badge></div><div className="three-d-asset-actions"><Button size="icon" aria-label={t('View 3D scene')} tooltip={t('View 3D scene')} onClick={onView}><Eye size={16}/></Button><Button size="icon" aria-label={t('Export offline 3D package')} tooltip={t('Export offline 3D package')} disabled={busy} onClick={onExport}>{busy?<Spinner/>:<Download size={16}/>}</Button><Disclosure icon={Info} summary={t('3D source details')} contentContainer={details}><ThreeDSourceDetails asset={asset}/></Disclosure></div></div></div><div ref={setDetails} className="three-d-card-details"/></Surface>;}
export function ThreeDLibrary(){const{t,number}=useI18n();const[assets,setAssets]=useState([]),[loading,setLoading]=useState(true),[error,setError]=useState(''),[adding,setAdding]=useState(false),[viewing,setViewing]=useState(null),[busy,setBusy]=useState('');
  const refresh=async(signal)=>{setLoading(true);setError('');try{const ps=await threeDRequest('list',{},signal);setAssets(ps.sort((a,b)=>b.createdAt.localeCompare(a.createdAt)));}catch(e){if(!signal?.aborted)setError(e.message);}finally{if(!signal?.aborted)setLoading(false);}};
  useEffect(()=>{const a=new AbortController();refresh(a.signal);return()=>a.abort();},[]);
  const exportAsset=async(asset)=>{setBusy(asset.id);setError('');try{await exportThreeD(asset.id);}catch(e){setError(e.message);}finally{setBusy('');}};
  return <section className="three-d-library" aria-label={t('3D assets')}><div className="three-d-toolbar"><Button primary size="sm" icon={Plus} onClick={()=>setAdding(true)}>{t('Add 3D assets')}</Button><Button size="icon" aria-label={t('Refresh 3D assets')} tooltip={t('Refresh 3D assets')} disabled={loading} onClick={()=>refresh()}><RefreshCw size={16}/></Button><span>{t('{count} scenes',{count:number(assets.length)})}</span></div>{error&&<p role="alert" className="three-d-error">{t(error)}</p>}{loading?<div className="three-d-loading"><Spinner/>{t('Loading 3D assets')}</div>:assets.length?<div className="three-d-grid">{assets.map(asset=><ThreeDAssetCard key={asset.id} asset={asset} onView={()=>setViewing(asset)} onExport={()=>exportAsset(asset)} busy={busy===asset.id}/>)}</div>:!error&&<EmptyState icon={Box} title={t('No saved 3D scenes')} description={t('Add public or owned 3D assets, then inspect and export them offline.')}/>}{adding&&<ThreeDSourceDialog onClose={()=>setAdding(false)} onSaved={()=>refresh()}/>}{viewing&&<Modal wide className="three-d-viewer-dialog" title={viewing.name} onClose={()=>setViewing(null)} closeLabel={t('Close')}><Suspense fallback={<div className="three-d-loading"><Spinner/>{t('Loading 3D viewer')}</div>}><Viewer asset={viewing}/></Suspense></Modal>}</section>;
}
export function ThreeDSourcesPanel(){const{t}=useI18n();const[open,setOpen]=useState(false);return <><Surface className="settings-panel three-d-settings"><div><h2>{t('3D sources')}</h2><p className="settings-help">{t('Save public or owned tilesets with their complete dependencies.')}</p></div><Button size="sm" icon={Box} onClick={()=>setOpen(true)}>{t('Add 3D assets')}</Button></Surface>{open&&<ThreeDSourceDialog onClose={()=>setOpen(false)} onSaved={()=>{location.hash=encodeURIComponent('My Data')+'?view=3d';}}/>}</>;}
