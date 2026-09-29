import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Compass,
  Layers,
  Folder,
  Workflow,
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
  Check,
  ExternalLink,
  ArrowUpRight,
  ArrowLeft,
  Play,
  Pause,
  RotateCcw,
  Sun,
  Moon,
  HelpCircle,
  MapPin,
  Info,
  Satellite,
  Mountain,
  Box,
  Image as ImageIcon,
  File,
  PanelRightClose,
  PanelRightOpen,
  GitBranch,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  MoreHorizontal,
  Keyboard,
  Save,
  Trash2,
} from "lucide-react";
import "./ui/foundation.css";
import { Button, Badge, Input, Textarea, Select, Switch, Progress, Modal, EmptyState,
  Table, THead, TBody, TR, TH, TD, Disclosure, Surface, SidebarNav,
  SegmentedControl, Toast, Spinner } from "./ui/index.jsx";
import "./styles.css";
import "./catalog.css";
import { INITIAL_SEARCH, SAMPLE_BBOX, normalizeSample, searchURL, validateBounds, validateSearch, compatibleScenes, createSearchRunner } from "./catalog.js";
import { RuntimeProvider, DownloadAssetButton, RuntimeTasks, RuntimeLibrary } from "./runtime-ui.jsx";
import { ExecutableRecipes } from "./processing-ui.jsx";
import { DiagnosticsPanel } from "./diagnostics-ui.jsx";
import { I18nProvider, useI18n } from "./i18n.jsx";

const WorkspaceMap = React.lazy(() => import("./workspace-map.jsx").then(module => ({ default: module.WorkspaceMap })));
const AreaPicker = React.lazy(() => import("./area-picker.jsx").then(module => ({ default: module.AreaPicker })));
const ExploreMap = React.lazy(() => import("./explore-map.jsx").then(module => ({ default: module.ExploreMap })));

const nav = [
  ["Explore", Compass],
  ["Workspace", Layers],
  ["My Data", Folder],
  ["Recipes", Workflow],
  ["Tasks", ListTodo],
  ["Sources", Database],
];
const domains = [
  ["Satellite", Satellite],
  ["Imagery", ImageIcon],
  ["Elevation", Mountain],
  ["Vector", GitBranch],
  ["3D", Box],
  ["Local Data", Folder],
];
const stamp = (value) => value.slice(0, 10);
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
  return src && !failed ? <img src={src} alt={alt} onError={() => setFailed(true)} /> : <span className="catalog-thumbnail-missing" role="img" aria-label={t("Preview unavailable: {description}", { description: alt })}>{t("Preview unavailable")}</span>;
}

function App() {
  const { t, locale, setLocale, date, number } = useI18n();
  const [sampleCatalog, setSampleCatalog] = useState(null),
    [loadError, setLoadError] = useState(false);
  const [catalogMode, setCatalogMode] = useState("sample");
  const [liveCatalog, setLiveCatalog] = useState(null);
  const [searchInput, setSearchInput] = useState(INITIAL_SEARCH);
  const [liveState, setLiveState] = useState("idle");
  const [liveError, setLiveError] = useState("");
  const [appliedSearch, setAppliedSearch] = useState(null);
  const [areaPolygon, setAreaPolygon] = useState(null);
  const searchRunner = useRef(null);
  if (!searchRunner.current) searchRunner.current = createSearchRunner();
  const live = catalogMode === "live";
  const catalog = live ? liveCatalog : sampleCatalog;
  let pendingBounds = SAMPLE_BBOX;
  if (live) {
    try { pendingBounds = validateBounds(searchInput.bbox); }
    catch { pendingBounds = appliedSearch?.bbox || SAMPLE_BBOX; }
  }
  const bbox = live ? (appliedSearch?.bbox || pendingBounds) : SAMPLE_BBOX;
  const areaName = live ? (areaPolygon?.place?.name || "Custom search area") : "San Francisco Bay";
  const [page, setPage] = useState(
    [...nav.map((n) => n[0]), "Settings", "Cloud"].includes(
      decodeURIComponent(location.hash.slice(1)),
    )
      ? decodeURIComponent(location.hash.slice(1))
      : "Explore",
  );
  const [domain, setDomain] = useState("Satellite"),
    [selected, setSelected] = useState(null),
    [query, setQuery] = useState(""),
    [cloud, setCloud] = useState(60),
    [sort, setSort] = useState("date");
  const exploreMap = useRef(null);
  const [period, setPeriod] = useState("all"),
    [condition, setCondition] = useState("ready");
  const [compare, setCompare] = useState(false),
    [compareId, setCompareId] = useState(""),
    [split, setSplit] = useState(50),
    [showArea, setShowArea] = useState(true),
    [inspector, setInspector] = useState(window.innerWidth >= 1280);
  const [modal, setModal] = useState(null),
    [theme, setTheme] = useState(stored("theme", "light")),
    [toast, setToast] = useState(""),
    [format, setFormat] = useState("COG"),
    [recipeName, setRecipeName] = useState("San Francisco · Sentinel-2");
  const [recipes, setRecipes] = useState(stored("recipes", [])),
    [tasks, setTasks] = useState(stored("tasks", [])),
    [outputs, setOutputs] = useState(stored("outputs", [])),
    [telemetry, setTelemetry] = useState(false),
    [localFile, setLocalFile] = useState(null);
  const [enabled, setEnabled] = useState(true),
    [cmd, setCmd] = useState("");
  const load = () => {
    setLoadError(false);
    fetch("./samples/manifest.json")
      .then((r) => {
        if (!r.ok) throw new Error();
        return r.json();
      })
      .then((d) => {
        d = normalizeSample(d);
        setSampleCatalog(d);
        setSelected(
          d.scenes.find((s) => s.id === "S2C_10SEG_20250617_0_L2A") ||
            d.scenes[0],
        );
        setCompareId(d.scenes[1]?.id);
      })
      .catch(() => setLoadError(true));
  };
  useEffect(load, []);
  useEffect(() => () => searchRunner.current.cancel(), []);
  const switchCatalog = (mode) => {
    searchRunner.current.cancel();
    setAreaPolygon(null);
    setCatalogMode(mode);
    setLiveState("idle");
    setLiveError("");
    setCompare(false);
    setQuery("");
    setPeriod("all");
    setCondition("ready");
    const first = mode === "sample" ? sampleCatalog?.scenes[0] : liveCatalog?.scenes[0];
    setSelected(first || null);
    setRecipeName(mode === "sample" ? "San Francisco · Sentinel-2" : "Custom area · Sentinel-2");
  };
  const runSearch = async (more = false, submittedInput = searchInput) => {
    let submitted, url;
    try {
      submitted = more ? appliedSearch : validateSearch(submittedInput);
      url = more ? liveCatalog?.next : searchURL(submitted);
      if (!url) return;
    } catch (error) { setLiveError(error.message); return; }
    setLiveError("");
    setLiveState(more ? "more" : "loading");
    if (!more) {
      setLiveCatalog(null);
      setSelected(null);
      setCompare(false);
      setAppliedSearch(submitted);
      setQuery("");
    }
    try {
      const result = await searchRunner.current.run(url);
      if (!result) return;
      if (more) setLiveCatalog((old) => ({ ...result, query: old.query, scenes: [...new Map([...old.scenes, ...result.scenes].map((s) => [s.id, s])).values()] }));
      else {
        setLiveCatalog(result);
        setSelected(result.scenes[0] || null);
        setRecipeName("Custom area · " + (result.scenes[0]?.date.slice(0, 10) || "Sentinel-2"));
      }
      setLiveState("ready");
    } catch (error) {
      setLiveError(error.name === "AbortError" ? "Search cancelled." : error.name === "TimeoutError" ? "Earth Search did not respond within 30 seconds. Try again." : error.message);
      setLiveState("error");
    }
  };
  const cancelSearch = () => { searchRunner.current.cancel(); setLiveState("idle"); setLiveError("Search cancelled. Run a search to retrieve scenes."); };
  const catalogError = (message) => {
    const httpError = /^Earth Search returned HTTP (\d+)\. Try again later\.$/.exec(message);
    return httpError ? t("Earth Search returned HTTP {status}. Try again later.", { status: httpError[1] }) : t(message);
  };
  const updateSearchField = (event) => {
    const { name, value } = event.currentTarget;
    if (name === 'bbox') setAreaPolygon(null);
    setSearchInput((current) => ({ ...current, [name]: name === "cloud" || name === "limit" ? Number(value) : value }));
  };
  const submitSearch = (event) => {
    event.preventDefault();
    // Submit exactly the values visible in native form controls, including date pickers.
    const submittedInput = Object.fromEntries(new FormData(event.currentTarget));
    setSearchInput((current) => ({ ...current, ...submittedInput }));
    runSearch(false, submittedInput);
  };
  const applyMapArea = ({ bounds, geometry, place }) => {
    const next = { ...searchInput, bbox: bounds.join(", ") };
    setSearchInput(next);
    setModal(null);
    if (!live) switchCatalog("live");
    setAreaPolygon(geometry ? { geometry, place, bounds } : null);
    runSearch(false, next);
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
    const change = () =>
      setPage(decodeURIComponent(location.hash.slice(1)) || "Explore");
    window.addEventListener("hashchange", change);
    return () => window.removeEventListener("hashchange", change);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem("geod-design-theme", JSON.stringify(theme));
  }, [theme]);
  useEffect(() => {
    localStorage.setItem("geod-design-recipes", JSON.stringify(recipes));
  }, [recipes]);
  useEffect(() => {
    localStorage.setItem("geod-design-tasks", JSON.stringify(tasks));
  }, [tasks]);
  useEffect(() => {
    localStorage.setItem("geod-design-outputs", JSON.stringify(outputs));
  }, [outputs]);
  useEffect(() => {
    const f = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault();
        setModal("commands");
      }
      if (e.key === "Escape" && !document.querySelector('[role="dialog"][data-state="open"]'))
        setCompare(false);
    };
    window.addEventListener("keydown", f);
    return () => window.removeEventListener("keydown", f);
  }, []);
  // These are explicit design simulations. No worker, network download or raster output is implied.
  useEffect(() => {
    if (!tasks.some((t) => t.status === "Running")) return;
    const timer = setTimeout(
      () =>
        setTasks((items) =>
          items.map((t) =>
            t.status === "Running"
              ? {
                  ...t,
                  progress: Math.min(t.progress + 20, 100),
                  status: t.progress >= 80 ? "Succeeded" : "Running",
                }
              : t,
          ),
        ),
      650,
    );
    return () => clearTimeout(timer);
  }, [tasks]);
  useEffect(() => {
    const done = tasks.filter((t) => t.status === "Succeeded");
    if (done.length)
      setOutputs((old) => {
        const fresh = done.filter((t) => !old.some((o) => o.id === t.id));
        return fresh.length
          ? [
              ...fresh.map((t) => ({
                ...t,
                kind: "Simulation report",
                createdAt: new Date().toISOString(),
              })),
              ...old,
            ]
          : old;
      });
  }, [tasks]);
  const showJSON = (filename, value) =>
    setModal({ type: "json", filename, value });
  const go = (p) => {
    location.hash = encodeURIComponent(p);
    setPage(p);
  };
  const scenes = catalog?.scenes || [];
  const filtered = enabled
    ? scenes
        .filter(
          (s) =>
            (live || (s.cloud ?? 101) <= cloud) &&
            (s.id.toLowerCase().includes(query.toLowerCase()) ||
              s.date.includes(query)) &&
            (live || period === "all" || Number(s.date.slice(8, 10)) <= 15),
        )
        .sort((a, b) =>
          sort === "cloud" ? (a.cloud ?? 101) - (b.cloud ?? 101) : b.date.localeCompare(a.date),
        )
    : [];
  const comparisons = scenes.filter((s) => compatibleScenes(selected, s));
  const other = comparisons.find((s) => s.id === compareId) || comparisons[0];
  const comparing = compare && !!other;
  const recipe = () => ({
    schemaVersion: "design-prototype/v1",
    status: "proposed",
    name: recipeName,
    area: { name: areaName, bbox, crs: "EPSG:4326" },
    input: {
      provider: "earth-search",
      collection: "sentinel-2-l2a",
      itemId: selected.id,
      asset: "visual",
      href: selected.assets.visual?.href,
    },
    processing: [{ operator: "clip_to_area", implemented: false }],
    output: { format, crs: selected.crs, resolution: selected.gsd },
    execution: { mode: "design-only", requiresCoreIntegration: true },
    provenance: {
      catalogQuery: catalog.query,
      retrievedAt: catalog.retrievedAt,
    },
  });
  const saveRecipe = () => {
    const r = {
      ...recipe(),
      id: crypto.randomUUID(),
      savedAt: new Date().toISOString(),
    };
    setRecipes((old) => [r, ...old]);
    setModal(null);
    setToast("Design recipe saved in this browser.");
  };
  const simulate = () => {
    const t = {
      id: crypto.randomUUID(),
      name: recipeName,
      sceneId: selected.id,
      format,
      status: "Running",
      progress: 0,
      simulation: true,
    };
    setTasks((old) => [t, ...old]);
    setModal(null);
    go("Tasks");
  };
  const changeTask = (id, status) =>
    setTasks((old) => old.map((t) => (t.id === id ? { ...t, status } : t)));
  const workspace = page === "Explore" || page === "Workspace";
  const sourceRows = [
    [
      "Earth Search",
      "Sentinel-2 L2A",
      "Public catalog",
      "Live search + 7 cached sample records",
    ],
    [
      "Copernicus Data Space",
      "Satellite data",
      "Planned",
      "Authentication not connected",
    ],
    [
      "OpenStreetMap",
      "Vector features",
      "Planned",
      "Feature adapter not connected",
    ],
    [
      "Authorized imagery",
      "WMS / WMTS / XYZ",
      "Planned",
      "Bring your own source",
    ],
    [
      "Local files",
      "COG / GeoJSON / 3D Tiles",
      "Planned",
      "Arbitrary local file import not connected",
    ],
  ];
  if (loadError && !live)
    return (
      <main className="boot">
        <EmptyState
          icon={AlertCircle}
          title={t("The sample catalog could not load")}
          action={
            <Button onClick={load} icon={RotateCcw}>{t("Retry")}</Button>
          }
        >{t("Start the included local preview server and try again.")}</EmptyState>
      </main>
    );
  if (!sampleCatalog && !live)
    return (
      <main className="boot">
        <Spinner />{t("Loading the sample workspace…")}</main>
    );
  return (
    <div className="app">
      <SidebarNav
        ariaLabel={t("GeoD home")}
        brand={<a className="brand" href="#Explore" aria-label={t("GeoD home")}><span className="brand-mark"><Layers size={20} /></span><strong>{t("GeoD")}</strong></a>}
        items={nav.map(([name, icon]) => ({ id: name, label: t(name), icon, href: "#" + encodeURIComponent(name), active: page === name }))}
        footerItems={[
          { id: "Cloud", label: t("Cloud"), icon: Cloud, href: "#Cloud", active: page === "Cloud" },
          { id: "Settings", label: t("Settings"), icon: Settings, href: "#Settings", active: page === "Settings" },
          { id: "Help", label: t("Help"), icon: HelpCircle, onClick: () => setModal("about") },
        ]}
        footer={<span className="sidebar-local-label"><ShieldCheck size={14} />{t("Local workspace")}</span>}
      />
      <div className="app-main">
        <header className="topbar">
          <div className="breadcrumb">
            <span className="project-icon">
              <Folder size={16} />
            </span>
            <strong>{live ? t("Earth Search workspace") : t("Bay Area study")}</strong>
            <ChevronRight size={14} />
            <span>{t(page)}</span>
          </div>
          <div className="top-actions">
            <Badge>{live ? t("Live catalog") : t("Sample catalog")}</Badge>
            <Button
              className="command-trigger"
              aria-label={t("Search commands")}
              onClick={() => setModal("commands")}
            >
              <Search size={15} />
              <span>{t("Search commands")}</span>
              <kbd>{t("Ctrl K")}</kbd>
            </Button>
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
        {page === "Workspace" ? <React.Suspense fallback={<main className="wm-map-loading" role="status">{t("Loading local map…")}</main>}><WorkspaceMap /></React.Suspense> : workspace ? (
          <div className={"workspace " + (!inspector || !selected ? "no-inspector" : "")}>
            <aside className="discovery">
              <div className="panel-heading">
                <div>
                  <span className="eyebrow">{t("DATA EXPLORER")}</span>
                  <h1>
                    {page === "Workspace"
                      ? t("Your layers")
                      : t("Find your next dataset")}
                  </h1>
                </div>
                <Button
                  className="icon-btn"
                  aria-label={t("Source information")}
                  onClick={() => go("Sources")}
                >
                  <MoreHorizontal size={20} />
                </Button>
              </div>
              <Button className="area-picker" onClick={() => setModal("area")}>
                <MapPin size={17} />
                <span>
                  <strong>{t(areaName)}</strong>
                  <small>{live ? t("WGS 84 · editable search bounds") : t("Saved area · California, US")}</small>
                </span>
                <ChevronDown size={16} />
              </Button>
              <SegmentedControl className="catalog-switch" aria-label={t("Catalog mode")} value={catalogMode} onValueChange={switchCatalog}
                items={[{ value: "sample", label: t("Sample catalog") }, { value: "live", label: t("Live catalog") }]} />
              <SegmentedControl className="domain-tabs" aria-label={t("Data categories")} value={domain} onValueChange={setDomain}
                items={domains.map(([value, icon]) => ({ value, label: t(value), icon }))} />
              {domain === "Satellite" ? (
                <>
                  <div className="filters">
                    {live && <form className="catalog-form" onSubmit={submitSearch}>
                      <label>{t("WGS 84 bounds · west, south, east, north")}<Input name="bbox" aria-label={t("Search bounding box")} value={searchInput.bbox} onChange={updateSearchField} />
                      </label>
                      <Button type="button" icon={SquareDashed} onClick={() => setModal("area")}>{t("Draw area on map")}</Button>
                      <div className="catalog-dates">
                        <label>{t("From (UTC)")}<Input name="start" aria-label={t("Search start date")} type="date" value={searchInput.start} onInput={updateSearchField} onChange={updateSearchField} /></label>
                        <label>{t("Through (UTC)")}<Input name="end" aria-label={t("Search end date")} type="date" value={searchInput.end} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      </div>
                      <label className="range-label"><span>{t("Scene cloud cover ≤ {percent}", { percent: number(Number(searchInput.cloud) / 100, { style: "percent" }) })}</span><Input name="cloud" type="range" aria-label={t("Live maximum cloud cover")} min="0" max="100" value={searchInput.cloud} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      <div className="catalog-search-actions">
                        <label>{t("Per page")}<Select name="limit" aria-label={t("Scenes per page")} value={searchInput.limit} onChange={updateSearchField}><option value="10">10</option><option value="20">20</option><option value="50">50</option></Select></label>
                        <Button primary icon={Search} type="submit">{t("Search catalog")}</Button>
                      </div>
                      {(liveState === "loading" || liveState === "more") && <Button type="button" onClick={cancelSearch}>{t("Cancel search")}</Button>}
                    </form>}
                    {liveError && <p className="catalog-error" role="alert">{catalogError(liveError)}</p>}
                    {live && appliedSearch && <p className="catalog-query-note">{t("{status}: {start} – {end} · clouds ≤ {cloud} · [{bbox}]", { status: catalog ? t("Showing") : t("Requested"), start: date(appliedSearch.start), end: date(appliedSearch.end), cloud: number(appliedSearch.cloud / 100, { style: "percent" }), bbox: appliedSearch.bbox.join(", ") })}</p>}
                    <label className="search-input">
                      <Search size={16} />
                      <Input
                        aria-label={t("Search scenes")}
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder={t("Search scene ID or date")}
                      />
                    </label>
                    {!live && <div className="filter-row">
                      <label className="select-wrap">{t("Date")}<Select
                          aria-label={t("Date range")}
                          value={period}
                          onChange={(e) => setPeriod(e.target.value)}
                        >
                          <option value="all">{t("Jun 1–30, 2025")}</option>
                          <option value="first">{t("Jun 1–15, 2025")}</option>
                        </Select>
                      </label>
                      <Button
                        className="filter-btn"
                        aria-label={t("Sample states")}
                        onClick={() => setModal("states")}
                      >
                        <SlidersHorizontal size={16} />
                      </Button>
                    </div>}
                    {!live && <label className="range-label">
                      <span>{t("Scene cloud cover")}</span>
                      <strong>≤ {number(cloud / 100, { style: "percent" })}</strong>
                      <Input
                        type="range"
                        aria-label={t("Maximum cloud cover")}
                        min="0"
                        max="100"
                        step="1"
                        value={cloud}
                        onChange={(e) => setCloud(Number(e.target.value))}
                      />
                    </label>}
                  </div>
                  <div className="results-heading">
                    <span>
                      <strong>
                        {condition === "empty" ? 0 : filtered.length}
                      </strong>{" "}{t("scenes")}<span className="muted">· {live ? t("loaded results") : t("catalog snapshot")}</span>
                    </span>
                    <Select
                      aria-label={t("Sort scenes")}
                      value={sort}
                      onChange={(e) => setSort(e.target.value)}
                    >
                      <option value="date">{t("Newest")}</option>
                      <option value="cloud">{t("Clearest")}</option>
                    </Select>
                  </div>
                  <div className="scene-list">
                    {live && liveState === "loading" ? <div className="loading-state" role="status"><Spinner />{t("Searching Earth Search…")}</div> : live && !liveCatalog ? <EmptyState icon={Search} title={t(liveState === "error" ? "Catalog request failed" : "Search the live catalog")}>{t("Set your area and dates above. Results come directly from Earth Search; the sample catalog is separate.")}</EmptyState> : condition === "error" ? (
                      <EmptyState
                        icon={AlertCircle}
                        title={t("Source unavailable")}
                        action={
                          <Button
                            onClick={() => setCondition("ready")}
                            icon={RotateCcw}
                          >{t("Retry sample")}</Button>
                        }
                      >{t("The design scenario represents a failed catalog request. Your workspace is kept.")}</EmptyState>
                    ) : condition === "loading" ? (
                      <div className="loading-state">
                        <Spinner />{t("Loading sample results…")}<Button onClick={() => setCondition("ready")}>{t("Show loaded state")}</Button>
                      </div>
                    ) : condition === "empty" || !filtered.length ? (
                      <EmptyState
                        icon={Search}
                        title={t("No matching scenes")}
                        action={
                          <Button
                            onClick={() => {
                              setQuery("");
                              setCloud(100);
                              setPeriod("all");
                              setEnabled(true);
                              setCondition("ready");
                              if (live) {
                                const reset = { ...(appliedSearch || searchInput), cloud: 100 };
                                setSearchInput({ ...reset, bbox: Array.isArray(reset.bbox) ? reset.bbox.join(", ") : reset.bbox });
                                runSearch(false, reset);
                              }
                            }}
                          >{t("Reset filters")}</Button>
                        }
                      >{t("Try a wider date range or allow more cloud cover.")}</EmptyState>
                    ) : (
                      filtered.map((s) => (
                        <Button variant="quiet" size="row" aria-pressed={selected?.id === s.id}
                          key={s.id}
                          className={
                            "scene " + (selected?.id === s.id ? "selected" : "")
                          }
                          onClick={() => {
                            setSelected(s);
                            setRecipeName((live ? "Custom area · " : "San Francisco · ") + stamp(s.date));
                          }}
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
                          {selected?.id === s.id && (
                            <CheckCircle2
                              className="selection-check"
                              size={16}
                            />
                          )}
                        </Button>
                      ))
                    )}
                  </div>
                  {live && liveCatalog?.next && <div className="catalog-next"><Button disabled={liveState === "more"} onClick={() => runSearch(true)}>{liveState === "more" ? t("Loading more…") : t("Load more scenes")}</Button><span>{t("Only loaded results are counted and sorted.")}</span></div>}
                  <div className="panel-foot">
                    <Database size={13} />
                    <span>{t("Earth Search ·")} {live ? t("live HTTPS catalog") : t("June 2025 snapshot")}</span>
                  </div>
                </>
              ) : (
                <div className="domain-placeholder">
                  <div className="domain-title">
                    <span className="eyebrow">{t(domain).toUpperCase()}</span>
                    <h2>
                      {t({
                          Imagery: "Explore imagery sources",
                          Elevation: "Prepare terrain products",
                          Vector: "Build a project dataset",
                          "3D": "Inspect spatial assets",
                          "Local Data": "Bring your own data",
                        }[domain])}
                    </h2>
                  </div>
                  {domain === "Local Data" ? (
                    <>
                      <p>{t("This file picker reads names and sizes. Inspect and clip downloaded SCL rasters from My Data.")}</p>
                      <label className="field file-picker">
                        <Plus size={16} />{t("Choose a local file")}<Input
                          type="file"
                          onChange={(e) =>
                            setLocalFile(e.target.files[0] || null)
                          }
                        />
                      </label>
                      {localFile && (
                        <Surface className="info-box">
                          <strong>{localFile.name}</strong>
                          <p>
                            {number(localFile.size / 1024, { maximumFractionDigits: 1 })} {t("KB · stays on this device")}</p>
                        </Surface>
                      )}
                    </>
                  ) : (
                    <>
                      {{
                        Imagery: [
                          "Historical imagery",
                          "Authorized WMS / WMTS",
                          "Custom tile service",
                        ],
                        Elevation: [
                          "Elevation model",
                          "Hillshade & slope",
                          "Contours",
                        ],
                        Vector: [
                          "Buildings & roads",
                          "GeoJSON & GeoPackage",
                          "PMTiles & MBTiles",
                        ],
                        "3D": [
                          "Remote 3D Tiles",
                          "Local tileset",
                          "Terrain & models",
                        ],
                      }[domain].map((label) => (
                        <Button
                          className="domain-option"
                          key={label}
                          onClick={() => setModal("planned")}
                        >
                          <span>{t(label)}</span>
                          <ChevronRight size={16} />
                        </Button>
                      ))}
                      <p className="muted">{t("This category is part of the full design. Its source adapter is not connected in this prototype.")}</p>
                    </>
                  )}
                </div>
              )}
            </aside>
            {selected ? <main className="map-workspace">
              <div className="map-toolbar">
                <SegmentedControl className="preview-mode" aria-label={t("Preview")} value={compare ? "compare" : "preview"}
                  onValueChange={value => { setCompare(value === "compare"); if (value === "compare") setCompareId(other?.id || ""); }}
                  items={[
                    { value: "preview", label: t("Preview"), icon: Layers },
                    { value: "compare", label: t("Compare"), icon: SlidersHorizontal, disabled: !comparisons.length,
                      title: t(comparisons.length ? "Compare scenes with matching source grids" : "Comparison needs two true-color COGs with the same CRS, transform and dimensions") },
                  ]} />
                <div className="toolbar-end">
                  <Badge tone="on-map">{t("True color")}</Badge>
                  <Button variant="secondary" size="icon"
                    className="map-icon"
                    aria-label={t(inspector ? "Hide inspector" : "Show inspector")}
                    onClick={() => setInspector(!inspector)}
                  >
                    {inspector ? (
                      <PanelRightClose size={18} />
                    ) : (
                      <PanelRightOpen size={18} />
                    )}
                  </Button>
                </div>
              </div>
              {!comparisons.length && <p className="catalog-compare-note">{t("Comparison needs another scene with the same CRS, transform and dimensions.")}</p>}
              <div className="imagery-canvas">
                <React.Suspense fallback={<div className="explore-map-loading" role="status">{t("Loading georeferenced imagery…")}</div>}>
                  <ExploreMap key={selected.crs || selected.id} ref={exploreMap} scene={selected} reference={comparing ? other : null} split={split} area={bbox} areaGeometry={live ? areaPolygon?.geometry : null} showArea={showArea} />
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
              <div className="map-controls">
                <Button variant="secondary" size="icon"
                  className="map-icon"
                  aria-label={t("Zoom in")}
                  onClick={() => exploreMap.current?.zoomIn()}
                >
                  <Plus size={18} />
                </Button>
                <Button variant="secondary" size="icon"
                  className="map-icon"
                  aria-label={t("Zoom out")}
                  onClick={() => exploreMap.current?.zoomOut()}
                >
                  <Minus size={18} />
                </Button>
                <Button variant="secondary" size="icon"
                  className="map-icon"
                  aria-label={t("Fit scene")}
                  onClick={() => exploreMap.current?.fit()}
                >
                  <Maximize size={16} />
                </Button>
                <div className="control-separator" />
                <Button variant="secondary" size="icon"
                  className={"map-icon " + (showArea ? "control-active" : "")}
                  aria-pressed={showArea}
                  aria-label={t("Toggle saved area")}
                  title={t("Show the searched area")}
                  onClick={() => setShowArea(!showArea)}
                >
                  <SquareDashed size={18} />
                </Button>
              </div>
              <Surface as="div" className="scene-caption">
                <Badge tone="on-map">{t("SENTINEL-2 L2A")}</Badge>
                <h2>{live ? selected.properties["grid:code"] || t("Selected observation") : t(areaName)}</h2>
                <p>
                  {date(selected.date)} <span>·</span>{" "}
                  {selected.cloud == null ? t("Unknown") : number(selected.cloud / 100, { style: "percent", maximumFractionDigits: 1 })} {t("scene cloud cover")}</p>
              </Surface>
              <div className="map-attribution">
                <span>Contains Copernicus Sentinel data ({selected.date.slice(0, 4)}) · Earth Search</span>
                <Button onClick={() => setModal("provenance")}>{t("Georeferenced COG display · source details")}<Info size={12} />
                </Button>
              </div>
              <div className="timeline">
                <div className="timeline-label">
                  <span className="eyebrow">{t("OBSERVATIONS")}</span>
                  <strong>{live ? t("Loaded scenes") : t("June 2025")}</strong>
                </div>
                <div className="timeline-track">
                  {[...scenes].reverse().map((s) => (
                    <Button variant="quiet" aria-pressed={selected?.id === s.id}
                      key={s.id}
                      className={s.id === selected.id ? "selected" : ""}
                      onClick={() => setSelected(s)}
                      aria-label={t("Select observation {date}", { date: date(s.date) })}
                    >
                      <span className="date-line" />
                      <span className="observation-dot" />
                      <small>{live ? date(s.date, { year: undefined, month: "2-digit", day: "2-digit" }) : number(Number(s.date.slice(8, 10)))}</small>
                    </Button>
                  ))}
                </div>
                <Button
                  className="icon-btn"
                  aria-label={t("Timeline help")}
                  onClick={() => setModal("provenance")}
                >
                  <Info size={16} />
                </Button>
              </div>
            </main> : <main className="catalog-blank"><EmptyState icon={Search} title={t(liveState === "loading" ? "Searching your area" : liveCatalog ? "No scenes for this search" : "Choose your next observation")}>{t("Use the catalog on the left to choose an area and dates. The selected scene preview will appear here.")}</EmptyState></main>}
            {inspector && selected && (
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
                <div className="preview-image">
                  <SceneThumbnail
                    src={selected.thumbnail || undefined}
                    alt={t("Selected scene thumbnail")}
                  />
                  <span>{t("RGB PREVIEW")}</span>
                </div>
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
                </div>
                <div className="detail-section">
                  <h3>{t("Area & output")}</h3>
                  <dl>
                    <dt>{t("Saved area")}</dt>
                    <dd>{t(areaName)}</dd>
                    <dt>{t("Selection")}</dt>
                    <dd>{t("Bounding box")}</dd>
                    <dt>{t("Processing")}</dt>
                    <dd>{t("Local · planned")}</dd>
                  </dl>
                  <Button
                    className="text-link"
                    onClick={() => setModal("area")}
                  >{t("Inspect area")}<ArrowUpRight size={14} />
                  </Button>
                </div>
                <div className="detail-section">
                  <h3>{t("Source & provenance")}</h3>
                  <div className="source-line">
                    <span className="source-symbol">
                      <Satellite size={17} />
                    </span>
                    <div>
                      <strong>{t("Copernicus Sentinel")}</strong>
                      <small>{t("Catalog by Earth Search")}</small>
                    </div>
                  </div>
                    <Button
                    className="text-link"
                    disabled={!selected}
                    onClick={() => setModal("provenance")}
                  >{t("View metadata & source")}<ArrowUpRight size={14} />
                  </Button>
                </div>
                <div className="inspector-bottom">
                  <DownloadAssetButton scene={selected} />
                  <Button icon={Folder} onClick={() => go("My Data")}>{t("Open downloaded files and clipping")}</Button>
                  <p>{t("Choose SCL, true-color imagery, or a JPEG preview in Download. Files are saved to this workspace without cropping. To make a cropped GeoTIFF, download SCL first, then use Clip raster in My Data.")}</p>
                </div>
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
                <RuntimeTasks areaBounds={bbox} areaPolygon={live ? areaPolygon : null} />
                <Disclosure className="design-simulations" summary={<>{t("Design simulations below · {count} sample tasks", { count: number(tasks.length) })}</>}>
                <Surface className="notice">
                  <Info size={17} />
                  <span>{t("These sample tasks simulate processing. Real downloads and clipping jobs appear above.")}</span>
                </Surface>
                {!tasks.length ? (
                  <EmptyState
                    icon={ListTodo}
                    title={t("Your next task starts with an area")}
                    action={
                      <Button primary onClick={() => go("Explore")}>{t("Explore data")}</Button>
                    }
                  >{t("Choose a scene and prepare an export to review the task lifecycle.")}</EmptyState>
                ) : (
                  <div className="task-list">
                    {tasks.map((task) => (
                      <Surface as="article" className="task-card" key={task.id}>
                        <div className="task-icon">
                          <Download size={22} />
                        </div>
                        <div className="task-main">
                          <div className="task-title">
                            <h3>{task.name}</h3>
                            <Badge
                              tone={
                                task.status === "Succeeded"
                                  ? "green"
                                  : task.status === "Failed"
                                    ? "red"
                                    : "blue"
                              }
                            >
                              {task.status === "Succeeded"
                                ? t("Simulation complete")
                                : t(task.status)}
                            </Badge>
                          </div>
                          <p>
                            {task.sceneId} · {task.format} {t("design")}</p>
                          <Progress value={task.progress} max={100} aria-label={t("Design simulation only")} />
                          <div className="task-stage">
                            <span>
                              {task.status === "Succeeded"
                                ? t("Sample report available; no raster was created")
                                : task.status === "Failed"
                                  ? t("Simulated connection interruption; retry as a new attempt")
                                  : task.status === "Cancelled"
                                    ? t("Simulation cancelled")
                                    : t("Simulated {stage}", { stage: t(task.progress < 40 ? "source read" : task.progress < 80 ? "processing" : "validation") })}
                            </span>
                            <strong>{task.progress}%</strong>
                          </div>
                        </div>
                        <div className="task-actions">
                          {task.status === "Running" ? (
                            <>
                              <Button
                                icon={Pause}
                                onClick={() => changeTask(task.id, "Paused")}
                              >{t("Pause")}</Button>
                              <Button onClick={() => changeTask(task.id, "Failed")}>{t("Simulate failure")}</Button>
                            </>
                          ) : task.status === "Paused" ? (
                            <Button
                              icon={Play}
                              onClick={() => changeTask(task.id, "Running")}
                            >{t("Resume")}</Button>
                          ) : task.status === "Failed" ? (
                            <Button
                              icon={RotateCcw}
                              onClick={() =>
                                setTasks((old) => [
                                  {
                                    ...task,
                                    id: crypto.randomUUID(),
                                    parentId: task.id,
                                    progress: 0,
                                    status: "Running",
                                  },
                                  ...old,
                                ])
                              }
                            >{t("Retry")}</Button>
                          ) : null}
                          {["Running", "Paused"].includes(task.status) && (
                            <Button
                              className="text-link danger"
                              onClick={() => changeTask(task.id, "Cancelled")}
                            >{t("Cancel")}</Button>
                          )}
                          {task.status === "Succeeded" && (
                            <Button onClick={() => go("My Data")}>{t("View report")}</Button>
                          )}
                        </div>
                      </Surface>
                    ))}
                  </div>
                )}
                </Disclosure>
              </>
            ) : page === "My Data" ? (
              <>
                <PageHeading
                  eyebrow={t("LOCAL LIBRARY")}
                  title={t("My Data")}
                  sub={t("Find downloaded files and clipping results ready to inspect or use.")}
                />
                <RuntimeLibrary areaBounds={bbox} areaPolygon={live ? areaPolygon : null} />
                <Disclosure className="design-simulations" summary={<>{t("Design simulation reports · {count} reports", { count: number(outputs.length) })}</>}>
                {!outputs.length ? (
                  <EmptyState
                    title={t("A place for finished work")}
                    action={
                      <Button primary onClick={() => go("Explore")}>{t("Prepare an export")}</Button>
                    }
                  >{t("Sample tasks add simulation reports here. Real GeoTIFF outputs are listed above.")}</EmptyState>
                ) : (
                  <div className="output-grid">
                    {outputs.map((o) => (
                      <Surface as="article" className="output-card" key={o.id}>
                        <img
                          src={
                            (sampleCatalog?.scenes.find((s) => s.id === o.sceneId) || scenes.find((s) => s.id === o.sceneId))?.thumbnail
                          }
                          alt={t("Source scene thumbnail, not exported output")}
                        />
                        <div>
                          <Badge>{t("Simulation report")}</Badge>
                          <h3>{o.name}</h3>
                          <p>{t("Source preview · No raster output")}</p>
                          <div className="row-actions">
                            <Button
                              icon={Download}
                              onClick={() =>
                                showJSON("geod-design-report.json", {
                                  ...o,
                                  warning:
                                    "Design simulation only. No raster output or scientific validation.",
                                })
                              }
                            >{t("Report JSON")}</Button>
                            <Button
                              className="icon-btn"
                              aria-label={t("Remove report {name}", { name: o.name })}
                              onClick={() =>
                                setModal({ type: "delete", id: o.id })
                              }
                            >
                              <Trash2 size={16} />
                            </Button>
                          </div>
                        </div>
                      </Surface>
                    ))}
                  </div>
                )}
                </Disclosure>
              </>
            ) : page === "Recipes" ? (
              <>
                <PageHeading
                  eyebrow={t("REPEATABLE WORK")}
                  title={t("Recipes")}
                  sub={t("Repeat a verified rectangle or polygon clip from a pinned SCL source file.")}
                />
                <ExecutableRecipes areaBounds={bbox} areaPolygon={live ? areaPolygon : null} onReviewJSON={showJSON} />
                <Disclosure className="design-simulations" summary={<>{t("Design recipe simulations · {count} recipes", { count: number(recipes.length) })}</>}>
                <Surface className="notice">
                  <Workflow size={17} />{t("These design recipes use design-prototype/v1 and do not execute. Saved executable recipes are listed above.")}</Surface>
                {!recipes.length ? (
                  <EmptyState
                    icon={Workflow}
                    title={t("Make a good workflow repeatable")}
                    action={
                      <Button primary disabled={!selected} onClick={() => setModal("recipe")}>{t("Save current selection")}</Button>
                    }
                  >{t("Save an area, a fixed scene and output preferences. Your recipe stays in this browser.")}</EmptyState>
                ) : (
                  <div className="table-wrap">
                    <Table>
                      <THead>
                        <TR>
                          <TH>{t("Recipe")}</TH>
                          <TH>{t("Input")}</TH>
                          <TH>{t("Output")}</TH>
                          <TH>{t("Saved")}</TH>
                          <TH>{t("Actions")}</TH>
                        </TR>
                      </THead>
                      <TBody>
                        {recipes.map((r) => (
                          <TR key={r.id}>
                            <TD>
                              <strong>{r.name}</strong>
                              <small className="block">
                                {t(r.area?.name || "Saved area")} {t("· fixed scene")}</small>
                            </TD>
                            <TD>{t("Sentinel-2 L2A")}</TD>
                            <TD>{r.output.format}</TD>
                            <TD>{date(r.savedAt)}</TD>
                            <TD>
                              <div className="row-actions">
                                <Button
                                  icon={Download}
                                  onClick={() =>
                                    showJSON("geod-design-recipe.json", r)
                                  }
                                >{t("JSON")}</Button>
                                <Button
                                  icon={Play}
                                  disabled={!scenes.some((s) => s.id === r.input.itemId)}
                                  title={t("Review is available when the saved scene is loaded in the current catalog")}
                                  onClick={() => {
                                    setSelected(
                                      scenes.find(
                                        (s) => s.id === r.input.itemId,
                                      ) || selected,
                                    );
                                    setRecipeName(r.name);
                                    setFormat(r.output.format);
                                    setModal("export");
                                  }}
                                >{t("Review")}</Button>
                              </div>
                            </TD>
                          </TR>
                        ))}
                      </TBody>
                    </Table>
                  </div>
                )}
                </Disclosure>
              </>
            ) : page === "Sources" ? (
              <>
                <PageHeading
                  eyebrow={t("DATA CONNECTIONS")}
                  title={t("Sources")}
                  sub={t("Know where your data comes from, before you use it.")}
                  action={
                    <Button icon={Plus} onClick={() => setModal("planned")}>{t("Add source")}</Button>
                  }
                />
                <div className="table-wrap">
                  <Table>
                    <THead>
                      <TR>
                        <TH>{t("Source")}</TH>
                        <TH>{t("Data")}</TH>
                        <TH>{t("Connection")}</TH>
                        <TH>{t("Availability")}</TH>
                        <TH>{t("Action")}</TH>
                      </TR>
                    </THead>
                    <TBody>
                      {sourceRows.map(([name, data, status, desc], i) => (
                        <TR key={name}>
                          <TD>
                            <strong>{t(name)}</strong>
                          </TD>
                          <TD>{t(data)}</TD>
                          <TD>
                            <Badge tone={i === 0 ? "green" : ""}>
                              {i === 0 && !enabled ? t("Disabled") : t(status)}
                            </Badge>
                          </TD>
                          <TD>{t(desc)}</TD>
                          <TD>
                            {i === 0 ? (
                              <Button onClick={() => setEnabled(!enabled)}>
                                {enabled ? t("Hide source results") : t("Show source results")}
                              </Button>
                            ) : (
                              <Button onClick={() => setModal("planned")}>{t("View plan")}</Button>
                            )}
                          </TD>
                        </TR>
                      ))}
                    </TBody>
                  </Table>
                </div>
                <Surface className="info-box">
                  <ShieldCheck size={23} />
                  <h3>{t("Access and permission travel together")}</h3>
                  <p>{t("The full product will track preview, download, offline use and redistribution separately. A connected source alone will not enable every operation.")}</p>
                  <Button
                    className="text-link"
                    disabled={!selected}
                    onClick={() => setModal("provenance")}
                  >{t("Inspect selected scene evidence")}<ArrowUpRight size={14} />
                  </Button>
                </Surface>
              </>
            ) : page === "Cloud" ? (
              <>
                <PageHeading
                  eyebrow={t("OPTIONAL SERVICES")}
                  title={t("A workspace you can share")}
                  sub={t("Local work remains yours. Collaboration is a separate product decision.")}
                />
                <div className="cloud-layout">
                  <Surface className="cloud-diagram">
                    <Surface className="diagram-node">
                      <Folder />
                      <strong>{t("Local workspace")}</strong>
                      <span>{t("Files & processing")}</span>
                    </Surface>
                    <div className="diagram-connector" />
                    <Surface className="diagram-node outline">
                      <Cloud />
                      <strong>{t("Optional sync")}</strong>
                      <span>{t("Selected metadata only")}</span>
                    </Surface>
                  </Surface>
                  <div className="cloud-copy">
                    <Badge>{t("Proposal · not connected")}</Badge>
                    <h2>{t("Share the workflow.")}<br />{t("Keep control of the data.")}</h2>
                    <p>{t("Private recipe versions, shared source configuration and run summaries are proposed collaboration features.")}</p>
                    <ul>
                      <li>{t("Choose exactly which metadata leaves your device.")}</li>
                      <li>{t("Use your own storage and execution environment.")}</li>
                      <li>{t("Export your recipes when you leave.")}</li>
                    </ul>
                    <Surface className="notice">{t("Pricing and the commercial model are undecided. No checkout or account creation is active.")}</Surface>
                  </div>
                </div>
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
                  <div>
                    <span>
                      <strong>{t("Usage analytics")}</strong>
                      <small>{t("No analytics is sent by this prototype.")}</small>
                    </span>
                    <Switch checked={telemetry} onCheckedChange={setTelemetry} aria-label={t("Usage analytics design toggle")} />
                  </div>
                  <div>
                    <span>
                      <strong>{t("Local design data")}</strong>
                      <small>{t("Design recipes, sample tasks and reports use browser storage. Executable recipes and real files use the local service.")}</small>
                    </span>
                    <Button onClick={() => setModal("reset")}>{t("Clear design data")}</Button>
                  </div>
                  <div>
                    <span>
                      <strong>{t("Keyboard navigation")}</strong>
                      <small>{t("Open command search with Ctrl / Cmd + K. Close dialogs with Esc.")}</small>
                    </span>
                    <Keyboard size={22} />
                  </div>
                </Surface>
                <DiagnosticsPanel />
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
          <span>
            {tasks.filter((t) => t.status === "Running").length
              ? t("{count} simulation running", { count: number(tasks.filter((task) => task.status === "Running").length) })
              : live ? t("Live catalog · original source assets") : t("Sample catalog · cached scene metadata")}
            <span className="status-divider">/</span>
            <Button onClick={() => setModal("about")}>{t("Development 0.1")}</Button>
          </span>
        </footer>
      </div>
      <Toast message={toast ? t(toast) : ""} onDismiss={() => setToast("")} closeLabel={t("Close dialog")} />
      {modal && (
        <Modal closeLabel={t("Close dialog")}
          title={
            t(typeof modal === "object"
              ? modal.type === "json"
                ? "Export JSON"
                : "Remove report"
              : {
                  export: "Prepare export",
                  recipe: "Save design recipe",
                  area: "Select search area",
                  provenance: "Data provenance",
                  commands: "Search commands",
                  states: "Review interface states",
                  planned: "Planned capability",
                  about: "About this workspace",
                  reset: "Clear local design data",
                }[modal])
          }
          onClose={() => setModal(null)}
          wide={modal === "export" || modal === "area"}
        >
          {modal === "export" && selected ? (
            <>
              <div className="dialog-body export-layout">
                <div>
                  <Badge tone="blue">{t("DESIGN SIMULATION")}</Badge>
                  <h3>{recipeName}</h3>
                  <p className="muted">{t("Preview planned export options. Real SCL clipping starts from a downloaded file in My Data.")}</p>
                  <label className="field">{t("Output format")}<Select
                      value={format}
                      onChange={(e) => setFormat(e.target.value)}
                    >
                      <option>{t("COG")}</option>
                      <option>{t("GeoTIFF")}</option>
                    </Select>
                  </label>
                  <label className="field">{t("Coordinate reference")}<Input
                      value={selected.crs || t("Source CRS not specified")}
                      readOnly
                    />
                  </label>
                  <div className="two-fields">
                    <label className="field">{t("Pixel size")}<Input value={selected.gsd ? t("{resolution} meters", { resolution: number(selected.gsd) }) : t("Not specified")} readOnly />
                    </label>
                    <label className="field">{t("Processing location")}<Input value={t("Design simulation only")} readOnly />
                    </label>
                  </div>
                </div>
                <Surface className="export-summary">
                  <h3>{t("Export plan")}</h3>
                  <div>
                    <Check size={16} />{t("Use the fixed source scene")}</div>
                  <div>
                    <Check size={16} />{t("Clip to saved bounding box")}</div>
                  <div>
                    <Check size={16} />{t("Preserve source & recipe")}</div>
                  <div>
                    <Info size={16} />{t("This export configuration is a design simulation")}</div>
                  <hr />
                  <p>{t("This design dialog does not create files. Download SCL, then use Clip raster in My Data to create a GeoTIFF.")}</p>
                </Surface>
              </div>
              <div className="dialog-footer">
                <Button onClick={() => setModal(null)}>{t("Cancel")}</Button>
                <Button
                  icon={Download}
                  onClick={() => showJSON("geod-design-recipe.json", recipe())}
                >{t("Download plan JSON")}</Button>
                <Button primary icon={Play} onClick={simulate}>{t("Simulate task")}</Button>
              </div>
            </>
          ) : modal === "recipe" && selected ? (
            <>
              <div className="dialog-body">
                <label className="field">{t("Recipe name")}<Input
                    autoFocus
                    value={recipeName}
                    onChange={(e) => setRecipeName(e.target.value)}
                  />
                </label>
                <dl>
                  <dt>{t("Area")}</dt>
                  <dd>{t(areaName)}</dd>
                  <dt>{t("Fixed observation")}</dt>
                  <dd>{date(selected.date)}</dd>
                  <dt>{t("Planned output")}</dt>
                  <dd>{format} · {selected.gsd ? t("{resolution} meters", { resolution: number(selected.gsd) }) : t("source resolution")}</dd>
                </dl>
                <Surface className="notice">{t("Saved locally. No credentials are included. This design recipe is not yet executable.")}</Surface>
              </div>
              <div className="dialog-footer">
                <Button onClick={() => setModal(null)}>{t("Cancel")}</Button>
                <Button
                  primary
                  icon={Save}
                  disabled={!recipeName.trim()}
                  onClick={saveRecipe}
                >{t("Save recipe")}</Button>
              </div>
            </>
          ) : modal === "area" ? (
            <React.Suspense fallback={<p role="status"><Spinner/>{t("Loading reference map…")}</p>}><AreaPicker initialBbox={pendingBounds} sample={!live} onApply={applyMapArea} onExport={exportMapArea} onClose={() => setModal(null)}/></React.Suspense>
          ) : modal === "provenance" && selected && catalog ? (
            <div className="dialog-body">
              <Badge tone="green">{live ? t("LIVE CATALOG RESPONSE") : t("REAL CATALOG SNAPSHOT")}</Badge>
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
                {live ? t("Scene metadata is queried from Earth Search using the submitted area, dates and cloud limit. Counts and local sorting cover loaded pages only. The map renders the source true-color COG; comparison requires matching source grids and does not perform scientific band math.") : t("Scene metadata and small thumbnails come from seven saved catalog records. The map streams original true-color COG data; sample filters run locally. Original downloads, SCL inspection and local clipping use the task service.")}
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
          ) : modal === "commands" ? (
            <div className="dialog-body">
              <label className="search-input">
                <Search size={17} />
                <Input
                  autoFocus
                  placeholder={t("Go to a page or action…")}
                  value={cmd}
                  onChange={(e) => setCmd(e.target.value)}
                  aria-label={t("Command search")}
                />
              </label>
              <div className="command-results">
                {[
                  ...nav.map(([n]) => [n, () => go(n)]),
                  ["Cloud", () => go("Cloud")],
                  ["Settings", () => go("Settings")],
                ]
                  .filter(([n]) => n.toLowerCase().includes(cmd.toLowerCase()) || t(n).toLocaleLowerCase(locale).includes(cmd.toLocaleLowerCase(locale)))
                  .map(([n, fn]) => (
                    <Button
                      key={n}
                      onClick={() => {
                        setModal(null);
                        fn();
                        setCmd("");
                      }}
                    >
                      <span>{t(n)}</span>
                      <ChevronRight size={16} />
                    </Button>
                  ))}
              </div>
            </div>
          ) : modal === "states" ? (
            <div className="dialog-body">
              <p>{t("These controls preview catalog interface states. They do not alter the source service.")}</p>
              {[
                ["ready", "Ready · real cached scenes"],
                ["empty", "Empty results"],
                ["loading", "Loading"],
                ["error", "Source error"],
              ].map(([v, label]) => (
                <Button
                  key={v}
                  className="state-option"
                  onClick={() => {
                    setCondition(v);
                    setModal(null);
                  }}
                >
                  <span>{t(label)}</span>
                  {condition === v && <Check size={17} />}
                </Button>
              ))}
            </div>
          ) : modal === "planned" ? (
            <div className="dialog-body">
              <p>{t("This belongs to the full product scope. The current design prototype does not connect this adapter or execute this operation.")}</p>
              <p className="muted">{t("The implementation map and release gates in the specification track the remaining work.")}</p>
              <Button
                onClick={() => {
                  setModal(null);
                  go("Sources");
                }}
              >{t("View source catalog")}</Button>
            </div>
          ) : modal === "reset" ? (
            <>
              <div className="dialog-body">
                <p>{t("Remove browser design recipes, sample tasks and reports? Real downloads, executable recipes and output files are retained.")}</p>
              </div>
              <div className="dialog-footer">
                <Button onClick={() => setModal(null)}>{t("Keep data")}</Button>
                <Button
                  primary
                  onClick={() => {
                    setRecipes([]);
                    setTasks([]);
                    setOutputs([]);
                    setModal(null);
                    setToast("Local design data cleared.");
                  }}
                >{t("Clear design data")}</Button>
              </div>
            </>
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
          ) : typeof modal === "object" ? (
            <>
              <div className="dialog-body">
                <p>{t("Remove this simulation report from the library? Files on your computer are unaffected.")}</p>
              </div>
              <div className="dialog-footer">
                <Button onClick={() => setModal(null)}>{t("Keep report")}</Button>
                <Button
                  primary
                  onClick={() => {
                    setOutputs((old) => old.filter((o) => o.id !== modal.id));
                    setTasks((old) => old.filter((t) => t.id !== modal.id));
                    setModal(null);
                  }}
                >{t("Remove report")}</Button>
              </div>
            </>
          ) : (
            <div className="dialog-body">
              <span className="brand-mark">
                <Layers />
              </span>
              <h3>{t("GeoD Global · local workspace")}</h3>
              <p>{t("A local geospatial workspace with live catalog search, original downloads, SCL inspection, rectangle and polygon clipping, and executable recipes. Other processing tools remain design previews.")}</p>
              <ul>
                <li>{t("Live Earth Search queries and a separate cached sample catalog.")}</li>
                <li>{t("Catalog filters and compatible scene comparison with local preferences.")}</li>
                <li>{t("Original source asset downloads with local task history.")}</li>
                <li>{t("Verified SCL pixel inspection, rectangle and polygon GeoTIFF clips, and reusable local recipes.")}</li>
                <li>{t("Six data domains, with unconnected adapters marked.")}</li>
                <li>{t("Cloud features and commercial terms remain proposals.")}</li>
              </ul>
              <p className="muted">{t("Inter and sample previews are bundled locally. Live searches, remote previews and asset downloads contact their source providers. This workspace sends no analytics.")}</p>
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
