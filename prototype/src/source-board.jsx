import React, { useState } from 'react';
import { ArrowUpRight, Clock3, Globe2 } from 'lucide-react';
import { Badge, Button, EmptyState, Select, Surface } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { PROVIDERS, SOURCE_SERIES, canDisplayImagery } from './providers.js';
import { catalogPreviewKind } from './catalog-preview.js';
import { sourcePresentation } from './catalog-source-panel.jsx';
import { SourceSeriesMark, sourceIdentity } from './source-identity.jsx';
import { DIRECTORY_FAMILIES, SOURCE_DIRECTORY, SOURCE_GROUPS } from './source-directory.js';
import './source-board.css';

const categories = [{id:'all',label:'All types'}, {id:'imagery',label:'Imagery'}, {id:'radar',label:'Radar'},
  {id:'thematic',label:'Thematic rasters'},
  {id:'elevation',label:'Elevation'}, {id:'maps',label:'Map images'}, {id:'vectors',label:'Vector data'},
  {id:'tiles',label:'Offline tiles'}, {id:'local',label:'Local files'}];
const statuses = [{id:'all',label:'All statuses'}, {id:'ready',label:'Available'},
  {id:'auth',label:'Authorization required'}, {id:'planned',label:'Pending integration'}];
const accessLabels = {public:'Public download',registration:'Registration required',application:'Application required',samples:'Limited free samples',mixed:'Dataset-specific access'};

// Display inventory only. Selecting a service opens its form; pending entries
// have no action. Provider availability still follows the shared release policy.
export function SourceBoard({ onChoose, onOpenEntry }) {
  const { t } = useI18n();
  const [filter, setFilter] = useState('all');
  const [series, setSeries] = useState('all');
  const [status, setStatus] = useState('all');
  const sources = SOURCE_DIRECTORY.filter(source => (filter === 'all' || (filter === 'local' ? source.series === 'local' : source.category === filter))
    && (series === 'all' || source.series === series) && (status === 'all' || source.status === status));
  const renderCard = source => {
    const provider = PROVIDERS.find(provider => provider.id === source.providerId);
    const identity = provider ? sourceIdentity[provider.id] : source;
    const pending = source.status === 'planned';
    const released = source.status === 'ready';
    const local = source.action?.kind === 'library';
    const [accessibleName] = provider ? sourcePresentation[provider.id] || [provider.name] : [source.name];
    const preview = provider && (catalogPreviewKind(provider) ? 'Online preview' : canDisplayImagery(provider) ? 'Load to preview' : 'Footprints only');
    const content = <>
      <span className="source-board-card-heading"><SourceSeriesMark mark={identity.mark}/><span className="source-board-platform" title={t(identity.platform)}>{t(identity.platform)}</span>{!pending && <ArrowUpRight size={15} className="source-board-arrow" aria-hidden="true"/>}</span>
      <span className="source-board-copy"><strong>{t(identity.name)}</strong><span className="source-board-detail">{t(identity.detail)}</span></span>
      <span className="source-board-capabilities">
        <Badge tone={pending?'neutral':released?'accent':'neutral'}>{pending && <Clock3 size={11} aria-hidden="true"/>}{t(pending?'Pending integration':provider?released?'Downloadable':'Catalog only':local?'Local import':'Connection available')}</Badge>
        {preview && <span>{t(preview)}</span>}
        {source.access && <span>{t(accessLabels[source.access])}</span>}
        {provider && !released && <span>{t('Authorization required · originals pending verification')}</span>}
        {!provider && !pending && <span>{t(local?'Open in My Data':source.action.kind==='map'?'Rendered map imagery':source.action.kind==='vector'?'Public feature service':source.action.kind==='tiles'?'Public vector tile archives':'Public raster source')}</span>}
      </span>
    </>;
    return pending ? <Surface as="article" key={source.id} data-source-id={source.id} className="source-board-card source-board-card-pending" aria-label={t(source.name)}>{content}</Surface>
      : <Button key={source.id} data-source-id={source.id} variant="quiet" className="source-board-card" aria-label={t(provider?'Explore {source}':local?'Open {source}':'Connect {source}', {source:t(accessibleName)})} onClick={() => provider ? onChoose(provider.id) : onOpenEntry(source.id)}>{content}</Button>;
  };
  return <section className="source-board" aria-label={t('2D data source directory')}>
    <div className="source-board-heading">
      <div className="source-board-title"><h2><Globe2 size={18} aria-hidden="true"/>{t('Explore data sources')}<span aria-live="polite">{sources.length === SOURCE_DIRECTORY.length ? SOURCE_DIRECTORY.length : `${sources.length} / ${SOURCE_DIRECTORY.length}`}</span></h2><p>{t('Imagery, elevation, maps and vectors. Available and planned sources, together.')}</p></div>
      <div className="source-board-filters">
        <label className="source-board-series">
          <span>{t('Series / source family')}</span>
          <Select aria-label={t('Series / source family')} value={series} onChange={event => setSeries(event.target.value)}>
            <option value="all">{t('All series')}</option>
            <optgroup label={t('Satellite / product series')}>{SOURCE_SERIES.map(({id,label}) => <option key={id} value={id}>{t(label)}</option>)}</optgroup>
            <optgroup label={t('Services and files')}>{DIRECTORY_FAMILIES.map(({id,label}) => <option key={id} value={id}>{t(label)}</option>)}</optgroup>
          </Select>
        </label>
        <label className="source-board-category"><span>{t('Data type')}</span><Select aria-label={t('Data source category')} value={filter} onChange={event=>setFilter(event.target.value)}>{categories.map(({id,label})=><option key={id} value={id}>{t(label)}</option>)}</Select></label>
        <label className="source-board-status"><span>{t('Integration status')}</span><Select aria-label={t('Integration status')} value={status} onChange={event=>setStatus(event.target.value)}>{statuses.map(({id,label})=><option key={id} value={id}>{t(label)}</option>)}</Select></label>
      </div>
    </div>
    {sources.length ? <div className="source-board-groups">{SOURCE_GROUPS.map(group=>{
      const entries=sources.filter(source=>source.group===group.id);
      return entries.length ? <section className="source-board-group" key={group.id} data-product-group={group.id} aria-label={t(group.label)}><h3>{t(group.label)}<span>{entries.length}</span></h3><div className="source-board-grid">{entries.map(renderCard)}</div></section> : null;
    })}</div> : <EmptyState className="source-board-empty" title={t('No sources match these filters.')} action={<Button variant="outline" onClick={() => {setSeries('all');setFilter('all');setStatus('all');}}>{t('Reset filters')}</Button>}/>}
    <p className="source-board-credits">{t('Mission marks: Sentinel © ESA · MODIS NASA/GSFC · SRTM NASA, via DLR.')}</p>
  </section>;
}
