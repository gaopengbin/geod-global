import React, { useContext, useEffect, useState } from 'react';
import { Compass, Files, FolderOpen } from 'lucide-react';
import { Button, Disclosure, SegmentedControl } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { RuntimeContext } from './runtime-context.js';
import { RuntimeLibrary } from './runtime-ui.jsx';
import { ProjectsLibrary } from './projects-ui.jsx';
import { ExecutableRecipes } from './processing-ui.jsx';
import './library-page.css';

const viewFromHash = () => new URLSearchParams(location.hash.split('?')[1] || '').get('view') === 'files' ? 'files' : 'projects';

export function LibraryPage({ focusedProjectId, onOpenProject, onCloseProject, onContinueExploring, areaBounds, areaPolygon, onReviewJSON }) {
  const { t, number } = useI18n();
  const { jobs } = useContext(RuntimeContext);
  const [view, setView] = useState(viewFromHash);
  useEffect(() => {
    const restore = () => setView(viewFromHash());
    window.addEventListener('hashchange', restore);
    return () => window.removeEventListener('hashchange', restore);
  }, []);
  const choose = next => {
    setView(next);
    location.hash = encodeURIComponent('My Data') + (next === 'files' ? '?view=files' : '');
  };
  const projects = <ProjectsLibrary focusedProjectId={focusedProjectId} onOpenProject={onOpenProject} onCloseProject={onCloseProject} onContinueExploring={onContinueExploring}/>;
  if (focusedProjectId) return projects;
  const fileCount = jobs.filter(job => job.status === 'succeeded').length;
  return <section className="data-library-page" aria-label={t('My Data')}>
    <header className="data-library-heading">
      <div><h1>{t('My Data')}</h1><p>{t('Projects keep scenes together. Files are your downloaded sources and processing results.')}</p></div>
      <Button variant="primary" onClick={() => onContinueExploring?.()}><Compass size={16}/>{t('Explore imagery')}</Button>
    </header>
    <SegmentedControl variant="navigation" aria-label={t('Library view')} value={view} onValueChange={choose} items={[
      { value: 'projects', icon: FolderOpen, label: t('Projects') },
      { value: 'files', icon: Files, label: `${t('All files')} · ${number(fileCount)}` },
    ]}/>
    <div className="data-library-view" key={view}>
      {view === 'projects' ? projects : <>
        <RuntimeLibrary areaBounds={areaBounds} areaPolygon={areaPolygon}/>
        <Disclosure className="saved-clip-plans" summary={t('Saved clip plans · advanced')}>
          <ExecutableRecipes areaBounds={areaBounds} areaPolygon={areaPolygon} onReviewJSON={onReviewJSON}/>
        </Disclosure>
      </>}
    </div>
  </section>;
}
