# GeoD Global — 代码基线与完整实施映射

**日期：2026-09-21 · 版本：1.1 · 类型：国内源码参考审计与 Global 实施映射**

**对应：01 产品总纲、02 工作包与验收。本文保留完整海外产品范围；实施批次不等于缩减产品。**

**现行边界：用户已确认 Global 独立建仓。** 下文国内源码、HEAD、脏工作树与测试记录保留为此前审计证据，不是 Global 的源码基线，不代表本轮改变国内版。执行必须遵守[附件08](08-Repository-Boundary.md)：不复制、移动、提交或重置国内工作树，不通过相邻源码或依赖目录连接两个仓库。

## 1. 审计结论与证据边界

现有 GeoD 已具备值得研究的瓦片下载、缓存、地图范围选择、任务恢复、影像/高程导出、矢量提取、3D Tiles 获取和 CLI 基础。它们是 Global 的能力与接口参考；只有经过单独版本化、许可检查和接口验证的库，才作为可复用依赖进入新仓库。Global 先在自己的目录建立契约、产品壳与真实链路，不复制国内工作树，也不以国内工程改造作为建仓前置条件。

需要优先解决的结构性缺口是：**桌面、CLI、MCP 尚未使用同一个持久化 Job；当前 TileSource 不是通用 Provider；已有 bundle manifest 不是完整 Recipe/Artifact 图；现有六个模式也不等于总纲定义的六大数据域。** Satellite 场景检索、科学栅格处理、Local Data 工作区、完整成果库、配方、云协作与支付仍需新建或明显扩展。

本文区分三种证据：

| 等级 | 本文含义 | 不代表什么 |
|---|---|---|
| 源码已核对 | 已读取本地实现、调用入口或测试源码，给出文件与符号位置 | 不自动代表运行成功、安装包包含或已发布 |
| 本次单测通过 | 本次实际执行指定命令并取得退出码 0 | 不代表真实数据端到端、GUI、故障恢复或跨平台通过 |
| 待验证/新增 | 当前审计范围未找到满足总纲的完整实现，或实现需扩展 | 不是断言所有其他仓库都没有相关能力 |

本次没有运行完整 Rust/Tauri 构建、启动桌面、访问真实 Provider、下载卫星数据、重新执行 OSM 离线样例、测试安装包或支付。本文没有将以前的记录改写为本次验收结果。

## 2. 可追溯基线

| 项目 | 本次读取结果 |
|---|---|
| 国内参考代码仓库 | `G:\code\tif-downloader`，只读历史审计；Global 位于独立目录 `G:\code\geod-global` |
| HEAD | `0eeea16a080e2d6f4be01c5bc4c4f17360fc814d` |
| HEAD 日期/标题 | `2026-09-14` / `Document published npm installation command` |
| 工作树 | **非干净状态**；14 个 tracked 文件修改，另有未跟踪文件/目录；本次未提交、重置或修改这些应用文件 |
| Desktop 配置版本 | `src-tauri/tauri.conf.json` 为 `3.6.10`；这只是本地配置，不证明最新公开版本 |
| Core 配置版本 | `crates/geod-core/Cargo.toml` 为 `0.1.1` |
| Workspace | 根 `Cargo.toml` 只列 `geod-core`、`geod-cli`，显式排除 `src-tauri`；根 Cargo 测试不能替代桌面测试 |
| 许可证 | 本地 `LICENSE` 首行为 MIT；本次未更改许可或商业边界 |

代码行号以本次工作树为准，后续修改可能移动。下文使用绝对路径链接与符号名定位，接手者应同时按符号查找。

### 2.1 国内工作树历史记录（不迁移）

已修改内容包括 `Cargo.lock`、Core 的 Cargo/lib/pipeline、OSM 策略与中英文文案、地图缓存层，以及桌面 commands/downloader/lib。未跟踪内容包括：

- `crates/geod-core/examples/`，含 OSM 离线打包样例。
- `packages/geod-mcp/` 与 `scripts/package-geod-mcp.ps1`。
- `src-tauri/src/tile_policy.rs`、`src-tauri/src/tile_payload.rs`。
- `frontend/tests/`、`work-logs/`、CLI/GeoStyle 与 OSM 离线设计文档。

这些记录解释了此前仅凭 HEAD 不能完整描述国内开发状态。**现行方案不从国内 HEAD 创建 Global worktree，也不将这些未提交内容搬入新仓库。** 它们继续由国内项目维护；本轮不为独立建仓而整理、提交、重置或清理国内变更。未来若有具体共享库抽取任务，再单独选定已提交来源版本、许可、公开接口与发布产物，并记录来源；不要把历史清单误读为迁移指令。

## 3. 当前架构与复用边界

```text
当前桌面：React UI → Tauri commands.rs → desktop task/settings/history + 处理模块
当前 CLI：geod-cli → geod-core::pipeline → #[path] 引入部分 src-tauri 处理模块
当前 MCP：Node service → 启动 CLI/渲染子进程 → 自己保存 job.json 与状态

Global 目标：独立 Desktop / CLI / MCP
                  ↓ 同一 Global 契约、策略与命令服务
      Provider registry + 持久化 Job engine + Artifact/Recipe store
                  ↓
      本仓处理器，或通过明确版本引入的共享库

国内仓库独立维护，不作为 Global 构建目录或运行时输入
```

### 3.1 具体证据目录

| 模块 | 本地源码与符号 | 已存在的基础 | 海外规格仍缺什么 |
|---|---|---|---|
| Core 抽离 | [lib.rs](G:/code/tif-downloader/crates/geod-core/src/lib.rs:5) 的 `#[path]` 模块 | 不导入 Tauri 的共享下载、瓦片、缓存、exporter 等代码 | 源码仍归属 desktop 目录；task/settings/history、DEM/3D/streaming 还没有统一纳入同一 Core 服务 |
| 现有输入与成果 | [pipeline.rs](G:/code/tif-downloader/crates/geod-core/src/pipeline.rs:17) 的 `Request`；同文件 `Asset`、`Manifest`（82、116 行） | schemaVersion、bounds、影像/矢量请求、资源限额、hash、CRS、来源与 quality | Area/Item/Layer/Recipe/Job 关联、处理器版本、许可快照、认证引用、科学单位、完整迁移规则 |
| 桌面任务 | [task.rs](G:/code/tif-downloader/src-tauri/src/task.rs:15) 的 `TaskStatus`、[PersistedTask](G:/code/tif-downloader/src-tauri/src/task.rs:569) | 等待决策、带缺块完成、暂停、原子保存请求与可恢复工作目录 | 与 CLI/MCP 一致的状态/事件、通用 DAG、可重放输入、各处理阶段恢复协议 |
| 桌面恢复/容量 | [commands.rs](G:/code/tif-downloader/src-tauri/src/commands.rs:684) 的 `ensure_download_disk_space`、[resume_task](G:/code/tif-downloader/src-tauri/src/commands.rs:2200) | 下载前容量检查、瓦片任务续跑、部分导出入口 | 大资产 HTTP Range、ETag 变化、处理阶段断点、所有数据域的磁盘满/杀进程证据 |
| 图源 | [config.rs](G:/code/tif-downloader/src-tauri/src/config.rs:45) 的 `TileSource`、[source_analyzer.rs](G:/code/tif-downloader/src-tauri/src/source_analyzer.rs:33) 的 `analyze` | 瓦片模板、样例探测、URL 坐标参数解析 | collection/query/items/assets/认证/许可独立建模；解析 WMTS URL 不等于支持任意 TileMatrixSet |
| OSM 离线策略 | [tile_policy.rs](G:/code/tif-downloader/src-tauri/src/tile_policy.rs:17) 的 `ensure_offline_allowed`；[pipeline.rs](G:/code/tif-downloader/crates/geod-core/src/pipeline.rs:213) 的调用 | 公共 Standard 瓦片主机和重定向禁用离线下载；前后端均有策略代码 | 这是针对特定来源的后端规则，尚非全来源四态许可系统；不能推断其他任意 URL 自动获准下载 |
| 凭据 | [assistant.rs](G:/code/tif-downloader/src-tauri/src/assistant.rs:125) 的 `keyring_entry`；[settings.rs](G:/code/tif-downloader/src-tauri/src/settings.rs:46) 的 `AppSettings` | Assistant API key 使用系统凭据库；图源 token 字段仍存在于序列化 settings 模型 | 将 Provider 凭据统一迁移至安全存储，契约仅保留 credentialRef，旧配置迁移与导出脱敏 |
| RGB 影像导出 | [exporter.rs](G:/code/tif-downloader/src-tauri/src/exporter.rs:90) 的 `write_geotiff_tags`、`export_tiff_bytes`（123 行） | RGB/RGBA PNG/JPEG/TIFF，GeoTIFF 3857 标签和文件导出路径 | 通用多波段、数据类型/scale/offset、mask、目标 CRS/网格、COG 与独立数值验收 |
| 裁剪 | [clip.rs](G:/code/tif-downloader/crates/geod-core/src/clip.rs:248) 的 `clip_raster`；[裁剪集成测试](G:/code/tif-downloader/crates/geod-core/tests/clipping.rs:182) | CLI 栅格 Polygon union、holes、alpha 裁剪；JPEG 对透明裁剪拒绝 | AOI 跨日期变更线、通用科学栅格 mask、矢量精确几何裁切不是同一能力 |
| DEM | [terrarium.rs](G:/code/tif-downloader/src-tauri/src/dem/terrarium.rs:11) 的 `decode_pixel`；[streaming_tiff.rs](G:/code/tif-downloader/src-tauri/src/streaming_tiff.rs:493) 的 `merge_and_export_dem_streaming` | Terrarium 解码、Float32 高程、NoData、流式写出 | 垂直基准/单位、坡度/等高线、更多原始 DEM Provider、重投影与地形派生验证 |
| 瓦片包 | [tile_pack.rs](G:/code/tif-downloader/src-tauri/src/tile_pack.rs:275) 的 `append_zoom_to_mbtiles`、`append_zoom_to_gpkg`（522 行） | MBTiles XYZ→TMS 行转换、GeoPackage、原始瓦片目录 | PMTiles 写出、任意投影/512px matrix、自包含样式字体、设备/第三方客户端验收 |
| OSM 离线样例 | [osm_offline_pack.rs](G:/code/tif-downloader/crates/geod-core/examples/osm_offline_pack.rs:1) | 读取离线 renderer 样例 PNG 并调用生产打包 writer；明确 P1 harness | 不是已整合的桌面用户流程；UI、renderer sidecar 分发、干净安装、OruxMaps 真机仍须验收 |
| 3D | [filter.rs](G:/code/tif-downloader/src-tauri/src/tiles3d/filter.rs:363) 的 `filter_tileset_with_parent_transform`；[fetcher.rs](G:/code/tif-downloader/src-tauri/src/tiles3d/fetcher.rs:231) 的 `download` | 带 transform 的范围筛选、嵌套 tileset 获取与 URI 重写；存在失败计数和失败返回 | 全部外部纹理/glTF 依赖闭包、implicit tiling/subtree、扩展兼容、独立完整性校验尚无本次验收 |
| 前端结构 | [App.tsx](G:/code/tif-downloader/frontend/src/App.tsx:49) 的 `MODES`；[map-canvas.tsx](G:/code/tif-downloader/frontend/src/features/map/map-canvas.tsx:474) 的 `MapCanvas` | Imagery、DEM、Wayback、3D、MVT、OSM 六种模式；Leaflet、MapLibre 图层与 Cesium | Explore/Workspace/My Data/Recipes 的任务导向 IA；六个旧模式并非 Satellite/Imagery/Elevation/Vector/3D/Local Data 六域 |
| AOI/Viewer | [selection-store.ts](G:/code/tif-downloader/frontend/src/store/selection-store.ts:22) 的 `SelectionState`；[cesium-canvas.tsx](G:/code/tif-downloader/frontend/src/features/map/cesium-canvas.tsx:40) 的 `CesiumCanvas` | 共享选区状态、导入区域、Cesium 懒加载与选区同步 | Area 持久化对象、多 Viewer 契约、多栅格预览与比较；OpenLayers 是候选引擎，尚非当前桌面实现 |
| 历史库 | [history.rs](G:/code/tif-downloader/src-tauri/src/history.rs:148) 的 `HistoryStore` | SQLite 下载记录、分页、旧 JSON 迁移、日志关联 | 面向 Artifact 的检索/地图查看、重定位、recipe/source 追溯、文件记录与实物生命周期分离 |
| CLI | [main.rs](G:/code/tif-downloader/crates/geod-cli/src/main.rs:59) 的 `run` | plan/fetch/inspect/geostyle-import；JSON stdout、进度 stderr、取消退出码；fetch 同步 | 通用 recipe 命令、持久化 job status/cancel/resume、查询 Provider、所有处理器覆盖 |
| MCP | [service.mjs](G:/code/tif-downloader/packages/geod-mcp/src/service.mjs:17) 的 `GeoDService`；[schemas.mjs](G:/code/tif-downloader/packages/geod-mcp/src/schemas.mjs:15) | 严格请求、plan 前置、路径限制、任务 ID、并发限制、成果 hash、响应限额 | 当前是 CLI 外层自己的 job 管理；capabilities 明确 `downloadsResumeAfterRestart: false`，尚非统一 Job 服务 |
| 产品隔离 | [tauri.conf.json](G:/code/tif-downloader/src-tauri/tauri.conf.json:3)；[selection-store.ts](G:/code/tif-downloader/frontend/src/store/selection-store.ts:40) | 单个 productName/app identifier、`geo-downloader` 存储命名 | CN/Global 的 app ID、存储根、URL scheme、更新渠道与品牌配置独立化 |

### 3.2 四个不能提前宣布完成的能力

1. **通用科学 GeoTIFF/COG**：现有非 GDAL TIFF 主路径有实际实现；[exporter.rs:362](G:/code/tif-downloader/src-tauri/src/exporter.rs:362) 的可选 GDAL `export_geotiff_bytes` 仍为 `unimplemented!`。不能因此说所有 GeoTIFF 都未实现，也不能因为存在 GDAL feature 就说任意多波段、重投影、COG 已支持。
2. **成果验证**：[pipeline.rs:615](G:/code/tif-downloader/crates/geod-core/src/pipeline.rs:615) 的 `inspect` 校验版本、路径、重复 ID、大小与 SHA-256，尚未逐个打开格式验证 CRS、像素类型、NoData、COG 布局、空间范围、几何或 3D 引用。hash 正确不等于科学数据正确。
3. **统一恢复**：桌面 `PersistedTask`、Core 同步 `fetch` 和 Node MCP `job.json` 是三条不同路径。Core [fetch](G:/code/tif-downloader/crates/geod-core/src/pipeline.rs:321) 使用 staging、目标目录不覆盖、最后写 manifest 的提交标记，但这不等于重启续跑；提交中断后也仍需有显式恢复/清理协议。
4. **Provider/Recipe/云**：本次针对 `crates`、`src-tauri/src`、`frontend/src`、`packages/geod-mcp/src` 搜索并读相关入口，未找到满足总纲的 STAC/Sentinel 场景适配、通用 Provider trait/capabilities、完整 Recipe 对象、云账号/组织/支付账本实现。它们按新增工作登记，不用页面名或第三方依赖代替交付。

## 4. 工作包到实现的映射

本表中的国内文件只用于定位参考能力。“复用”是待评估的版本化库接入；“改造”是 Global 适配器或未来单独维护的共享库接口工作；“新增”是在 Global 建立产品能力。它们均不授权直接修改国内版。旧文件兼容仅针对用户选择导入的版本化文件，不自动访问国内数据库或任务目录。同一工作包可同时包含三类工作。

| WP | 可复用 | 需改造 | 新增与验收落点 |
|---|---|---|---|
| A01 共享契约 | Request/Manifest、DownloadRequest、SelectionState | 旧数据通过 adapter 映射，保留 schemaVersion 1.0 reader | Area/Dataset/Item/Layer/Recipe/Job/Artifact schema、版本迁移、统一错误与状态 |
| A02 Global 独立仓库与壳 | 本仓原型、React/Tauri技术经验 | 独立依赖/锁文件/配置/CI；为Global定义存储/命名/更新 | 新 IA、独立 app ID/数据目录、干净检出构建、并存/升级/回滚验收 |
| A03 凭据许可 | Assistant keyring、OSM 后端规则、MCP 路径限制 | 将 URL 凭据/设置字段转引用，统一许可判定 | 每来源能力与许可四态、导入预检、secret 扫描、分享脱敏 |
| P01 Provider SDK | TileSource、source_analyzer、下载 HTTP 客户端 | 瓦片型图源包装为 adapter；诊断与错误统一 | discover/query/preview/assets/auth/policy 契约；两类 Provider 契约测试 |
| P02 Sentinel/STAC | 通用 HTTP、AOI 与限额基础 | 把搜索结果映射到 Item/Asset，禁止把 RGB 瓦片当科学波段 | collection、分页、时间/云量/footprint、资产路径验证、真实小 AOI |
| P03 多路径公开数据 | 下载重试/缓存 | 每平台认证、费用、资产链接策略单独配置 | CDSE/USGS 等逐路径联调；不要按“STAC通过”整批勾选 |
| P04 地图服务 | XYZ/TMS URL、缓存、MBTiles writer | tile size、zoom offset、CRS、matrix 不再固定推断 | WMTS capabilities、WMS/ArcGIS 真样例，512px/多投影验证 |
| P05 自定义/商业来源 | 自定义模板、代理与探测 | Provider 权限、credentialRef、允许域名 | 认证能力/许可证据/错误诊断；未授权离线请求后端拒绝 |
| V01 二维 Viewer | Leaflet 范围/绘制、MapLibre 预览 | 统一 extent/AOI/layer adapter、生命周期 | OL 对照样例与性能测量后再决定替换；Local Data、比较视图 |
| V02 栅格预览 | RGB 图层和导出预览 | 预览样式与数值数据独立 | 多波段、拉伸、指数、NoData、图例、像素值与对照验证 |
| V03 三维 Viewer | Cesium 加载/预览/选区同步 | adapter、相机/范围、资源释放与 UI 状态 | 授权引用与真实 tileset 样例验收 |
| X01 栅格处理 | 拼接、RGBA mask、流式 RGB/DEM TIFF | 将处理器脱离 Tauri、固定输入与网格元数据 | 通用波段/掩膜/重投影/COG/时序、独立科学数值验收 |
| X02 高程/矢量 | Terrarium、GeoJSON/Overpass、MVT/瓦片包 | 坐标/单位/拓扑与原始要素语义明确 | 地形派生、精确 vector clip、更多格式、PMTiles 与离线自包含制图 |
| X03 3D 打包 | 范围相交、transform、嵌套引用重写 | 提取为 Core 处理器并输出可验证 Artifact | 外部资源闭包、implicit tiling、扩展支持表与缺失引用测试 |
| J01 统一 Job | TaskStatus/TaskManager、请求快照与进度 | 保留 legacy 状态映射，CLI/MCP 改调用同一服务 | 统一状态/事件、阶段、组合任务依赖与 Partial 聚合规则 |
| J02 恢复/磁盘 | atomic_write、work_dir、容量检查、缓存 | 跨 UI/CLI/MCP 持久化与清理归属一致 | 大资产断点/ETag、阶段重启、故障注入、不覆盖/误删验证 |
| F01 Artifact | manifest assets/hash/provenance/quality | 从下载记录迁入可追溯成果集合 | 完整 source/recipe/processor/permission snapshot，结果关联 |
| F02 结果验证 | inspect、现有 TIFF/MBTiles 测试 | `inspect` 分离通用包校验与各格式验证 | 先定义 Validator 接口，随 X01–03 增加验证器；破损样例能失败 |
| F03 My Data | SQLite HistoryStore、打开文件夹 | 下载历史与成果生命周期分离 | 地图查看/搜索、移动重定位、文件丢失、只删记录/删文件分别确认 |
| W01 Recipes | CLI request schema 与 plan 的限额/来源检查 | 旧 request adapter，固定 assets 不依赖再次随机选景 | 版本化图、导入预检、去敏分享、批量区域、重跑/来源变化解释 |
| W02 CLI | plan/fetch/inspect、机器输出/退出码 | 与共享 Job/Recipe 对齐、保留旧命令兼容期 | 统一任务命令、provider/search/recipe 能力、无 UI E2E |
| W03 MCP/AI | Zod schema、进程调用、路径/响应限额、artifact URI的设计经验 | Global MCP调用同一Global Job；国内MCP保持独立维护 | 预算/许可/审批不能绕过，真实成果从同一 Artifact 提取 |
| C01–03 云与 Teams/Runner | 可复用去敏契约，不复用桌面信任边界 | 本地与云引用分离，Runner 只消费批准契约 | 账号、同步冲突、RBAC、审计、绑定/心跳/预算、离线等待 |
| PAY01–05/B01–02/O01–02 | 产品总纲、商业假设与本地免费能力基础 | 商业方向仍为待确认方案；主体/商品规则逐项核对 | 准入、订单、账本、回调/权益、取消/退款/到账、服务交付与支持流程 |
| G01–08 | 已有官网/分发 workflow、telemetry consent 等可作参考 | 国际文案、事件字典、下载归因与承诺按真实能力重写 | Global 官网/教程/演示/分享/渠道/支持；发布前逐项实测 |
| D01–04/R01–02 | 组件、双语键、当前用户旅程可作参考 | 从模式面板改为 AOI/任务导向；研究结果不可捏造 | 全页面状态原型、视觉/可访问性、访谈、替代工具任务基准 |
| Q01–03 | 当前测试、release.yml 与各平台包配置 | 分离 Core、桌面、CLI/MCP、Global 的 CI 门禁 | 固定许可样例、独立格式工具、OS 签名/公证、真实安装/更新/卸载 |

## 5. 无循环的依赖顺序

02 中“先结果验证、再所有处理器”的粗粒度图应按以下顺序理解并细化，避免 F02 与 X01–03 相互等待：

```text
D01 / 全页面设计 + A01 契约 + A03 策略
                 ├─ P01 Provider 接口 → P02/P03/P04/P05 adapters
                 ├─ J01 状态/事件 → J02 恢复与存储
                 ├─ F01 Artifact + F02-a Validator 接口/基础包检查
                 └─ V01/V03 viewer adapters + A02 Global shell

Pxx + J01 + F01 + F02-a → X01/X02/X03 各处理器
每个处理器交付时同步交付自己的 F02-b 格式/数值验证器
                 ↓
F03 My Data + W01 Recipes → W02 CLI / W03 MCP 对齐
                 ↓
完整桌面旅程、恢复/离线、跨平台与正确性验收

A01 / W01 去敏契约 → C01 同步 → C02 组织 → C03 Runner
PAY01 准入 → PAY02/03/04；服务收入另按 PAY05
G01–08 / O01–02 与研发并行，外部发布按各自门禁
```

F02-a/F02-b 是同一 WP-F02 的内部交付切分，不新增相互冲突的公开工作包编号。测试样例和期望值从接口阶段准备，避免处理器全部做完再补“正确性”。

## 6. 完整范围的实施批次

以下按依赖划分交付，不承诺未经估算的工期。每批结束形成可检查的页面、命令、产物或研究记录；终态功能没有从范围中移除。

| 批次 | 并行工作 | 退出证据 |
|---|---|---|
| 0 独立基线与全局设计 | Global建仓与依赖隔离、A01 schema/显式导入adapter、完整 IA/全状态原型、商业决策登记、固定数据样例计划 | 可独立安装与构建；对象/状态/错误样例可校验；每个目标页面有状态清单 |
| 1 Global底座 | A03/P01/J01/F01/F02-a；A02 独立配置与 Global 导航壳；V01/V03 adapter | 相同输入在Global桌面/CLI产生相同计划；禁止源均拒绝；两个产品并存不串库 |
| 2 全数据域接入 | P02–05；Satellite 搜索/资产、地图/Wayback、Elevation、Vector、3D、Local Data 页面与处理连接 | 每个声明支持的 Provider 有真实小样例/空结果/认证错误；六域入口都有真实可用行为 |
| 3 处理与交付 | V02、X01–03 + 对应 F02-b、J02、F03；同 AOI 组合任务与时间序列 | 独立工具读成果；完整/Partial/失败正确；断网/磁盘满/杀进程可解释恢复 |
| 4 可重复工作 | W01–03、批量/比较/分享、离线使用、E2E-01–06；全部设计状态补齐 | 桌面保存 → plan → CLI 执行 → MCP 查询同一 Job/Artifact；去敏 recipe 重跑有产物 |
| 5 托管与运营 | C01–03、B/PAY/O 全链路，G01–08 内容分发；可从批次 0 开始研究/设计与准入准备 | 同步/权限/退出、支付/退款/结算各自取得证据；不得把付费假设写成已验证 |
| 6 发行验收 | Q01–03、全旅程/数据正确性/可访问性、真实安装与升级、官网声明审计 | 明确通过的平台/版本/功能表；未完成能力不在发布页冒称已支持 |

### 6.1 下一轮可直接领取的工程任务

**任务 A：共享契约与兼容适配（A01 + J01/F01 接口部分）**

- 输入：本规格包、本仓 Global 契约样例；国内 `Request`/`Manifest`/`TaskStatus`/`PersistedTask` 的版本化文件结构仅作只读参考。
- 交付：Global schema、合法/非法样例、显式导入旧 request/manifest/task 的转换器与兼容错误；不迁动国内下载器或数据库。
- 必须证明：旧 schema 1.0 仍可读取；未知版本可解释拒绝；`CompletedWithGaps` 不被转换成完整成功；文件/secret 引用不直接进入可分享 recipe。

**任务 B：Provider 和处理器适配界面（P01 + F02-a）**

- 输入：TileSource、source_analyzer、tile_policy、pipeline plan/inspect。
- 交付：一个现有 XYZ adapter 与一个 STAC adapter 的统一契约、能力/许可快照、基础 Artifact validator 接口。
- 必须证明：来源能力不同能表达；认证不足、禁止导出、零结果、缺资产分别返回明确错误；真实数据测试与固定 fixture 测试分开记录。

**任务 C：Global 产品壳与完整导航（A02 + D01–04）**

- 输入：本仓完整页面原型与设计状态表；国内 shell 留在其原仓库。
- 交付：独立依赖、构建与配置、Explore/Workspace/My Data/Recipes/Tasks/Sources/Settings、稳定 AOI 状态与真实 backend adapter 接口。
- 必须证明：样例内容显式标记；未接后端的按钮不能假装成功；CN/Global 不共用同一数据库、缓存键或更新源；在规定窗口尺寸完成可见验收。

任务 A 完成前可以做 B/C 的界面和样例设计；接入实际任务执行后必须消费 A 的同一契约，不再新增第二套前端 request/job 格式。

## 7. 三类发布门禁分别检查

| 发布类型 | 必须满足 | 不构成该类型前置条件 |
|---|---|---|
| 免费桌面/CLI/MCP 发行 | 声明范围的 E2E-01–06、来源许可、成果正确性、恢复、离线、隐私、CN/Global 隔离、安装/升级/签名与文档 | 支付商户审批、组织付费权益、首笔真实结算 |
| 云服务/团队功能上线 | 账号/同步与删除导出、RBAC/审计、备份、Runner 最小权限与预算、E2E-08、服务支持边界；免费试用也要满足这些 | 尚未启用收费时的生产 Checkout 与真实打款；不得因此免除云安全要求 |
| 收费启用 | 已通过主体/商品准入、已可用的被售服务、PAY 测试矩阵、订单/账本/权益/退款、税与收据说明；按条件跟踪首笔真实结算 | 不把交易回跳、沙箱付款、支付 SDK 安装当“已到账” |

三个门禁只解决不同发布对象的依赖，不宣告任何产品已可发布。完整产品设计继续覆盖三者；商业模式/定价选择需要记录负责人决策，不能把本文件或原总纲中的推荐当已批准方案。

## 8. 此前代码审计的实际检查结果与后续验证

以下结果属于原型阶段的国内代码参考审计，不是独立建仓这轮重跑结果，也不是 Global 的测试通过记录。本轮建仓验证另见附件08的工程记录。

| 命令/检查 | 结果 | 证据范围 |
|---|---|---|
| `rtk proxy npm test`，cwd 为 `G:\code\tif-downloader\frontend` | **34/34 通过，退出码 0** | URL/TMS、选择、OSM 下载策略、任务文案、语言键、区域/坐标、KML 等纯逻辑测试 |
| `rtk proxy node --test packages/geod-mcp/test/service.test.mjs packages/geod-mcp/test/server-limit.test.mjs`，cwd 为仓库根 | **6/6 通过，退出码 0** | MCP 作业关闭/槽位释放、base64/JSON 协议响应限额；不包含真实 CLI 下载或远程渲染 |
| git status/rev-parse/log/diff stat 与源码检索 | 已读取 | HEAD 与工作树内容；本次没有新增应用代码差异 |
| Core pipeline/clipping/MBTiles/3D 测试源码 | 已读取部分关键样例，**未执行 Rust 测试** | 如 [pipeline.rs:138](G:/code/tif-downloader/crates/geod-core/tests/pipeline.rs:138) 的 hash/像素/GeoTIFF 标签、244 行损坏检测、[clipping.rs:182](G:/code/tif-downloader/crates/geod-core/tests/clipping.rs:182) 的 holes/PNG/GeoTIFF |

后续开始修改 Global 时，在本仓运行所涉 Core/desktop/package 的定向测试，再执行相应的真实数据与 GUI 验收。版本化共享库按实际引入版本验证，不把国内测试通过当作 Global 验收。只有新增变更或失败风险才扩展测试范围；不要反复运行与本次变更无关的长构建来替代可见功能交付。

验收记录最少包含：**工作树/commit、命令、环境、输入许可与 hash、输出路径与 hash、独立验证结果、截图或运行记录、已知限制**。不能仅提交“代码已生成”或“页面已打开”。
