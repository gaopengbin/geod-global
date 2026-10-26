<div align="center">

<img src="docs/images/release-0.2.0-cover.jpg" width="480" alt="GeoD Global 0.2.0 品牌插画">

# GeoD Global
**遥感影像与地理空间数据下载工具：卫星影像、DEM 高程、专题、矢量与地图服务。**

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-2563eb?style=flat-square)](docs/releases/WINDOWS-README.md) [![Release](https://img.shields.io/github/v/release/gaopengbin/geod-global?style=flat-square)](https://github.com/gaopengbin/geod-global/releases/latest) [![GPL](https://img.shields.io/badge/license-GPL--3.0-0f766e?style=flat-square)](LICENSE) [![Checks](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml/badge.svg)](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml)

**[下载 0.2.3 · CDN 加速](https://geod-global.laogao.xyz/zh/index.html#download)** · **[中文官网](https://geod-global.laogao.xyz/zh/index.html)** · **[问题反馈](https://github.com/gaopengbin/geod-global/issues)**

[English](README.md) · 简体中文
</div>

在一个 Windows 工作台里，按类型选数据、按区域检索、多选预览、下载并查看本地成果。
支持手动探索，也可以连接自己的模型，通过 Agent 准备检索和下载任务。**本地使用无需注册 Global 账号。**

**17 个分类 · 110 个可用入口 · 2 个需授权入口**

Earth Search · Microsoft Planetary Computer · NASA Earthdata / GIBS · JAXA 公开产品 · INPE / Brazil Data Cube · OpenTopography · 地理空间数据云

## 先完成一个具体任务

| 你想做什么 | 从这里开始 | 完成后核对什么 |
| :--- | :--- | :--- |
| 批量下载 Landsat | [实机教程与首次使用步骤](https://geod-global.laogao.xyz/zh/workflows/landsat.html#first-run) | 先选 1 景、B4/B3/B2，确认 3 个源文件完成，再打开本地真彩色 |
| 查找 Sentinel-2 影像 | [区域检索与源文件指南](https://geod-global.laogao.xyz/zh/workflows/sentinel-2.html) | 下载前核对日期、云量和场景覆盖范围 |
| 获取 GLO-30 高程 | [公共瓦片与高程说明](https://geod-global.laogao.xyz/zh/workflows/elevation.html) | 核对产品覆盖、原始数值和后续 GIS 使用所需的地理参考 |

Landsat 案例实测于 2026-10-23：3 期、9 份完整源波段 GeoTIFF，共 798,438,650 字节，逐文件核对 SHA-256 并完整解码。未宣称按选区裁剪、质量掩膜或全波段产品包。[2.2 KB 入门参数包](https://geod-global.laogao.xyz/assets/cases/landsat-20261023/Landsat-first-task-parameters.zip)只有范围、日期、场景编号和手动步骤，没有软件或影像，不是可导入工程。

卡住时请[按阶段反馈](https://github.com/gaopengbin/geod-global/issues/new?template=data-workflow.yml)：安装、启动、检索、预览、确认、传输或打开成果。附软件版本、数据源与界面错误即可，不要提供密码、密钥、授权链接或私人数据。

## 一分钟看功能

[![GeoD Global 0.2.0 功能演示](docs/images/release-0.2.0-video.jpg)](https://geod-global.laogao.xyz/zh/index.html#demo)

**[观看 62 秒功能演示](https://geod-global.laogao.xyz/zh/index.html#demo)** · [仓库内 MP4](docs/media/geod-global-0.2.0-features.mp4)

介绍多类型数据渠道、区域检索、多选与地图预览、下载和本地成果管理。实际 Windows 0.2.0 安装版录屏，中文配音与字幕。

## 可以找哪些数据

| 数据类型 | 已接入的代表产品或渠道 |
| :--- | :--- |
| 光学与航空影像 | Sentinel-2、Landsat、NAIP、MODIS、VIIRS、HLS；CBERS 与 EnMAP 公开产品 |
| SAR 雷达 | Sentinel-1 RTC |
| 地形高程与海底地形 | Copernicus DEM、SRTM、NASADEM、ALOS AW3D30、USGS 3DEP、ETOPO、SRTM15+ |
| 土地覆盖 | ESA WorldCover、Esri / Impact Observatory、GLAD、GLC_FCS、CLCD 2004、SinoLC-1 上海产品 |
| 植被与森林 | MODIS NDVI / EVI、Hansen 森林变化、GEDI 冠层高度、Global Mangrove Watch、JAXA 森林产品 |
| 水体与水文 | JRC 地表水、HydroSHEDS 流向与汇水量、DE Africa 年度水体观测 |
| 海洋 | 海表温度、海面风速、叶绿素、海洋热含量；JAXA、NOAA 产品 |
| 人口与聚居地 | WorldPop、GHSL、World Settlement Footprint、HREA |
| 土壤 | JAXA 土壤湿度、SoilGrids WRB 2017 |
| 气候与降水 | CHIRPS、CHELSA、NOAA NClimGrid；JAXA 地表温度、降水、辐射、蒸散和气溶胶产品 |
| 夜间灯光 | OpenLandMap 年度夜光 DN 产品 |
| 矢量数据与服务 | geoBoundaries、OpenStreetMap / Overpass、WFS、OGC API Features、ArcGIS Feature Service；Microsoft 建筑与 Geofabrik 公开文件 |
| 地图影像服务 | NASA GIBS、WMS、WMTS、XYZ、TMS、ArcGIS MapServer / ImageServer |
| 离线地图 | Protomaps / PMTiles 公共瓦片档案 |
| 栅格目录与覆盖服务 | STAC API、静态 STAC、单个 STAC Item、COG / GeoTIFF URL、WCS；接收自己 Earth Engine 项目的导出链接 |
| 本地文件 | GeoTIFF / COG、GeoJSON、GeoPackage、Shapefile、OSM XML / PBF、PMTiles、MBTiles |
| 数据门户 | 地理空间数据云：连接自己的账号，检索并下载已支持产品 |

目录共 **133 项**，包含具体产品、连接协议和本地格式；其中 **21 项为计划中入口**。
表中产品按已支持版本、地区或文件提供，账号要求和产品范围见[完整数据目录](docs/data-catalog.md)。

<img src="docs/images/release-0.2.0-sources.png" width="100%" alt="实际安装版的数据源选择器：按产品分类，显示渠道与访问条件">

## 从选数据到看成果，一条流程

| 选来源 | 做探索 | 确认任务 | 下载执行 | 查看成果 |
| :--- | :--- | :--- | :--- | :--- |
| 分类产品与访问条件 | 紧凑列表、多选、地图预览 | 核对来源、区域、文件和保存位置 | 持久任务与明确的完成状态 | 本地文件、来源、校验和原始像元 |

- **先显示最新 100 条。** 更多结果按需加载；NAIP 优先最近拍摄年份，切换区域不会自动拉完整历史目录。
- **自己的模型，自己的任务。** Agent 使用受限原生工具、任务确认和持久作业。首页会话与探索侧栏保留各自场景。
- **地图与成果对应真实数据。** 多图层预览、原始像元读取，以及已支持的兼容网格裁剪和拼接；地图服务导出与原始科学波段保留各自含义。

<img src="docs/images/release-0.2.0-explore.png" width="100%" alt="GeoD Global 0.2.0 安装版：多选结果与 Sentinel-2 真实地图预览">

<sub>实际安装版截图，2026 年 10 月 19 日。Copernicus Sentinel 影像由 Earth Search 提供。[图片来源](docs/images/README.md) · [版本验收](docs/releases/0.2.0-acceptance.md)。</sub>

## 开始使用

1. 在[官网加速下载](https://geod-global.laogao.xyz/zh/index.html#download)获取 **Windows x64 安装 EXE**，或完整解压免安装 ZIP。
2. 设备需要 Microsoft Edge WebView2 Evergreen；无需另装 Node、Rust、Python 或系统 GDAL。
3. 进入“探索”选择来源和区域，检索、多选并确认下载。需要 Agent 时，在设置中连接自己的模型。

程序文件**尚未签名**。请核对 SHA256SUMS.txt 和发行清单，校验值不能替代 Windows 发布者签名。
升级前从托盘退出；当前使用手动下载升级。卸载保留用户数据。[Windows 使用说明](docs/releases/WINDOWS-README.md)

## 授权、隐私和支持范围

数据源账号与软件账号相互独立。受保护产品需要自己的有效授权；
Earth Engine 使用自己的项目，可能消耗该项目额度。Planned 表示仍未完整接入。

匿名使用统计**默认开启**，设置中可关闭，已保存的关闭选择会保留。
不采集对话、模型密钥、账号资料、精确位置、影像编号或文件路径。[统计范围与保留期限](docs/usage-analytics.md)。
模型请求访问你配置的提供方，数据请求访问相应数据源。

当前聚焦 **2D 地理数据**，包含高程栅格。不包含托管模型、正式付费服务、云同步、任意跨网格处理或其他系统安装包。
[数据源清单](docs/releases/source-scope.json) · [对接状态](docs/provider-integration-status.md) · [0.2.3 发行说明](docs/releases/0.2.3.md) · [发布验收](docs/releases/0.2.3-acceptance.md)

## 开发与贡献

开发环境：Windows x64、Node 22.13+、npm 10+、Rust 1.91.1+、Visual Studio C++ Build Tools 和 WebView2。
在仓库根目录依次运行 npm ci、npm run agent:prepare、npm run desktop:dev。

[开发指南](docs/development-guide.md) · [Agent 架构](docs/agent.md) · [CLI / MCP](docs/mcp.md) ·
[处理示例](docs/workflows/clip-sentinel-scl.md) · [Windows 打包](docs/releases/windows-packaging.md)

欢迎通过 [Issues](https://github.com/gaopengbin/geod-global/issues) 提交真实需求和可复现问题。
请附产品、时间范围、步骤和版本，不要提交凭据或客户私有数据。

## 许可

第一方代码采用 **[GPL-3.0-only](LICENSE)**，© 2026 Gao Pengbin。
遵守 GPL 义务即可商业使用，也可洽谈[单独商业授权](COMMERCIAL-LICENSING.md)。数据集和组件保留各自许可。
