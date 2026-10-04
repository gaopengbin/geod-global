import React, { createContext, useContext, useEffect, useState } from 'react';
import { Group, Panel, Separator, useDefaultLayout } from 'react-resizable-panels';
import './resizable.css';

// Official MIT dependency owns pointer capture, keyboard resizing, constraints
// and layout persistence. Only its appearance is adapted to GeoD's UI tokens.
const layoutStorage = {
  getItem(key) {
    try {
      const value = localStorage.getItem(key);
      if (!value) return null;
      const layout = JSON.parse(value);
      if (!layout || Array.isArray(layout) || typeof layout !== 'object') return null;
      const sizes = Object.values(layout);
      return sizes.length > 0 && sizes.every(size => Number.isFinite(size) && size >= 0 && size <= 100)
        && Math.abs(sizes.reduce((sum, size) => sum + size, 0) - 100) < 0.1 ? value : null;
    } catch { return null; }
  },
  setItem(key, value) { try { localStorage.setItem(key, value); } catch { /* Resizing still works when storage is unavailable. */ } },
};
const ResizeContext = createContext(true);

function useDesktopLayout() {
  const [desktop, setDesktop] = useState(() => window.innerWidth > 760);
  useEffect(() => {
    if (!window.matchMedia) return;
    const query = window.matchMedia('(min-width: 761px)');
    const change = () => setDesktop(query.matches);
    change();
    query.addEventListener('change', change);
    return () => query.removeEventListener('change', change);
  }, []);
  return desktop;
}

export function ResizableGroup({ storageKey, panelIds, persist = true, onLayoutChanged: onChanged, className = '', children, ...props }) {
  const desktop = useDesktopLayout();
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: `geod-panel-layout-v1-${storageKey}`, panelIds, storage: layoutStorage,
    onlySaveAfterUserInteractions: true,
  });
  return <ResizeContext.Provider value={desktop}>{desktop
    ? <Group className={`bui-panel-group ${className}`} defaultLayout={persist ? defaultLayout : undefined}
      onLayoutChanged={(layout, meta) => {
        if (persist) onLayoutChanged(layout, meta);
        onChanged?.(layout, meta);
      }} resizeTargetMinimumSize={{ fine: 8, coarse: 24 }} {...props}>{children}</Group>
    : <div className={`bui-panel-stack ${className}`}>{children}</div>}
  </ResizeContext.Provider>;
}

export function ResizablePanel({ className = '', ...props }) {
  const desktop = useContext(ResizeContext);
  return desktop ? <Panel className={`bui-panel-content ${className}`} {...props}/>
    : <div id={props.id} className={`bui-panel-content ${className}`}>{props.children}</div>;
}

export function ResizeHandle({ label, hint, ...props }) {
  const desktop = useContext(ResizeContext);
  return desktop ? <Separator className="bui-panel-divider" aria-label={label} title={hint} {...props}><span aria-hidden="true"/></Separator> : null;
}
