import React, { useContext, useEffect, useRef, useState } from 'react';
import { Files, FolderOpen, Shapes, Image as ImageIcon, Map as MapIcon } from 'lucide-react';
import { PageHeader, SegmentedControl } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { RuntimeLibrary } from './runtime-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';
import { VectorLibrary } from './vector-library.jsx';
import { MapImageLibrary } from './wms-ui.jsx';
import { TileLibrary } from './tiles-ui.jsx';
import './library-page.css';

const viewFromHash = () => {const view=new URLSearchParams(location.hash.split('?')[1] || '').get('view');return ['files','vectors','maps','tiles'].includes(view)?view:'projects';};

export function LibraryPage({ focusedProjectId, onOpenProject, onCloseProject, onContinueExploring, areaBounds, areaPolygon }) {
  const { t, number } = useI18n();
  const { jobs } = useContext(RuntimeContext);
  const [view, setView] = useState(viewFromHash);
  const navigation=useRef(null);
  useEffect(()=>{
    const nav=navigation.current;if(!nav)return;
    const reveal=()=>{
      const active=nav.querySelector('[data-state="on"]');if(!active)return;
      const item=active.getBoundingClientRect(),container=nav.getBoundingClientRect();
      if(item.right>container.right)nav.scrollLeft+=item.right-container.right;
      else if(item.left<container.left)nav.scrollLeft+=item.left-container.left;
    };
    reveal();const observer=new ResizeObserver(reveal);observer.observe(nav);
    return()=>observer.disconnect();
  },[view,focusedProjectId]);
  useEffect(() => {
    const restore = () => setView(viewFromHash());
    window.addEventListener('hashchange', restore);
    return () => window.removeEventListener('hashchange', restore);
  }, []);
  const choose = next => {
    setView(next);
    location.hash = encodeURIComponent('My Data') + (next !== 'projects' ? '?view='+next : '');
  };
  const projects = <ProjectsLibrary focusedProjectId={focusedProjectId} onOpenProject={onOpenProject} onCloseProject={onCloseProject} onContinueExploring={onContinueExploring}/>;
  if (focusedProjectId) return projects;
  const fileCount = jobs.filter(job => job.status === 'succeeded').length;
  return <section className="data-library-page" aria-label={t('My Data')}>
    <div className="data-library-heading">
    <PageHeader title={t('My Data')}/>
    <SegmentedControl ref={navigation} variant="navigation" aria-label={t('Library view')} value={view} onValueChange={choose} items={[
      { value: 'projects', icon: FolderOpen, label: t('Projects') },
      { value: 'files', icon: Files, label: `${t('Raster files')} · ${number(fileCount)}` },
      { value: 'vectors', icon: Shapes, label: t('Vector files') },
      { value: 'maps', icon: ImageIcon, label: t('Map images') },
      { value: 'tiles', icon: MapIcon, label: t('Offline tiles') },
    ]}/>
    </div>
    <div className="data-library-view" key={view}>
      {view === 'projects' ? projects : view === 'vectors' ? <VectorLibrary areaBounds={areaBounds} areaPolygon={areaPolygon}/> : view === 'maps' ? <MapImageLibrary areaBounds={areaBounds} areaPolygon={areaPolygon}/> : view === 'tiles' ? <TileLibrary areaBounds={areaBounds}/> : <RuntimeLibrary areaBounds={areaBounds} areaPolygon={areaPolygon}/>}
    </div>
  </section>;
}
