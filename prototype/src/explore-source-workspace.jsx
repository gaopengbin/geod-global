import React from 'react';
import { createPortal } from 'react-dom';
import { useI18n } from './i18n.jsx';
import { PublicDatasetExploreMap } from './public-dataset-explore-map.jsx';
import './explore-source-workspace.css';

// Every connected source keeps the same Explore region, sidebar and canvas.
// Adapters supply filters and real results; they do not create another page.
export function ExploreSourceFrame({ sidebarTarget, listHidden = false, onShowList,
  onRequestArea, title, children, actions, map, areaBounds, areaGeometry, ariaLabel = 'Data source exploration' }) {
  const { t } = useI18n();
  const sidebar = <section className="explore-source-sidebar" aria-label={t('Source search and results')}>
    {!sidebarTarget && <header className="explore-source-heading"><h1>{t(title || 'Data source')}</h1></header>}
    <div className="explore-source-body">{children}</div>
    {actions && <div className="explore-source-actions">{actions}</div>}
  </section>;
  return <main className={`explore-source-workspace${!sidebarTarget && !listHidden ? ' explore-source-standalone' : ''}`} aria-label={t(ariaLabel)}>
    {!listHidden && (sidebarTarget ? createPortal(sidebar, sidebarTarget) : sidebar)}
    <div className="explore-source-canvas">{map || <PublicDatasetExploreMap areaBounds={areaBounds} areaGeometry={areaGeometry} listHidden={listHidden} onShowList={onShowList} onRequestArea={onRequestArea}/>}</div>
  </main>;
}
