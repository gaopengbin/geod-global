import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Compass,
  Layers,
  Folder,
  ListTodo,
  Database,
  Settings,
  Cloud,
  Search,
  ChevronDown,
  ChevronRight,
  X,
  Plus,
  Minus,
  Maximize,
  SquareDashed,
  SlidersHorizontal,
  Download,
  ExternalLink,
  ArrowUpRight,
  Sun,
  Moon,
  HelpCircle,
  MapPin,
  Info,
  PanelRightClose,
  PanelRightOpen,
  PanelLeftClose,
  PanelLeftOpen,
  ShieldCheck,
  CheckCircle2,
  ListChecks,
  MousePointer2,
  Mountain,
  MessageSquare,
} from "lucide-react";
import "./ui/foundation.css";
import { Button, Badge, Input, Textarea, Select, Modal, EmptyState,
  Disclosure, Surface, SidebarNav,
  SegmentedControl, Spinner, ResizableGroup, ResizablePanel, ResizeHandle } from "./ui/index.jsx";
import "./styles.css";
import "./catalog.css";
import { compositePeriodLabel } from './composite-period.js';
import { defaultLiveSearch, searchURL, validateSearch, compatibleScenes, createSearchRunner } from "./catalog.js";
import { RuntimeProvider, DownloadAssetButton, RuntimeTasks } from "./runtime-ui.jsx";
import { SaveProjectButton } from "./projects-ui.jsx";
import { LibraryPage } from "./library-page.jsx";
import { scenesForDownload } from "./projects-client.js";
import { desktopAvailable, runtimeRequest } from "./runtime-client.js";
import { mergeProjectCatalog, projectCatalogScenes, projectExploreSearch } from "./project-explore.js";
import { SettingsPage } from './settings-page.jsx';
import { DistributionProvider, NotificationCenter } from './distribution-ui.jsx';
import { StacSourceDialog } from './stac-ui.jsx';
import { WcsSourceDialog } from './wcs-ui.jsx';
import { MapServiceDialog } from './wms-ui.jsx';
import { FeatureServiceDialog } from './features-ui.jsx';
import { TileSourceDialog } from './tiles-ui.jsx';
import { SOURCE_DIRECTORY } from './source-directory.js';
import { normalizeNavigationHash } from './navigation.js';
import { I18nProvider, useI18n } from "./i18n.jsx";
import { AppHeader, GeoDBrand } from './app-header.jsx';
import { CatalogFilters } from './catalog-filters.jsx';
import { CatalogSourcePanel } from './catalog-source-panel.jsx';
import { CatalogPreviewControls } from './catalog-preview-ui.jsx';
import { catalogPreviewKind, catalogPreviewChannel, catalogBrowsePreview } from './catalog-preview.js';
import { providerById, prepareAssetAccess, canDisplayImagery, scenePlatformLabel, demTileLabel, copDemLabel } from './providers.js';
import { imageryHrefs } from './explore-imagery.js';
import { RELEASE_VERSION } from './release-policy.js';
import { AgentPanel } from './agent-panel.jsx';
import {IdentityProvider, IdentityEntry, SignInPage} from './identity-ui.jsx';
import { agentMapContext, selectedSearchBounds } from './startup.js';

const WorkspaceMap = React.lazy(() => import("./workspace-map.jsx").then(module => ({ default: module.WorkspaceMap })));
const VectorWorkspace = React.lazy(() => import("./vector-map.jsx").then(module => ({ default: module.VectorWorkspace })));
const MapImageWorkspace = React.lazy(() => import("./wms-map.jsx").then(module => ({ default: module.MapImageWorkspace })));
const TileWorkspace = React.lazy(() => import("./tiles-map.jsx").then(module => ({ default: module.TileWorkspace })));
const AreaPicker = React.lazy(() => import("./area-picker.jsx").then(module => ({ default: module.AreaPicker })));
const ExploreMap = React.lazy(() => import("./explore-map.jsx").then(module => ({ default: module.ExploreMap })));
const AgentPlanMapPreview = React.lazy(() => import('./agent-plan-map-preview.jsx'));

const nav = [
  ["Home", MessageSquare],
  ["Explore", Compass],
  ["Workspace", Layers],
  ["My Data", Folder],
  ["Tasks", ListTodo],
];
const pageFromHash = () => {
  let requested;
  try { requested = decodeURIComponent(location.hash.slice(1).split('?')[0]); }
  catch { return "Home"; }
  if (requested === "Recipes") return "My Data";
  return [...nav.map(([name]) => name), "Settings", "SignIn"].includes(requested) ? requested : "Home";
};
const projectFromHash = () => {
  if (!['My Data', 'Explore'].includes(pageFromHash())) return null;
  const id = new URLSearchParams(location.hash.split('?')[1] || '').get('project');
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id || '') ? id : null;
};
function stored(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem("geod-design-" + key)) ?? fallback;
  } catch {
    return fallback;
  }
}
function downloadJSON(name, value) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function SceneThumbnail({ src, alt }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => { setFailed(false); setLoaded(false); }, [src]);
  return src && !failed ? <span className="catalog-thumbnail" data-loaded={loaded}>
    {!loaded && <Spinner size={18} aria-label={t('Loading file preview')}/>}
    <img src={src} alt={alt} loading="lazy" onLoad={() => setLoaded(true)} onError={() => setFailed(true)} />
  </span> : <span className="catalog-thumbnail-missing" role="img" aria-label={t("Preview unavailable: {description}", { description: alt })}>{t("Preview unavailable")}</span>;
}

export function App() {
  return <IdentityProvider><DistributionProvider><AppContent/></DistributionProvider></IdentityProvider>;
}
function AppContent() {
  const { t, date, number, locale } = useI18n();
  const [liveCatalog, setLiveCatalog] = useState(null);
  const [searchInput, setSearchInput] = useState(defaultLiveSearch);
  const sourceProvider = providerById(searchInput.provider);
  const canLoadMap = canDisplayImagery(sourceProvider);
  const isElevation = sourceProvider.domain === 'elevation';
  const isSrtm = sourceProvider.id === 'nasa-srtm';
  const isRadar = sourceProvider.domain === 'radar';
  const isComposite = sourceProvider.domain === 'composite';
  const isVegetation = sourceProvider.id === 'planetary-vegetation';
  const isViirs = sourceProvider.id.startsWith('nasa-viirs-');
  const isAerial = sourceProvider.domain === 'aerial';
  const previewKind = catalogPreviewKind(sourceProvider);
  const beforeAerialDates = useRef(null);
  const [preparingImagery, setPreparingImagery] = useState(false);
  const imagerySequence = useRef(0);
  const [liveState, setLiveState] = useState("idle");
  const [liveError, setLiveError] = useState("");
  const [appliedSearch, setAppliedSearch] = useState(null);
  const [areaPolygon, setAreaPolygon] = useState(null);
  const searchRunner = useRef(null);
  if (!searchRunner.current) searchRunner.current = createSearchRunner();
  const catalog = liveCatalog;
  const pendingBounds = selectedSearchBounds(searchInput);
  const bbox = selectedSearchBounds(searchInput, appliedSearch);
  const areaName = areaPolygon?.place?.name || "Custom search area";
  const [page, setPage] = useState(pageFromHash);
  const [vectorId,setVectorId]=useState(()=>new URLSearchParams(location.hash.split('?')[1]||'').get('vector'));
  const [mapImageId,setMapImageId]=useState(()=>new URLSearchParams(location.hash.split('?')[1]||'').get('map'));
  const [tilePackageId,setTilePackageId]=useState(()=>new URLSearchParams(location.hash.split('?')[1]||'').get('tiles'));
  const [focusedProjectId, setFocusedProjectId] = useState(() => pageFromHash() === 'My Data' ? projectFromHash() : null);
  const [exploringProjectId, setExploringProjectId] = useState(() => pageFromHash() === 'Explore' ? projectFromHash() : null);
  const [activeProject, setActiveProject] = useState(null);
  const [projectError, setProjectError] = useState('');
  const restoredProjectId = useRef(null);
  const [selected, setSelected] = useState(null),
    [query, setQuery] = useState(""),
    [sort, setSort] = useState("date");
  const [selectedIds, setSelectedIds] = useState([]);
  const [loadedIds, setLoadedIds] = useState([]);
  const [visibleLoadedIds, setVisibleLoadedIds] = useState([]);
  const [activeDay, setActiveDay] = useState(null);
  const [vegetationIndex, setVegetationIndex] = useState('ndvi');
  const [radarPolarization, setRadarPolarization] = useState('vv');
  const [mapMatches, setMapMatches] = useState([]);
  const [boxSelect, setBoxSelect] = useState(false);
  useEffect(() => { setMapMatches([]); }, [query]);
  const exploreMap = useRef(null);
  const timelineTrack = useRef(null);
  const sceneListRef = useRef(null);
  const attributionRef = useRef(null);
  const listEndRef = useRef(null);
  const [visibleListCount, setVisibleListCount] = useState(100);
  const [footprintCount, setFootprintCount] = useState(0);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [discoveryCollapsed, setDiscoveryCollapsed] = useState(false);
  const [navCollapsed, setNavCollapsed] = useState(stored("nav-collapsed", false));
  const navigationPanel = useRef(null);
  const expandedNavigationSize = useRef(Math.max(180, Math.min(280, Number(stored('nav-width', 216)) || 216)));
  const initialNavigationSize = useRef(navCollapsed ? 72 : expandedNavigationSize.current);
  const [compare, setCompare] = useState(false),
    [compareId, setCompareId] = useState(""),
    [split, setSplit] = useState(50),
    [showArea, setShowArea] = useState(true),
    [inspector, setInspector] = useState(window.innerWidth >= 1280);
  const [stacOpen, setStacOpen] = useState(false);
  const [sourceEntry, setSourceEntry] = useState(null);
  const [agentOpen, setAgentOpen] = useState(() => pageFromHash() === 'Home');
  const [mapPreview,setMapPreview]=useState(null);
  useEffect(()=>setMapPreview(null),[page]);
  const agentClose = useRef(null);
  const [wcsOpen, setWcsOpen] = useState(false);
  const [modal, setModal] = useState(null),
    [theme, setTheme] = useState(stored("theme", "light"));
  const runSearch = async (submittedInput = searchInput, restoredProject = null) => {
    let submitted, url;
    try {
      submitted = validateSearch({ ...submittedInput, limit: 100 });
      url = searchURL(submitted);
    } catch (error) { setLiveError(error.message); return; }
    setLiveError("");
    imagerySequence.current += 1;
    const mapSequence = imagerySequence.current;
    setPreparingImagery(false);
    setFiltersOpen(false);
    setLiveState("loading");
    const savedScenes = restoredProject ? projectCatalogScenes(restoredProject) : [];
    setLiveCatalog(savedScenes.length ? mergeProjectCatalog({ complete: false, pages: 0 }, savedScenes) : null);
    setSelected(savedScenes[0] || null);
    setSelectedIds(savedScenes.map(scene => scene.id));
    setLoadedIds([]);
    setVisibleLoadedIds([]);
    setActiveDay(null);
    setMapMatches([]);
    setCompare(false);
    setAppliedSearch(submitted);
    setQuery("");
    let previewRestored = false;
    try {
      const result = await searchRunner.current.runAll(url, { onPage: page => {
        const merged = savedScenes.length ? mergeProjectCatalog(page, savedScenes) : page;
        setLiveCatalog(merged);
        if (!savedScenes.length && catalogPreviewKind(submitted.provider)) {
          setSelected(current => merged.scenes.find(scene => scene.id === current?.id) || merged.scenes[0] || null);
        }
        if (savedScenes.length) {
          setSelected(current => merged.scenes.find(scene => scene.id === current?.id) || current);
          if (!previewRestored) {
            const savedIds = new Set(savedScenes.map(scene => scene.id));
            const visible = merged.scenes.filter(scene => savedIds.has(scene.id) && imageryHrefs(scene).length).slice(0, 16);
            if (visible.length) {
              previewRestored = true;
              prepareAssetAccess(visible.flatMap(imageryHrefs)).then(() => {
                if (mapSequence !== imagerySequence.current) return;
                setLoadedIds(visible.map(scene => scene.id));
                setVisibleLoadedIds(visible.map(scene => scene.id));
                setSelected(visible[0]);
              }).catch(error => { if (mapSequence === imagerySequence.current) setLiveError(error.message); });
            }
          }
        }
      } });
      if (!result) return;
      setLiveState("ready");
    } catch (error) {
      setLiveError(error.name === "AbortError" ? "Search cancelled." : error.name === "TimeoutError" ? "The data source did not respond within 30 seconds. Try again." : error.message);
      setLiveState("error");
    }
  };
  // Catalog searches begin only after an explicit area/filter/source action,
  // or when restoring the actual area of a saved project.
  useEffect(() => () => searchRunner.current.cancel(), []);
  const cancelSearch = () => { searchRunner.current.cancel(); setLiveState("idle"); setLiveError("Search cancelled. Run a search to retrieve scenes."); };
  const catalogError = (message) => {
    const httpError = /^(.*?) returned HTTP (\d+)\. Try again later\.$/.exec(message);
    return httpError ? t("{source} returned HTTP {status}. Try again later.", { source: httpError[1], status: httpError[2] }) : t(message);
  };
  const applyFilters = values => {
    if (values.bbox !== searchInput.bbox) setAreaPolygon(null);
    setSearchInput(values);
    runSearch(values);
  };
  const applyMapArea = ({ bounds, geometry, place }) => {
    const next = { ...searchInput, bbox: bounds.join(", ") };
    setSearchInput(next);
    setModal(null);
    setAreaPolygon(geometry ? { geometry, place, bounds } : null);
    runSearch(next);
  };
  const exportMapArea = ({ bounds, geometry, place }) => showJSON("geod-search-area.geojson", {
    type: "Feature",
    properties: { name: place?.name || "Custom search area", administrativeCode: place?.code || null, boundarySource: place?.source || null, fixture: false },
    geometry: geometry || { type: "Polygon", coordinates: [[
      [bounds[0], bounds[1]], [bounds[2], bounds[1]], [bounds[2], bounds[3]],
      [bounds[0], bounds[3]], [bounds[0], bounds[1]],
    ]] },
  });
  useEffect(() => {
    const change = () => {
      const next = pageFromHash();
      const project = projectFromHash();
      const hash = normalizeNavigationHash(location.hash);
      if (location.hash !== hash) history.replaceState(null, "", hash);
      setPage(next);
      if (next === 'Home') setAgentOpen(true);
      setVectorId(new URLSearchParams(hash.split('?')[1]||'').get('vector'));
      setMapImageId(new URLSearchParams(hash.split('?')[1]||'').get('map'));
      setTilePackageId(new URLSearchParams(hash.split('?')[1]||'').get('tiles'));
      setFocusedProjectId(next === 'My Data' ? project : null);
      setExploringProjectId(next === 'Explore' ? project : null);
    };
    window.addEventListener("hashchange", change);
    change();
    return () => window.removeEventListener("hashchange", change);
  }, []);
  useEffect(() => {
    const appearance = page === 'SignIn' ? 'dark' : theme;
    document.documentElement.dataset.theme = appearance;
    document.documentElement.classList.toggle("dark", appearance === "dark");
    localStorage.setItem("geod-design-theme", JSON.stringify(theme));
  }, [theme, page]);
  useEffect(() => {
    localStorage.setItem("geod-design-nav-collapsed", JSON.stringify(navCollapsed));
  }, [navCollapsed]);
  useEffect(() => {
    const onKeyDown = (event) => {
      if (event.key === "Escape" && !document.querySelector('[role="dialog"][data-state="open"]')) setCompare(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
  useEffect(() => {
    if (!exploringProjectId) return;
    let current = true;
    const controller = new AbortController();
    setProjectError('');
    runtimeRequest('projects', undefined, controller.signal).then(projects => {
      if (!current) return;
      const project = projects.find(item => item.id === exploringProjectId);
      if (!project) throw new Error('This project could not be found. Return to all projects or refresh the list.');
      setActiveProject(project);
      if (restoredProjectId.current !== project.id) {
        restoredProjectId.current = project.id;
        const next = projectExploreSearch(project, searchInput);
        setSearchInput(next);
        setAreaPolygon({ geometry: project.geometry || null, bounds: project.bounds, place: { name: project.name } });
        runSearch(next, project);
      }
    }).catch(error => { if (current) setProjectError(error.message); });
    return () => { current = false; controller.abort(); };
  }, [exploringProjectId]);
  const showJSON = (filename, value) =>
    setModal({ type: "json", filename, value });
  const go = (p, projectId = null) => {
    location.hash = encodeURIComponent(p) + (projectId ? `?project=${projectId}` : '');
    setPage(p);
    setFocusedProjectId(p === 'My Data' ? projectId : null);
    setExploringProjectId(p === 'Explore' ? projectId : null);
  };
  const chooseSource = provider => {
    const next = { ...searchInput, provider };
    if (next.provider === 'planetary-naip' && sourceProvider.id !== next.provider) {
      beforeAerialDates.current = {start:searchInput.start,end:searchInput.end};
      next.start = '2010-01-01'; next.end = new Date().toISOString().slice(0,10);
    } else if (sourceProvider.id === 'planetary-naip' && next.provider !== sourceProvider.id && beforeAerialDates.current) {
      Object.assign(next,beforeAerialDates.current); beforeAerialDates.current = null;
    }
    setSearchInput(next);
    if (selectedSearchBounds(next)) runSearch(next);
  };
  const exploreSource = provider => {
    setInspector(false);
    chooseSource(provider);
    go('Explore');
  };
  const openSourceEntry = id => {
    const action = SOURCE_DIRECTORY.find(source => source.id === id)?.action;
    if (!action) return;
    if (action.kind === 'provider') return exploreSource(action.providerId);
    if (action.kind === 'library') {
      location.hash = `My%20Data?view=${action.view}`;
      return;
    }
    // A close animation may still be mounted when another card is selected.
    setSourceEntry(previous=>({...action,id,generation:(previous?.generation || 0)+1}));
  };
  const openProject = (id) => go('My Data', id);
  const continueInProject = project => go('Explore', project?.id || activeProject?.id || null);
  const currentProject = activeProject?.id === exploringProjectId ? activeProject : null;
  const scenes = useMemo(() => catalog?.scenes || [], [catalog]);
  useEffect(() => {
    const track = timelineTrack.current;
    if (!track) return;
    const revealSelected = () => {
      const active = track.querySelector('[aria-pressed="true"]');
      if (active) track.scrollTo({ left: active.offsetLeft + active.offsetWidth / 2 - track.clientWidth / 2, behavior: "auto" });
    };
    revealSelected();
    const observer = new ResizeObserver(revealSelected);
    observer.observe(track);
    return () => observer.disconnect();
  }, [selected?.id, scenes.length, page]);
  const filtered = useMemo(() => scenes
    .filter((s) =>
      (isElevation || !activeDay || s.date.slice(0, 10) === activeDay)
      && (s.id.toLowerCase().includes(query.toLowerCase()) || s.date.includes(query)),
    )
    .sort((a, b) => isElevation ? a.id.localeCompare(b.id) : sort === "cloud" ? (a.cloud ?? 101) - (b.cloud ?? 101) : b.date.localeCompare(a.date)), [scenes, activeDay, query, sort, isElevation]);
  useEffect(() => { setVisibleListCount(100); sceneListRef.current?.scrollTo({ top: 0 }); }, [query, sort, activeDay, appliedSearch]);
  useEffect(() => {
    if (!listEndRef.current || !sceneListRef.current || visibleListCount >= filtered.length) return;
    const observer = new IntersectionObserver(entries => {
      if (entries[0]?.isIntersecting) setVisibleListCount(count => Math.min(filtered.length, count + 100));
    }, { root: sceneListRef.current, rootMargin: '200px' });
    observer.observe(listEndRef.current);
    return () => observer.disconnect();
  }, [filtered.length, visibleListCount]);
  const sceneById = useMemo(() => new Map(scenes.map(scene => [scene.id, scene])), [scenes]);
  const awaitingSelectedGrid = liveState === 'loading' && selectedIds.some(id => !imageryHrefs(sceneById.get(id)).length);
  const loadedScenes = useMemo(() => loadedIds.map(id => sceneById.get(id)).filter(Boolean), [loadedIds, sceneById]);
  const downloadScenes = useMemo(() => scenesForDownload({ scenes, selectedIds, loadedIds, currentScene: selected }), [scenes, selectedIds, loadedIds, selected]);
  const visibleLoadedScenes = useMemo(() => loadedScenes.filter(scene => visibleLoadedIds.includes(scene.id)), [loadedScenes, visibleLoadedIds]);
  const mapScene = previewKind ? filtered.find(scene => scene.id === selected?.id) || filtered[0] : selected || filtered[0] || scenes[0];
  const previewChannel = mapScene && previewKind && (previewKind !== 'elevation' || mapScene.grid?.shape) ? catalogPreviewChannel(mapScene,isVegetation ? vegetationIndex : radarPolarization) : undefined;
  useEffect(() => {
    const attribution = attributionRef.current;
    if (!attribution) return;
    const measure = () => attribution.parentElement?.style.setProperty('--map-attribution-height', `${attribution.getBoundingClientRect().height}px`);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(attribution);
    return () => observer.disconnect();
  }, [page, scenes.length]);
  const days = useMemo(() => [...new Set(scenes.map(scene => scene.date.slice(0, 10)))].sort(), [scenes]);
  const dayCounts = useMemo(() => scenes.reduce((counts, scene) => {
    const day = scene.date.slice(0, 10);
    counts.set(day, (counts.get(day) || 0) + 1);
    return counts;
  }, new Map()), [scenes]);
  const updateSelection = value => {
    imagerySequence.current += 1;
    setPreparingImagery(false);
    setSelectedIds(value);
  };
  const toggleScene = id => {
    updateSelection(current => current.includes(id) ? current.filter(value => value !== id) : [...current, id]);
    if (!canLoadMap) setSelected(sceneById.get(id) || null);
  };
  const loadSelected = async () => {
    if (awaitingSelectedGrid) return;
    const chosen = selectedIds.map(id => sceneById.get(id)).filter(scene => imageryHrefs(scene).length);
    if (chosen.length !== selectedIds.length) { setLiveError('The selected scene has no supported georeferenced true-color grid.'); return; }
    if (!chosen.length || chosen.length > 16) return;
    const sequence = ++imagerySequence.current;
    setLiveError('');
    setPreparingImagery(true);
    try { await prepareAssetAccess(chosen.flatMap(imageryHrefs)); }
    catch (error) { if (sequence === imagerySequence.current) { setLiveError(error.message); setPreparingImagery(false); } return; }
    if (sequence !== imagerySequence.current) return;
    setPreparingImagery(false);
    setLoadedIds(chosen.sort((a, b) => a.date.localeCompare(b.date)).map(scene => scene.id));
    setVisibleLoadedIds(chosen.map(scene => scene.id));
    setSelected(chosen.filter(scene => !activeDay || scene.date.slice(0, 10) === activeDay).sort((a, b) => b.date.localeCompare(a.date))[0] || null);
    setMapMatches([]);
  };
  const showDay = day => {
    setActiveDay(day);
    setMapMatches([]);
    setSelected((previewKind || !canLoadMap ? scenes : visibleLoadedScenes).filter(scene => !day || scene.date.slice(0, 10) === day).sort((a, b) => b.date.localeCompare(a.date))[0] || null);
  };
  const comparisons = scenes.filter((s) => compatibleScenes(selected, s));
  const other = comparisons.find((s) => s.id === compareId) || comparisons[0];
  const comparing = compare && !!other;
  const home = page === "Home";
  const agentVisible = page !== 'SignIn' && (home || agentOpen);
  const previewing=Boolean(mapPreview&&agentVisible);
  const workAreaVisible=!home&&!previewing;
  const workspace = page === "Explore" || page === "Workspace";
  const hasInspector = inspector && Boolean(selected || mapMatches.length);
  const workspaceMinWidth = page === 'Explore'
    ? 240 + (discoveryCollapsed ? 0 : 246) + (hasInspector ? 226 : 0)
    : 360;
  const resizeHint = t('Drag to resize · Double-click to reset · Arrow keys to adjust');
  if (page === 'SignIn') return <div className="app app-sign-in">
    <AppHeader theme="dark" minimal/>
    <main className="identity-standalone"><SignInPage onContinue={()=>go('Home')}/></main>
  </div>;
  return (
    <div className={`app ${home ? 'app-home' : ''}`}>
      <AppHeader theme={theme} collapsed={navCollapsed} onToggleNavigation={() => {
        const panel = navigationPanel.current;
        if (!panel) { setNavCollapsed(value => !value); return; }
        if (panel.isCollapsed()) panel.resize(expandedNavigationSize.current);
        else {
          try { localStorage.setItem('geod-design-nav-width', JSON.stringify(expandedNavigationSize.current)); } catch { /* Keep the in-memory size. */ }
          panel.collapse();
        }
      }}
        leading={page === "Explore" && discoveryCollapsed && <Button variant="secondary" size="sm" icon={PanelLeftOpen} className="discovery-toggle" aria-label={t("Show scene list")} aria-controls="explore-discovery" aria-expanded={false} onClick={() => setDiscoveryCollapsed(false)}>{t("Imagery scenes")}</Button>}
        context={<div className="breadcrumb">
          <strong>{home ? t('AI workspace') : currentProject?.name || t("{source} workspace", { source: page === 'Explore' ? sourceProvider.name : 'GeoD Global' })}</strong>
          <ChevronRight size={14} aria-hidden="true" /><span>{t(page==='SignIn'?'Sign in':page)}</span>
        </div>}
        actions={<>
          <NotificationCenter/>
          {!home && page !=='SignIn' && <Button size="icon" variant="quiet" icon={MessageSquare} aria-label={t(agentOpen ? 'Close Agent' : 'Open Agent')} tooltip={t(agentOpen ? 'Close Agent' : 'Open Agent')}
            aria-controls="geod-agent-panel" aria-expanded={agentOpen} onClick={() => {
              if (agentOpen) { agentClose.current?.(); return; }
              setInspector(false);
              if (window.innerWidth < 1100) setDiscoveryCollapsed(true);
              setAgentOpen(true);
            }}/>}
          {page === 'Explore' && exploringProjectId && <>
            <Button size="sm" icon={Folder} onClick={() => openProject(exploringProjectId)}>{t('Return to project details')}</Button>
            <Button size="icon" variant="quiet" aria-label={t('Leave project exploration')} title={t('Leave project exploration')} onClick={() => { setActiveProject(null); restoredProjectId.current = null; go('Explore'); }}><X size={15}/></Button>
          </>}
          <Button size="icon" variant="quiet" aria-label={t("Toggle color theme")} onClick={() => setTheme(theme === "light" ? "dark" : "light")}>
            {theme === "light" ? <Moon size={17} /> : <Sun size={17} />}
          </Button>
          <span className="local-status"><span />{t("Local workspace")}</span>
        </>} />
      <ResizableGroup className={`app-body ${home ? "app-home-body" : ""} ${previewing ? 'app-map-preview-body' : ''}`} storageKey="shell" panelIds={['navigation-pane', ...(workAreaVisible ? ['work-area-pane'] : []), ...(agentVisible ? ['agent-pane'] : []), ...(previewing ? ['agent-map-preview-pane'] : [])]} persist={false}
        onLayoutChanged={(_, meta) => {
          if (meta.isUserInteraction && !navigationPanel.current?.isCollapsed()) {
            try { localStorage.setItem('geod-design-nav-width', JSON.stringify(expandedNavigationSize.current)); } catch { /* Keep the in-memory size. */ }
          }
        }}>
      <ResizablePanel key="navigation-pane" id="navigation-pane" minSize={180} maxSize={280} collapsible collapsedSize={72} defaultSize={initialNavigationSize.current}
        panelRef={navigationPanel} groupResizeBehavior="preserve-pixel-size"
        onResize={size => {
          setNavCollapsed(size.inPixels < 179);
          if (size.inPixels >= 179) expandedNavigationSize.current = size.inPixels;
        }}>
      <SidebarNav
        id="primary-navigation"
        className={navCollapsed ? "nav-collapsed" : ""}
        ariaLabel={t("GeoD home")}
        items={nav.map(([name, icon]) => ({ id: name, label: t(name), icon, href: "#" + encodeURIComponent(name), active: page === name }))}
        footerItems={[
          { id: "Settings", label: t("Settings"), icon: Settings, href: "#Settings", active: page === "Settings" },
          { id: "Help", label: t("Help"), icon: HelpCircle, onClick: () => setModal("about") },
        ]}
        footer={<IdentityEntry onOpen={()=>go('SignIn')}/>}
      />
      </ResizablePanel>
      <ResizeHandle key="navigation-divider" label={t('Resize navigation panel')} hint={resizeHint} disabled={navCollapsed}/>
      {workAreaVisible && <ResizablePanel key="work-area-pane" id="work-area-pane" minSize={workspaceMinWidth}>
      <div className="app-main">
        {page === 'Explore' && exploringProjectId && !currentProject && !projectError && <p className="project-context-status" role="status"><Spinner size={15}/>{t('Loading project scenes…')}</p>}
        {page === 'Explore' && exploringProjectId && projectError && <p className="project-context-status projects-error" role="alert">{t(projectError)}</p>}
        {page === "Workspace" ? <React.Suspense fallback={<main className="wm-map-loading" role="status">{t("Loading local map…")}</main>}>{tilePackageId?<TileWorkspace id={tilePackageId}/>:mapImageId?<MapImageWorkspace id={mapImageId}/>:vectorId?<VectorWorkspace id={vectorId}/>:<WorkspaceMap />}</React.Suspense> : workspace ? (
          <ResizableGroup className="workspace" storageKey="explore" panelIds={[
            ...(!discoveryCollapsed ? ['discovery-pane'] : []), 'explore-map-pane', ...(hasInspector ? ['inspector-pane'] : []),
          ]}>
            {!discoveryCollapsed && <ResizablePanel id="discovery-pane" defaultSize={300} minSize={240} maxSize={520} groupResizeBehavior="preserve-pixel-size"><aside className="discovery" id="explore-discovery">
              <div className="panel-heading">
                <div>
                  <h1>{t("Imagery scenes")}</h1>
                </div>
                <div className="scene-panel-actions">
                  <Button variant="quiet" size="icon" aria-label={t("Select filtered · {count}", { count: filtered.length })} tooltip={t("Select filtered · {count}", { count: filtered.length })} disabled={!filtered.length} onClick={() => updateSelection(current => [...new Set([...current, ...filtered.map(scene => scene.id)])])}><ListChecks size={18}/></Button>
                  <Button variant="quiet" size="icon" aria-label={t("Hide scene list")} title={t("Hide scene list")} aria-controls="explore-discovery" aria-expanded={true} onClick={() => setDiscoveryCollapsed(true)}><PanelLeftClose size={18} /></Button>
                </div>
              </div>
              <CatalogSourcePanel provider={sourceProvider} onOpenStac={() => setStacOpen(true)} onOpenWcs={() => setWcsOpen(true)} onChange={chooseSource}/>
              <Button className="area-picker" onClick={() => setModal("area")}>
                <MapPin size={17} />
                <span>
                  <strong>{t(bbox ? areaName : "Choose a search area")}</strong>
                  <small>{t("WGS 84 · editable search bounds")}</small>
                </span>
                <ChevronDown size={16} />
              </Button>
              {appliedSearch && !isElevation && <div className="catalog-active-filters">{date(appliedSearch.start)} – {date(appliedSearch.end)}{!isAerial && !isComposite && !isRadar && <> · {t("clouds {minimum}–{maximum}", { minimum: number(appliedSearch.cloudMin / 100, { style: "percent" }), maximum: number(appliedSearch.cloud / 100, { style: "percent" }) })}</>}</div>}
              <>
                  <label className="search-input scene-search">
                    <Search size={16} />
                    <Input aria-label={t("Search scenes")} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t(isElevation ? "Search elevation tile" : "Search scene ID or date")} />
                  </label>
                  {liveError && <p className="catalog-error" role="alert">{catalogError(liveError)}</p>}
                  <div className="catalog-fetch-status" role="status">
                    {liveState === "loading" ? <><Spinner />{t("Fetching all catalog pages… {count} scenes from {pages} pages", { count: scenes.length, pages: catalog?.pages || 0 })}<Button variant="quiet" size="xs" onClick={cancelSearch}>{t("Stop catalog search")}</Button></>
                      : catalog?.complete ? t("Catalog complete · {count} scenes", { count: scenes.length })
                        : catalog ? t("Catalog partial · {count} scenes", { count: scenes.length }) : null}
                  </div>
                  <div className="results-heading">
                    <span className="results-count">
                      <strong>{filtered.length}</strong><span className="results-count-label"> {t(isElevation ? "elevation tiles" : "scenes")}</span>
                    </span>
                    <div className="results-actions">
                      <Button size="sm" icon={SlidersHorizontal} className="filter-toggle" aria-label={t("Filters")} aria-haspopup="dialog" aria-expanded={filtersOpen} onClick={() => setFiltersOpen(true)}><span className="filter-toggle-label">{t("Filters")}</span></Button>
                      {!isElevation && <Select aria-label={t("Sort scenes")} value={sort} onChange={(e) => setSort(e.target.value)}>
                        <option value="date">{t("Newest")}</option>
                        {!isAerial && !isComposite && !isRadar && <option value="cloud">{t("Clearest")}</option>}
                      </Select>}
                    </div>
                  </div>
                  <div className="scene-list" ref={sceneListRef}>
                    {liveState === "loading" && !catalog ? <div className="loading-state" role="status"><Spinner />{t("Searching {source}…", { source: sourceProvider.name })}</div> : !catalog ? <EmptyState icon={Search} title={t(liveState === "error" ? "Catalog request failed" : "Search the live catalog")}>{t(isElevation ? "Choose a search area to find public elevation tiles." : "Set your area and dates, choose a data source, then search for imagery.")}</EmptyState> : !filtered.length ? (
                      <EmptyState
                        icon={Search}
                        title={t("No matching scenes")}
                        action={
                          <Button
                            onClick={() => {
                              if (query) { setQuery(""); return; }
                              setQuery("");
                              const reset = { ...(appliedSearch || searchInput), cloudMin: 0, cloud: 100 };
                              setSearchInput({ ...reset, bbox: Array.isArray(reset.bbox) ? reset.bbox.join(", ") : reset.bbox });
                              runSearch(reset);
                            }}
                          >{t("Reset filters")}</Button>
                        }
                      >{t(query ? "No scene ID or date matches this text." : isRadar ? "Try a wider date range, another orbit or polarization." : isComposite ? "Try a wider composite period or a different area." : isAerial ? "No NAIP imagery covers this area and date range. Coverage is limited to published US aerial acquisitions." : isSrtm ? "No SRTMGL1 tiles cover this area. Published land coverage extends from 56° S to 60° N." : isElevation ? "No published elevation tiles cover this area. Coverage depends on the selected product; ocean tiles are absent." : "Try a wider date range or allow more cloud cover.")}</EmptyState>
                    ) : (
                      filtered.slice(0, visibleListCount).map((s) => (
                        <Button variant="quiet" size="row" aria-pressed={selectedIds.includes(s.id)}
                          key={s.id}
                          className={
                            "scene " + (selectedIds.includes(s.id) ? "selected" : "")
                          }
                          onClick={() => toggleScene(s.id)}
                          aria-label={t("Select scene {date} {id}", { date: isElevation ? demTileLabel(s.id) : compositePeriodLabel(s, date), id: s.id })}
                        >
                          {previewKind === 'elevation' ? <span className="catalog-elevation-tile" aria-label={t('Elevation tile, {id}',{id:demTileLabel(s.id)})}><Mountain size={25} aria-hidden="true"/><span>{number(s.gsd)} m</span></span> : <SceneThumbnail
                            src={catalogBrowsePreview(s)}
                            alt={isVegetation ? t('Vegetation index browse image') : isRadar ? t('Radar backscatter browse image') : isElevation ? t('Elevation browse preview, {id}', { id: demTileLabel(s.id) }) : t("True-color preview, {date}", { date: compositePeriodLabel(s, date) })}
                          />}
                          <div className="scene-info">
                            <strong title={isComposite ? compositePeriodLabel(s, date) : undefined}>{isElevation ? demTileLabel(s.id) : compositePeriodLabel(s, date, locale)}</strong>
                            <span>
                              {t(scenePlatformLabel(s))}{" "}
                              <span className="muted">{isRadar ? '· IW RTC' : isViirs ? '· 8-day · v002' : isVegetation ? '· 16-day · v6.1' : isComposite ? '· 8-day · v6.1' : isAerial ? '· RGB + NIR' : isSrtm ? '· v003' : isElevation ? `· ${copDemLabel([s])}` : s.provider === 'nasa-earthdata' ? '· v2.0' : s.provider === 'planetary-landsat' ? '· L2' : t("· L2A")}</span>
                            </span>
                            <small>
                              {isRadar ? <span>{s.properties['sar:polarizations']?.join(' / ')} · {t(s.properties['sat:orbit_state'] === 'ascending' ? 'Ascending' : s.properties['sat:orbit_state'] === 'descending' ? 'Descending' : 'Orbit unavailable')}</span> : isComposite ? <span>{t(isVegetation ? '250 m · 16-day composite' : isViirs ? '1 km · 8-day composite' : '500 m · 8-day composite')}</span> : isAerial ? <span>{t('{resolution} meters', {resolution:number(s.gsd)})}</span> : isElevation ? <span>{isSrtm ? 'Int16 · EGM96' : 'Float32 · EGM2008'}</span> : <><Cloud size={12} />
                              {s.cloud == null ? t("Unknown") : number(s.cloud / 100, { style: "percent", maximumFractionDigits: 1 })}<span>{s.gsd ? t(canLoadMap ? "{resolution} m RGB" : "{resolution} meters", { resolution: number(s.gsd) }) : t("Preview")}</span></>}
                            </small>
                          </div>
                          {selectedIds.includes(s.id) && (
                            <CheckCircle2
                              className="selection-check"
                              size={16}
                            />
                          )}
                        </Button>
                      ))
                    )}
                    {filtered.length > visibleListCount && <div ref={listEndRef} className="catalog-list-progress" role="status">{t("Showing {shown} of {total} scenes · scroll for more", { shown: visibleListCount, total: filtered.length })}</div>}
                  </div>
                  {catalog && selectedIds.length > 0 && <div className="catalog-selection-actions">
                    <div><strong title={t("Click footprints or drag a box on the map to choose scenes.")}>{t("{count} scenes selected", { count: selectedIds.length })}</strong><Button variant="quiet" size="xs" onClick={() => updateSelection([])}>{t("Clear selection")}</Button></div>
                    {canLoadMap && <Button primary onClick={loadSelected} disabled={awaitingSelectedGrid || preparingImagery || !selectedIds.length || selectedIds.length > 16}>{awaitingSelectedGrid ? <><Spinner />{t('Reading imagery metadata…')}</> : preparingImagery ? <><Spinner />{t('Preparing imagery access…')}</> : t("Load selected imagery · {count}", { count: selectedIds.length })}</Button>}
                    {sourceProvider.download && (exploringProjectId || !canLoadMap ? (!exploringProjectId || currentProject) && <DownloadAssetButton key={[exploringProjectId, ...downloadScenes.map(scene => scene.id)].join('|')} scene={selected || downloadScenes[0]} scenes={downloadScenes} areaBounds={bbox} areaPolygon={areaPolygon?.geometry} areaName={areaName} project={currentProject || undefined} onProjectUpdated={setActiveProject} onOpenProject={openProject}/> : <SaveProjectButton scenes={selectedIds.map(id => sceneById.get(id)).filter(Boolean)} bounds={bbox} geometry={areaPolygon?.geometry} areaName={areaName} onSaved={project => openProject(project.id)}/>)}
                    {canLoadMap && selectedIds.length > 16 && <span className="selection-limit">{t("Select at most 16 COGs for this browser map. Narrow the filters or clear some scenes.")}</span>}
                  </div>}
                  <div className="panel-foot">
                    <Database size={13} />
                    <span>{sourceProvider.name} · {t("live HTTPS catalog")}{catalog && <> · {t("{count} footprints on map", { count: footprintCount })}</>}</span>
                  </div>
              </>
            </aside></ResizablePanel>}
            {!discoveryCollapsed && <ResizeHandle label={t('Resize imagery list')} hint={resizeHint}/>}
            <ResizablePanel id="explore-map-pane" className="explore-map-content" minSize={240}>
            {scenes.length ? <main className="map-workspace">
              <div className="map-toolbar">
                <div className="map-view-controls">
                {previewKind ? <CatalogPreviewControls kind={previewKind} scene={mapScene} value={previewChannel} onValueChange={isVegetation ? setVegetationIndex : setRadarPolarization}/> : <SegmentedControl className="preview-mode" aria-label={t("Preview")} value={compare ? "compare" : "preview"}
                  onValueChange={value => { setCompare(value === "compare"); if (value === "compare") setCompareId(other?.id || ""); }}
                  items={canLoadMap ? [
                    { value: "preview", label: t("Preview"), icon: Layers },
                    { value: "compare", label: t("Compare"), icon: SlidersHorizontal, disabled: !comparisons.length,
                      title: t(comparisons.length ? "Compare scenes with matching source grids" : "Comparison needs two true-color COGs with the same CRS, transform and dimensions") },
                  ] : [{value:'preview',label:t('Footprints'),icon:SquareDashed}]} />}
                {visibleLoadedScenes.filter(scene => !activeDay || scene.date.slice(0, 10) === activeDay).length > 1 && <Select className="front-layer-picker" aria-label={t("Front imagery layer")} value={selected?.id || ''} onChange={event => setSelected(sceneById.get(event.target.value))}>
                  {visibleLoadedScenes.filter(scene => !activeDay || scene.date.slice(0, 10) === activeDay).map(scene => <option key={scene.id} value={scene.id}>{date(scene.date, { year: undefined, month: '2-digit', day: '2-digit' })} · {scene.properties["grid:code"] || scene.id}</option>)}
                </Select>}
                </div>
                <div className="toolbar-end">
                  <div className="map-controls" role="group" aria-label={t("Map controls")}>
                    <Button variant="secondary" size="icon" className={"map-icon " + (!boxSelect ? "control-active" : "")} aria-pressed={!boxSelect} aria-label={t("Click scene footprints")} title={t("Click scene footprints")} onClick={() => setBoxSelect(false)}><MousePointer2 size={17} /></Button>
                    <Button variant="secondary" size="icon" className={"map-icon " + (boxSelect ? "control-active" : "")} aria-pressed={boxSelect} aria-label={t("Drag a box to select scene footprints")} title={t("Drag a box to select scene footprints")} onClick={() => setBoxSelect(true)}><SquareDashed size={17} /></Button>
                    <span className="control-separator" aria-hidden="true" />
                    <Button variant="secondary" size="icon" className="map-icon" aria-label={t("Zoom in")} title={t("Zoom in")} onClick={() => exploreMap.current?.zoomIn()}><Plus size={18} /></Button>
                    <Button variant="secondary" size="icon" className="map-icon" aria-label={t("Zoom out")} title={t("Zoom out")} onClick={() => exploreMap.current?.zoomOut()}><Minus size={18} /></Button>
                    <Button variant="secondary" size="icon" className="map-icon" aria-label={t("Fit all scene footprints")} title={t("Fit all scene footprints")} onClick={() => exploreMap.current?.fit()}><Maximize size={16} /></Button>
                    <span className="control-separator" aria-hidden="true" />
                    <Button variant="secondary" size="icon" className={"map-icon " + (showArea ? "control-active" : "")} aria-pressed={showArea} aria-label={t("Toggle saved area")} title={t("Show the searched area")} onClick={() => setShowArea(!showArea)}><SquareDashed size={18} /></Button>
                    <span className="control-separator" aria-hidden="true" />
                    <Button variant="secondary" size="icon" className="map-icon" aria-label={t(inspector ? "Hide inspector" : "Show inspector")} title={t(inspector ? "Hide inspector" : "Show inspector")} onClick={() => setInspector(!inspector)}>
                      {inspector ? <PanelRightClose size={18} /> : <PanelRightOpen size={18} />}
                    </Button>
                  </div>
                </div>
              </div>
              <div className="imagery-canvas">
                <React.Suspense fallback={<div className="explore-map-loading" role="status">{t("Loading georeferenced imagery…")}</div>}>
                  <ExploreMap ref={exploreMap} scene={mapScene || scenes[0]} scenes={filtered} loadedScenes={visibleLoadedScenes} selectedIds={selectedIds} focusedIds={mapMatches} activeSceneId={mapScene?.id} activeDay={activeDay} reference={comparing ? other : null} split={split} area={bbox} areaGeometry={areaPolygon?.geometry} showArea={showArea} boxSelect={boxSelect} previewChannel={previewChannel} onFootprintsPick={ids => { setMapMatches(ids); if (ids.length) setInspector(true); }} onFootprintsChange={setFootprintCount} />
                </React.Suspense>
                {comparing && (
                  <div className="compare-line" style={{ left: `calc(${split}% - 22px)` }} role="slider" tabIndex={0}
                    aria-label={t("Comparison split")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={split}
                    title={t("Drag the vertical divider to compare scenes")}
                    onPointerDown={event => { event.preventDefault(); event.stopPropagation(); event.currentTarget.setPointerCapture(event.pointerId); }}
                    onPointerMove={event => { if (!event.currentTarget.hasPointerCapture(event.pointerId)) return; const box = event.currentTarget.parentElement.getBoundingClientRect(); setSplit(Math.max(0, Math.min(100, Math.round((event.clientX - box.left) * 100 / box.width)))); }}
                    onKeyDown={event => { if (["ArrowLeft", "ArrowDown", "ArrowRight", "ArrowUp", "Home", "End"].includes(event.key)) { event.preventDefault(); setSplit(current => event.key === "Home" ? 0 : event.key === "End" ? 100 : Math.max(0, Math.min(100, current + (["ArrowLeft", "ArrowDown"].includes(event.key) ? -2 : 2)))); } }}>
                    <span>
                      <SlidersHorizontal size={19} />
                    </span>
                  </div>
                )}
              </div>
              {comparing && (
                <div className="compare-controls">
                  <Surface as="div" className="compare-scene compare-reference">
                    <span>{t("Reference")}</span><Select
                      aria-label={t("Reference scene")}
                      value={other.id}
                      onChange={(e) => setCompareId(e.target.value)}
                    >
                      {comparisons
                        .map((s) => (
                          <option key={s.id} value={s.id}>
                            {compositePeriodLabel(s, date)}
                          </option>
                        ))}
                    </Select>
                  </Surface>
                  <Surface as="div" className="compare-scene compare-current">
                    <span>{t("Selected observation")}</span>
                    <strong>{date(selected.date)}</strong>
                  </Surface>
                </div>
              )}
              <div className="map-attribution" ref={attributionRef}>
                <span>{t(isViirs ? "NASA/NOAA VIIRS composites ({year}) · {source} · Natural Earth overview" : isComposite ? "NASA MODIS composites ({year}) · {source} · Natural Earth overview" : isAerial ? "USDA NAIP aerial imagery ({year}) · {source} · Natural Earth overview" : isSrtm ? "NASA SRTMGL1 v003 · {source} · Natural Earth overview" : isElevation ? "Copernicus DEM · {source} · Natural Earth overview" : sourceProvider.id === 'planetary-landsat' ? "USGS Landsat data ({year}) · {source} · Natural Earth overview" : sourceProvider.id === 'nasa-earthdata' ? "NASA HLS data ({year}) · {source} · Natural Earth overview" : "Copernicus Sentinel data ({year}) · {source} · Natural Earth overview", { source: sourceProvider.name, year: (selected || scenes[0]).date.slice(0, 4) })}</span>
                <Button variant="quiet" disabled={!selected} onClick={() => setModal("provenance")}>{t(isVegetation ? 'Online index preview · source details' : previewKind ? 'Online map preview · source details' : visibleLoadedScenes.length ? "Georeferenced COG display · source details" : "Scene footprints · source details")}<Info size={12} />
                </Button>
              </div>
              {!isElevation && <div className="timeline">
                <div className="timeline-track" ref={timelineTrack} aria-label={t("Observation timeline")}>
                  <div className="timeline-items">
                    <Button variant="quiet" aria-pressed={!activeDay} className={!activeDay ? "selected" : ""} onClick={() => showDay(null)} aria-label={t("Show all dates and {count} scenes", { count: scenes.length })}>
                      <span className="observation-dot" aria-hidden="true" /><span className="timeline-date">{t("All dates")}</span><span className="timeline-cloud">{t("{count} scenes", { count: scenes.length })}</span>
                    </Button>
                    {days.map(day => <Button variant="quiet" key={day} aria-pressed={activeDay === day} className={activeDay === day ? "selected" : ""} onClick={() => showDay(day)} aria-label={t("Show {date} and {count} scenes", { date: date(day), count: dayCounts.get(day) })}>
                      <span className="observation-dot" aria-hidden="true" /><span className="timeline-date">{date(day, { year: undefined, month: "2-digit", day: "2-digit" })}</span><span className="timeline-cloud">{t("{count} scenes", { count: dayCounts.get(day) })}</span>
                    </Button>)}
                  </div>
                </div>
              </div>}
            </main> : <main className="catalog-blank"><EmptyState icon={Search} title={t(liveState === "loading" ? "Searching your area" : liveCatalog ? "No scenes for this search" : "Choose your next observation")}>{t(liveState === "loading" ? "Catalog footprints will appear here as pages arrive." : isElevation ? "Choose a search area to find public elevation tiles." : isRadar ? "Choose an area, dates and orbit to find radar footprints." : "Choose an area, dates and cloud limit to find imagery footprints.")}</EmptyState></main>}
            </ResizablePanel>
            {hasInspector && <ResizeHandle label={t('Resize details panel')} hint={resizeHint}/>}
            {hasInspector && <ResizablePanel id="inspector-pane" defaultSize={280} minSize={220} maxSize={480} groupResizeBehavior="preserve-pixel-size">
            {mapMatches.length > 0 ? <aside className="inspector footprint-inspector" aria-label={t("Scenes in the selected map area")}>
              <div className="inspector-heading"><span className="eyebrow">{t("MAP SELECTION")}</span><Button variant="quiet" size="icon" aria-label={t("Close map selection")} onClick={() => setMapMatches([])}><X size={16} /></Button></div>
              <h2>{t("{count} scenes in this footprint", { count: mapMatches.length })}</h2>
              <p>{t("Overlapping dates share a footprint. Check the scenes you want to load.")}</p>
              <div className="footprint-select-actions"><Button size="sm" onClick={() => updateSelection(current => [...new Set([...current, ...mapMatches])])}>{t("Select these scenes")}</Button><Button size="sm" onClick={() => updateSelection(current => current.filter(id => !mapMatches.includes(id)))}>{t("Remove these scenes")}</Button></div>
              <div className="footprint-match-list">{mapMatches.map(id => sceneById.get(id)).filter(Boolean).sort((a, b) => b.date.localeCompare(a.date)).map(scene => <label key={scene.id} className="footprint-match">
                <Input type="checkbox" checked={selectedIds.includes(scene.id)} onChange={() => toggleScene(scene.id)} aria-label={t("Select scene {date} {id}", { date: date(scene.date), id: scene.id })} />
                <span><strong>{date(scene.date)}</strong><small>{scene.properties["grid:code"] || scene.id} · {scene.cloud == null ? t("Unknown") : number(scene.cloud / 100, { style: "percent", maximumFractionDigits: 1 })}</small></span>
              </label>)}</div>
              {canLoadMap && <div className="inspector-bottom"><Button primary onClick={loadSelected} disabled={awaitingSelectedGrid || preparingImagery || !selectedIds.length || selectedIds.length > 16}>{awaitingSelectedGrid ? t('Reading imagery metadata…') : t("Load selected imagery · {count}", { count: selectedIds.length })}</Button></div>}
            </aside> : selected && (
              <aside className="inspector">
                <div className="inspector-heading">
                  <span className="eyebrow">{t("DATASET DETAILS")}</span>
                  <Button
                    className="icon-btn"
                    aria-label={t("Close details panel")}
                    onClick={() => setInspector(false)}
                  >
                    <X size={16} />
                  </Button>
                </div>
                <h2>{t(selected.dataset || "Sentinel-2 L2A")}</h2>
                <p className="muted">{t(isAerial ? 'Aerial imagery · four original channels' : isSrtm ? "SRTMGL1 v003 · February 2000 · 1 arc-second" : isElevation ? "Digital surface model · 2021 public release" : isVegetation ? "Vegetation index collection" : "Surface reflectance collection")}</p>
                {loadedScenes.length > 1 && <Disclosure className="loaded-layer-disclosure" summary={t("Loaded layers · {count}", { count: loadedScenes.length })}>
                  <p>{t("Overlapping COGs cover one another. Hide a layer here or change the front layer in the map toolbar.")}</p>
                  {loadedScenes.slice().reverse().map(scene => <label key={scene.id} className="loaded-layer-row">
                    <Input type="checkbox" checked={visibleLoadedIds.includes(scene.id)} disabled={visibleLoadedIds.length === 1 && visibleLoadedIds.includes(scene.id)} onChange={event => {
                      const next = event.currentTarget.checked ? [...visibleLoadedIds, scene.id] : visibleLoadedIds.filter(id => id !== scene.id);
                      setVisibleLoadedIds(next);
                      if (!next.includes(selected.id)) setSelected(loadedScenes.find(item => next.includes(item.id) && (!activeDay || item.date.slice(0, 10) === activeDay)) || null);
                    }} aria-label={t("Show imagery layer {date} {id}", { date: date(scene.date), id: scene.id })} />
                    <span>{date(scene.date)} · {scene.properties["grid:code"] || scene.id}</span>
                  </label>)}
                </Disclosure>}
                <div className="detail-section">
                  <h3>{t(isElevation ? "Surface elevation" : "Observation")}</h3>
                  <dl>
                    {isElevation ? <><dt>{t("Height reference")}</dt><dd>{isSrtm ? 'EGM96 · EPSG:5773' : 'EGM2008 · EPSG:3855'}</dd><dt>{t("Height unit")}</dt><dd>{t("metres")}</dd></> : <><dt>{t(isComposite ? "Composite period" : "Acquired")}</dt>
                    <dd>{compositePeriodLabel(selected, date)}</dd>
                    {!isAerial && !isComposite && !isRadar && <><dt>{t("Scene clouds")}</dt>
                    <dd>{selected.cloud == null ? t("Unknown") : number(selected.cloud / 100, { style: "percent", maximumFractionDigits: 2 })}</dd></>}</>}
                    {isRadar && <><dt>{t('Polarization')}</dt><dd>{selected.properties['sar:polarizations']?.join(' / ')}</dd><dt>{t('Orbit direction')}</dt><dd>{t(selected.properties['sat:orbit_state'] === 'ascending' ? 'Ascending' : selected.properties['sat:orbit_state'] === 'descending' ? 'Descending' : 'Orbit unavailable')}</dd></>}
                    <dt>{t(isElevation ? "Nominal resolution" : canLoadMap ? "RGB resolution" : "Spatial resolution")}</dt>
                    <dd>{isSrtm ? t('1 arc-second · approximately 30 m') : selected.gsd ? t("{resolution} meters", { resolution: number(selected.gsd) }) : t("Not specified")}</dd>
                    <dt>{t("Source grid")}</dt>
                    <dd className="mono">{selected.crs || t("Not specified")}</dd>
                  </dl>
                  <p className="scene-id mono">{selected.id}</p>
                  <Button className="text-link" onClick={() => setModal("provenance")}>{t("View metadata & source")}<ArrowUpRight size={14} /></Button>
                </div>
                <div className="detail-section">
                  <h3>{t("Area & output")}</h3>
                  <dl>
                    <dt>{t("Saved area")}</dt>
                    <dd>{t(areaName)}</dd>
                    <dt>{t("Selection")}</dt>
                    <dd>{t("Bounding box")}</dd>
                  </dl>
                  <Button
                    className="text-link"
                    onClick={() => setModal("area")}
                  >{t("Inspect area")}<ArrowUpRight size={14} />
                  </Button>
                </div>
                {!exploringProjectId && canLoadMap && <div className="inspector-bottom">
                  <DownloadAssetButton key={[selected.id, ...downloadScenes.map(scene => scene.id)].join('|')} scene={selected} scenes={downloadScenes} areaBounds={bbox} areaPolygon={areaPolygon?.geometry} areaName={areaName} onOpenProject={openProject} />
                </div>}
                {!sourceProvider.download && <div className="inspector-bottom"><Button asChild><a href={sourceProvider.id === 'nasa-earthdata' ? 'https://search.earthdata.nasa.gov/' : 'https://browser.dataspace.copernicus.eu/'} target="_blank" rel="noreferrer"><ExternalLink size={16}/>{t(sourceProvider.id === 'nasa-earthdata' ? 'Open Earthdata Search' : 'Open Copernicus Browser')}</a></Button></div>}
              </aside>
            )}
            </ResizablePanel>}
          </ResizableGroup>
        ) : (
          <main className="content-page">
            <div className="content-stack">
            {page === "Tasks" ? (
              <RuntimeTasks areaBounds={bbox} areaPolygon={areaPolygon} />
            ) : page === "My Data" ? (
              <>
                <LibraryPage focusedProjectId={focusedProjectId} onOpenProject={openProject} onCloseProject={() => go('My Data')} onContinueExploring={continueInProject} areaBounds={bbox} areaPolygon={areaPolygon}/>
              </>
            ) : page === "Settings" ? (
              <SettingsPage theme={theme} onThemeChange={setTheme} onOpenRasterSources={() => setStacOpen(true)} onOpenCoverageSources={() => setWcsOpen(true)} areaBounds={bbox}/>
            ) : (
              <EmptyState
                title={t("Choose a workspace page")}
                action={<Button onClick={() => go("Explore")}>{t("Explore")}</Button>}
              >{t("Use the navigation to return to your data.")}</EmptyState>
            )}
            </div>
          </main>
        )}
      </div>
      </ResizablePanel>}
      {agentVisible && workAreaVisible && <ResizeHandle key="agent-divider" label={t('Resize Agent panel')} hint={resizeHint}/>}
      {agentVisible && <ResizablePanel key="agent-pane" id="agent-pane" defaultSize={previewing ? '42%' : home ? undefined : 360} minSize={home||previewing ? 320 : 280} maxSize={home||previewing ? undefined : 640} groupResizeBehavior="preserve-pixel-size">
        <AgentPanel closeRef={agentClose} variant={home ? 'home' : 'sidebar'} onChooseSource={exploreSource} onOpenSourceEntry={openSourceEntry} onPreviewPlan={setMapPreview} context={agentMapContext({page,input:searchInput,appliedSearch,areaPolygon,projectId:currentProject?.id || focusedProjectId})} onClose={() => {setMapPreview(null);setAgentOpen(false);}} onOpenProject={openProject} onOpenTasks={id => { location.hash = 'Tasks?job=' + id; }} onOpenResult={view=>{location.hash='Workspace?'+(view.kind==='vector'?'vector':'file')+'='+view.id;}}/>
      </ResizablePanel>}
      {previewing&&<ResizeHandle key="agent-map-preview-divider" label={t('Resize map preview panel')} hint={resizeHint}/>}
      {previewing&&<ResizablePanel key="agent-map-preview-pane" id="agent-map-preview-pane" defaultSize="58%" minSize={320}>
        <React.Suspense fallback={<p role="status"><Spinner/>{t('Reading task area…')}</p>}><AgentPlanMapPreview {...mapPreview} onClose={()=>setMapPreview(null)}/></React.Suspense>
      </ResizablePanel>}
      </ResizableGroup>
      {filtersOpen && page === 'Explore' && <CatalogFilters initialValues={searchInput} onClose={() => setFiltersOpen(false)} onApply={applyFilters} onArea={values => {
        setSearchInput(values);
        setFiltersOpen(false);
        setModal('area');
      }} />}
      {wcsOpen && <WcsSourceDialog areaBounds={bbox} currentProject={currentProject} onClose={() => setWcsOpen(false)}/>}
      {stacOpen && <StacSourceDialog areaBounds={bbox} currentProject={currentProject} onClose={() => setStacOpen(false)}/>}
      {sourceEntry?.kind === 'stac' && <StacSourceDialog key={`${sourceEntry.id}:${sourceEntry.generation}`} initialKind={sourceEntry.sourceType} areaBounds={bbox} currentProject={currentProject} onClose={()=>setSourceEntry(null)}/>}
      {sourceEntry?.kind === 'wcs' && <WcsSourceDialog key={`${sourceEntry.id}:${sourceEntry.generation}`} areaBounds={bbox} currentProject={currentProject} onClose={()=>setSourceEntry(null)}/>}
      {sourceEntry?.kind === 'map' && <MapServiceDialog key={`${sourceEntry.id}:${sourceEntry.generation}`} initialProtocol={sourceEntry.protocol} initialPreset={sourceEntry.preset} areaBounds={bbox} areaPolygon={areaPolygon} onClose={()=>setSourceEntry(null)}/>}
      {sourceEntry?.kind === 'vector' && <FeatureServiceDialog key={`${sourceEntry.id}:${sourceEntry.generation}`} initialProtocol={sourceEntry.protocol} areaBounds={bbox} areaPolygon={areaPolygon} onClose={()=>setSourceEntry(null)}/>}
      {sourceEntry?.kind === 'tiles' && <TileSourceDialog key={`${sourceEntry.id}:${sourceEntry.generation}`} areaBounds={bbox} onClose={()=>setSourceEntry(null)}/>}
      {modal && (
        <Modal closeLabel={t("Close dialog")}
          title={
            t(typeof modal === "object"
              ? "Export JSON"
              : {
                  area: "Select search area",
                  provenance: "Data provenance",
                  about: "About this workspace",
                }[modal])
          }
          onClose={() => setModal(null)}
          wide={modal === "area"}
        >
          {modal === "area" ? (
            <React.Suspense fallback={<p role="status"><Spinner/>{t("Loading reference map…")}</p>}><AreaPicker initialBbox={pendingBounds} onApply={applyMapArea} onExport={exportMapArea} onClose={() => setModal(null)}/></React.Suspense>
          ) : modal === "provenance" && selected && catalog ? (
            <div className="dialog-body">
              <Badge tone="green">{t("LIVE CATALOG RESPONSE")}</Badge>
              <h3>{selected.id}</h3>
              <p>{catalog.attribution}</p>
              <dl>
                <dt>{t(isElevation ? "Catalog reference date" : isComposite ? "Composite period" : "Acquisition")}</dt>
                <dd>{compositePeriodLabel(selected, date)}</dd>
                <dt>{t("Metadata fetched")}</dt>
                <dd>{date(catalog.retrievedAt)}</dd>
                <dt>{t("Map source")}</dt>
                <dd>{t(isRadar ? 'Online radar previews display original linear gamma0 as −30–0 dB grayscale for the selected polarization. Downloads keep the original Float32 values. No extra calibration or speckle filtering is applied.' : isViirs ? 'VIIRS footprints and public browse images. Authorized downloads retain the complete original HDF5. Prepare M5, M4 and M3 in the project to view and process local science bands; no QA mask is applied.' : isVegetation ? 'Online NDVI/EVI previews use Planetary Computer PNG tiles in Web Mercator, colored from −0.2 to 1.0 with the RdYlGn palette. No quality mask is applied. Original downloads retain signed DN and the sinusoidal grid; science and QA layers are available separately.' : isComposite ? 'Online MODIS RGB previews use bands 1, 4 and 3 with reflectance 0–0.3 and gamma 2.2. Downloads retain signed DN and the sinusoidal grid. No quality mask is applied; QA layers are separate.' : isAerial ? 'Original four-band aerial COG; RGB display uses bands 1–3. Near-infrared is retained and can be read from downloaded originals in Workspace.' : isSrtm ? 'SRTM tile footprints and provider browse images. Download original HGT ZIPs to inspect signed heights in Workspace.' : isElevation ? 'Online height previews read original Float32 COG samples and preserve the Point grid. Grayscale stretches −100–1000 metres above EGM2008 for display only. Downloaded originals can be queried in Workspace.' : sourceProvider.id === 'planetary-landsat' ? 'Original red, green and blue COGs; display reflectance 0–0.3 with gamma 2.2. Downloads retain original DN.' : canLoadMap ? 'Georeferenced true-color COG; list thumbnails are provider previews' : sourceProvider.id === 'copernicus' ? 'Scene footprints and provider thumbnails; original downloads are complete SAFE product archives' : sourceProvider.download ? 'Scene footprints and provider thumbnails; downloads contain original reflectance bands' : 'Scene footprints and provider thumbnails; original product access requires authorization')}</dd>
                {!isElevation && !isAerial && !isComposite && !isRadar && <><dt>{t("Cloud cover")}</dt>
                <dd>{t("Full scene, not AOI-specific")}</dd></>}
              </dl>
              <p>
                {t(isRadar ? "RTC is a provider-derived radar product. Its calibration and terrain correction come from the provider; this application does not claim to process raw GRD or SLC." : isViirs ? "VIIRS pixels are selected within an 8-day composite period. Browse images do not prove cloud-free coverage or original-data access." : isVegetation ? "Different pixels in a 16-day composite may represent different observation dates. No QA mask is applied here; the index layers do not prove cloud-free coverage." : isComposite ? "NASA MODIS uses per-pixel observations selected within an 8-day period. No scene-cloud filter or automatic quality masking is applied." : isAerial ? "USDA NAIP is aerial imagery with red, green, blue and near-infrared channels. Dates vary by state; cloud filtering does not apply." : isSrtm ? "SRTMGL1 v003 is a void-filled surface elevation product. It uses EGM96 and shared boundary samples; it is not interchangeable with Copernicus DEM." : isElevation ? "This public DSM includes buildings and vegetation. Coverage is limited to published land tiles; missing tiles are not filled with invented heights." : "Catalog pages load automatically for the submitted area, dates and cloud limit. Footprints locate scenes. A thumbnail is a preview; loading COGs and downloading original files are separate actions.")}
              </p>
              {selected.sha256 && <p className="mono hash">{t("Cached preview SHA-256:")} {selected.sha256}</p>}
              <div className="link-stack">
                <a href={selected.itemURL} target="_blank" rel="noreferrer">{t("View source catalog")}<ExternalLink size={14} />
                </a>
                <a href={catalog.query} target="_blank" rel="noreferrer">{t("Original STAC query")}<ExternalLink size={14} />
                </a>
                <a href={catalog.registry} target="_blank" rel="noreferrer">{t("Dataset registry & terms")}<ExternalLink size={14} />
                </a>
              </div>
            </div>
          ) : typeof modal === "object" && modal.type === "json" ? (
            <>
              <div className="dialog-body">
                <p>{modal.filename}</p>
                <p className="muted">{t("Review or copy the full file below. If your embedded browser blocks downloads, open this local preview in your regular browser.")}</p>
                <Textarea
                  className="json-preview mono"
                  aria-label={t("Exported JSON")}
                  readOnly
                  value={JSON.stringify(modal.value, null, 2)}
                />
              </div>
              <div className="dialog-footer">
                <Button onClick={() => setModal(null)}>{t("Close")}</Button>
                <Button
                  primary
                  icon={Download}
                  onClick={() => downloadJSON(modal.filename, modal.value)}
                >{t("Save JSON file")}</Button>
              </div>
            </>
          ) : (
            <div className="dialog-body">
              <GeoDBrand />
              <h3>GeoD Global <Badge>{RELEASE_VERSION}</Badge></h3>
              <p>{t("Search public imagery catalogs, download supported source files, inspect pixels, and clip local rasters by rectangle or administrative polygon.")}</p>
              <ul>
                <li>{t('Search public Sentinel-2, Landsat, MODIS, Sentinel-1 RTC, NAIP and elevation catalogs.')}</li>
                <li>{t('Download verified public files into named projects, inspect pixels, and process compatible local grids.')}</li>
                <li>{t('Reopen local files and previews offline. Closing the window keeps tasks running in the tray.')}</li>
                <li>{t('NASA and Copernicus account setup is included; protected original downloads are deferred until real-account verification.')}</li>
              </ul>
              <p className="muted">{t('Release candidate. Check the included release notes for supported products and limits. Software updates and notifications are in Settings.')}</p>
              <p className="muted">{t("Inter is bundled locally. Catalog searches, imagery previews and asset downloads contact their source providers. This workspace sends no analytics.")}</p>
            </div>
          )}
        </Modal>
      )}
    </div>
  );
}
if (document.getElementById("root")) createRoot(document.getElementById("root")).render(<I18nProvider><RuntimeProvider><App /></RuntimeProvider></I18nProvider>);
