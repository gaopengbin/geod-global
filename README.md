# GeoD Global

当前收敛为 **Windows 0.1.0-rc.1 发布候选**：先完成已有能力的桌面交付与试用，其他格式和平台扩展放到下一阶段。功能范围、账号限制和交付检查边界见[候选说明](docs/releases/0.1.0-rc.1.md)，安装与使用见[Windows 说明](docs/releases/WINDOWS-README.md)。候选准备不等同于公开发布；此前固定的开发程序仍见[开发版试用](docs/development-preview.md)。

**MODIS NDVI / EVI · Planetary Computer** 已接入 Terra / Aqua 的十二个科学 COG 图层，包括实时检索、工程下载、本地原值查看、持久缩略图和同网格处理。新增四文件联合质量筛选：NDVI / EVI、VI 质量与像元可靠性共同选景，新景不合格时整组回退；二十份真实成果的全部 3,038,664 个 DN、六个实际界面处理和无父级原件的离线恢复通过。既有[指数验收](docs/modis-vegetation-indices.md)及[辅助图层](docs/modis-science-layers.md)保留，新规则见[质量筛选](docs/modis-vegetation-quality.md)；完整 NASA HDF 和其他科学产品继续接入。

公开或自有的三维数据已增加软件内入口：**设置 → 三维数据源**、**我的数据 → 三维资产**。支持有界显式 3D Tiles、glTF / GLB 及其模型、缓冲区和 PNG / JPEG 纹理依赖，原件持久保存，可离线查看和导入导出。真实公开演示的全部依赖与原件已独立核对；区域选取、几何裁剪、隐式瓦片、点云和账号型三维服务仍待接入，见 [三维数据范围与验收](docs/three-d-assets.md)。

公开 WMTS 已增加能力文件 XML 地址和服务声明的 REST 瓦片模板。NASA 实际取图、界面保存、坐标导出、重启后离线读取与旧记录兼容检查通过，见 [WMTS REST 接入](docs/wmts-rest-map-images.md)。

公开 XYZ / TMS 模板已接到地图影像入口，提供明确网格配置、区域保存、原瓦片归档、离线地图和带坐标导出。NASA 真实 XYZ 的 17 块瓦片、334,154 个输出像素及真实重启已独立核对；TMS 行号与 512 像素采用合成网格验收，见 [XYZ / TMS 说明](docs/xyz-tms-map-images.md)。


公开 ArcGIS REST MapServer / ImageServer 已接入统一地图影像入口，支持独立图层 / 服务默认渲染、区域保存、实际返回范围配准、持久来源、离线查看和带坐标导出。两个真实公共服务和一次界面保存的全部 378,880 像素已验收，详见 [ArcGIS 地图影像](docs/arcgis-map-images.md)。

独立海外桌面产品仓库。产品目标是围绕同一区域发现、预览、比较、获取、处理和导出空间数据，并保留来源及可重复工作流。

原文件下载已移除正常传输的 30 分钟总时长限制，保留连接和 45 秒无数据超时；已审核公开原文件支持经前缀校验及强 ETag 确认的续传，受保护 NASA / Copernicus、自定义 STAC 和 WCS 响应仍从头重试。规则与开发版验证见[下载恢复说明](docs/source-transfer.md)。

“我的数据 → 地图影像”已接入公开 WMS 和 WMTS KVP / REST 服务目录、图层 / 日期选择、按范围保存、地理显示和影像 / 坐标 / 来源 ZIP 导出。WMTS 支持 WGS84 / Web Mercator、PNG / JPEG 瓦片，按服务声明网格组装并保留原瓦片归档；真实 NASA GIBS 的 KVP 与 REST 输出均由独立工具核对。它们是渲染影像，不替代原始科学产品；具体支持范围和剩余工作见[WMS 说明](docs/wms-map-images.md)与[WMTS 说明](docs/wmts-map-images.md)。

“我的数据 → 矢量文件”支持本地 WGS84 GeoJSON / 完整几何 Overpass JSON 的引用、明确复制、地图属性和 GeoJSON 导出；真实 OSM 文件已逐要素独立核对，见[本地矢量](docs/vector-local-data.md)。现另支持[本地 OSM XML / PBF](docs/osm-local-data.md)完整快照的原文件保存、标签 / 元数据、对象筛选与离线恢复；公开原包拒绝和明确生成的完整地理子集分别验收。同页另支持 [GeoPackage 多图层与原文件](docs/geopackage-local.md)和 [Shapefile 文件组](docs/shapefile-local.md)：保留原件、属性与 Z / M，记录水平坐标方法，分别导出转换数据和原件；SHP 支持中文编码、删除 / 空记录与精确数值字符串。“获取矢量数据”提供用户配置的 OSM Overpass 入口，可小范围提取建筑、道路、水体、土地利用和兴趣点，保存原始响应与完整来源，见[OSM 在线提取与边界](docs/osm-overpass.md)。

“获取矢量数据”已接入公开 HTTPS OGC API Features 服务连接、集合发现与区域分页提取。结果保存为带查询来源的受管 GeoJSON，可离线查看属性和导出；真实湖泊数据 13 页 / 25 个要素及重启恢复已验收，范围和限制见[OGC 矢量服务](docs/ogc-features.md)。

同一入口现支持公开 **ArcGIS Feature Service**：图层发现、按当前区域分批获取完整二维要素、字段摘要与来源记录、地图属性及离线导出。实际点 / 线 / 面共 17 个要素的全部属性与几何已核对；受保护服务、附件及原始地理数据库不在当前适配范围，见[ArcGIS 验收](docs/arcgis-features.md)。

另已接入公开 **WFS 2.0**：要素类型发现、GML 3.2 / GeoJSON、固定参数分页与完整双遍复核、原始响应持久归档、属性地图和离线 GeoJSON 导出。支持范围、格式转换及原始文件区别见 [WFS 说明](docs/wfs-features.md)。

探索页与设置中新增 **自定义栅格来源**：公开 STAC API 搜索、单个 STAC Item 或 GeoTIFF / COG URL，可选择原文件、加入工程并进入现有下载任务；本地提供灰度地图、文件坐标与原始像元检查、持久化缩略图。真实分类栅格与 Float32 高程文件已完成下载和独立抽检；通用科学处理、认证源、跨主机元数据及 MCP 自定义源工具尚未接入，见[支持范围与验收](docs/custom-stac.md)。

探索页与设置另有 **WCS 区域栅格** 入口：连接公开覆盖服务、检查定义、按原生网格准备区域请求，保存工程并交由现有任务下载。软件保留服务端生成的 GeoTIFF 及原始元数据，支持本地地图、原始取值和持久化缩略图。CLI 与 MCP 共用这些原生能力，真实文件的 MCP 读取、工程复用和重连已验收；二维网格支持范围与验收边界见 [WCS 说明](docs/wcs-coverages.md)。

**当前状态：Windows 发布候选准备中，完整规划继续保留。** 历史整改与验收记录见[产品质量整改记录](docs/releases/2026-09-30-product-quality-review.md)。独立工程已接入中英文界面、Earth Search / Planetary Computer / Copernicus / Landsat / NASA HLS / Copernicus DEM 实时目录、Rust 文件下载、多景工程与双类型批量下载、同网格真彩色和 SCL 拼接裁剪、真实二维栅格工作区、RGB / SCL / Landsat / HLS / DEM / NAIP 原始像元检查、SCL 单景裁剪、可执行配方、成果交付包、CLI 和本地 MCP。各来源能力与真实文件验收见[数据源说明](docs/providers.md)。Tauri 桌面壳与浏览器调试入口复用同一执行核心。完整产品规格仍在逐步实施。

已实现通过离线参考地图绘制WGS84矩形选区，或输入精确坐标，再按范围、UTC日期与云量检索 Sentinel-2；Explore 地图按地理坐标流式显示原始10米真彩色 COG，可平移缩放并对同网格场景做卷帘比较。探索页下载时自动创建可命名的本地工程，支持选择原始 SCL、真彩色 GeoTIFF 或两者，取消、同一任务重试、持久化任务及查看成果来源与 SHA-256。探索页可切换十五个数据源入口，卫星光学数据打开时默认检索最近30天，NAIP 航空影像使用较宽的历史日期范围；Earth Search 和 Planetary Computer Sentinel-2 支持原有真彩色 / SCL 流程，Landsat 8/9 Collection 2 L2 支持红、绿、蓝三个 30 米 UInt16 原始反射率波段的工程下载、参数持久化、[在线 RGB 地图](docs/landsat-map.md)、本地灰度显示和 DN / 反射率检查，并支持按工程区域裁剪和同网格拼接，保留原始 DN、NoData 和转换参数，见[独立验收](docs/reflectance-processing.md)。Copernicus 与 NASA HLS L30 v2.0 已接入原生授权及原文件下载适配，成功授权下载仍需真实账号验收；HLS Int16 本地波段读取已用夹具验证，SAFE ZIP 的 TCI / SCL JP2 准备、显示和处理已用合成文件验证，完整生产 SAFE 待验收。MODIS Terra / Aqua MOD09A1 / MYD09A1 v061 已接入公开 8 天合成目录和三波段 COG 工程下载、本地正弦投影显示、DN / 反射率与持久化缩略图，并用实际 COG 核对全部 RGB 预览像元，见[MODIS 验收](docs/modis-inspection.md)；PC 转换 COG 不标为 NASA 原始 HDF，MODIS 单波段正弦投影工程拼接、矩形及带孔洞多边形裁剪已用两份实际 COG 核对 2,193,265 个输出 DN，成果地图、缓存与重启恢复已验收，见[MODIS 处理](docs/modis-processing.md)；MODIS 原始与匹配的工程三波段已支持[科学 RGB 导出](docs/scientific-rgb.md)，QC / State 两类质量 COG 的工程下载、原始无符号位解码、本地地图及离线持久化缩略图已用实际文件验收，见[MODIS 质量层](docs/modis-quality.md)；五类文件的单景 / 多景 / 带孔多边形工程处理已用全部 15 份实际原件和 15 份成果独立验收，见[五类文件处理](docs/modis-quality-processing.md)。MODIS 科学 RGB 已支持[同景质量筛选](docs/modis-rgb-quality-mask.md)和[多景联合质量选景](docs/modis-coupled-rgb.md)，真实文件、界面、MCP 与离线恢复已分别验收；其他产品质量规则与 NASA 原始 HDF 仍待接入。VIIRS NOAA-21 / NOAA-20 / Suomi-NPP 的 09A1 v002 实时目录、公开预览、完整合成时段、工程恢复及 Earthdata 原始 HDF5 下载适配已接入；M5/M4/M3 科学层准备、原值读取、本地 RGB、缩略图与工程区域处理已用合成文件独立验证，生产 HDF5、成功授权与专项多景处理仍待验收，见[VIIRS 处理](docs/viirs-processing.md)。完整来源规划、待验收和待接入项见[接入状态](docs/provider-integration-status.md)。选区用于目录检索，Copernicus DEM GLO-30 Public 和独立 GLO-90 入口已接入匿名高程瓦片下载、原始 Float32 显示 / 取值与持久化缩略图，EPSG:4326 坐标标为度、高程标为米；不提供云量或观测日期筛选；已支持同网格工程裁剪和拼接，逐位保留 Float32 原值，见[高程验收](docs/elevation-inspection.md)和[GLO-90 实际文件与处理验收](docs/glo90-inspection.md)。NAIP · Planetary Computer 已接入美国航空影像实时目录、在线 RGB 地图与四通道 COG 原文件下载；保留 NAD83 网格和近红外通道，本地 RGB 显示、原始 RGB / NIR 取值、工程恢复和持久化缩略图已用实际 0.6 米文件验收，见[NAIP 验收](docs/naip-inspection.md)。NAIP 不提供云量筛选；已支持四通道工程裁剪 / 同网格拼接，用独立 TIFF 内部掩膜记录未覆盖区域，保留有效的黑色像元与 NIR=0。源资产下载仍为整景。工程页集中显示该工程的下载、文件与处理结果，支持改名，并分别按同一 UTM 像元网格拼接与裁剪到工程区域；见[下载与工程操作](docs/workflows/scene-project-download.md)和[多景工程与验收边界](GeoD-Global-Spec/14-Multi-Scene-Projects.md)。重投影、通用多波段处理和科学计算尚未实现；字节续传的适用范围与验收边界见[下载恢复说明](docs/source-transfer.md)。

Landsat 8/9 C2 L2 另可下载 QA_PIXEL / QA_RADSAT 原始 UInt16 质量文件，支持质量地图、位字段解读、完整分辨率统计、持久化缩略图及只读 MCP，见[原始质量文件验收](docs/landsat-quality.md)。已支持单景裁剪、同网格多景拼接和带孔洞多边形裁剪；六份实际 Landsat 8/9 原件、八份成果的全部 64,025,460 个原值及覆盖位、三组界面和无父级原件的离线缓存恢复通过，见[质量工程处理](docs/landsat-quality-processing.md)。新增可选[同景科学 RGB 质量筛选](docs/landsat-rgb-quality-mask.md)，基于真实 Landsat 9 原件的十二份整景 / 矩形 / 带孔区域成果，其全部 878,416,812 个 DN、预览、界面、MCP 和无父级文件的离线缓存通过。进一步完成[Landsat 8 整景与 Landsat 8/9 多景联合选景](docs/landsat-coupled-rgb.md)，十六份实际成果的全部 871,015,284 个 DN、三组界面、两种 MCP 和十六份无父级文件的持久缓存 / ZIP 通过；较新景被剔除时，整组三通道回退到合格旧景。其他 QA 层、分析与通用网格处理仍待接入。

NASA SRTMGL1 v003 已新增实时目录、公开浏览预览和软件内 Earthdata 授权入口，原生 HGT ZIP 下载适配、本地有符号 Int16 高程读取、EGM96 / Point 网格、灰度地图与持久化缩略图已接入；工程裁剪和拼接输出保留共享边缘采样的 Int16 GeoTIFF 1.1，合成输入的实际运行时结果已由独立 GDAL 逐值验证。真实账号原包仍待验收，见 [SRTM 验收边界](docs/srtm-inspection.md)和[处理证据](docs/srtm-processing.md)。

在 **Explore / 探索** 点击区域名称，或展开 **筛选** 后点击 **在地图上选区**。离线参考地图显示全球国界；搜索索引包含 Natural Earth 1:1000 万数据中的 4596 个一级行政区，并按国家加载边界。可用英文、中文或数据中的本地名称搜索、点击边界定位，再明确选用行政区多边形、外接矩形，或在地图上拖绘更小矩形。点击“搜索此区域”会将西、南、东、北四个WGS84坐标写入检索条件并立即查询在线目录；非法范围会阻止提交。目录检索仍使用矩形；选中的多边形可供已下载 SCL 的本地遮罩裁剪。范围与例外见[全球行政区说明](GeoD-Global-Spec/13-Global-Administrative-Boundaries.md)。

在 **Settings / 设置 → Language / 语言** 切换 English / 简体中文，选择会保留。**My Data / 我的数据 → 文件详情与来源 → 检查栅格** 可读取已完成的 RGB / SCL / Landsat / HLS / MODIS 波段，重新校验 SHA-256，并显示原始空间元数据和真实文件预览；SCL 另外显示分类统计，反射率波段显示采样灰度拉伸参数，工作空间读取原始 DN 及未截断反射率。当前读取支持 MODIS 固定正弦投影、WGS84 UTM 北 / 南网格、NAIP 的 NAD83 UTM 及 Copernicus DEM 的经纬度网格，应用无需系统 GDAL 或 Python；详见[原始波段检查与独立核对](docs/reflectance-inspection.md)。SCL 的原始验收见[多语言与栅格检查](GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md)，本轮 RGB 独立核对见[产品质量整改记录](docs/releases/2026-09-30-product-quality-review.md)。

**我的数据 → 工程** 可按工程保存的区域裁剪和拼接。全部文件列表的 **裁剪栅格** 支持对本地 SCL 使用工作区经纬度范围或手工源坐标，在真实预检后保存处理计划或执行。输出保留源像元、坐标系和分辨率，并附带独立 JSON 来源清单。高级的处理计划保存在“我的数据 → 已保存裁剪计划”，支持导入、审阅及重跑；同一流程也可通过 CLI 执行。见[处理与配方验收](GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md)和[英文操作教程](docs/workflows/clip-sentinel-scl.md)。

**Workspace / 工作区** 可加载最多 4 个同坐标系的本地 RGB / SCL / 反射率 / 原始质量 / NAIP / DEM 文件，按原始网格叠加预览，支持显隐、透明度、定位、卸载和收起图层面板。同一景 Landsat / HLS 或同一时段 MODIS 三个源波段下载完成后，文件图层图标可直接打开[本地 RGB 组合](docs/local-rgb.md)，重新校验三份原文件并读取逐通道 DN / 反射率，不依赖在线合成；原始或匹配的工程波段可显式生成[科学 RGB GeoTIFF](docs/scientific-rgb.md)，保留 16 位原值、标定、NoData 和源网格，支持独立读取、缩略图及交付包。其他文件页“在工作空间打开”会直接加载指定文件，工作空间也提供返回所属工程的入口。显示概览最大边长 768 像素；点击地图读取经过校验的原始分辨率像元，RGB 返回 R / G / B 三个通道，NAIP 另外返回第四通道原始 NIR 数值。SCL 支持绘制矩形或输入源坐标并进入裁剪预检；真彩色裁剪从所属工程执行。地图不请求在线底图，不对像元重投影；当前图层选择仅保留在打开的页面会话中。

**我的数据** 支持按名称、场景或任务ID搜索，以及下载源文件／派生成果筛选。成功的裁剪成果可准备ZIP交付包，包含GeoTIFF、来源清单、配方、说明和逐文件校验值；仅接受已核验的托管成果，普通裁剪交付TIFF上限32 MiB；科学RGB使用独立的有界流式交付，上限512 MiB。交付包包含配方名称及空间范围，分享前需审阅。**设置 → 本地诊断** 按需生成版本、能力和任务计数报告，不含本机路径、坐标、源URL或用户名称，不自动上传。新增流程与验收边界见[工作区、Agent与交付记录](GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md)。

本地Agent可通过 `geod-runtime serve-mcp` 调用同一任务、像元与配方核心，默认只开放读取和预检；写入需启动参数显式启用。独立目录模式与连接已有loopback服务模式分别管理任务生命周期，具体配置和完成判定见[MCP说明](docs/mcp.md)。

Sentinel-1 IW RTC · Planetary Computer 已接入实时目录、升降轨 / 极化筛选、工程原文件下载、本地 Float32 gamma0 / dB 显示、原始像元、持久化缩略图及工程裁剪 / 对齐网格拼接。两份完整 VV 原文件及三份裁剪、拼接成果已实际验收，全部 6,685,308 个输出值经独立逐位核对，见[雷达读取与边界](docs/radar-inspection.md)和[工程处理验收](docs/radar-processing.md)。供应商已完成的 RTC 不代表软件具备原始 GRD / SLC 处理；滤波、重投影和其他极化原文件验收仍待推进。

## 仓库边界

- 海外版拥有自己的 Git 历史、依赖锁文件、配置、构建和发布流程。
- 国内版是独立项目；本仓库不包含国内版工作树，也不读取它的源码或依赖来启动。
- 以后需要复用核心能力时，使用经过审查、有明确版本的公共包或库；不使用跨仓路径、软链接或本机依赖回退。
- 最终形态为桌面主产品、配套公开网站及可选 Web 协作服务。当前浏览器页面是本地调试与交互验收入口，不代表纯 Web 产品改向。

## 快速启动

**我的数据 → 离线瓦片** 已接入公开 PMTiles v3 MVT 的来源发现、区域与级别提取、持久保存、地图属性查看和标准 PMTiles 导出，也可[打开本地 PMTiles](docs/pmtiles-local.md)及 [MBTiles 矢量 / PNG / JPEG](docs/mbtiles-local.md)，保留完整输入副本。公开文件、原瓦片、界面与移走输入后的离线重启均经独立核对。保存的是完整瓦片，当前最多 512 块 / 128 MiB；大范围任务、样式依赖和精确裁剪仍待接入。

桌面版是主要开发与验收入口。需要 Node.js 22.12+、npm 10+、Rust 1.91.1+ 和 [Tauri 平台依赖](src-tauri/README.md)。在仓库根目录执行：

```sh
npm ci
npm run desktop:dev
```

此命令启动 GeoD Global 桌面窗口并管理 Vite 热更新；不要同时另开 `npm run dev` 占用 4317 端口。桌面版通过 Tauri IPC 直接调用 Rust 核心，不需要 4318 服务。任务与文件保存在独立应用 ID `xyz.laogao.geod.global` 的本地应用数据目录，与浏览器调试目录分开。

浏览器仅用于辅助调试。在未运行桌面开发入口时启动以下两个命令，然后打开 <http://127.0.0.1:4317/>。实时目录无需登录：

```sh
npm run dev
npm run runtime
```

服务只监听 `127.0.0.1:4318`，浏览器来源固定为 `http://127.0.0.1:4317`，任务与文件保存于本仓忽略目录 `.geod-global/`。资产限制为已审核的 Sentinel 真彩色 / SCL、Landsat 8/9 表面反射率和原始 QA 波段，Copernicus DEM GLO-30 Public / GLO-90、NAIP RGB + NIR COG，以及需要原生授权的 HLS L30 波段、SRTMGL1 原包、VIIRS 09A1 v002 HDF5 和 Copernicus SAFE 产品路径。普通栅格每文件最多512 MiB，已审核的 NAIP、Sentinel-1 RTC 和 SAFE 原产品最多4 GiB，SRTM HGT ZIP 最多64 MiB。设置页的“源文件下载代理”可选跟随系统（默认，读取 `HTTPS_PROXY` 等环境变量和 Windows 系统代理）、直连或自定义 HTTP(S)/SOCKS5 代理；选择保存在本地工作空间，新开始的下载任务立即使用。正在传输的任务保持原连接。Planetary Computer 下载端在内存共用只读容器 SAS、检查有效期并合并并发申请，官方目录和签名接口的 429 共用有界等待期，详见[数据源访问与批量验收](docs/providers.md)。目录搜索和地图请求仍遵循浏览器网络设置，连接本机 API 的客户端明确绕过代理。下载无带宽限速，同时最多传输两个文件，始终获取整景源文件。关闭并重新启动服务后，未完成任务标为 interrupted，可手动重试；符合恢复条件的公开原文件继续下载，其余从头开始。

桌面开发与本机调试构建：

```sh
npm run desktop:dev
npm run desktop:build
```

桌面版直接调用Rust核心，不需要单独启动4318服务，数据保存在独立应用ID `xyz.laogao.geod.global` 的本地应用数据目录。`desktop:build` 生成调试版可执行文件，尚不是签名发行版或安装包；平台依赖和命令见 [桌面说明](src-tauri/README.md)。

关闭桌面窗口会收进系统托盘，下载和影像处理继续运行。点击托盘图标可恢复窗口，右键菜单可打开任务页。选择“退出并停止任务”会保存任务状态及有效的公开原文件恢复记录，清理其余未完成部分并退出；再次打开后可明确重试已中断任务。托盘菜单跟随界面语言，设置页提供后台运行说明。

`npm run test:desktop` 校验桌面导航、来源链接和本地文件访问边界；需先生成前端资源。`npm run verify:all` 会先构建前端，再运行处理核心和桌面测试，因此需要上述 Tauri 平台依赖。自动化测试和进程启动检查不能代替原生窗口中的实际操作验收。

Windows便携ZIP与NSIS安装器已从干净提交生成，逐文件校验、实际包CLI裁剪流程和桌面进程启动检查通过；文件位置与SHA-256见[本地评估包验收](docs/releases/2026-09-22-windows-artifact-acceptance.md)。重建、第三方许可收集和校验流程见[Windows打包说明](docs/releases/windows-packaging.md)。这些是未签名的本地评估产物；原生桌面交互、干净机器安装／卸载、签名和公开发布仍需分别完成。

生产资源预览：

```sh
npm run build
npm run preview
```

这几个命令仅需要本仓库，不依赖另一个 GeoD checkout。

## 验证

契约校验另需 Python 3.12（建议使用虚拟环境）：

```sh
python -m pip install -r requirements-dev.txt
npm run verify
```

`verify` 检查四个关键依赖的真实解析路径、七张真实样本的 SHA-256、规格相对链接、拟议契约和当前可执行配方Schema正反例、多语言、目录与下载/栅格/处理客户端、地图几何测试和 Vite 构建。`npm run test:runtime` 运行Rust下载、栅格读取/裁剪、配方持久化、像元、交付包、诊断、MCP与HTTP边界测试；`npm run verify:all` 同时运行两部分。完整工作区与桌面测试可运行 `cargo test --locked --workspace`。契约草案不是当前生产API；本轮运行时接口见 [runtime说明](crates/geod-runtime/README.md)。

GitHub Actions 已配置分支／PR 的 Windows/Linux 检查、手动 Windows ZIP／NSIS 构建和 `v*` 标签预发布；发布会校验版本、干净提交、构建来源及服务器上的产物哈希。使用方式与失败恢复见[提交和发布自动化](docs/releases/automation.md)。配置存在不代表远程 CI 已通过，应以实际运行记录为准。

## 目录

```text
GeoD-Global-Spec/       完整产品规格、代码参考审计、决策及拟议契约
prototype/             桌面共用前端、本地调试入口与独立标记的设计预览
  src/                 React 页面与样式
  public/              真实场景快照、缩略图及字体许可
  qa/                  原型视觉验收截图
scripts/               独立依赖与样本检查
crates/geod-runtime/    持久化下载/处理/配方/交付、CLI、MCP与loopback服务
schemas/               当前可执行配方Schema（区别于规格草案）
examples/              实际下载请求与固定本地输入的配方示例
docs/                  英文操作教程、MCP接入与发行构建说明
src-tauri/             独立桌面壳，直接调用同一Rust核心
.github/workflows/     独立构建检查
```

[规格导航](GeoD-Global-Spec/00-README.md) · [原型使用说明](prototype/README.md) · [仓库隔离决定](GeoD-Global-Spec/08-Repository-Boundary.md)

## 样本、许可和发布状态

探索页通过选定的官方在线目录查询 Sentinel-2 场景；主地图从支持的 Sentinel COG 存储请求原始真彩色数据，因此目录查询与地图显示都需要网络。Planetary Computer 资产读取使用临时签名，工程和任务保存无签名链接。Copernicus 已接入桌面账号入口、原始 SAFE 下载适配及 [JP2 本地准备与工程处理](docs/safe-processing.md)，真实账号下载和完整生产 SAFE 仍待正向验收。`prototype/public/samples/manifest.json` 及新增目录响应保留为可复现的测试与来源校验资料，不作为应用目录加载。Inter 字体许可随包保存，中文使用平台字体回退。下载阶段检查传输大小、文件签名及SHA-256；单独检查本地栅格时读取空间标签和像元，SCL 另外统计类别。这些检查不代表分类精度认证，其他栅格类型尚未实现通用读取。

新产品代码的对外许可和商业包装尚待决定，根包以 `private: true` / `UNLICENSED` 防止被误当作已发布公共软件包。这不改变国内版或第三方资产已有权利。今后引入共享库必须保留其许可通知。

独立远程仓库为 [gaopengbin/geod-global](https://github.com/gaopengbin/geod-global)，创建时为私有仓库。GitHub 源码访问、Actions 产物与 Release 下载均受仓库权限控制；公开发布和更改源码许可是另外的决定。
