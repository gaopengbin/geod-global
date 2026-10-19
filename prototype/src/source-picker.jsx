import React, { useMemo, useRef, useState } from 'react';
import { Check, Clock3, Search, Star, History } from 'lucide-react';
import { Badge, Button, DialogClose, EmptyState, InfoDetails, Input, Modal, SegmentedControl, Select, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { SOURCE_DIRECTORY, SOURCE_GROUPS } from './source-directory.js';
import { SourceSeriesMark, sourceIdentity } from './source-identity.jsx';
import { sourcePresentation } from './catalog-source-panel.jsx';
import sourceDirectoryChinese from './locales/source-directory.zh-CN.js';
import publicDatasetChinese from './locales/public-dataset.zh-CN.js';
import pickerChinese from './locales/source-picker.zh-CN.js';
import './source-picker.css';
import { catalogProducts, channelLabel, readProductPreferences, writeProductPreferences } from './product-catalog.js';

const seriesAliases = {
  'sentinel-1':'哨兵一号 哨兵1 雷达 SAR', 'sentinel-2':'哨兵二号 哨兵2 光学 真彩色',
  landsat:'陆地卫星 陆地资源卫星', modis:'中分辨率 反射率', viirs:'夜间灯光 可见光 红外',
  'copernicus-dem':'哥白尼 数字高程 地形', srtm:'航天飞机 雷达 地形 高程',
  hls:'陆地卫星 哨兵 协同', naip:'航空 航拍 正射',
};
const statusMatches = (source, status) => status === 'all' || (status === 'connected' ? ['ready','auth'].includes(source.status) : source.status === status);
const identityFor = source => source.providerId ? sourceIdentity[source.providerId] : source;
const chinese = text => pickerChinese[text] || publicDatasetChinese[text] || sourceDirectoryChinese[text] || '';
function presentation(source,t,locale) {
  const identity=identityFor(source),field=key=>locale==='zh-CN'&&identity[key+'Zh']?identity[key+'Zh']:t(identity[key]||'');
  return {name:field('name'),detail:field('detail'),platform:field('platform'),accessibleName:source.providerId?t(sourcePresentation[source.providerId]?.[0]||identity.name):field('name')};
}
function searchable(source, t,locale) {
  const identity=identityFor(source);
  const display=presentation(source,t,locale);
  const fields=[source.id,source.name,source.nameZh,source.detail,source.detailZh,source.platform,source.platformZh,source.series,
    identity.name,identity.detail,identity.platform,display.name,display.detail,display.platform,display.accessibleName,SOURCE_GROUPS.find(group=>group.id===source.group)?.label];
  return [...fields,...fields.map(text=>text&&t(text)),...fields.map(chinese),seriesAliases[identity.mark] || ''].filter(Boolean).join(' ').normalize('NFKC').toLocaleLowerCase();
}

// This picker reads the product directory only. Source selection continues in
// the host's existing explorer or service form; discovery and downloads remain explicit.
export function SourcePickerDialog({ selectedSourceId, onChoose, onOpenEntry, onClose, returnFocusRef, disabled = false }) {
  const { t,locale } = useI18n();
  const [query,setQuery]=useState(''),[category,setCategory]=useState('all'),[status,setStatus]=useState('connected'),[leaving,setLeaving]=useState(false);
  const [preferences,setPreferences]=useState(()=>readProductPreferences());
  const [channelChoices,setChannelChoices]=useState({});
  const products=useMemo(()=>catalogProducts(),[]);
  const remember=value=>{setPreferences(value);writeProductPreferences(value);};
  const pending=useRef(null),finished=useRef(false),latest=useRef(null),searchRef=useRef(null);
  latest.current={disabled,onChoose,onOpenEntry,onClose};
  const terms=query.normalize('NFKC').toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  const matches=products.map(product=>({...product,channels:product.channels.filter(source=>statusMatches(source,status)&&terms.every(term=>searchable(source,t,locale).includes(term)||`${t(product.name||'')} ${t(product.detail||'')}`.toLocaleLowerCase().includes(term)))})).filter(product=>product.channels.length);
  const entries=category==='all'?matches:category==='recent'?preferences.recent.map(id=>matches.find(product=>product.id===id)).filter(Boolean):category==='favorites'?matches.filter(product=>preferences.favorites.includes(product.id)):matches.filter(source=>source.group===category);
  const counts={connected:products.filter(product=>product.channels.some(source=>['ready','auth'].includes(source.status))).length,verification:products.filter(product=>product.channels.some(source=>source.status==='verification')).length,all:products.length,planned:products.filter(product=>product.channels.some(source=>source.status==='planned')).length};
  const selectedId=selectedSourceId?.startsWith('dataset-')?selectedSourceId.slice(8):selectedSourceId;
  function finish(event) {
    event?.preventDefault();
    if(finished.current)return;
    returnFocusRef?.current?.focus();
    finished.current=true;
    const source=pending.current;
    pending.current=null;
    latest.current.onClose?.();
    if(source) queueMicrotask(()=>{
      if(latest.current.disabled)return;
      if(source.providerId)latest.current.onChoose?.(source.providerId);
      else latest.current.onOpenEntry?.(source.id);
    });
  }
  function choose(source) {
    if(latest.current.disabled||pending.current||finished.current||source.status==='planned'&&!source.partialAction||!source.action)return;
    pending.current=source;
    const product=products.find(product=>product.channels.some(channel=>channel.id===source.id));
    if(product)remember({...preferences,recent:[product.id,...preferences.recent.filter(id=>id!==product.id)].slice(0,8),preferred:{...preferences.preferred,[product.id]:source.id}});
    setLeaving(true);
  }
  function card(product) {
    const source=product.channels.find(source=>source.id===channelChoices[product.id])||product.channels.find(source=>source.id===selectedId)||product.channels.find(source=>source.id===preferences.preferred[product.id])||product.channels[0];
    const identity=identityFor(source),planned=source.status==='planned',blocked=planned&&!source.partialAction,current=source.id===selectedId;
    const label=source.verifiedOriginalProducts?.length?'Some originals available':source.partialAction?(source.id==='gebco'?'Regional elevation and global TID · whole elevation pending':source.id==='sinolc-1'?'Verification in progress':'Limited workflow · family pending'):source.status==='verification'?'Verification in progress':planned?'Pending integration':source.status==='auth'?'Catalog only':source.providerId?'Downloadable':source.action?.kind==='dataset'?'Product connector':source.action?.kind==='library'?'Local import':'Connection available';
    const display=presentation(source,t,locale);
    if(product.channels.length>1){display.name=t(product.name);display.accessibleName=display.name;display.detail=t(product.detail);display.platform=t('{count} channels',{count:product.channels.length});}
    const favorite=preferences.favorites.includes(product.id);
    const button=<Button variant="quiet" className="source-picker-select" aria-label={t('Choose {source}',{source:display.accessibleName})} aria-pressed={current}
      disabled={disabled||leaving||blocked||!source.action} onClick={()=>choose(source)}>
      <SourceSeriesMark mark={identity.mark}/>
      <span className="source-picker-copy"><strong title={display.name}>{display.name}</strong><span title={display.detail}>{display.detail}</span></span>
      <span className="source-picker-card-status"><Badge tone={source.status==='ready'?'accent':'neutral'}>{planned&&<Clock3 size={11} aria-hidden="true"/>}{t(label)}</Badge>{current&&<Check size={15} className="source-picker-current" aria-label={t('Current source')}/>}</span>
    </Button>;
    return <Surface as="article" key={product.id} className="source-picker-card" data-source-id={source.id} data-product-id={product.id} data-selected={current||undefined} data-status={source.status}>
      {blocked||!source.action?button:<DialogClose asChild>{button}</DialogClose>}
      <div className="source-picker-row-tools">
        {product.channels.length>1?<Select aria-label={t('Data channel')} value={source.id} disabled={disabled||leaving} onChange={event=>setChannelChoices(old=>({...old,[product.id]:event.target.value}))}>{product.channels.map(channel=><option key={channel.id} value={channel.id}>{channelLabel(channel)}</option>)}</Select>:<span className="source-picker-platform" title={display.platform}>{display.platform}</span>}
        <InfoDetails summary={t('Product and channel details')} closeLabel={t('Close')}><h3>{display.name}</h3><p>{display.detail}</p><p>{t(source.coverage||'')}</p><p>{t(source.period||'')}</p><p>{t(source.notes||'')}</p>{source.license&&<p>{t(source.license)}</p>}<p>{t('Selecting a source opens its existing workspace. Catalog-only sources have not passed original-download verification.')}</p></InfoDetails>
        <Button variant="quiet" size="icon" aria-label={t(favorite?'Unfavorite {source}':'Favorite {source}',{source:display.name})} aria-pressed={favorite} disabled={leaving} onClick={()=>remember({...preferences,favorites:favorite?preferences.favorites.filter(id=>id!==product.id):[...preferences.favorites,product.id]})}><Star size={15} fill={favorite?'currentColor':'none'}/></Button>
      </div>
    </Surface>;
  }
  return <Modal title={t('Choose a data source')} closeLabel={t('Close')} wide className="source-picker" onClose={finish} onCloseAutoFocus={finish}
    onOpenAutoFocus={event=>{event.preventDefault();searchRef.current?.focus();}}>
    <div className="source-picker-toolbar">
      <label className="source-picker-search"><Search size={17} aria-hidden="true"/><Input ref={searchRef} type="search" aria-label={t('Search data sources')} placeholder={t('Search names, platforms or products')} value={query} onChange={event=>setQuery(event.target.value)}/></label>
      <SegmentedControl aria-label={t('Source availability')} value={status} onValueChange={setStatus} items={[
        {value:'connected',label:t('Connected · {count}',{count:counts.connected})},
        {value:'verification',label:t('Verifying · {count}',{count:counts.verification})},
        {value:'all',label:t('All · {count}',{count:counts.all})},
        {value:'planned',label:t('Pending · {count}',{count:counts.planned})},
      ]}/>
    </div>
    <div className="source-picker-layout">
      <nav className="source-picker-categories" aria-label={t('Product categories')}>
        {[['recent','Recent products',History],['favorites','Favorite products',Star]].map(([id,label,Icon])=><Button key={id} variant="quiet" size="row" className="source-picker-category source-picker-collection" aria-current={category===id?'true':undefined} disabled={leaving} onClick={()=>setCategory(id)}><Icon size={15}/><span>{t(label)}</span><span>{preferences[id==='recent'?'recent':'favorites'].length}</span></Button>)}
        {[{id:'all',label:'All categories'},...SOURCE_GROUPS].map(group=><Button key={group.id} variant="quiet" size="row" disabled={leaving} className="source-picker-category" aria-current={category===group.id?'true':undefined} onClick={()=>setCategory(group.id)}>
          <span>{t(group.label)}</span><span>{group.id==='all'?matches.length:matches.filter(source=>source.group===group.id).length}</span>
        </Button>)}
      </nav>
      <div className="source-picker-results" aria-label={t('Data sources')}>
        <p className="source-picker-result-count" role="status">{t('{count} products',{count:entries.length})}</p>
        {entries.length?SOURCE_GROUPS.map(group=>{
          const sources=entries.filter(source=>source.group===group.id);
          return sources.length?<section key={group.id} className="source-picker-group" aria-label={t(group.label)}>
            <h3>{t(group.label)}<span>{sources.length}</span></h3><div className="source-picker-grid">{sources.map(card)}</div>
          </section>:null;
        }):<EmptyState className="source-picker-empty" title={t(category==='favorites'?'No favorite products yet.':category==='recent'?'No recent products yet.':'No sources match these filters.')} action={<Button variant="secondary" onClick={()=>{setQuery('');setCategory('all');setStatus('connected');}}>{t('Reset filters')}</Button>}/>}
      </div>
    </div>
    <p className="source-picker-note">{t('Select a product, then choose its delivery channel.')}</p>
  </Modal>;
}
