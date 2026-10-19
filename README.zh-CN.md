<div align="center">

<img src="docs/images/agent-data-overview.png" alt="GeoD Global 早期亮点示意图：丰富数据源与目标驱动 Agent。图内 85 个入口、16 个分类是历史快照，不代表当前目录规模。" width="100%">

### 丰富的数据源，目标驱动的 Agent。

在同一个 Windows 工作区里浏览丰富的 **二维数据目录**，用 **AI 对话**描述地点、时间和所需成果，让 Agent 准备可确认的下载与处理任务。

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-2563eb?style=flat-square)](src-tauri/README.md) [![开发中](https://img.shields.io/badge/status-in%20development-0f766e?style=flat-square)](docs/development-preview.md) [![协议](https://img.shields.io/badge/license-GPL--3.0-blue?style=flat-square)](LICENSE) [![检查](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml/badge.svg)](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml)

**[快速开始](#快速开始)** · **[数据源](#持续扩充的二维数据目录)** · **[Agent 架构](docs/agent.md)** · **[发行版本](https://github.com/gaopengbin/geod-global/releases)** · **[反馈](https://github.com/gaopengbin/geod-global/issues)**

[English](README.md) · 简体中文

</div>

## 观看 Agent 完整实操

[![观看 GeoD Global 完整实操视频](docs/images/agent-demo-20261009.jpg)](https://geod-global.laogao.xyz/zh/index.html#demo)

**[观看 3 分 10 秒完整演示](https://geod-global.laogao.xyz/zh/index.html#demo)**，中文配音与字幕。真实 Windows 开发版录屏：描述曼哈顿哨兵二号影像需求 → 检查行政区多边形与覆盖 → 确认任务 → 下载原文件 → 裁剪本地 GeoTIFF → 打开并核验这次新生成的成果。等待过程已压缩并标注。Copernicus Sentinel 数据（2026），通过 Earth Search 获取。

视频展示当前开发版，不代表发布了新安装包。[录屏来源](docs/images/README.md#agent-demo-20261009jpg)。

| **丰富的数据源聚合** | **围绕目标推进的 Agent** |
| :--- | :--- |
| **133 个入口、17 个分类**，涵盖卫星影像、地形、专题栅格、矢量、地图服务与本地文件。产品、供应平台、获取条件和接入状态分别展示。 | 自然语言描述需要的结果，**Codex 驱动的 Agent** 解析范围、检索支持的数据、通过选项卡询问决策，提交完整任务供确认，并跟踪本地任务直至文件校验。 |

<sub>亮点总览为早期产品说明图，地图缩略图为装饰示意，图内 85 个入口为历史统计。当前目录包含产品、服务连接和本地格式：**110 项 Ready · 2 项 Auth · 21 项 Planned。** [当前 SinoLC 原件验收](docs/sinolc-shanghai-original-20261014.md)。 [图片来源与生成提示词](docs/images/README.md#agent-data-overviewpng)。</sub>

<img src="docs/images/agent-home-zh-CN.png" alt="2026-10-07 开发版中文首页截图：自然语言任务输入框和按产品分类的紧凑数据源看板" width="100%">

<sub>2026-10-07 早期开发界面，使用空的受控工作区状态拍摄，没有调用模型或开始下载。[截图来源](docs/images/README.md)。</sub>

## Agent 模式：从一句需求，到本地成果

告诉 GeoD 地点、时间和需要的结果。Agent 帮你确定范围、检索支持的数据、预览选中的影像，准备完整任务供审阅。下载和支持的处理流程由本地 Rust 引擎执行，结果保留源身份、处理记录和校验值。

> “找北京市近期的哨兵二号影像，场景云量小于 5%，覆盖完整行政区，并按行政区边界裁剪。”

| 描述与澄清 | 预览与审阅 | 执行与检查 |
| :--- | :--- | :--- |
| 自然语言对话、流式回复、模型连接，需要决策时提供选项卡。 | 行政区多边形、右侧多景地图预览，以及完整任务确认卡。 | 本地持久任务、兼容网格裁剪 / 拼接、原始像元检查和已有交付流程。 |

设定目标后，侧边计划面板跟踪步骤、待回答选项、任务状态和已校验成果。默认由用户在执行前确认完整任务；明确选择自动执行时，仍按原生权限策略处理。任务是否完成取决于实际任务收尾和文件校验。

[目标模式](docs/agent-goals.md) · [选项与对话](docs/agent-conversation.md) · [行政区及覆盖检查](docs/agent-area-coverage.md) · [地图预览](docs/agent-map-preview.md)

## 持续扩充的二维数据目录

**133 个入口 · 17 类 · 110 Ready · 2 Auth · 21 Planned**，核对自 2026-10-19 当前开发目录，仍有 **23 条目录入口不属于 Ready**。[自动生成的范围清单](docs/releases/source-scope.json)按稳定 ID 记录状态和有限动作；这些数字不是已完整验收的产品集合数。[NASA 实际原件](docs/nasa-original-acceptance-20261015.md)、[SRTM NUM](docs/srtm-num-original-acceptance-20261015.md)与 [GEBCO 全球 TID](docs/gebco-tid-originals-20261015.md)均有完整文件、数值及普通重开证据。全球高程、更多产品家族和门户仍保留待办；Ready 不代表全球全部原件或每个用户的权限都已验收。

| 影像与地形 | 环境产品 | 专题与矢量产品 | 连接与文件 |
| :--- | :--- | :--- | :--- |
| 光学与航空影像 **21** | 土地覆盖 **14** | 人口与聚落 **7** | 离线地图归档 **1** |
| SAR 雷达影像 **2** | 植被与森林 **10** | 土壤 **2** | 栅格目录与覆盖服务 **9** |
| 高程与水深 **15** | 水体与水文 **5** | 夜间灯光 **2** | 本地二维文件 **7** |
| 地图影像与历史 **7** | 气候与降水 **12** | 矢量数据与服务 **9** | 公共数据门户 **5** |
| — | 海洋产品 **5** | — | — |

<sub>统计来自当前[数据源目录](prototype/src/source-directory.js)，包括待接入项。四列仅方便阅读；每个列出的分类对应软件看板中的一个分组。</sub>

<details>
<summary><strong>展开真实数据源看板：8 个专题产品分类、21 个入口</strong></summary>

<img src="docs/images/data-source-board-zh-CN.png" alt="软件真实数据源组件：选中专题栅格筛选，按土地覆盖、森林、水体、人口、土壤、气候和夜间灯光等产品分类展示，保留待接入与获取条件标签" width="100%">

<sub>早期 SourceBoard 组件的本地截图，使用中文和专题栅格筛选。待接入项不能发起连接；截图过程未调用模型或数据提供商。[拍摄范围](docs/images/README.md#data-source-board-enpng--data-source-board-zh-cnpng)。</sub>

</details>

| 产品类别 | 当前开发范围 |
| :--- | :--- |
| **光学与航空影像** | Earth Search / Planetary Computer 的 Sentinel-2 L2A、Landsat 8/9、MODIS 反射率、美国 NAIP。原文件与处理能力按产品分别记录。 |
| **SAR 与高程** | Sentinel-1 IW RTC 的 VV/VH/HH/HV；公开 Copernicus DEM GLO-30 / GLO-90。格式、网格与单位逐产品核对。 |
| **植被与质量** | MODIS NDVI/EVI 及辅助科学层；已有 Landsat/MODIS 质量筛选和科学 RGB 流程。 |
| **公开数据产品** | **72 类内置产品连接器、114 个数值图层选项**，直接读取实际原生 `/datasets` 目录统计。各产品分别保留发布年份、地区、原网格、单位、NoData 和质量边界；从探索页分类弹窗选择来源，核对结果，保存所选原件后显式下载。[当前待办清单](docs/pending-channels-20261014.md)。 |
| **受保护产品** | NASA HLS L30、SRTMGL1、SRTM NUM 与三类 VIIRS 已完成真实授权原件验收；用户仍需连接自己的 Earthdata 账号。Copernicus Sentinel-2 SAFE 仍待真实授权原件验收。 |
| **服务与本地数据** | STAC、COG/GeoTIFF URL、WCS、WMS/WMTS/XYZ/TMS、ArcGIS、OGC API Features、WFS、有界 Overpass、PMTiles，以及支持的本地矢量 / 瓦片文件。 |
| **调研候选** | **22 项 Planned、1 项 Auth** 保留明确的接入或真实账号原件验收缺口。协议支持或一次外部调研下载不代表完整产品接入。[逐项待办](docs/pending-channels-20261014.md)。 |

OpenLandMap 2024 年处理后夜间灯光 DN 已接入：固定 2024 原件已有软件完整下载、原值、预览及重开证据，并完成[真实探索页验收](docs/new-public-channel-sst-nightlights-amazonia-20261013.md)。它仍是作者处理 DN，源定标声明的未解决冲突保留，不称物理辐射值或整个 VIIRS 档案已接入。[2026-10-12 中断记录](docs/next2-nightlights-connection-diagnostic-20261012.md)作为历史失败证据保留。

既有 `enmap` 已接入[一幅官方内华达 L2A、224 波段 ENVI 原件](docs/enmap-original-acceptance-20261015.md)，完成 ZIP/BSQ/HDR 校验、本地波段切换、原始光谱和正常重开。当前范围是固定官方示例，不是全球 EnMAP 场景检索；原始数据再分发仍受 EnMAP 专项许可限制。

既有 `merit-hydro` 新开放 [MERIT Hydro IHU 5 角分三个固定原件](docs/merit-ihu-originals-20261015.md)：流向、上游汇水面积和河流级序，完整软件下载、原值和正常重开通过。90 米原版范围仍 Planned，该 IHU 动作不新增或核销目录 ID。

此前清单外扩展批次的十个独立产品包括 GHSL 面积、体积与人口、ETH 冠层高度、HydroSHEDS 汇流积累、ETOPO 大地水准面、HREA、BII 及两个 NOAA 气候产品。[十一件原件验收](docs/new-public-channel-next-ten-20261014.md)已通过实际探索、软件完整下载（**1,429,629,635 字节**）、完整哈希、**66 个代表原像元**、重启后对象/PNG 不变及**十一个工程真实 UI 重开**。验收限于这些原件；Hydro 全幅预览仍很暗，BII 一个原 mask 排除的海域像元仍被严格原始解码器明确拒绝，[公开回执](docs/next-ten-public-channel-acceptance-20261014.json)保留这两项限制。桌面本地构建不代表安装包、窗口启动或部署验收。[前一批十项](docs/new-public-channel-ten-products-20261014.md)保留独立回执。

前轮原待办十项完成了 WorldClim Nepal 气候平均值、两幅 FABDEM、有限 GLC_FCS10 / GLC_FCS30D、corrected SoilGrids WRB 2017、Vantor Nepal 视觉原件、微软预算内建筑分区、Monaco Geofabrik GIS 包、Overture 水深 GeoParquet 整分片及独立本地 GeoTIFF 导入。[十个原稳定 ID 验收](docs/original-backlog-channel-acceptance-20261014.md)保留该轮 133 / 101 Ready / 6 Auth / 26 Planned 的历史数字和有限范围。全球档案、年度 23 波段 GLC30D、raw PBF、当前 SoilGrids 深度属性及商业权限仍未随状态扩大。

本批原待办接入 SinoLC-1 固定上海分类原件：软件完整下载 **365,818,022 字节 ZIP** 及其未修改的 **427,679,242 字节 TIFF**，实际分类地图、原值、普通重启重开，以及重新检索后复用已完成原件均通过，新增下载任务为 **0**。**34×30 粗分类颜色预览**保留 **132000×114000 完整原网格**供原像元读取。2019–2021 混合输入时间、仅分类数据的 CC BY 4.0 许可和未执行 SinoLC 原 ZIP 的浏览器/UI 导出验收均明确保留。[SinoLC 原件与恢复验收](docs/sinolc-shanghai-original-20261014.md)。[GEBCO 官方区域数值子集](docs/gebco-regional-subsets-20261014.md)已通过区域路径闭环，仍为 Planned 的有限动作，不核销全球原始整包。

紧凑卡片按光学、雷达、高程、土地覆盖、水文、人口等产品分类摆放，独立显示供应平台和获取条件，随面板宽度调整列数。

<details>
<summary><strong>较早产品验收与科学边界</strong></summary>

2026-10-12 历史批次新增三个入口、四个已核对图层：[2020 参考年份的 GHSL GHS-SMOD V2 聚居地分类](docs/next2-ghsl-native-acceptance-20261012.json)、[GLAD v2 2020 年土地覆盖 / 利用组合图](docs/next2-glad-native-acceptance-20261012.json)，以及[毛伊岛 WV02 多光谱与全色原件](docs/next2-maxar-native-acceptance-20261012.json)。GHSL 保留类型内可表示的 NoData -200 和未解释代码 0；GLAD 保留有效代码 0，作者缺失值图例与已核原件没有单一 NoData 声明分别记录。GLAD 按实际 280 幅目录选择，完整原生验收覆盖的是精选原件。毛伊岛仅接一幅 2023-08-12 已核对采集瓦片，数据遵守 CC BY-NC 4.0 非商业使用条款：提供八波段 MS 和单波段 PAN 原值，不转换为反射率或 RGB。当时的毛伊岛检查使用 TIFF 标量 NoData 标记，不证明有效影像覆盖，也不是当前所有产品的统一掩膜能力结论。新 BII 连接器将原始逐层内部有效性掩膜与标量 NoData 分别保留。YCbCr 可视化影像、其他瓦片 / 事件及新版 Vantor 档案仍不在此入口范围。选择、审阅、新下载、地图 / 原值和工程重开另行跟踪于[该历史批次浏览器记录](docs/next2-public-channel-browser-acceptance-20261012.md)，这些检查不代表更新了安装包、完成原生窗口验收或部署。

开发引擎现已加入经过审阅的公开 STAC / 直接栅格原件条件续传。先重新校验保存的来源与所选图层；来源元数据或选层变化需要重新审阅。已校验的文件前缀使用强 ETag 和 `Range` / `If-Range`，只有 206 响应保留同一 ETag，并准确返回 `Content-Range` 的起点和总长度，才接续写入。没有强校验标识、HTTP 文件版本改变或来源忽略分段请求时，从头下载，不拼接不同版本。7 项本地 TCP 协议测试及原有 12 项恢复测试已通过。

一份真实毛伊岛多光谱原件也已通过这条恢复路径：自然网络失败后保留已校验的 782,896 字节前缀，正常重启运行时，再明确重试同一任务，最终续传并校验完整的 12,977,165 字节文件。回执记录为 `resumed`，没有换成另一个新下载任务。这是精选原件的恢复通过记录，不代表所有渠道都已通过，也不是强制崩溃测试；[浏览器记录](docs/next2-public-channel-browser-acceptance-20261012.md)分别记录这次传输以及地图、八波段读数和最终重开。该历史恢复回执不代表更新安装包或部署。

较早证据仍按产品保留：[四份跨平台原件与原生检查](docs/next-public-channel-native-acceptance-20261012.json)、[相应浏览器验收](docs/next-public-channel-browser-acceptance-20261012.md)、[五类产品及十八份原件](docs/new-public-channel-native-acceptance-20261012.md)及[实际探索流程](docs/new-public-channel-browser-acceptance-20261012.md)。CHELSA 保留固定 1981–2010 气候常年值，CGLS 仅支持永久水体比例；[WorldCereal](docs/public-dataset-worldcereal-20261012.md)、[专题产品](docs/public-dataset-curated-20261012.md)和[扩展回执](docs/public-dataset-expansion-20261012.md)保留各自地区、年份和图层限制。JAXA 研究数据仍要求署名与商业使用事先通知，免费发行不等于 CC 许可。

</details>

[完整范围与目录](docs/product-scope.md) · [接入状态](docs/provider-integration-status.md) · [开放数据调研表](docs/research/open-data-sources-2026-10-07.csv)

开放数据可能需要注册、科研申请，或仅开放有限样例。1 米分类图不能当作 1 米原始真彩色影像；灾害开放数据也不是全球按需免费影像库。当前只做 **二维**，包括高程栅格，不接入三维模型和点云。

<details>
<summary><strong>地图工作区与计划进度</strong></summary>

<img src="docs/images/sentinel2-desktop.jpg" alt="真实英文 Windows 桌面中的旧金山湾 Sentinel-2 地理配准预览" width="100%">

<sub>2026-09-30 真实 Windows 窗口截图，展示远程影像预览。来源为 Copernicus Sentinel data (2026)、Earth Search 和 Natural Earth，早于当前 Agent 界面，不作为下载完成证据。</sub>

<img src="prototype/qa/agent-plan-sidebar-home.png" alt="Agent 对话和右侧计划进度面板" width="100%">

<sub>使用受控任务状态的开发界面验收截图，展示布局和导航，不表示一次真实采集。[验证记录](prototype/qa/agent-plan-sidebar-verification.json)。</sub>

</details>

## 同一个原生核心，多种使用方式

```mermaid
flowchart LR
    U[用户需求] --> W[Windows 工作区]
    W --> A[Codex Agent 运行时]
    A <--> B[AI SDK 协议适配]
    B <--> M[用户配置的模型]
    A --> T[限定范围的 GeoD 工具]
    T --> R[Rust 任务与数据引擎]
    W --> P[原生任务确认]
    P --> R
    C[CLI 和本地 MCP] --> R
    R --> F[本地文件与来源记录]
```

Codex 负责 Agent 循环，AI SDK 适配所选模型协议，原生 GeoD 工具处理地理查询、计划和文件操作。桌面、CLI 与本地 MCP 共用 Rust 核心。模型密钥保存于 Windows 凭据管理器，每个连接保留独立历史。工作区在本地运行，对话与选中的附件仍会发送到所配置的模型服务，数据检索和下载会访问相应提供商。

[Agent 实现与验收](docs/agent-integration-status.md) · [CLI / MCP](docs/mcp.md) · [文档附件](docs/agent-documents.md)

## 快速开始

**[v0.2.0-rc.4 Windows 评估版](https://github.com/gaopengbin/geod-global/releases/tag/v0.2.0-rc.4)** 包含对话 Agent、数据源看板和 Global 独立账号入口，提供便携 ZIP 与当前用户安装包。它是未签名预发布版，具体范围和验收缺口见[发行记录](docs/releases/0.2.0-rc.4.md)。旧 rc.3 早于 Agent 接入。

桌面开发需要 **Windows x64**、Node.js **22.13+**、npm **10+**、Rust **1.91.1+**、Visual Studio C++ Build Tools 和 WebView2。在仓库根目录运行：

```sh
git clone https://github.com/gaopengbin/geod-global.git
cd geod-global
npm ci
npm run agent:prepare
npm run desktop:dev
```

`agent:prepare` 准备固定版本的 Windows Node/Codex 运行环境及许可清单。在软件里连接模型，再描述地点和任务即可。详见[桌面环境](src-tauri/README.md)与[开发指南](docs/development-guide.zh-CN.md)。

<details>
<summary><strong>浏览器预览、构建与检查</strong></summary>

```sh
# 浏览器界面：http://127.0.0.1:4317/
npm run dev

# 可选浏览器配套服务：http://127.0.0.1:4318/
npm run runtime

# 嵌入界面的调试程序，不制作安装包
npm run desktop:build

# 契约 / 夹具检查需要 Python 3.12
python -m pip install -r requirements-dev.txt
npm run verify:all
```

软件内 Agent 与安全模型配置需要原生 Windows 程序。浏览器预览不能替代桌面能力验收；夹具、受控界面截图与真实数据验证分别记录。

</details>

## 开发进度

开发代码已加入软件更新和通知中心，独立签名生产渠道待新签名版本发布，开发版不安装更新。[更新与安装体积核对](docs/software-updates.md)。

已有裁剪、拼接和科学处理按产品与网格限制执行。通用重投影、任意跨网格处理、更多数据适配、其余受保护原件与干净设备发行验收，分别作为后续工作。[能力与限制](docs/provider-integration-status.md) · [最新开发说明](docs/development-preview.md)。

## 文档与参与

| 从这里开始 | 深入了解 |
| :--- | :--- |
| [开发环境](docs/development-guide.zh-CN.md) | [产品规格](GeoD-Global-Spec/00-README.md) |
| [Agent 工作流](docs/agent.md) | [处理与配方](GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md) |
| [SCL 裁剪示例](docs/workflows/clip-sentinel-scl.md) | [Windows 打包](docs/releases/windows-packaging.md) |
| [数据源证据](docs/providers.md) | [工作区与成果交付](GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md) |

有真实任务或开放数据源推荐，欢迎[创建 Issue](https://github.com/gaopengbin/geod-global/issues)，说明地点、产品、时间和需要的结果。推荐数据源时请附官方获取入口、覆盖范围和使用条件，省略密钥与私有客户数据。

## 许可

自有代码采用 **[GPL-3.0-only](LICENSE)**，© 2026 Gao Pengbin。允许按 GPL 商用，分发时须履行相应源码与许可义务。[独立商业许可](COMMERCIAL-LICENSING.md)可另行讨论。第三方软件、数据、任务标识和其他资产保留各自条款。

<div align="center"><sub>GeoD Global · Gao Pengbin 独立开发 · 留下数据，也留下来源。</sub></div>
