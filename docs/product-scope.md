# 当前产品范围：二维数据聚合

2026-10-07，产品负责人确认：完整列出现有规划的二维来源，未接入项标为“待接入”；不接入三维。

此决定覆盖早期规划中的三维目标。当前范围包括卫星与航空影像、科学栅格、高程栅格、地图影像、二维矢量、离线瓦片包及对应本地文件。高程仍作为二维栅格支持；不增加三维查看、3D Tiles、glTF / GLB、点云、地形三维或 3DGS。

首页[来源目录](../prototype/src/source-directory.js)维护当前已整理的计划入口，随调研扩充，不是全球开放数据源的穷尽目录。它是来源与连接路径的展示清单，不是原文件授权清单。当前共 85 项：15 个具体产品入口、22 个已支持的服务连接或本地导入路径、48 个待接入项（本轮新增 35 个）。固定产品的可下载状态继续由共享发布策略决定；通用协议接入不意味着任意供应商或整个产品平台已经完成验收。

| 分组 | 当前目录 |
|---|---|
| 卫星影像与高程 | Sentinel-2（Earth Search / Planetary Computer / Copernicus）、Sentinel-1 RTC、Landsat 8/9、MODIS 反射率与 NDVI / EVI、NAIP、HLS、VIIRS 三个平台、Copernicus DEM GLO-30 / GLO-90、SRTMGL1；USGS 直连、更多 MODIS / VIIRS / NASA 产品、SRTMGL3、NUM、NASADEM 待接入 |
| 栅格目录与覆盖服务 | STAC API、静态目录、单个 Item、COG / GeoTIFF URL、WCS 2.0.1；Earth Search / Planetary Computer 的其他集合、Copernicus 其他产品待接入 |
| 地图影像与历史版本 | NASA GIBS、WMS、WMTS、XYZ、TMS、ArcGIS MapServer / ImageServer；Wayback、吉林一号 / 商业影像待接入 |
| 矢量服务 | OpenStreetMap / 用户配置的 Overpass、OGC API Features、WFS 2.0、ArcGIS Feature Service |
| 离线瓦片包 | Protomaps / 公开 PMTiles、本地 PMTiles、本地 MBTiles |
| 本地二维文件 | GeoJSON / Overpass JSON、GeoPackage、Shapefile / ZIP、OSM XML / PBF；独立本地 GeoTIFF / COG 文件导入待接入 |
| 新增影像与高程候选 | CBERS-4A、Maxar / Vantor 灾害开放影像、EnMAP、宏图一号 SAR 免费样例、FABDEM、USGS 3DEP 的二维 DEM、GEBCO，均待接入 |
| 专题栅格与环境 | WorldCover、Dynamic World、GLC_FCS30D / 10、CLCD、SinoLC-1、Hansen 森林变化、JRC 水体、WorldPop、GHSL、SoilGrids、HydroSHEDS、MERIT Hydro、ERA5-Land、CHIRPS、WorldClim、EOG 夜间灯光、CNLUCC，均待接入 |
| 新增矢量候选 | IRSAMap、Microsoft 建筑轮廓、Overture、geoBoundaries、Geofabrik，均待接入 |
| 开放数据门户 | ESA 第三方任务、JAXA Earth、国家地球系统科学数据中心、青藏高原科学数据中心、地理空间数据云，逐数据集核对权限，均待接入 |

本轮来源、覆盖、产品精度、时段、格式、访问条件、许可、优先级和限制保存在[结构化候选清单](../prototype/src/open-data-candidates.json)与[调研表格](research/open-data-sources-2026-10-07.csv)。公众号内容用作发现线索，尽量回到发布机构与作者的官方页面核对；未登录、申请或下载新原文件，因此不能宣称新增适配器已验收。SinoLC-1 的 1 米指分类网格，IRSAMap 是选定地区的标注样本，宏图一号免费数据是 SAR 样例；均不构成全球任意地点的免费亚米级真彩色影像。

看板按具体产品分组：光学与航空影像、SAR 雷达、高程与水深、土地覆盖、植被与森林、水体与水文、人口与聚居地、土壤、气候与降水、夜间灯光、矢量、地图影像，以及瓦片、栅格连接、本地文件和开放门户，共 16 组。产品、供应平台和获取条件分别展示，同一产品类型的多平台入口归到一起。卡片紧凑展示，宽窗口四列，随可用空间降为三列、两列或一列；保留完整能力说明与状态，不隐藏未接入来源。

界面用“可下载”“目录可用 / 需要授权”“可连接”“本地导入”和“待接入”区分状态。待接入卡片没有操作按钮；已接入的服务卡片打开相应连接表单，不自动连接、搜索或下载。所有新来源仍需按[来源进度与验收](provider-integration-status.md)的产品、协议及权限边界逐项落地。

“我的数据”和设置仅提供当前二维入口。旧 `?view=3d` 链接回到工程列表。旧三维模块、原生记录和历史验证文件保留，用于兼容与追溯，不构成当前产品能力承诺；不删除用户历史资产。
