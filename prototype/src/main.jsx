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
  MousePointer2,
} from "lucide-react";
import "./ui/foundation.css";
import { Button, Badge, Input, Textarea, Select, Modal, EmptyState,
  Disclosure, Surface, SidebarNav,
  SegmentedControl, Spinner } from "./ui/index.jsx";
import "./styles.css";
import "./catalog.css";
import { SAMPLE_BBOX, defaultLiveSearch, searchURL, validateBounds, validateSearch, compatibleScenes, createSearchRunner } from "./catalog.js";
import { RuntimeProvider, DownloadAssetButton, RuntimeTasks, RuntimeLibrary } from "./runtime-ui.jsx";
import { ProjectsLibrary, SaveProjectButton } from "./projects-ui.jsx";
import { scenesForDownload } from "./projects-client.js";
import { runtimeRequest } from "./runtime-client.js";
import { mergeProjectCatalog, projectCatalogScenes, projectExploreSearch } from "./project-explore.js";
import { ExecutableRecipes } from "./processing-ui.jsx";
import { DiagnosticsPanel } from "./diagnostics-ui.jsx";
import { ProxySettingsPanel } from "./proxy-ui.jsx";
import { I18nProvider, useI18n } from "./i18n.jsx";

const WorkspaceMap = React.lazy(() => import("./workspace-map.jsx").then(module => ({ default: module.WorkspaceMap })));
const AreaPicker = React.lazy(() => import("./area-picker.jsx").then(module => ({ default: module.AreaPicker })));
const ExploreMap = React.lazy(() => import("./explore-map.jsx").then(module => ({ default: module.ExploreMap })));

const nav = [
  ["Explore", Compass],
  ["Workspace", Layers],
  ["My Data", Folder],
  ["Tasks", ListTodo],
];
const pageFromHash = () => {
  let requested;
  try { requested = decodeURIComponent(location.hash.slice(1).split('?')[0]); }
  catch { return "Explore"; }
  if (requested === "Recipes") return "My Data";
  return [...nav.map(([name]) => name), "Settings"].includes(requested) ? requested : "Explore";
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
  useEffect(() => setFailed(false), [src]);
  return src && !failed ? <img src={src} alt={alt} loading="lazy" onError={() => setFailed(true)} /> : <span className="catalog-thumbnail-missing" role="img" aria-label={t("Preview unavailable: {description}", { description: alt })}>{t("Preview unavailable")}</span>;
}

function App() {
  const { t, locale, setLocale, date, number } = useI18n();
  const [liveCatalog, setLiveCatalog] = useState(null);
  const [searchInput, setSearchInput] = useState(defaultLiveSearch);
  const [liveState, setLiveState] = useState("idle");
  const [liveError, setLiveError] = useState("");
  const [appliedSearch, setAppliedSearch] = useState(null);
  const [areaPolygon, setAreaPolygon] = useState(null);
  const searchRunner = useRef(null);
  if (!searchRunner.current) searchRunner.current = createSearchRunner();
  const catalog = liveCatalog;
  let pendingBounds = SAMPLE_BBOX;
  try { pendingBounds = validateBounds(searchInput.bbox); }
  catch { pendingBounds = appliedSearch?.bbox || SAMPLE_BBOX; }
  const bbox = appliedSearch?.bbox || pendingBounds;
  const areaName = areaPolygon?.place?.name || "Custom search area";
  const [page, setPage] = useState(pageFromHash);
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
  const [mapMatches, setMapMatches] = useState([]);
  const [boxSelect, setBoxSelect] = useState(false);
  useEffect(() => { setMapMatches([]); }, [query]);
  const exploreMap = useRef(null);
  const timelineTrack = useRef(null);
  const sceneListRef = useRef(null);
  const listEndRef = useRef(null);
  const [visibleListCount, setVisibleListCount] = useState(100);
  const [footprintCount, setFootprintCount] = useState(0);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [discoveryCollapsed, setDiscoveryCollapsed] = useState(false);
  const [navCollapsed, setNavCollapsed] = useState(stored("nav-collapsed", false));
  const [compare, setCompare] = useState(false),
    [compareId, setCompareId] = useState(""),
    [split, setSplit] = useState(50),
    [showArea, setShowArea] = useState(true),
    [inspector, setInspector] = useState(window.innerWidth >= 1280);
  const [modal, setModal] = useState(null),
    [theme, setTheme] = useState(stored("theme", "light"));
  const runSearch = async (submittedInput = searchInput, restoredProject = null) => {
    let submitted, url;
    try {
      submitted = validateSearch({ ...submittedInput, limit: 100 });
      url = searchURL(submitted);
    } catch (error) { setLiveError(error.message); return; }
    setLiveError("");
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
        if (savedScenes.length) {
          setSelected(current => merged.scenes.find(scene => scene.id === current?.id) || current);
          if (!previewRestored) {
            const savedIds = new Set(savedScenes.map(scene => scene.id));
            const visible = merged.scenes.filter(scene => savedIds.has(scene.id) && scene.assets.visual && scene.grid?.shape?.length === 2 && scene.grid?.transform?.length === 6).slice(0, 16);
            if (visible.length) {
              previewRestored = true;
              setLoadedIds(visible.map(scene => scene.id));
              setVisibleLoadedIds(visible.map(scene => scene.id));
              setSelected(visible[0]);
            }
          }
        }
      } });
      if (!result) return;
      setLiveState("ready");
      setFiltersOpen(false);
    } catch (error) {
      setLiveError(error.name === "AbortError" ? "Search cancelled." : error.name === "TimeoutError" ? "Earth Search did not respond within 30 seconds. Try again." : error.message);
      setLiveState("error");
      setFiltersOpen(true);
    }
  };
  useEffect(() => {
    if (!(pageFromHash() === 'Explore' && projectFromHash())) runSearch(searchInput);
    return () => searchRunner.current.cancel();
  }, []);
  const cancelSearch = () => { searchRunner.current.cancel(); setLiveState("idle"); setLiveError("Search cancelled. Run a search to retrieve scenes."); };
  const catalogError = (message) => {
    const httpError = /^Earth Search returned HTTP (\d+)\. Try again later\.$/.exec(message);
    return httpError ? t("Earth Search returned HTTP {status}. Try again later.", { status: httpError[1] }) : t(message);
  };
  const updateSearchField = (event) => {
    const { name, value } = event.currentTarget;
    if (name === 'bbox') setAreaPolygon(null);
    setSearchInput((current) => ({ ...current, [name]: name === "cloud" || name === "cloudMin" || name === "limit" ? Number(value) : value }));
  };
  const submitSearch = (event) => {
    event.preventDefault();
    // Submit exactly the values visible in native form controls, including date pickers.
    const submittedInput = Object.fromEntries(new FormData(event.currentTarget));
    setSearchInput((current) => ({ ...current, ...submittedInput }));
    runSearch(submittedInput);
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
      const hash = "#" + encodeURIComponent(next) + (project ? `?project=${project}` : '');
      if (location.hash !== hash) history.replaceState(null, "", hash);
      setPage(next);
      setFocusedProjectId(next === 'My Data' ? project : null);
      setExploringProjectId(next === 'Explore' ? project : null);
    };
    window.addEventListener("hashchange", change);
    change();
    return () => window.removeEventListener("hashchange", change);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem("geod-design-theme", JSON.stringify(theme));
  }, [theme]);
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
      (!activeDay || s.date.slice(0, 10) === activeDay)
      && (s.id.toLowerCase().includes(query.toLowerCase()) || s.date.includes(query)),
    )
    .sort((a, b) => sort === "cloud" ? (a.cloud ?? 101) - (b.cloud ?? 101) : b.date.localeCompare(a.date)), [scenes, activeDay, query, sort]);
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
  const loadedScenes = useMemo(() => loadedIds.map(id => sceneById.get(id)).filter(Boolean), [loadedIds, sceneById]);
  const downloadScenes = useMemo(() => scenesForDownload({ scenes, selectedIds, loadedIds, currentScene: selected }), [scenes, selectedIds, loadedIds, selected]);
  const visibleLoadedScenes = useMemo(() => loadedScenes.filter(scene => visibleLoadedIds.includes(scene.id)), [loadedScenes, visibleLoadedIds]);
  const days = useMemo(() => [...new Set(scenes.map(scene => scene.date.slice(0, 10)))].sort(), [scenes]);
  const dayCounts = useMemo(() => scenes.reduce((counts, scene) => {
    const day = scene.date.slice(0, 10);
    counts.set(day, (counts.get(day) || 0) + 1);
    return counts;
  }, new Map()), [scenes]);
  const toggleScene = id => setSelectedIds(current => current.includes(id) ? current.filter(value => value !== id) : [...current, id]);
  const loadSelected = () => {
    const chosen = selectedIds.map(id => sceneById.get(id)).filter(scene => scene?.assets?.visual?.href);
    if (!chosen.length || chosen.length > 16) return;
    setLoadedIds(chosen.sort((a, b) => a.date.localeCompare(b.date)).map(scene => scene.id));
    setVisibleLoadedIds(chosen.map(scene => scene.id));
    setSelected(chosen.filter(scene => !activeDay || scene.date.slice(0, 10) === activeDay).sort((a, b) => b.date.localeCompare(a.date))[0] || null);
    setMapMatches([]);
  };
  const showDay = day => {
    setActiveDay(day);
    setMapMatches([]);
    setSelected(visibleLoadedScenes.filter(scene => !day || scene.date.slice(0, 10) === day).sort((a, b) => b.date.localeCompare(a.date))[0] || null);
  };
  const comparisons = scenes.filter((s) => compatibleScenes(selected, s));
  const other = comparisons.find((s) => s.id === compareId) || comparisons[0];
  const comparing = compare && !!other;
  const workspace = page === "Explore" || page === "Workspace";
  return (
    <div className="app">
      <SidebarNav
        className={navCollapsed ? "nav-collapsed" : ""}
        ariaLabel={t("GeoD home")}
        brand={<a className="brand" href="#Explore" aria-label={t("GeoD home")}><span className="brand-mark"><Layers size={20} /></span><strong>{t("GeoD")}</strong></a>}
        items={nav.map(([name, icon]) => ({ id: name, label: t(name), icon, href: "#" + encodeURIComponent(name), active: page === name }))}
        footerItems={[
          { id: "Toggle navigation", label: t(navCollapsed ? "Expand navigation" : "Collapse navigation"), icon: navCollapsed ? PanelLeftOpen : PanelLeftClose, onClick: () => setNavCollapsed(value => !value), "data-nav-toggle": true },
          { id: "Settings", label: t("Settings"), icon: Settings, href: "#Settings", active: page === "Settings" },
          { id: "Help", label: t("Help"), icon: HelpCircle, onClick: () => setModal("about") },
        ]}
        footer={<span className="sidebar-local-label"><ShieldCheck size={14} />{t("Local workspace")}</span>}
      />
      <div className="app-main">
        <header className="topbar">
          <div className="topbar-left">
            {page === "Explore" && discoveryCollapsed && <Button variant="secondary" size="sm" icon={PanelLeftOpen} className="discovery-toggle" aria-label={t("Show scene list")} aria-controls="explore-discovery" aria-expanded={false} onClick={() => setDiscoveryCollapsed(false)}>{t("Imagery scenes")}</Button>}
            <div className="breadcrumb">
            <span className="project-icon">
              <Folder size={16} />
            </span>
            <strong>{currentProject?.name || t("Earth Search workspace")}</strong>
            <ChevronRight size={14} />
            <span>{t(page)}</span>
            </div>
          </div>
          <div className="top-actions">
            {page === 'Explore' && exploringProjectId && <>
              <Button size="sm" icon={Folder} onClick={() => openProject(exploringProjectId)}>{t('Return to project details')}</Button>
              <Button size="icon" variant="quiet" aria-label={t('Leave project exploration')} title={t('Leave project exploration')} onClick={() => { setActiveProject(null); restoredProjectId.current = null; go('Explore'); }}><X size={15}/></Button>
            </>}
            <Button
              className="icon-btn"
              aria-label={t("Toggle color theme")}
              onClick={() => setTheme(theme === "light" ? "dark" : "light")}
            >
              {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
            </Button>
            <span className="local-status">
              <span />{t("Local workspace")}</span>
          </div>
        </header>
        {page === 'Explore' && exploringProjectId && !currentProject && !projectError && <p className="project-context-status" role="status"><Spinner size={15}/>{t('Loading project scenes…')}</p>}
        {page === 'Explore' && exploringProjectId && projectError && <p className="project-context-status projects-error" role="alert">{t(projectError)}</p>}
        {page === "Workspace" ? <React.Suspense fallback={<main className="wm-map-loading" role="status">{t("Loading local map…")}</main>}><WorkspaceMap /></React.Suspense> : workspace ? (
          <div className={"workspace " + (!inspector || (!selected && !mapMatches.length) ? "no-inspector " : "") + (discoveryCollapsed ? "no-discovery" : "")}>
            {!discoveryCollapsed && <aside className="discovery" id="explore-discovery">
              <div className="panel-heading">
                <div>
                  <h1>{t("Imagery scenes")}</h1>
                </div>
                <Button variant="quiet" size="icon" aria-label={t("Hide scene list")} title={t("Hide scene list")} aria-controls="explore-discovery" aria-expanded={true} onClick={() => setDiscoveryCollapsed(true)}><PanelLeftClose size={18} /></Button>
              </div>
              <Button className="area-picker" onClick={() => setModal("area")}>
                <MapPin size={17} />
                <span>
                  <strong>{t(areaName)}</strong>
                  <small>{t("WGS 84 · editable search bounds")}</small>
                </span>
                <ChevronDown size={16} />
              </Button>
              {appliedSearch && <div className="catalog-active-filters">{date(appliedSearch.start)} – {date(appliedSearch.end)} · {t("clouds {minimum}–{maximum}", { minimum: number(appliedSearch.cloudMin / 100, { style: "percent" }), maximum: number(appliedSearch.cloud / 100, { style: "percent" }) })}</div>}
              <>
                  <label className="search-input scene-search">
                    <Search size={16} />
                    <Input aria-label={t("Search scenes")} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Search scene ID or date")} />
                  </label>
                  {liveError && <p className="catalog-error" role="alert">{catalogError(liveError)}</p>}
                  <div className="filters" id="scene-filters" hidden={!filtersOpen}>
                    <form className="catalog-form" onSubmit={submitSearch}>
                      <label>{t("WGS 84 bounds · west, south, east, north")}<Input name="bbox" aria-label={t("Search bounding box")} value={searchInput.bbox} onChange={updateSearchField} />
                      </label>
                      <Button type="button" icon={SquareDashed} onClick={() => setModal("area")}>{t("Draw area on map")}</Button>
                      <div className="catalog-dates">
                        <label>{t("From (UTC)")}<Input name="start" aria-label={t("Search start date")} type="date" value={searchInput.start} onInput={updateSearchField} onChange={updateSearchField} /></label>
                        <label>{t("Through (UTC)")}<Input name="end" aria-label={t("Search end date")} type="date" value={searchInput.end} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      </div>
                      <label className="range-label"><span>{t("Scene cloud cover ≥ {percent}", { percent: number(Number(searchInput.cloudMin) / 100, { style: "percent" }) })}</span><Input name="cloudMin" type="range" aria-label={t("Minimum cloud cover")} min="0" max="100" value={searchInput.cloudMin} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      <label className="range-label"><span>{t("Scene cloud cover ≤ {percent}", { percent: number(Number(searchInput.cloud) / 100, { style: "percent" }) })}</span><Input name="cloud" type="range" aria-label={t("Live maximum cloud cover")} min="0" max="100" value={searchInput.cloud} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      <div className="catalog-search-actions"><Button primary icon={Search} type="submit">{t("Search catalog")}</Button></div>
                    </form>
                    {appliedSearch && <p className="catalog-query-note">{t("{status}: {start} – {end} · clouds {minimum}–{maximum} · [{bbox}]", { status: catalog ? t("Showing") : t("Requested"), start: date(appliedSearch.start), end: date(appliedSearch.end), minimum: number(appliedSearch.cloudMin / 100, { style: "percent" }), maximum: number(appliedSearch.cloud / 100, { style: "percent" }), bbox: appliedSearch.bbox.join(", ") })}</p>}
                  </div>
                  <div className="catalog-fetch-status" role="status">
                    {liveState === "loading" ? <><Spinner />{t("Fetching all catalog pages… {count} scenes from {pages} pages", { count: scenes.length, pages: catalog?.pages || 0 })}<Button variant="quiet" size="xs" onClick={cancelSearch}>{t("Stop catalog search")}</Button></>
                      : catalog?.complete ? t("Catalog complete · {count} scenes", { count: scenes.length })
                        : catalog ? t("Catalog partial · {count} scenes", { count: scenes.length }) : null}
                  </div>
                  <div className="results-heading">
                    <span className="results-count">
                      <strong>{filtered.length}</strong><span className="results-count-label"> {t("scenes")}</span>
                    </span>
                    <div className="results-actions">
                      <Button size="sm" icon={SlidersHorizontal} className="filter-toggle" aria-label={t("Filters")} aria-expanded={filtersOpen} aria-controls="scene-filters" onClick={() => setFiltersOpen(value => !value)}>{t("Filters")}</Button>
                      <Select aria-label={t("Sort scenes")} value={sort} onChange={(e) => setSort(e.target.value)}>
                        <option value="date">{t("Newest")}</option>
                        <option value="cloud">{t("Clearest")}</option>
                      </Select>
                    </div>
                  </div>
                  <div className="scene-list" ref={sceneListRef}>
                    {liveState === "loading" && !catalog ? <div className="loading-state" role="status"><Spinner />{t("Searching Earth Search…")}</div> : !catalog ? <EmptyState icon={Search} title={t(liveState === "error" ? "Catalog request failed" : "Search the live catalog")}>{t("Set your area and dates above, then search Earth Search for imagery.")}</EmptyState> : !filtered.length ? (
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
                      >{t(query ? "No scene ID or date matches this text." : "Try a wider date range or allow more cloud cover.")}</EmptyState>
                    ) : (
                      filtered.slice(0, visibleListCount).map((s) => (
                        <Button variant="quiet" size="row" aria-pressed={selectedIds.includes(s.id)}
                          key={s.id}
                          className={
                            "scene " + (selectedIds.includes(s.id) ? "selected" : "")
                          }
                          onClick={() => toggleScene(s.id)}
                          aria-label={t("Select scene {date} {id}", { date: date(s.date), id: s.id })}
                        >
                          <SceneThumbnail
                            src={s.thumbnail}
                            alt={t("True-color preview, {date}", { date: date(s.date) })}
                          />
                          <div className="scene-info">
                            <strong>{date(s.date)}</strong>
                            <span>
                              {s.id.startsWith("S2C")
                                ? t("Sentinel-2C")
                                : s.id.startsWith("S2B")
                                  ? t("Sentinel-2B")
                                  : t("Sentinel-2A")}{" "}
                              <span className="muted">{t("· L2A")}</span>
                            </span>
                            <small>
                              <Cloud size={12} />
                              {s.cloud == null ? t("Unknown") : number(s.cloud / 100, { style: "percent", maximumFractionDigits: 1 })}<span>{s.gsd ? t("{resolution} m RGB", { resolution: number(s.gsd) }) : t("RGB preview")}</span>
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
                  {catalog && <div className="catalog-selection-actions">
                    <div><strong>{t("{count} scenes selected", { count: selectedIds.length })}</strong><span>{t("Click footprints or drag a box on the map to choose scenes.")}</span></div>
                    <div className="catalog-selection-buttons">
                      <Button size="sm" onClick={() => setSelectedIds(current => [...new Set([...current, ...filtered.map(scene => scene.id)])])} disabled={!filtered.length}>{t("Select filtered · {count}", { count: filtered.length })}</Button>
                      <Button size="sm" onClick={() => setSelectedIds([])} disabled={!selectedIds.length}>{t("Clear")}</Button>
                    </div>
                    <Button primary onClick={loadSelected} disabled={!selectedIds.length || selectedIds.length > 16}>{t("Load selected imagery · {count}", { count: selectedIds.length })}</Button>
                    {exploringProjectId ? currentProject && <DownloadAssetButton key={[exploringProjectId, ...downloadScenes.map(scene => scene.id)].join('|')} scene={selected || downloadScenes[0]} scenes={downloadScenes} areaBounds={bbox} areaPolygon={areaPolygon?.geometry} areaName={areaName} project={currentProject} onProjectUpdated={setActiveProject} onOpenProject={openProject}/> : <SaveProjectButton scenes={selectedIds.map(id => sceneById.get(id)).filter(Boolean)} bounds={bbox} geometry={areaPolygon?.geometry} areaName={areaName} onSaved={project => openProject(project.id)}/>}
                    {selectedIds.length > 16 && <span className="selection-limit">{t("Select at most 16 COGs for this browser map. Narrow the filters or clear some scenes.")}</span>}
                  </div>}
                  <div className="panel-foot">
                    <Database size={13} />
                    <span>{t("Earth Search ·")} {t("live HTTPS catalog")}{catalog && <> · {t("{count} footprints on map", { count: footprintCount })}</>}</span>
                  </div>
              </>
            </aside>}
            {scenes.length ? <main className="map-workspace">
              <div className="map-toolbar">
                <SegmentedControl className="preview-mode" aria-label={t("Preview")} value={compare ? "compare" : "preview"}
                  onValueChange={value => { setCompare(value === "compare"); if (value === "compare") setCompareId(other?.id || ""); }}
                  items={[
                    { value: "preview", label: t("Preview"), icon: Layers },
                    { value: "compare", label: t("Compare"), icon: SlidersHorizontal, disabled: !comparisons.length,
                      title: t(comparisons.length ? "Compare scenes with matching source grids" : "Comparison needs two true-color COGs with the same CRS, transform and dimensions") },
                  ]} />
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
              {selected && !comparisons.length && <p className="catalog-compare-note">{t("Comparison needs another scene with the same CRS, transform and dimensions.")}</p>}
              <div className="imagery-canvas">
                <React.Suspense fallback={<div className="explore-map-loading" role="status">{t("Loading georeferenced imagery…")}</div>}>
                  <ExploreMap ref={exploreMap} scene={selected || filtered[0] || scenes[0]} scenes={filtered} loadedScenes={visibleLoadedScenes} selectedIds={selectedIds} focusedIds={mapMatches} activeSceneId={selected?.id} activeDay={activeDay} reference={comparing ? other : null} split={split} area={bbox} areaGeometry={areaPolygon?.geometry} showArea={showArea} boxSelect={boxSelect} onFootprintsPick={ids => { setMapMatches(ids); if (ids.length) setInspector(true); }} onFootprintsChange={setFootprintCount} />
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
                            {date(s.date)}
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
              <div className="map-attribution">
                <span>{t("Copernicus Sentinel data ({year}) · Earth Search · Natural Earth overview", { year: (selected || scenes[0]).date.slice(0, 4) })}</span>
                <Button variant="quiet" disabled={!selected} onClick={() => setModal("provenance")}>{t("Georeferenced COG display · source details")}<Info size={12} />
                </Button>
              </div>
              <div className="timeline">
                <Surface as="div" variant="inset" className="scene-caption" aria-label={t("Imagery selection status")}>
                  <Badge>{t("SENTINEL-2 L2A")}</Badge>
                  <h2>{activeDay
                    ? t("{count} candidates · {visible}/{total} loaded visible", { count: filtered.length, visible: visibleLoadedScenes.filter(scene => scene.date.slice(0, 10) === activeDay).length, total: loadedScenes.length })
                    : t("{count} candidates · {loaded} loaded", { count: filtered.length, loaded: visibleLoadedScenes.length })}</h2>
                  {selected && visibleLoadedScenes.length > 1 ? <Select aria-label={t("Front imagery layer")} value={selected.id} onChange={event => setSelected(sceneById.get(event.target.value))}>
                    {visibleLoadedScenes.filter(scene => !activeDay || scene.date.slice(0, 10) === activeDay).map(scene => <option key={scene.id} value={scene.id}>{date(scene.date)} · {scene.properties["grid:code"] || scene.id}</option>)}
                  </Select> : <p>{selected ? `${date(selected.date)} · ${selected.properties["grid:code"] || selected.id}` : t("Choose footprints, then load imagery")}</p>}
                </Surface>
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
              </div>
            </main> : <main className="catalog-blank"><EmptyState icon={Search} title={t(liveState === "loading" ? "Searching your area" : liveCatalog ? "No scenes for this search" : "Choose your next observation")}>{t(liveState === "loading" ? "Catalog footprints will appear here as pages arrive." : "Choose an area, dates and cloud limit to find imagery footprints.")}</EmptyState></main>}
            {inspector && mapMatches.length > 0 ? <aside className="inspector footprint-inspector" aria-label={t("Scenes in the selected map area")}>
              <div className="inspector-heading"><span className="eyebrow">{t("MAP SELECTION")}</span><Button variant="quiet" size="icon" aria-label={t("Close map selection")} onClick={() => setMapMatches([])}><X size={16} /></Button></div>
              <h2>{t("{count} scenes in this footprint", { count: mapMatches.length })}</h2>
              <p>{t("Overlapping dates share a footprint. Check the scenes you want to load.")}</p>
              <div className="footprint-select-actions"><Button size="sm" onClick={() => setSelectedIds(current => [...new Set([...current, ...mapMatches])])}>{t("Select these scenes")}</Button><Button size="sm" onClick={() => setSelectedIds(current => current.filter(id => !mapMatches.includes(id)))}>{t("Remove these scenes")}</Button></div>
              <div className="footprint-match-list">{mapMatches.map(id => sceneById.get(id)).filter(Boolean).sort((a, b) => b.date.localeCompare(a.date)).map(scene => <label key={scene.id} className="footprint-match">
                <Input type="checkbox" checked={selectedIds.includes(scene.id)} onChange={() => toggleScene(scene.id)} aria-label={t("Select scene {date} {id}", { date: date(scene.date), id: scene.id })} />
                <span><strong>{date(scene.date)}</strong><small>{scene.properties["grid:code"] || scene.id} · {scene.cloud == null ? t("Unknown") : number(scene.cloud / 100, { style: "percent", maximumFractionDigits: 1 })}</small></span>
              </label>)}</div>
              <div className="inspector-bottom"><Button primary onClick={loadSelected} disabled={!selectedIds.length || selectedIds.length > 16}>{t("Load selected imagery · {count}", { count: selectedIds.length })}</Button></div>
            </aside> : inspector && selected && (
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
                <h2>{t("Sentinel-2 L2A")}</h2>
                <p className="muted">{t("Surface reflectance collection")}</p>
                {loadedScenes.length > 1 && <Disclosure className="loaded-layer-disclosure" summary={t("Loaded layers · {count}", { count: loadedScenes.length })}>
                  <p>{t("Overlapping COGs cover one another. Hide the front layer or choose another front layer below the map.")}</p>
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
                  <h3>{t("Observation")}</h3>
                  <dl>
                    <dt>{t("Acquired")}</dt>
                    <dd>{date(selected.date)}</dd>
                    <dt>{t("Scene clouds")}</dt>
                    <dd>{selected.cloud == null ? t("Unknown") : number(selected.cloud / 100, { style: "percent", maximumFractionDigits: 2 })}</dd>
                    <dt>{t("RGB resolution")}</dt>
                    <dd>{selected.gsd ? t("{resolution} meters", { resolution: number(selected.gsd) }) : t("Not specified")}</dd>
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
                {!exploringProjectId && <div className="inspector-bottom">
                  <DownloadAssetButton key={[selected.id, ...downloadScenes.map(scene => scene.id)].join('|')} scene={selected} scenes={downloadScenes} areaBounds={bbox} areaPolygon={areaPolygon?.geometry} areaName={areaName} onOpenProject={openProject} />
                </div>}
              </aside>
            )}
          </div>
        ) : (
          <main className="content-page">
            {page === "Tasks" ? (
              <>
                <PageHeading
                  eyebrow={t("EXECUTION")}
                  title={t("Tasks")}
                  sub={t("See downloads and clipping in progress, and retry failed tasks.")}
                  action={
                    <Button
                      onClick={() => {
                        go("Explore");
                      }}
                    >{t("Explore data")}</Button>
                  }
                />
                <RuntimeTasks areaBounds={bbox} areaPolygon={areaPolygon} />
              </>
            ) : page === "My Data" ? (
              <>
                {!focusedProjectId && <PageHeading
                  eyebrow={t("LOCAL LIBRARY")}
                  title={t("My Data")}
                  sub={t("Find downloaded files and clipping results ready to inspect or use.")}
                />}
                <ProjectsLibrary focusedProjectId={focusedProjectId} onOpenProject={openProject} onCloseProject={() => go('My Data')} onContinueExploring={continueInProject} />
                {!focusedProjectId && <RuntimeLibrary areaBounds={bbox} areaPolygon={areaPolygon} />}
                {!focusedProjectId && <Disclosure className="saved-clip-plans" summary={t("Saved clip plans · advanced")}>
                  <ExecutableRecipes areaBounds={bbox} areaPolygon={areaPolygon} onReviewJSON={showJSON} />
                </Disclosure>}
              </>
            ) : page === "Settings" ? (
              <>
                <PageHeading
                  eyebrow={t("YOUR WORKSPACE")}
                  title={t("Settings")}
                  sub={t("A local-first workspace, on your terms.")}
                />
                <Surface className="settings-list">
                  <div>
                    <span>
                      <strong>{t("Language")}</strong>
                      <small>{t("Applies immediately and stays on this device.")}</small>
                    </span>
                    <Select aria-label={t("Interface language")} value={locale} onChange={(event) => setLocale(event.target.value)}>
                      <option value="en" lang="en">English</option>
                      <option value="zh-CN" lang="zh-CN">简体中文</option>
                    </Select>
                  </div>
                  <div>
                    <span>
                      <strong>{t("Appearance")}</strong>
                      <small>{t("Saved on this browser.")}</small>
                    </span>
                    <Select
                      aria-label={t("Appearance")}
                      value={theme}
                      onChange={(e) => setTheme(e.target.value)}
                    >
                      <option value="light">{t("Light")}</option>
                      <option value="dark">{t("Dark")}</option>
                    </Select>
                  </div>
                </Surface>
                <ProxySettingsPanel />
                <Disclosure className="settings-diagnostics" summary={t("Local diagnostics · advanced")}><DiagnosticsPanel /></Disclosure>
              </>
            ) : (
              <EmptyState
                title={t("Choose a workspace page")}
                action={<Button onClick={() => go("Explore")}>{t("Explore")}</Button>}
              >{t("Use the navigation to return to your data.")}</EmptyState>
            )}
          </main>
        )}
        <footer className="statusbar">
          <span>
            <span className="status-dot" />{t("Local workspace")}<span className="status-divider">/</span>{t("No account required")}</span>
          <span>{t("Live catalog · original source assets")}</span>
        </footer>
      </div>
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
                <dt>{t("Acquisition")}</dt>
                <dd>{date(selected.date)}</dd>
                <dt>{t("Metadata fetched")}</dt>
                <dd>{date(catalog.retrievedAt)}</dd>
                <dt>{t("Map source")}</dt>
                <dd>{t("Georeferenced true-color COG; list thumbnails are provider JPEGs")}</dd>
                <dt>{t("Cloud cover")}</dt>
                <dd>{t("Full scene, not AOI-specific")}</dd>
              </dl>
              <p>
                {t("Earth Search pages load automatically for the submitted area, dates and cloud limit. All returned footprints appear on the map; selecting scenes loads their source true-color COGs. Overlapping imagery is drawn in layer order and is not a cloud-free mosaic or scientific band calculation.")}
              </p>
              {selected.sha256 && <p className="mono hash">{t("Cached preview SHA-256:")} {selected.sha256}</p>}
              <div className="link-stack">
                <a href={selected.assets.visual?.href || selected.itemURL} target="_blank" rel="noreferrer">{t("Original true-color COG asset")}<ExternalLink size={14} />
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
              <span className="brand-mark">
                <Layers />
              </span>
              <h3>{t("GeoD Global · local workspace")}</h3>
              <p>{t("Search Earth Search, download source files, inspect SCL pixels, and clip a raster locally by rectangle or administrative polygon.")}</p>
              <ul>
                <li>{t("Search current Sentinel-2 scenes by area, date and cloud cover through Earth Search.")}</li>
                <li>{t("Catalog filters and compatible scene comparison with local preferences.")}</li>
                <li>{t("Original source asset downloads with local task history.")}</li>
                <li>{t("Verified SCL pixel inspection and rectangle or polygon GeoTIFF clips.")}</li>
              </ul>
              <p className="muted">{t("Inter is bundled locally. Catalog searches, imagery previews and asset downloads contact their source providers. This workspace sends no analytics.")}</p>
            </div>
          )}
        </Modal>
      )}
    </div>
  );
}
function PageHeading({ eyebrow, title, sub, action }) {
  return (
    <div className="page-heading">
      <div>
        <span className="eyebrow">{eyebrow}</span>
        <h1>{title}</h1>
        <p>{sub}</p>
      </div>
      {action}
    </div>
  );
}
createRoot(document.getElementById("root")).render(<I18nProvider><RuntimeProvider><App /></RuntimeProvider></I18nProvider>);
