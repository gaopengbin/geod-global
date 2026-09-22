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
import "./styles.css";
import "./catalog.css";
import { INITIAL_SEARCH, SAMPLE_BBOX, normalizeSample, searchURL, validateSearch, compatibleScenes, createSearchRunner } from "./catalog.js";
import { RuntimeProvider, DownloadAssetButton, RuntimeTasks, RuntimeLibrary } from "./runtime-ui.jsx";

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
const outline =
  "361.566,269.521 546.275,268.408 545.100,106.733 360.789,107.848";
const date = (value) =>
  new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(value));
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
function Badge({ children, tone = "" }) {
  return <span className={"badge " + tone}>{children}</span>;
}
function SceneThumbnail({ src, alt }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  return src && !failed ? <img src={src} alt={alt} onError={() => setFailed(true)} /> : <span className="catalog-thumbnail-missing" role="img" aria-label={alt + " unavailable"}>Preview unavailable</span>;
}
function Btn({
  children,
  icon: Icon,
  onClick,
  primary = false,
  className = "",
  ...props
}) {
  return (
    <button
      className={(primary ? "button primary" : "button") + " " + className}
      onClick={onClick}
      {...props}
    >
      {Icon && <Icon size={15} />} {children}
    </button>
  );
}
function Modal({ title, children, onClose, wide = false }) {
  const ref = useRef();
  useEffect(() => {
    const el = ref.current;
    el.showModal();
    return () => el.close();
  }, []);
  return (
    <dialog
      ref={ref}
      aria-labelledby="modal-title"
      className={wide ? "wide" : ""}
      onCancel={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <header>
        <h2 id="modal-title">{title}</h2>
        <button
          className="icon-btn"
          onClick={onClose}
          aria-label="Close dialog"
        >
          <X size={20} />
        </button>
      </header>
      {children}
    </dialog>
  );
}
function Empty({ icon: Icon = Folder, title, children, action }) {
  return (
    <div className="empty">
      <div className="empty-icon">
        <Icon size={28} />
      </div>
      <h2>{title}</h2>
      <p>{children}</p>
      {action}
    </div>
  );
}

function App() {
  const [sampleCatalog, setSampleCatalog] = useState(null),
    [loadError, setLoadError] = useState(false);
  const [catalogMode, setCatalogMode] = useState("sample");
  const [liveCatalog, setLiveCatalog] = useState(null);
  const [searchInput, setSearchInput] = useState(INITIAL_SEARCH);
  const [liveState, setLiveState] = useState("idle");
  const [liveError, setLiveError] = useState("");
  const [appliedSearch, setAppliedSearch] = useState(null);
  const searchRunner = useRef(null);
  if (!searchRunner.current) searchRunner.current = createSearchRunner();
  const live = catalogMode === "live";
  const catalog = live ? liveCatalog : sampleCatalog;
  const bbox = live ? (appliedSearch?.bbox || SAMPLE_BBOX) : SAMPLE_BBOX;
  const areaName = live ? "Custom search area" : "San Francisco Bay";
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
  const [previewError, setPreviewError] = useState(false);
  useEffect(() => setPreviewError(false), [selected?.thumbnail]);
  const [period, setPeriod] = useState("all"),
    [condition, setCondition] = useState("ready");
  const [compare, setCompare] = useState(false),
    [compareId, setCompareId] = useState(""),
    [split, setSplit] = useState(50),
    [zoom, setZoom] = useState(1),
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
    setCatalogMode(mode);
    setLiveState("idle");
    setLiveError("");
    setCompare(false);
    setQuery("");
    setPeriod("all");
    setCondition("ready");
    setZoom(1);
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
      setZoom(1);
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
  const updateSearchField = (event) => {
    const { name, value } = event.currentTarget;
    setSearchInput((current) => ({ ...current, [name]: name === "cloud" || name === "limit" ? Number(value) : value }));
  };
  const submitSearch = (event) => {
    event.preventDefault();
    // Submit exactly the values visible in native form controls, including date pickers.
    const submittedInput = Object.fromEntries(new FormData(event.currentTarget));
    setSearchInput((current) => ({ ...current, ...submittedInput }));
    runSearch(false, submittedInput);
  };
  useEffect(() => {
    const change = () =>
      setPage(decodeURIComponent(location.hash.slice(1)) || "Explore");
    window.addEventListener("hashchange", change);
    return () => window.removeEventListener("hashchange", change);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
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
    if (!toast) return;
    const id = setTimeout(() => setToast(""), 3500);
    return () => clearTimeout(id);
  }, [toast]);
  useEffect(() => {
    const f = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault();
        setModal("commands");
      }
      if (e.key === "Escape" && !document.querySelector("dialog[open]"))
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
    setToast("Recipe saved in this browser.");
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
      "Native processing not connected",
    ],
  ];
  if (loadError && !live)
    return (
      <main className="boot">
        <Empty
          icon={AlertCircle}
          title="The sample catalog could not load"
          action={
            <Btn onClick={load} icon={RotateCcw}>
              Retry
            </Btn>
          }
        >
          Start the included local preview server and try again.
        </Empty>
      </main>
    );
  if (!sampleCatalog && !live)
    return (
      <main className="boot">
        <span className="loader" />
        Loading the sample workspace…
      </main>
    );
  return (
    <div className="app">
      <aside className="rail">
        <a className="brand" href="#Explore" aria-label="GeoD home">
          <span className="brand-mark">
            <Layers size={23} />
          </span>
          <strong>GeoD</strong>
        </a>
        <nav>
          {nav.map(([name, Icon]) => (
            <a
              key={name}
              href={"#" + encodeURIComponent(name)}
              className={page === name ? "nav-item active" : "nav-item"}
              title={name}
            >
              <Icon size={21} />
              <span>{name}</span>
              {name === "Tasks" &&
                tasks.some((t) => t.status === "Running") && (
                  <i className="nav-dot" />
                )}
            </a>
          ))}
        </nav>
        <div className="rail-bottom">
          <a
            className={page === "Cloud" ? "nav-item active" : "nav-item"}
            href="#Cloud"
          >
            <Cloud size={21} />
            <span>Cloud</span>
          </a>
          <a
            className={page === "Settings" ? "nav-item active" : "nav-item"}
            href="#Settings"
          >
            <Settings size={21} />
            <span>Settings</span>
          </a>
          <button className="nav-item" onClick={() => setModal("about")}>
            <HelpCircle size={21} />
            <span>Help</span>
          </button>
          <span className="local-avatar">GP</span>
        </div>
      </aside>
      <div className="app-main">
        <header className="topbar">
          <div className="breadcrumb">
            <span className="project-icon">
              <Folder size={16} />
            </span>
            <strong>{live ? "Earth Search workspace" : "Bay Area study"}</strong>
            <ChevronRight size={14} />
            <span>{page}</span>
          </div>
          <div className="top-actions">
            <Badge>{live ? "Live catalog" : "Sample catalog"}</Badge>
            <button
              className="command-trigger"
              onClick={() => setModal("commands")}
            >
              <Search size={15} />
              <span>Search commands</span>
              <kbd>Ctrl K</kbd>
            </button>
            <button
              className="icon-btn"
              aria-label="Toggle color theme"
              onClick={() => setTheme(theme === "light" ? "dark" : "light")}
            >
              {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
            </button>
            <span className="local-status">
              <span />
              Local workspace
            </span>
          </div>
        </header>
        {workspace ? (
          <div className={"workspace " + (!inspector || !selected ? "no-inspector" : "")}>
            <aside className="discovery">
              <div className="panel-heading">
                <div>
                  <span className="eyebrow">DATA EXPLORER</span>
                  <h1>
                    {page === "Workspace"
                      ? "Your layers"
                      : "Find your next dataset"}
                  </h1>
                </div>
                <button
                  className="icon-btn"
                  aria-label="Source information"
                  onClick={() => go("Sources")}
                >
                  <MoreHorizontal size={20} />
                </button>
              </div>
              <button className="area-picker" onClick={() => setModal("area")}>
                <MapPin size={17} />
                <span>
                  <strong>{areaName}</strong>
                  <small>{live ? "WGS 84 · editable search bounds" : "Saved area · California, US"}</small>
                </span>
                <ChevronDown size={16} />
              </button>
              <div className="catalog-switch" aria-label="Catalog mode">
                <button aria-pressed={!live} onClick={() => switchCatalog("sample")}>Sample catalog</button>
                <button aria-pressed={live} onClick={() => switchCatalog("live")}>Live catalog</button>
              </div>
              <div
                className="domain-tabs"
                role="tablist"
                aria-label="Data categories"
              >
                {domains.map(([d, Icon]) => (
                  <button
                    key={d}
                    role="tab"
                    aria-selected={domain === d}
                    onClick={() => setDomain(d)}
                    className={domain === d ? "active" : ""}
                  >
                    <Icon size={16} />
                    {d}
                  </button>
                ))}
              </div>
              {domain === "Satellite" ? (
                <>
                  <div className="filters">
                    {live && <form className="catalog-form" onSubmit={submitSearch}>
                      <label>WGS 84 bounds · west, south, east, north
                        <input name="bbox" aria-label="Search bounding box" value={searchInput.bbox} onChange={updateSearchField} />
                      </label>
                      <div className="catalog-dates">
                        <label>From (UTC)<input name="start" aria-label="Search start date" type="date" value={searchInput.start} onInput={updateSearchField} onChange={updateSearchField} /></label>
                        <label>Through (UTC)<input name="end" aria-label="Search end date" type="date" value={searchInput.end} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      </div>
                      <label className="range-label"><span>Scene cloud cover ≤ {searchInput.cloud}%</span><input name="cloud" type="range" aria-label="Live maximum cloud cover" min="0" max="100" value={searchInput.cloud} onInput={updateSearchField} onChange={updateSearchField} /></label>
                      <div className="catalog-search-actions">
                        <label>Per page<select name="limit" aria-label="Scenes per page" value={searchInput.limit} onChange={updateSearchField}><option value="10">10</option><option value="20">20</option><option value="50">50</option></select></label>
                        <Btn primary icon={Search} type="submit">Search catalog</Btn>
                      </div>
                      {(liveState === "loading" || liveState === "more") && <Btn type="button" onClick={cancelSearch}>Cancel search</Btn>}
                    </form>}
                    {liveError && <p className="catalog-error" role="alert">{liveError}</p>}
                    {live && appliedSearch && <p className="catalog-query-note">{catalog ? "Showing" : "Requested"}: {appliedSearch.start} – {appliedSearch.end} · clouds ≤ {appliedSearch.cloud}% · [{appliedSearch.bbox.join(", ")}]</p>}
                    <label className="search-input">
                      <Search size={16} />
                      <input
                        aria-label="Search scenes"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder="Search scene ID or date"
                      />
                    </label>
                    {!live && <div className="filter-row">
                      <label className="select-wrap">
                        Date
                        <select
                          aria-label="Date range"
                          value={period}
                          onChange={(e) => setPeriod(e.target.value)}
                        >
                          <option value="all">Jun 1–30, 2025</option>
                          <option value="first">Jun 1–15, 2025</option>
                        </select>
                      </label>
                      <button
                        className="filter-btn"
                        aria-label="Sample states"
                        onClick={() => setModal("states")}
                      >
                        <SlidersHorizontal size={16} />
                      </button>
                    </div>}
                    {!live && <label className="range-label">
                      <span>Scene cloud cover</span>
                      <strong>≤ {cloud}%</strong>
                      <input
                        type="range"
                        aria-label="Maximum cloud cover"
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
                      </strong>{" "}
                      scenes <span className="muted">· {live ? "loaded results" : "catalog snapshot"}</span>
                    </span>
                    <select
                      aria-label="Sort scenes"
                      value={sort}
                      onChange={(e) => setSort(e.target.value)}
                    >
                      <option value="date">Newest</option>
                      <option value="cloud">Clearest</option>
                    </select>
                  </div>
                  <div className="scene-list">
                    {live && liveState === "loading" ? <div className="loading-state" role="status"><span className="loader" />Searching Earth Search…</div> : live && !liveCatalog ? <Empty icon={Search} title={liveState === "error" ? "Catalog request failed" : "Search the live catalog"}>Set your area and dates above. Results come directly from Earth Search; the sample catalog is separate.</Empty> : condition === "error" ? (
                      <Empty
                        icon={AlertCircle}
                        title="Source unavailable"
                        action={
                          <Btn
                            onClick={() => setCondition("ready")}
                            icon={RotateCcw}
                          >
                            Retry sample
                          </Btn>
                        }
                      >
                        The design scenario represents a failed catalog request.
                        Your workspace is kept.
                      </Empty>
                    ) : condition === "loading" ? (
                      <div className="loading-state">
                        <span className="loader" />
                        Loading sample results…
                        <Btn onClick={() => setCondition("ready")}>
                          Show loaded state
                        </Btn>
                      </div>
                    ) : condition === "empty" || !filtered.length ? (
                      <Empty
                        icon={Search}
                        title="No matching scenes"
                        action={
                          <Btn
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
                          >
                            Reset filters
                          </Btn>
                        }
                      >
                        Try a wider date range or allow more cloud cover.
                      </Empty>
                    ) : (
                      filtered.map((s) => (
                        <button
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
                            alt={"True-color preview, " + date(s.date)}
                          />
                          <div className="scene-info">
                            <strong>{date(s.date)}</strong>
                            <span>
                              {s.id.startsWith("S2C")
                                ? "Sentinel-2C"
                                : s.id.startsWith("S2B")
                                  ? "Sentinel-2B"
                                  : "Sentinel-2A"}{" "}
                              <span className="muted">· L2A</span>
                            </span>
                            <small>
                              <Cloud size={12} />
                              {s.cloud == null ? "Unknown" : s.cloud.toFixed(1) + "%"}<span>{s.gsd ? `${s.gsd} m RGB` : "RGB preview"}</span>
                            </small>
                          </div>
                          {selected?.id === s.id && (
                            <CheckCircle2
                              className="selection-check"
                              size={16}
                            />
                          )}
                        </button>
                      ))
                    )}
                  </div>
                  {live && liveCatalog?.next && <div className="catalog-next"><Btn disabled={liveState === "more"} onClick={() => runSearch(true)}>{liveState === "more" ? "Loading more…" : "Load more scenes"}</Btn><span>Only loaded results are counted and sorted.</span></div>}
                  <div className="panel-foot">
                    <Database size={13} />
                    <span>Earth Search · {live ? "live HTTPS catalog" : "June 2025 snapshot"}</span>
                  </div>
                </>
              ) : (
                <div className="domain-placeholder">
                  <div className="domain-title">
                    <span className="eyebrow">{domain.toUpperCase()}</span>
                    <h2>
                      {
                        {
                          Imagery: "Explore imagery sources",
                          Elevation: "Prepare terrain products",
                          Vector: "Build a project dataset",
                          "3D": "Inspect spatial assets",
                          "Local Data": "Bring your own data",
                        }[domain]
                      }
                    </h2>
                  </div>
                  {domain === "Local Data" ? (
                    <>
                      <p>
                        Choose a file to inspect its name and size. Native
                        previews and processing are planned.
                      </p>
                      <label className="button file-button">
                        <Plus size={16} />
                        Choose a local file
                        <input
                          type="file"
                          onChange={(e) =>
                            setLocalFile(e.target.files[0] || null)
                          }
                        />
                      </label>
                      {localFile && (
                        <div className="info-box">
                          <strong>{localFile.name}</strong>
                          <p>
                            {(localFile.size / 1024).toFixed(1)} KB · stays on
                            this device
                          </p>
                        </div>
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
                        <button
                          className="domain-option"
                          key={label}
                          onClick={() => setModal("planned")}
                        >
                          <span>{label}</span>
                          <ChevronRight size={16} />
                        </button>
                      ))}
                      <p className="muted">
                        This category is part of the full design. Its source
                        adapter is not connected in this prototype.
                      </p>
                    </>
                  )}
                </div>
              )}
            </aside>
            {selected ? <main className="map-workspace">
              <div className="map-toolbar">
                <div className="segmented">
                  <button
                    className={!compare ? "active" : ""}
                    onClick={() => setCompare(false)}
                  >
                    <Layers size={15} />
                    Preview
                  </button>
                  <button
                    className={compare ? "active" : ""}
                    disabled={!comparisons.length}
                    title={comparisons.length ? "Compare scenes with matching source grids" : "Comparison needs two previews with the same CRS, transform and dimensions"}
                    onClick={() => {
                      setCompare(true);
                      setCompareId(other?.id || "");
                    }}
                  >
                    <SlidersHorizontal size={15} />
                    Compare
                  </button>
                </div>
                <div className="toolbar-end">
                  <Badge tone="on-map">True color</Badge>
                  <button
                    className="map-icon"
                    aria-label={inspector ? "Hide inspector" : "Show inspector"}
                    onClick={() => setInspector(!inspector)}
                  >
                    {inspector ? (
                      <PanelRightClose size={18} />
                    ) : (
                      <PanelRightOpen size={18} />
                    )}
                  </button>
                </div>
              </div>
              {!comparisons.length && <p className="catalog-compare-note">Comparison needs another scene with the same CRS, transform and dimensions.</p>}
              <div className="imagery-canvas">
                <svg
                  viewBox={`${500 - 500 / zoom} ${500 - 500 / zoom} ${1000 / zoom} ${1000 / zoom}`}
                  preserveAspectRatio="xMidYMid slice"
                  aria-label={live ? "Sentinel-2 provider thumbnail, no georeferenced area overlay" : "Real Sentinel-2 thumbnail preview with approximate saved area overlay"}
                >
                  <defs>
                    <clipPath id="comparisonClip">
                      <rect x="0" y="0" width={split * 10} height="1000" />
                    </clipPath>
                  </defs>
                  <image
                    href={selected.thumbnail || undefined}
                    onError={() => setPreviewError(true)}
                    width="1000"
                    height="1000"
                  />
                  {showArea && !live && (
                    <g>
                      <polygon
                        points={outline}
                        fill="rgba(78,149,255,.16)"
                        stroke="#d4e6ff"
                        strokeWidth="2.4"
                        strokeDasharray="8 5"
                        vectorEffect="non-scaling-stroke"
                      />
                      {[
                        [361.56, 269.52],
                        [546.27, 268.4],
                        [545.1, 106.73],
                        [360.78, 107.84],
                      ].map(([x, y], i) => (
                        <rect
                          key={i}
                          x={x - 3}
                          y={y - 3}
                          width="6"
                          height="6"
                          fill="#fff"
                        />
                      ))}
                      <rect
                        x="360"
                        y="77"
                        width="173"
                        height="24"
                        rx="4"
                        fill="#fff"
                      />
                      <text
                        x="370"
                        y="94"
                        fill="#17212b"
                        fontSize="13"
                        fontFamily="Inter"
                      >
                        San Francisco Bay · AOI
                      </text>
                    </g>
                  )}
                </svg>
                {comparing && (
                  <svg
                    className="compare-overlay"
                    style={{ clipPath: "inset(0 " + (100 - split) + "% 0 0)" }}
                    viewBox={`${500 - 500 / zoom} ${500 - 500 / zoom} ${1000 / zoom} ${1000 / zoom}`}
                    preserveAspectRatio="xMidYMid slice"
                    aria-label="Reference scene thumbnail"
                  >
                    <image
                      href={other.thumbnail}
                      onError={() => { setCompare(false); setToast("The reference thumbnail could not load. Try another scene."); }}
                      width="1000"
                      height="1000"
                    />
                    {showArea && !live && (
                      <polygon
                        points={outline}
                        fill="rgba(78,149,255,.16)"
                        stroke="#d4e6ff"
                        strokeWidth="2.4"
                        strokeDasharray="8 5"
                        vectorEffect="non-scaling-stroke"
                      />
                    )}
                  </svg>
                )}
                {comparing && (
                  <div className="compare-line" style={{ left: split + "%" }}>
                    <span>
                      <SlidersHorizontal size={19} />
                    </span>
                  </div>
                )}
              </div>
              {(previewError || !selected.thumbnail) && <div className="catalog-preview-unavailable" role="status"><ImageIcon size={25} /><strong>Preview unavailable</strong><span>The provider thumbnail could not load. Scene metadata and original assets are still available.</span><a href={selected.itemURL} target="_blank" rel="noreferrer">Open source metadata <ExternalLink size={13} /></a></div>}
              {comparing && (
                <div className="compare-controls">
                  <label>
                    Reference
                    <select
                      aria-label="Reference scene"
                      value={other.id}
                      onChange={(e) => setCompareId(e.target.value)}
                    >
                      {comparisons
                        .map((s) => (
                          <option key={s.id} value={s.id}>
                            {date(s.date)}
                          </option>
                        ))}
                    </select>
                  </label>
                  <input
                    type="range"
                    aria-label="Comparison split"
                    min="0"
                    max="100"
                    value={split}
                    onChange={(e) => setSplit(+e.target.value)}
                  />
                  <span>{date(selected.date)}</span>
                </div>
              )}
              <div className="map-controls">
                <button
                  className="map-icon"
                  aria-label="Zoom in"
                  onClick={() => setZoom(Math.min(zoom + 0.25, 2.5))}
                >
                  <Plus size={18} />
                </button>
                <button
                  className="map-icon"
                  aria-label="Zoom out"
                  onClick={() => setZoom(Math.max(zoom - 0.25, 1))}
                >
                  <Minus size={18} />
                </button>
                <button
                  className="map-icon"
                  aria-label="Fit scene"
                  onClick={() => setZoom(1)}
                >
                  <Maximize size={16} />
                </button>
                <div className="control-separator" />
                <button
                  className={"map-icon " + (showArea ? "control-active" : "")}
                  aria-pressed={showArea && !live}
                  aria-label="Toggle saved area"
                  disabled={live}
                  title={live ? "Live thumbnails are not georeferenced; the search box is not drawn over them" : "Show the sample area"}
                  onClick={() => setShowArea(!showArea)}
                >
                  <SquareDashed size={18} />
                </button>
              </div>
              <div className="scene-caption">
                <Badge tone="on-map">SENTINEL-2 L2A</Badge>
                <h2>{live ? selected.properties["grid:code"] || "Selected observation" : areaName}</h2>
                <p>
                  {date(selected.date)} <span>·</span>{" "}
                  {selected.cloud == null ? "Unknown" : selected.cloud.toFixed(1) + "%"} scene cloud cover
                </p>
              </div>
              <div className="map-attribution">
                <span>
                  Contains Copernicus Sentinel data ({selected.date.slice(0, 4)}) · Earth Search
                </span>
                <button onClick={() => setModal("provenance")}>
                  Thumbnail, not analytical data <Info size={12} />
                </button>
              </div>
              <div className="timeline">
                <div className="timeline-label">
                  <span className="eyebrow">OBSERVATIONS</span>
                  <strong>{live ? "Loaded scenes" : "June 2025"}</strong>
                </div>
                <div className="timeline-track">
                  {[...scenes].reverse().map((s) => (
                    <button
                      key={s.id}
                      className={s.id === selected.id ? "selected" : ""}
                      onClick={() => setSelected(s)}
                      aria-label={"Select observation " + stamp(s.date)}
                    >
                      <span className="date-line" />
                      <span className="observation-dot" />
                      <small>{live ? s.date.slice(5, 10) : s.date.slice(8, 10)}</small>
                    </button>
                  ))}
                </div>
                <button
                  className="icon-btn"
                  aria-label="Timeline help"
                  onClick={() => setModal("provenance")}
                >
                  <Info size={16} />
                </button>
              </div>
            </main> : <main className="catalog-blank"><Empty icon={Search} title={liveState === "loading" ? "Searching your area" : liveCatalog ? "No scenes for this search" : "Choose your next observation"}>Use the catalog on the left to choose an area and dates. The selected scene preview will appear here.</Empty></main>}
            {inspector && selected && (
              <aside className="inspector">
                <div className="inspector-heading">
                  <span className="eyebrow">DATASET DETAILS</span>
                  <button
                    className="icon-btn"
                    aria-label="Close details panel"
                    onClick={() => setInspector(false)}
                  >
                    <X size={16} />
                  </button>
                </div>
                <h2>Sentinel-2 L2A</h2>
                <p className="muted">Surface reflectance collection</p>
                <div className="preview-image">
                  <SceneThumbnail
                    src={selected.thumbnail || undefined}
                    alt="Selected scene thumbnail"
                  />
                  <span>RGB PREVIEW</span>
                </div>
                <div className="detail-section">
                  <h3>Observation</h3>
                  <dl>
                    <dt>Acquired</dt>
                    <dd>{date(selected.date)}</dd>
                    <dt>Scene clouds</dt>
                    <dd>{selected.cloud == null ? "Unknown" : selected.cloud.toFixed(2) + "%"}</dd>
                    <dt>RGB resolution</dt>
                    <dd>{selected.gsd ? `${selected.gsd} meters` : "Not specified"}</dd>
                    <dt>Source grid</dt>
                    <dd className="mono">{selected.crs || "Not specified"}</dd>
                  </dl>
                  <p className="scene-id mono">{selected.id}</p>
                </div>
                <div className="detail-section">
                  <h3>Area & output</h3>
                  <dl>
                    <dt>Saved area</dt>
                    <dd>{areaName}</dd>
                    <dt>Selection</dt>
                    <dd>Bounding box</dd>
                    <dt>Processing</dt>
                    <dd>Local · planned</dd>
                  </dl>
                  <button
                    className="text-link"
                    onClick={() => setModal("area")}
                  >
                    Inspect area <ArrowUpRight size={14} />
                  </button>
                </div>
                <div className="detail-section">
                  <h3>Source & provenance</h3>
                  <div className="source-line">
                    <span className="source-symbol">
                      <Satellite size={17} />
                    </span>
                    <div>
                      <strong>Copernicus Sentinel</strong>
                      <small>Catalog by Earth Search</small>
                    </div>
                  </div>
                    <button
                    className="text-link"
                    disabled={!selected}
                    onClick={() => setModal("provenance")}
                  >
                    View metadata & source <ArrowUpRight size={14} />
                  </button>
                </div>
                <div className="inspector-bottom">
                  <DownloadAssetButton scene={selected} />
                  <Btn
                    icon={Download}
                    onClick={() => setModal("export")}
                  >
                    Review processing plan
                  </Btn>
                  <Btn icon={Workflow} onClick={() => setModal("recipe")}>
                    Save as recipe
                  </Btn>
                  <p>Processing plans and recipes remain design simulations.</p>
                </div>
              </aside>
            )}
          </div>
        ) : (
          <main className="content-page">
            {page === "Tasks" ? (
              <>
                <PageHeading
                  eyebrow="EXECUTION"
                  title="Tasks"
                  sub="Follow every step, from source to output."
                  action={
                    <Btn
                      icon={Plus}
                      onClick={() => {
                        go("Explore");
                      }}
                    >
                      New task
                    </Btn>
                  }
                />
                <RuntimeTasks />
                <details className="design-simulations">
                  <summary>Design simulations below · {tasks.length} sample tasks</summary>
                <div className="notice">
                  <Info size={17} />
                  <span>
                    These sample tasks simulate processing. Real downloads appear above.
                  </span>
                </div>
                {!tasks.length ? (
                  <Empty
                    icon={ListTodo}
                    title="Your next task starts with an area"
                    action={
                      <Btn primary onClick={() => go("Explore")}>
                        Explore data
                      </Btn>
                    }
                  >
                    Choose a scene and prepare an export to review the task
                    lifecycle.
                  </Empty>
                ) : (
                  <div className="task-list">
                    {tasks.map((t) => (
                      <article className="task-card" key={t.id}>
                        <div className="task-icon">
                          <Download size={22} />
                        </div>
                        <div className="task-main">
                          <div className="task-title">
                            <h3>{t.name}</h3>
                            <Badge
                              tone={
                                t.status === "Succeeded"
                                  ? "green"
                                  : t.status === "Failed"
                                    ? "red"
                                    : "blue"
                              }
                            >
                              {t.status === "Succeeded"
                                ? "Simulation complete"
                                : t.status}
                            </Badge>
                          </div>
                          <p>
                            {t.sceneId} · {t.format} design
                          </p>
                          <div className="progress">
                            <span style={{ width: t.progress + "%" }} />
                          </div>
                          <div className="task-stage">
                            <span>
                              {t.status === "Succeeded"
                                ? "Sample report available; no raster was created"
                                : t.status === "Failed"
                                  ? "Simulated connection interruption; retry as a new attempt"
                                  : t.status === "Cancelled"
                                    ? "Simulation cancelled"
                                    : `Simulated ${t.progress < 40 ? "source read" : t.progress < 80 ? "processing" : "validation"}`}
                            </span>
                            <strong>{t.progress}%</strong>
                          </div>
                        </div>
                        <div className="task-actions">
                          {t.status === "Running" ? (
                            <>
                              <Btn
                                icon={Pause}
                                onClick={() => changeTask(t.id, "Paused")}
                              >
                                Pause
                              </Btn>
                              <Btn onClick={() => changeTask(t.id, "Failed")}>
                                Simulate failure
                              </Btn>
                            </>
                          ) : t.status === "Paused" ? (
                            <Btn
                              icon={Play}
                              onClick={() => changeTask(t.id, "Running")}
                            >
                              Resume
                            </Btn>
                          ) : t.status === "Failed" ? (
                            <Btn
                              icon={RotateCcw}
                              onClick={() =>
                                setTasks((old) => [
                                  {
                                    ...t,
                                    id: crypto.randomUUID(),
                                    parentId: t.id,
                                    progress: 0,
                                    status: "Running",
                                  },
                                  ...old,
                                ])
                              }
                            >
                              Retry
                            </Btn>
                          ) : null}
                          {["Running", "Paused"].includes(t.status) && (
                            <button
                              className="text-link danger"
                              onClick={() => changeTask(t.id, "Cancelled")}
                            >
                              Cancel
                            </button>
                          )}
                          {t.status === "Succeeded" && (
                            <Btn onClick={() => go("My Data")}>View report</Btn>
                          )}
                        </div>
                      </article>
                    ))}
                  </div>
                )}
                </details>
              </>
            ) : page === "My Data" ? (
              <>
                <PageHeading
                  eyebrow="LOCAL LIBRARY"
                  title="My Data"
                  sub="Your outputs, with their story intact."
                />
                <RuntimeLibrary />
                <details className="design-simulations">
                  <summary>Design simulation reports · {outputs.length} reports</summary>
                {!outputs.length ? (
                  <Empty
                    title="A place for finished work"
                    action={
                      <Btn primary onClick={() => go("Explore")}>
                        Prepare an export
                      </Btn>
                    }
                  >
                    The prototype adds a simulation report here after a sample
                    task. It never claims to produce a GeoTIFF.
                  </Empty>
                ) : (
                  <div className="output-grid">
                    {outputs.map((o) => (
                      <article className="output-card" key={o.id}>
                        <img
                          src={
                            (sampleCatalog?.scenes.find((s) => s.id === o.sceneId) || scenes.find((s) => s.id === o.sceneId))?.thumbnail
                          }
                          alt="Source scene thumbnail, not exported output"
                        />
                        <div>
                          <Badge>Simulation report</Badge>
                          <h3>{o.name}</h3>
                          <p>Source preview · No raster output</p>
                          <div className="row-actions">
                            <Btn
                              icon={Download}
                              onClick={() =>
                                showJSON("geod-design-report.json", {
                                  ...o,
                                  warning:
                                    "Design simulation only. No raster output or scientific validation.",
                                })
                              }
                            >
                              Report JSON
                            </Btn>
                            <button
                              className="icon-btn"
                              aria-label={"Remove report " + o.name}
                              onClick={() =>
                                setModal({ type: "delete", id: o.id })
                              }
                            >
                              <Trash2 size={16} />
                            </button>
                          </div>
                        </div>
                      </article>
                    ))}
                  </div>
                )}
                </details>
              </>
            ) : page === "Recipes" ? (
              <>
                <PageHeading
                  eyebrow="REPEATABLE WORK"
                  title="Recipes"
                  sub="Keep the choices. Run them again when the data changes."
                  action={
                    <Btn icon={Plus} disabled={!selected} onClick={() => setModal("recipe")}>
                      Create recipe
                    </Btn>
                  }
                />
                <div className="notice">
                  <Workflow size={17} />
                  Recipe files use design-prototype/v1. They are not executable
                  Core recipes.
                </div>
                {!recipes.length ? (
                  <Empty
                    icon={Workflow}
                    title="Make a good workflow repeatable"
                    action={
                      <Btn primary disabled={!selected} onClick={() => setModal("recipe")}>
                        Save current selection
                      </Btn>
                    }
                  >
                    Save an area, a fixed scene and output preferences. Your
                    recipe stays in this browser.
                  </Empty>
                ) : (
                  <div className="table-wrap">
                    <table>
                      <thead>
                        <tr>
                          <th>Recipe</th>
                          <th>Input</th>
                          <th>Output</th>
                          <th>Saved</th>
                          <th>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {recipes.map((r) => (
                          <tr key={r.id}>
                            <td>
                              <strong>{r.name}</strong>
                              <small className="block">
                                {r.area?.name || "Saved area"} · fixed scene
                              </small>
                            </td>
                            <td>Sentinel-2 L2A</td>
                            <td>{r.output.format}</td>
                            <td>{date(r.savedAt)}</td>
                            <td>
                              <div className="row-actions">
                                <Btn
                                  icon={Download}
                                  onClick={() =>
                                    showJSON("geod-design-recipe.json", r)
                                  }
                                >
                                  JSON
                                </Btn>
                                <Btn
                                  icon={Play}
                                  disabled={!scenes.some((s) => s.id === r.input.itemId)}
                                  title="Review is available when the saved scene is loaded in the current catalog"
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
                                >
                                  Review
                                </Btn>
                              </div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            ) : page === "Sources" ? (
              <>
                <PageHeading
                  eyebrow="DATA CONNECTIONS"
                  title="Sources"
                  sub="Know where your data comes from, before you use it."
                  action={
                    <Btn icon={Plus} onClick={() => setModal("planned")}>
                      Add source
                    </Btn>
                  }
                />
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Source</th>
                        <th>Data</th>
                        <th>Connection</th>
                        <th>Availability</th>
                        <th>Action</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sourceRows.map(([name, data, status, desc], i) => (
                        <tr key={name}>
                          <td>
                            <strong>{name}</strong>
                          </td>
                          <td>{data}</td>
                          <td>
                            <Badge tone={i === 0 ? "green" : ""}>
                              {i === 0 && !enabled ? "Disabled" : status}
                            </Badge>
                          </td>
                          <td>{desc}</td>
                          <td>
                            {i === 0 ? (
                              <Btn onClick={() => setEnabled(!enabled)}>
                                {enabled ? "Hide source results" : "Show source results"}
                              </Btn>
                            ) : (
                              <Btn onClick={() => setModal("planned")}>
                                View plan
                              </Btn>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="info-box">
                  <ShieldCheck size={23} />
                  <h3>Access and permission travel together</h3>
                  <p>
                    The full product will track preview, download, offline use
                    and redistribution separately. A connected source alone will
                    not enable every operation.
                  </p>
                  <button
                    className="text-link"
                    disabled={!selected}
                    onClick={() => setModal("provenance")}
                  >
                    Inspect selected scene evidence <ArrowUpRight size={14} />
                  </button>
                </div>
              </>
            ) : page === "Cloud" ? (
              <>
                <PageHeading
                  eyebrow="OPTIONAL SERVICES"
                  title="A workspace you can share"
                  sub="Local work remains yours. Collaboration is a separate product decision."
                />
                <div className="cloud-layout">
                  <div className="cloud-diagram">
                    <div className="diagram-node">
                      <Folder />
                      <strong>Local workspace</strong>
                      <span>Files & processing</span>
                    </div>
                    <div className="diagram-connector" />
                    <div className="diagram-node outline">
                      <Cloud />
                      <strong>Optional sync</strong>
                      <span>Selected metadata only</span>
                    </div>
                  </div>
                  <div className="cloud-copy">
                    <Badge>Proposal · not connected</Badge>
                    <h2>
                      Share the workflow.
                      <br />
                      Keep control of the data.
                    </h2>
                    <p>
                      Private recipe versions, shared source configuration and
                      run summaries are proposed collaboration features.
                    </p>
                    <ul>
                      <li>Choose exactly which metadata leaves your device.</li>
                      <li>Use your own storage and execution environment.</li>
                      <li>Export your recipes when you leave.</li>
                    </ul>
                    <div className="notice">
                      Pricing and the commercial model are undecided. No
                      checkout or account creation is active.
                    </div>
                  </div>
                </div>
              </>
            ) : page === "Settings" ? (
              <>
                <PageHeading
                  eyebrow="YOUR WORKSPACE"
                  title="Settings"
                  sub="A local-first workspace, on your terms."
                />
                <div className="settings-list">
                  <div>
                    <span>
                      <strong>Appearance</strong>
                      <small>Saved on this browser.</small>
                    </span>
                    <select
                      aria-label="Appearance"
                      value={theme}
                      onChange={(e) => setTheme(e.target.value)}
                    >
                      <option value="light">Light</option>
                      <option value="dark">Dark</option>
                    </select>
                  </div>
                  <div>
                    <span>
                      <strong>Usage analytics</strong>
                      <small>No analytics is sent by this prototype.</small>
                    </span>
                    <button
                      role="switch"
                      aria-checked={telemetry}
                      aria-label="Usage analytics design toggle"
                      className={"switch " + (telemetry ? "on" : "")}
                      onClick={() => setTelemetry(!telemetry)}
                    >
                      <span />
                    </button>
                  </div>
                  <div>
                    <span>
                      <strong>Local design data</strong>
                      <small>
                        Recipes, simulations and reports are stored in browser
                        storage.
                      </small>
                    </span>
                    <Btn onClick={() => setModal("reset")}>
                      Clear design data
                    </Btn>
                  </div>
                  <div>
                    <span>
                      <strong>Keyboard navigation</strong>
                      <small>
                        Open command search with Ctrl / Cmd + K. Close dialogs
                        with Esc.
                      </small>
                    </span>
                    <Keyboard size={22} />
                  </div>
                </div>
              </>
            ) : (
              <Empty
                title="Choose a workspace page"
                action={<Btn onClick={() => go("Explore")}>Explore</Btn>}
              >
                Use the navigation to return to your data.
              </Empty>
            )}
          </main>
        )}
        <footer className="statusbar">
          <span>
            <span className="status-dot" />
            Local workspace <span className="status-divider">/</span> No
            account required
          </span>
          <span>
            {tasks.filter((t) => t.status === "Running").length
              ? `${tasks.filter((t) => t.status === "Running").length} simulation running`
              : live ? "Live catalog · original source assets" : "Sample catalog · cached scene metadata"}
            <span className="status-divider">/</span>
            <button onClick={() => setModal("about")}>Prototype 0.1</button>
          </span>
        </footer>
      </div>
      {toast && (
        <div className="toast" role="status">
          <CheckCircle2 size={17} />
          {toast}
        </div>
      )}
      {modal && (
        <Modal
          title={
            typeof modal === "object"
              ? modal.type === "json"
                ? "Export JSON"
                : "Remove report"
              : {
                  export: "Prepare export",
                  recipe: "Save recipe",
                  area: "Saved area",
                  provenance: "Data provenance",
                  commands: "Search commands",
                  states: "Review interface states",
                  planned: "Planned capability",
                  about: "About this design",
                  reset: "Clear local design data",
                }[modal]
          }
          onClose={() => setModal(null)}
          wide={modal === "export"}
        >
          {modal === "export" && selected ? (
            <>
              <div className="dialog-body export-layout">
                <div>
                  <Badge tone="blue">DESIGN SIMULATION</Badge>
                  <h3>{recipeName}</h3>
                  <p className="muted">
                    Review what the desktop export flow will collect.
                  </p>
                  <label className="field">
                    Output format
                    <select
                      value={format}
                      onChange={(e) => setFormat(e.target.value)}
                    >
                      <option>COG</option>
                      <option>GeoTIFF</option>
                    </select>
                  </label>
                  <label className="field">
                    Coordinate reference
                    <input
                      value={selected.crs || "Source CRS not specified"}
                      readOnly
                    />
                  </label>
                  <div className="two-fields">
                    <label className="field">
                      Pixel size
                      <input value={selected.gsd ? `${selected.gsd} meters` : "Not specified"} readOnly />
                    </label>
                    <label className="field">
                      Processing location
                      <input value="Local · not connected" readOnly />
                    </label>
                  </div>
                </div>
                <div className="export-summary">
                  <h3>Export plan</h3>
                  <div>
                    <Check size={16} />
                    Use the fixed source scene
                  </div>
                  <div>
                    <Check size={16} />
                    Clip to saved bounding box
                  </div>
                  <div>
                    <Check size={16} />
                    Preserve source & recipe
                  </div>
                  <div>
                    <Info size={16} />
                    Native raster engine required
                  </div>
                  <hr />
                  <p>
                    No size or cost estimate is available in this prototype. No
                    data download starts from this dialog.
                  </p>
                </div>
              </div>
              <div className="dialog-footer">
                <Btn onClick={() => setModal(null)}>Cancel</Btn>
                <Btn
                  icon={Download}
                  onClick={() => showJSON("geod-design-recipe.json", recipe())}
                >
                  Download plan JSON
                </Btn>
                <Btn primary icon={Play} onClick={simulate}>
                  Simulate task
                </Btn>
              </div>
            </>
          ) : modal === "recipe" && selected ? (
            <>
              <div className="dialog-body">
                <label className="field">
                  Recipe name
                  <input
                    autoFocus
                    value={recipeName}
                    onChange={(e) => setRecipeName(e.target.value)}
                  />
                </label>
                <dl>
                  <dt>Area</dt>
                  <dd>{areaName}</dd>
                  <dt>Fixed observation</dt>
                  <dd>{date(selected.date)}</dd>
                  <dt>Planned output</dt>
                  <dd>{format} · {selected.gsd ? `${selected.gsd} meters` : "source resolution"}</dd>
                </dl>
                <div className="notice">
                  Saved locally. No credentials are included. This design recipe
                  is not yet executable.
                </div>
              </div>
              <div className="dialog-footer">
                <Btn onClick={() => setModal(null)}>Cancel</Btn>
                <Btn
                  primary
                  icon={Save}
                  disabled={!recipeName.trim()}
                  onClick={saveRecipe}
                >
                  Save recipe
                </Btn>
              </div>
            </>
          ) : modal === "area" ? (
            <div className="dialog-body">
              <Badge tone="blue">{live ? "SEARCH BOUNDING BOX" : "SAVED BOUNDING BOX"}</Badge>
              <h3>{areaName}</h3>
              <p>
                {live ? "These are the last submitted search bounds. Edit the coordinates in Live catalog to search a new area." : "The sample workspace uses the following saved study area."}
              </p>
              <dl>
                {["West", "South", "East", "North"].map((name, i) => (
                  <React.Fragment key={name}>
                    <dt>{name}</dt>
                    <dd className="mono">{bbox[i]}°</dd>
                  </React.Fragment>
                ))}
              </dl>
              <p className="muted">
                {live ? "WGS 84 coordinates. The query area is not drawn over the provider thumbnail because this preview does not perform georeferencing." : "WGS 84 coordinates. The sample thumbnail overlay is projected to the source UTM grid and is for orientation only."}
              </p>
              <Btn
                icon={Download}
                onClick={() =>
                  showJSON("geod-search-area.geojson", {
                    type: "Feature",
                    properties: { name: areaName, fixture: !live },
                    geometry: {
                      type: "Polygon",
                      coordinates: [
                        [
                          [bbox[0], bbox[1]],
                          [bbox[2], bbox[1]],
                          [bbox[2], bbox[3]],
                          [bbox[0], bbox[3]],
                          [bbox[0], bbox[1]],
                        ],
                      ],
                    },
                  })
                }
              >
                Download area GeoJSON
              </Btn>
            </div>
          ) : modal === "provenance" && selected && catalog ? (
            <div className="dialog-body">
              <Badge tone="green">{live ? "LIVE CATALOG RESPONSE" : "REAL CATALOG SNAPSHOT"}</Badge>
              <h3>{selected.id}</h3>
              <p>{catalog.attribution}</p>
              <dl>
                <dt>Acquisition</dt>
                <dd>{date(selected.date)}</dd>
                <dt>Metadata fetched</dt>
                <dd>{date(catalog.retrievedAt)}</dd>
                <dt>Preview</dt>
                <dd>Provider JPEG thumbnail</dd>
                <dt>Cloud cover</dt>
                <dd>Full scene, not AOI-specific</dd>
              </dl>
              <p>
                {live ? "Scene metadata is queried from Earth Search using the submitted area, dates and cloud limit. Counts and local sorting cover loaded pages only. Provider thumbnails are visual previews; comparison requires matching source grids and does not perform scientific band math." : "Scene metadata and thumbnails come from seven saved catalog records. Sample filters run locally. Download original asset retrieves the remote source file; processing plans are design simulations."}
              </p>
              {selected.sha256 && <p className="mono hash">Cached preview SHA-256: {selected.sha256}</p>}
              <div className="link-stack">
                <a href={selected.source || selected.itemURL} target="_blank" rel="noreferrer">
                  Original preview asset <ExternalLink size={14} />
                </a>
                <a href={catalog.query} target="_blank" rel="noreferrer">
                  Original STAC query <ExternalLink size={14} />
                </a>
                <a href={catalog.registry} target="_blank" rel="noreferrer">
                  Dataset registry & terms <ExternalLink size={14} />
                </a>
              </div>
            </div>
          ) : modal === "commands" ? (
            <div className="dialog-body">
              <label className="search-input">
                <Search size={17} />
                <input
                  autoFocus
                  placeholder="Go to a page or action…"
                  value={cmd}
                  onChange={(e) => setCmd(e.target.value)}
                  aria-label="Command search"
                />
              </label>
              <div className="command-results">
                {[
                  ...nav.map(([n]) => [n, () => go(n)]),
                  ...(selected ? [["Review processing plan", () => setModal("export")], ["Save recipe", () => setModal("recipe")]] : []),
                  ["Cloud", () => go("Cloud")],
                  ["Settings", () => go("Settings")],
                ]
                  .filter(([n]) => n.toLowerCase().includes(cmd.toLowerCase()))
                  .map(([n, fn]) => (
                    <button
                      key={n}
                      onClick={() => {
                        setModal(null);
                        fn();
                        setCmd("");
                      }}
                    >
                      <span>{n}</span>
                      <ChevronRight size={16} />
                    </button>
                  ))}
              </div>
            </div>
          ) : modal === "states" ? (
            <div className="dialog-body">
              <p>
                These controls preview catalog interface states. They do not
                alter the source service.
              </p>
              {[
                ["ready", "Ready · real cached scenes"],
                ["empty", "Empty results"],
                ["loading", "Loading"],
                ["error", "Source error"],
              ].map(([v, label]) => (
                <button
                  key={v}
                  className="state-option"
                  onClick={() => {
                    setCondition(v);
                    setModal(null);
                  }}
                >
                  <span>{label}</span>
                  {condition === v && <Check size={17} />}
                </button>
              ))}
            </div>
          ) : modal === "planned" ? (
            <div className="dialog-body">
              <p>
                This belongs to the full product scope. The current design
                prototype does not connect this adapter or execute this
                operation.
              </p>
              <p className="muted">
                The implementation map and release gates in the specification
                track the remaining work.
              </p>
              <Btn
                onClick={() => {
                  setModal(null);
                  go("Sources");
                }}
              >
                View source catalog
              </Btn>
            </div>
          ) : modal === "reset" ? (
            <>
              <div className="dialog-body">
                <p>
                  Remove this prototype’s saved recipes, task simulations and
                  reports from browser storage? No files on your computer will
                  be removed.
                </p>
              </div>
              <div className="dialog-footer">
                <Btn onClick={() => setModal(null)}>Keep data</Btn>
                <Btn
                  primary
                  onClick={() => {
                    setRecipes([]);
                    setTasks([]);
                    setOutputs([]);
                    setModal(null);
                    setToast("Local design data cleared.");
                  }}
                >
                  Clear design data
                </Btn>
              </div>
            </>
          ) : typeof modal === "object" && modal.type === "json" ? (
            <>
              <div className="dialog-body">
                <p>{modal.filename}</p>
                <p className="muted">
                  Review or copy the full file below. If your embedded browser
                  blocks downloads, open this local preview in your regular
                  browser.
                </p>
                <textarea
                  className="json-preview mono"
                  aria-label="Exported JSON"
                  readOnly
                  value={JSON.stringify(modal.value, null, 2)}
                />
              </div>
              <div className="dialog-footer">
                <Btn onClick={() => setModal(null)}>Close</Btn>
                <Btn
                  primary
                  icon={Download}
                  onClick={() => downloadJSON(modal.filename, modal.value)}
                >
                  Save JSON file
                </Btn>
              </div>
            </>
          ) : typeof modal === "object" ? (
            <>
              <div className="dialog-body">
                <p>
                  Remove this simulation report from the library? Files on your
                  computer are unaffected.
                </p>
              </div>
              <div className="dialog-footer">
                <Btn onClick={() => setModal(null)}>Keep report</Btn>
                <Btn
                  primary
                  onClick={() => {
                    setOutputs((old) => old.filter((o) => o.id !== modal.id));
                    setTasks((old) => old.filter((t) => t.id !== modal.id));
                    setModal(null);
                  }}
                >
                  Remove report
                </Btn>
              </div>
            </>
          ) : (
            <div className="dialog-body">
              <span className="brand-mark">
                <Layers />
              </span>
              <h3>GeoD Global · local workspace</h3>
              <p>
                A local-first geospatial data workspace with live catalog search
                and original asset downloads. Planned processing tools remain
                visible as explicit design simulations.
              </p>
              <ul>
                <li>Live Earth Search queries and a separate cached sample catalog.</li>
                <li>
                  Working filters, comparison, recipes and local persistence.
                </li>
                <li>Original source asset downloads with local task history.</li>
                <li>Explicit simulations for raster processing and recipe execution.</li>
                <li>Six data domains, with unconnected adapters marked.</li>
                <li>Cloud features and commercial terms remain proposals.</li>
              </ul>
              <p className="muted">
                Inter and sample previews are bundled locally. Live searches,
                remote previews and asset downloads contact their source providers.
                This workspace sends no analytics.
              </p>
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
createRoot(document.getElementById("root")).render(<RuntimeProvider><App /></RuntimeProvider>);
